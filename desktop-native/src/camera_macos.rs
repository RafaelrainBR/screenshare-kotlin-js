//! The camera on macOS through AVFoundation (nokhwa), like Media Foundation on Windows: 1280×720
//! at up to 30 fps when the camera offers it, sent as the "camera" stream after the screen's.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use libwebrtc::{
    native::yuv_helper,
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
};
use nokhwa::{
    Camera as Device,
    pixel_format::RgbAFormat,
    utils::{ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution},
};

use crate::capture::CaptureEvent;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CameraDevice {
    /// The AVFoundation index, as nokhwa opens it.
    pub id: String,
    pub label: String,
}

fn index_id(index: &CameraIndex) -> String {
    match index {
        CameraIndex::Index(number) => number.to_string(),
        CameraIndex::String(text) => text.clone(),
    }
}

/// The system's camera prompt on first use; false when the user or a policy refused it.
fn permission() -> bool {
    if nokhwa::nokhwa_check() {
        return true;
    }
    let (tx, rx) = sync_channel(1);
    nokhwa::nokhwa_initialize(move |granted| {
        let _ = tx.try_send(granted);
    });
    // shortcut: the prompt waits for the user; give up after two minutes and let them try again.
    rx.recv_timeout(Duration::from_secs(120)).unwrap_or(false)
}

/// The cameras plugged in now, for the dock's picker.
pub fn list() -> Result<Vec<CameraDevice>, String> {
    if !nokhwa::nokhwa_check() {
        // Listing before the permission would raise the prompt from the picker; turning it on asks.
        return Ok(Vec::new());
    }
    let cameras = nokhwa::query(ApiBackend::AVFoundation).map_err(|error| format!("DEVICE_LOST: {error}"))?;
    Ok(cameras.iter().map(|info| CameraDevice { id: index_id(info.index()), label: info.human_name() }).collect())
}

pub struct Camera {
    pub source: NativeVideoSource,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Camera {
    /// Opens the camera on its own thread; `CameraReady` follows the first frame and `CameraEnded`
    /// any failure, like the screen capture.
    pub fn start(device_id: Option<String>, generation: u64, runtime: &tokio::runtime::Runtime, events: SyncSender<CaptureEvent>) -> Self {
        let _runtime = runtime.enter();
        let source = NativeVideoSource::new(VideoResolution { width: 1280, height: 720 }, false);
        let running = Arc::new(AtomicBool::new(true));
        let (thread_source, thread_running) = (source.clone(), running.clone());
        let thread = thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                if !permission() {
                    return Err("PERMISSION_DENIED: o macOS bloqueou a câmera. Permita em Ajustes do Sistema > Privacidade e Segurança > Câmera.".into());
                }
                let index = match device_id {
                    Some(id) => id.parse::<u32>().map(CameraIndex::Index).unwrap_or(CameraIndex::String(id)),
                    None => CameraIndex::Index(0),
                };
                let wanted = CameraFormat::new(Resolution::new(1280, 720), FrameFormat::NV12, 30);
                let format = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(wanted));
                let mut device = Device::new(index, format).map_err(|error| format!("DEVICE_LOST: câmera não encontrada ({error})."))?;
                device.open_stream().map_err(|error| format!("CAPTURE_ERROR: abrir a câmera ({error})"))?;
                let started = Instant::now();
                let mut ready = false;
                while thread_running.load(Ordering::Acquire) {
                    let frame = device.frame().map_err(|error| format!("DEVICE_LOST: a câmera parou ({error})."))?;
                    let Some(buffer) = i420(&frame) else { continue };
                    thread_source.capture_frame(&VideoFrame {
                        rotation: VideoRotation::VideoRotation0,
                        timestamp_us: started.elapsed().as_micros() as i64,
                        frame_metadata: None,
                        buffer,
                    });
                    if !ready {
                        ready = true;
                        let _ = events.try_send(CaptureEvent::CameraReady(generation));
                    }
                }
                let _ = device.stop_stream();
                Ok(())
            })();
            if let Err(reason) = result {
                if thread_running.load(Ordering::Acquire) {
                    let _ = events.try_send(CaptureEvent::CameraEnded(generation, reason));
                }
            }
        });
        Self { source, running, thread: Some(thread) }
    }
}

/// NV12 straight to I420; anything else through nokhwa's RGBA decoder (libyuv's "ABGR").
fn i420(frame: &nokhwa::Buffer) -> Option<I420Buffer> {
    let resolution = frame.resolution();
    let (width, height) = (resolution.width_x, resolution.height_y);
    if width == 0 || height == 0 {
        return None;
    }
    let mut buffer = I420Buffer::new(width, height);
    let (sy, su, sv) = buffer.strides();
    let (y, u, v) = buffer.data_mut();
    if frame.source_frame_format() == FrameFormat::NV12 {
        let data = frame.buffer();
        let (w, h) = (width as usize, height as usize);
        let uv_stride = w.div_ceil(2) * 2;
        let y_len = w * h;
        let planes = data.get(..y_len + uv_stride * h.div_ceil(2))?;
        let (src_y, src_uv) = planes.split_at(y_len);
        yuv_helper::nv12_to_i420(src_y, width, src_uv, uv_stride as u32, y, sy, u, su, v, sv, width as i32, height as i32);
    } else {
        let rgba = frame.decode_image::<RgbAFormat>().ok()?.into_raw();
        yuv_helper::abgr_to_i420(&rgba, width * 4, y, sy, u, su, v, sv, width as i32, height as i32);
    }
    Some(buffer)
}

impl Drop for Camera {
    fn drop(&mut self) {
        // frame() returns with the next frame (≤ 33 ms), then the loop sees the flag.
        self.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
