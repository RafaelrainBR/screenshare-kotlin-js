// shortcut: spike stub — o spike só recebe vídeo; a captura real vem de desktop-capture na F3.
use std::sync::mpsc::SyncSender;

use libwebrtc::{
    audio_source::native::NativeAudioSource,
    video_source::{VideoResolution, native::NativeVideoSource},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScreenAudioMode {
    #[default]
    None,
    Application,
    Computer,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowTarget {
    pub hwnd: u64,
    pub pid: u32,
    pub created: u64,
}

pub enum CaptureEvent {
    Ready(u64),
    Ended(u64, String),
}

pub struct Capture {
    pub source: NativeVideoSource,
    pub audio: Option<NativeAudioSource>,
}

impl Capture {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        width: i32,
        height: i32,
        _fps: i32,
        _audio_mode: ScreenAudioMode,
        _window: Option<WindowTarget>,
        generation: u64,
        runtime: &tokio::runtime::Runtime,
        events: SyncSender<CaptureEvent>,
    ) -> Self {
        let _runtime = runtime.enter();
        let _ = events.try_send(CaptureEvent::Ended(generation, "capture unavailable in spike".into()));
        Self {
            source: NativeVideoSource::new(
                VideoResolution { width: width as u32, height: height as u32 },
                true,
            ),
            audio: None,
        }
    }

    pub fn frames(&self) -> u64 {
        0
    }
}
