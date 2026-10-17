pub use desktop_capture::*;

// The WebView ring is the Tauri-only bridge to the page; the capture itself lives in desktop-capture.
#[cfg(target_os = "windows")]
pub mod shared;
