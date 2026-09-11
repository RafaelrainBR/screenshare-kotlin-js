use serde::Serialize;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HMONITOR, MONITORINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    MONITORINFOF_PRIMARY,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub title: String,
    pub hwnd: u64,
    pub pid: u32,
    pub rect: (i32, i32, i32, i32),
    pub kind: SourceKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Screen,
    Window,
}
pub fn enumerate_sources() -> Vec<Source> {
    let mut result = screens();
    result.extend(windows());
    result
}
fn screens() -> Vec<Source> {
    let mut out = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_proc),
            LPARAM(&mut out as *mut _ as isize),
        );
    };
    out
}
unsafe extern "system" fn monitor_proc(
    monitor: HMONITOR,
    _: windows::Win32::Graphics::Gdi::HDC,
    rect: *mut RECT,
    param: LPARAM,
) -> windows::core::BOOL {
    let out = &mut *(param.0 as *mut Vec<Source>);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = GetMonitorInfoW(monitor, &mut info);
    let r = if rect.is_null() {
        &info.rcMonitor
    } else {
        &*rect
    };
    let index = out.len();
    out.push(Source {
        id: format!("screen:{index}"),
        title: if info.dwFlags & MONITORINFOF_PRIMARY != 0 {
            "Tela principal".into()
        } else {
            format!("Tela {}", index + 1)
        },
        hwnd: 0,
        pid: 0,
        rect: (r.left, r.top, r.right, r.bottom),
        kind: SourceKind::Screen,
    });
    TRUE
}
fn windows() -> Vec<Source> {
    let mut out = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(window_proc), LPARAM(&mut out as *mut _ as isize));
    };
    out
}
unsafe extern "system" fn window_proc(hwnd: HWND, param: LPARAM) -> windows::core::BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buf);
    if len <= 0 {
        return TRUE;
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 || pid == std::process::id() {
        return TRUE;
    }
    let mut rect = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rect);
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return TRUE;
    }
    let out = &mut *(param.0 as *mut Vec<Source>);
    out.push(Source {
        id: format!("window:{}", hwnd.0 as usize),
        title: String::from_utf16_lossy(&buf[..len as usize]),
        hwnd: hwnd.0 as usize as u64,
        pid,
        rect: (rect.left, rect.top, rect.right, rect.bottom),
        kind: SourceKind::Window,
    });
    TRUE
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_serializes_without_internal_names() {
        let json = serde_json::to_string(&Source {
            id: "window:1".into(),
            title: "x".into(),
            hwnd: 1,
            pid: 2,
            rect: (0, 0, 1, 1),
            kind: SourceKind::Window,
        })
        .unwrap();
        assert!(json.contains("\"hwnd\":1") && json.contains("\"kind\":\"window\""));
    }
}
