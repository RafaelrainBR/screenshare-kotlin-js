//! macOS capture: desktop-capture's ScreenCaptureKit video (NV12) and audio, as the Tauri app uses
//! them. The system's own menu bar indicator is the sharing notice macOS requires.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::SyncSender,
};

use desktop_capture::{
    CHANNELS,
    mac::{self, FRAME_HEADER, Target},
};
use libwebrtc::{
    audio_source::{AudioSourceOptions, native::NativeAudioSource},
    native::yuv_helper,
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
};

use super::{CaptureEvent, ScreenAudioMode, ShareSource, audio_frame};

pub struct Capture {
    pub source: NativeVideoSource,
    pub audio: Option<NativeAudioSource>,
    /// Dropping these stops the streams.
    video: Mutex<mac::Capture>,
    _audio: Option<mac::Capture>,
    frames: Arc<AtomicU64>,
}

impl Capture {
    pub fn start(
        target: &ShareSource,
        fps: u32,
        audio_mode: ScreenAudioMode,
        screencast: bool,
        generation: u64,
        runtime: &tokio::runtime::Runtime,
        events: SyncSender<CaptureEvent>,
    ) -> Result<Self, String> {
        let (video_target, size, pid) = match *target {
            ShareSource::Screen { left, top, width, height } => (Target::DisplayAt(left, top), (width, height), None),
            ShareSource::Window { hwnd, pid } => (Target::Window(hwnd as u32), (1280, 720), Some(pid as i32)),
        };
        // Some(None): everything the Mac plays but this app; Some(Some(pid)): that app only.
        let audio_route = match (audio_mode, pid) {
            (ScreenAudioMode::None, _) => None,
            (ScreenAudioMode::Application, Some(pid)) => Some(Some(pid)),
            (ScreenAudioMode::Application, None) => {
                return Err("INVALID_ARGUMENT: selecione a janela para capturar o áudio do aplicativo.".into());
            }
            (ScreenAudioMode::Computer, _) => Some(None),
        };
        let _runtime = runtime.enter();
        let source = NativeVideoSource::new(VideoResolution { width: size.0, height: size.1 }, screencast);
        let frames = Arc::new(AtomicU64::new(0));
        let video = {
            let (source, frames, ready, events, ended) = (source.clone(), frames.clone(), AtomicBool::new(false), events.clone(), events.clone());
            mac::start_video(
                video_target,
                fps,
                // The wizard caps the share height at 1080p, as ScreenCaptureKit already does.
                u32::MAX,
                move |frame| {
                    let Some(i420) = i420(&frame) else { return };
                    source.capture_frame(&i420);
                    frames.fetch_add(1, Ordering::Relaxed);
                    if !ready.swap(true, Ordering::AcqRel) {
                        let _ = events.try_send(CaptureEvent::Ready(generation));
                    }
                },
                move |reason| {
                    let _ = ended.send(CaptureEvent::Ended(generation, reason));
                },
            )
            .map_err(|error| capture_error(&error))?
        };
        // An audio failure leaves the video going, as on Windows and in Tauri.
        let (audio, audio_capture) = match audio_route {
            None => (None, None),
            Some(pid) => {
                let audio = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, CHANNELS as u32, 0);
                let (sink, handle) = (audio.clone(), runtime.handle().clone());
                match mac::start_audio(pid, move |chunk| drop(handle.block_on(sink.capture_frame(&audio_frame(&chunk)))), || {}) {
                    Ok(capture) => (Some(audio), Some(capture)),
                    Err(error) => {
                        eprintln!("screen audio not started: {error}");
                        (None, None)
                    }
                }
            }
        };
        Ok(Self { source, audio, video: Mutex::new(video), _audio: audio_capture, frames })
    }

    /// ScreenCaptureKit changes the rate of a running stream.
    pub fn set_fps(&self, fps: u32) {
        if let Err(error) = self.video.lock().unwrap().set_fps(fps) {
            eprintln!("capture fps not changed: {error}");
        }
    }

    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }
}

/// desktop-capture's codes as the dialog and toasts read them.
fn capture_error(code: &str) -> String {
    match code {
        "screen-recording-denied" => {
            "PERMISSION_DENIED: o macOS bloqueou a gravação de tela. Permita em Ajustes do Sistema > Privacidade e Segurança > Gravação de Tela e Áudio do Sistema.".into()
        }
        "window-closed" => "CAPTURE_ERROR: a janela foi fechada.".into(),
        other => format!("CAPTURE_ERROR: {other}"),
    }
}

/// start_video's frame: FRAME_HEADER (width, height, timestamp µs, wall clock) then packed NV12.
fn i420(frame: &[u8]) -> Option<VideoFrame<I420Buffer>> {
    let header = frame.get(..FRAME_HEADER)?;
    let width = u32::from_le_bytes(header[0..4].try_into().ok()?);
    let height = u32::from_le_bytes(header[4..8].try_into().ok()?);
    let timestamp = f64::from_le_bytes(header[8..16].try_into().ok()?);
    let (w, h) = (width as usize, height as usize);
    let uv_stride = w.div_ceil(2) * 2;
    let y_len = w * h;
    let uv_len = uv_stride * h.div_ceil(2);
    let planes = frame.get(FRAME_HEADER..FRAME_HEADER + y_len + uv_len)?;
    if width == 0 || height == 0 {
        return None;
    }
    let (y, uv) = planes.split_at(y_len);
    let mut buffer = I420Buffer::new(width, height);
    let (sy, su, sv) = buffer.strides();
    let (dy, du, dv) = buffer.data_mut();
    yuv_helper::nv12_to_i420(y, width, uv, uv_stride as u32, dy, sy, du, su, dv, sv, width as i32, height as i32);
    Some(VideoFrame { rotation: VideoRotation::VideoRotation0, timestamp_us: timestamp as i64, frame_metadata: None, buffer })
}
