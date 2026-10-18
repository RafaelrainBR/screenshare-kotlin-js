//! The camera on macOS.
//! shortcut: not captured yet (the Windows one is Media Foundation); add AVFoundation here, the
//! next step of the macOS port. Until then turning it on says so instead of failing silently.
use std::sync::mpsc::SyncSender;

use libwebrtc::video_source::{VideoResolution, native::NativeVideoSource};

use crate::capture::CaptureEvent;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CameraDevice {
    pub id: String,
    pub label: String,
}

pub fn list() -> Result<Vec<CameraDevice>, String> {
    Ok(Vec::new())
}

pub struct Camera {
    pub source: NativeVideoSource,
}

impl Camera {
    pub fn start(_device_id: Option<String>, generation: u64, runtime: &tokio::runtime::Runtime, events: SyncSender<CaptureEvent>) -> Self {
        let _runtime = runtime.enter();
        let _ = events.try_send(CaptureEvent::CameraEnded(generation, "CAPTURE_ERROR: a câmera ainda não funciona no app nativo para macOS.".into()));
        Self { source: NativeVideoSource::new(VideoResolution { width: 1280, height: 720 }, false) }
    }
}
