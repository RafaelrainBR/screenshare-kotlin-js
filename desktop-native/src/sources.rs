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
