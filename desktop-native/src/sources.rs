//! Screens and windows the wizard offers, with their thumbnails, the same on Windows and macOS:
//! desktop-capture's Win32 listing there, its ScreenCaptureKit listing here.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Screen,
    Window,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    pub title: String,
    /// The HWND on Windows; the window or display id on macOS.
    pub hwnd: u64,
    pub pid: u32,
    /// left, top, right, bottom in desktop coordinates (points on macOS).
    pub rect: (i32, i32, i32, i32),
    pub kind: SourceKind,
}

/// Width, height and RGBA pixels of a preview no larger than 320 px.
pub type Thumbnail = (u32, u32, Vec<u8>);

#[cfg(windows)]
pub fn enumerate() -> Vec<Source> {
    use desktop_capture::sources::{SourceKind as Kind, enumerate_sources};
    enumerate_sources()
        .into_iter()
        .map(|s| Source {
            id: s.id,
            title: s.title,
            hwnd: s.hwnd,
            pid: s.pid,
            rect: s.rect,
            kind: if s.kind == Kind::Screen { SourceKind::Screen } else { SourceKind::Window },
        })
        .collect()
}

/// Each process's executable ("game.exe"), to know a restarted game by its program.
#[cfg(windows)]
pub fn exe_names() -> std::collections::HashMap<u32, String> {
    desktop_capture::process::process_list().unwrap_or_default().into_iter().map(|(pid, _, exe)| (pid, exe)).collect()
}

/// The first window of `exe` in a fresh listing: a restarted game's new window.
pub fn reopened(sources: Vec<Source>, names: &std::collections::HashMap<u32, String>, exe: &str) -> Option<Source> {
    sources.into_iter().find(|s| s.kind == SourceKind::Window && names.get(&s.pid).is_some_and(|name| name == exe))
}

#[test]
fn a_restarted_game_is_its_executable_window() {
    let window = |pid: u32, kind| Source { id: pid.to_string(), title: String::new(), hwnd: 0, pid, rect: (0, 0, 0, 0), kind };
    let names = [(1, "game.exe".to_owned()), (2, "other.exe".to_owned()), (3, "game.exe".to_owned())].into();
    let found = reopened(vec![window(3, SourceKind::Screen), window(2, SourceKind::Window), window(3, SourceKind::Window)], &names, "game.exe");
    assert_eq!(found.map(|s| s.pid), Some(3));
    assert!(reopened(vec![window(2, SourceKind::Window), window(9, SourceKind::Window)], &names, "game.exe").is_none());
}

/// shortcut: macOS never follows a restarted window; add proc_name lookups if Mac players ask.
#[cfg(target_os = "macos")]
pub fn exe_names() -> std::collections::HashMap<u32, String> {
    std::collections::HashMap::new()
}

#[cfg(windows)]
pub fn thumbnail(source: &Source) -> Option<Thumbnail> {
    use windows::Win32::Foundation::{HWND, RECT};
    let (left, top, right, bottom) = source.rect;
    let screen = (source.kind == SourceKind::Screen).then_some(RECT { left, top, right, bottom });
    let thumb = desktop_capture::thumb::capture_thumbnail(HWND(source.hwnd as usize as *mut _), screen, 320).ok()??;
    rgba(thumb.width, thumb.height, thumb.data)
}

#[cfg(target_os = "macos")]
pub fn enumerate() -> Vec<Source> {
    // Fails until the Screen Recording permission is granted; the first call asks for it.
    desktop_capture::mac::enumerate_sources()
        .unwrap_or_default()
        .into_iter()
        .map(|s| Source {
            id: s.id,
            title: s.title,
            hwnd: s.hwnd,
            pid: s.pid.max(0) as u32,
            rect: s.rect,
            kind: if s.kind == "screen" { SourceKind::Screen } else { SourceKind::Window },
        })
        .collect()
}

#[cfg(target_os = "macos")]
pub fn thumbnail(source: &Source) -> Option<Thumbnail> {
    use desktop_capture::mac::{Target, capture_thumbnail};
    let target = match source.kind {
        SourceKind::Screen => Target::Display(source.hwnd as u32),
        SourceKind::Window => Target::Window(source.hwnd as u32),
    };
    let thumb = capture_thumbnail(target).ok()??;
    rgba(thumb.width, thumb.height, thumb.data)
}

/// BGRA, as both captures produce it, to the RGBA iced draws; minimized windows come back 0×0.
fn rgba(width: u32, height: u32, mut data: Vec<u8>) -> Option<Thumbnail> {
    if width == 0 || height == 0 {
        return None;
    }
    data.chunks_exact_mut(4).for_each(|px| px.swap(0, 2));
    Some((width, height, data))
}
