//! Shared desktop-capture feeding libwebrtc: BGRA frames become I420 on a NativeVideoSource,
//! 10 ms f32 stereo chunks become i16 on a NativeAudioSource.
use desktop_capture::{AudioChunk, CHANNELS, FRAMES_PER_CHUNK};
use libwebrtc::audio_frame::AudioFrame;

#[cfg(windows)]
#[path = "capture_windows.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "capture_macos.rs"]
mod platform;
pub use platform::Capture;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScreenAudioMode {
    #[default]
    None,
    /// The shared window's application, including its child processes.
    Application,
    /// Everything the PC plays except Discord, so voice calls never leak into the share.
    Computer,
}

/// What to capture: a monitor rectangle (virtual-screen coordinates) or one window.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ShareSource {
    Screen { left: i32, top: i32, width: u32, height: u32 },
    Window { hwnd: u64, pid: u32 },
}

pub enum CaptureEvent {
    Ready(u64),
    Ended(u64, String),
    // Only the Windows camera reports a first frame until the macOS one exists.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    CameraReady(u64),
    CameraEnded(u64, String),
}

/// A 10 ms chunk of f32 stereo PCM from desktop-capture, as the i16 frame libwebrtc takes.
fn audio_frame(chunk: &AudioChunk) -> AudioFrame<'static> {
    let data: Vec<i16> = chunk
        .data
        .chunks_exact(4)
        .map(|b| (f32::from_le_bytes([b[0], b[1], b[2], b[3]]).clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();
    AudioFrame { data: data.into(), sample_rate: 48_000, num_channels: CHANNELS as u32, samples_per_channel: FRAMES_PER_CHUNK as u32 }
}
