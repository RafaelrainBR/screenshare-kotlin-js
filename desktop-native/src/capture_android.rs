//! Android: the app captures (MediaProjection + ImageReader, AudioRecord's playback capture,
//! CameraX) and pushes each frame here through JNI. The engine side is the same as the desktop's:
//! a NativeVideoSource (and a NativeAudioSource for the screen's audio), Ready after the first frame.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering},
    mpsc::SyncSender,
};

use libwebrtc::{
    audio_frame::AudioFrame,
    audio_source::{AudioSourceOptions, native::NativeAudioSource},
    native::yuv_helper,
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
};

use super::{CaptureEvent, ScreenAudioMode, ShareSource};

/// One running capture the app feeds; replaced on each start, so a late frame of an old one is dropped.
struct Feed {
    video: NativeVideoSource,
    audio: Option<NativeAudioSource>,
    runtime: tokio::runtime::Handle,
    generation: u64,
    camera: bool,
    events: SyncSender<CaptureEvent>,
    ready: AtomicBool,
    frames: AtomicU64,
    fps: AtomicU32,
    last_us: AtomicI64,
}

static SCREEN: Mutex<Option<Arc<Feed>>> = Mutex::new(None);
static CAMERA: Mutex<Option<Arc<Feed>>> = Mutex::new(None);

impl Feed {
    fn new(camera: bool, audio: bool, fps: u32, generation: u64, runtime: &tokio::runtime::Runtime, events: SyncSender<CaptureEvent>) -> Arc<Self> {
        let _runtime = runtime.enter();
        Arc::new(Self {
            video: NativeVideoSource::new(VideoResolution { width: 1280, height: 720 }, !camera),
            audio: audio.then(|| NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 2, 0)),
            runtime: runtime.handle().clone(),
            generation,
            camera,
            events,
            ready: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            fps: AtomicU32::new(fps),
            last_us: AtomicI64::new(i64::MIN),
        })
    }

    /// The share's rate: the display refreshes at 60 or 120 Hz, and a frame the encoder would drop
    /// anyway should not cost an RGBA → I420 conversion.
    fn due(&self, timestamp_us: i64) -> bool {
        let interval = 1_000_000 / i64::from(self.fps.load(Ordering::Relaxed).max(1));
        let last = self.last_us.load(Ordering::Relaxed);
        // 2 ms of slack: a 60 Hz display's 16.6 ms must not skip every other frame at 60 fps.
        if last != i64::MIN && timestamp_us - last < interval - 2_000 {
            return false;
        }
        self.last_us.store(timestamp_us, Ordering::Relaxed);
        true
    }

    fn deliver(&self, buffer: I420Buffer, rotation: VideoRotation, timestamp_us: i64) {
        self.video.capture_frame(&VideoFrame { rotation, timestamp_us, frame_metadata: None, buffer });
        self.frames.fetch_add(1, Ordering::Relaxed);
        if !self.ready.swap(true, Ordering::AcqRel) {
            let event = if self.camera { CaptureEvent::CameraReady(self.generation) } else { CaptureEvent::Ready(self.generation) };
            let _ = self.events.try_send(event);
        }
    }
}

fn current(slot: &Mutex<Option<Arc<Feed>>>) -> Option<Arc<Feed>> {
    slot.lock().unwrap_or_else(|poison| poison.into_inner()).clone()
}

fn clear(slot: &Mutex<Option<Arc<Feed>>>, feed: &Arc<Feed>) {
    let mut slot = slot.lock().unwrap_or_else(|poison| poison.into_inner());
    if slot.as_ref().is_some_and(|current| Arc::ptr_eq(current, feed)) {
        *slot = None;
    }
}

pub struct Capture {
    pub source: NativeVideoSource,
    pub audio: Option<NativeAudioSource>,
    feed: Arc<Feed>,
}

impl Capture {
    /// The app already holds the MediaProjection when it sends startScreen; `Ready` follows its first frame.
    pub fn start(
        _target: &ShareSource,
        fps: u32,
        audio_mode: ScreenAudioMode,
        _screencast: bool,
        generation: u64,
        runtime: &tokio::runtime::Runtime,
        events: SyncSender<CaptureEvent>,
    ) -> Result<Self, String> {
        if audio_mode == ScreenAudioMode::Application {
            return Err("INVALID_ARGUMENT: no Android o áudio é o do celular inteiro.".into());
        }
        let feed = Feed::new(false, audio_mode == ScreenAudioMode::Computer, fps, generation, runtime, events);
        *SCREEN.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(feed.clone());
        Ok(Self { source: feed.video.clone(), audio: feed.audio.clone(), feed })
    }

    pub fn set_fps(&self, fps: u32) {
        self.feed.fps.store(fps, Ordering::Relaxed);
    }

    pub fn frames(&self) -> u64 {
        self.feed.frames.load(Ordering::Relaxed)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        clear(&SCREEN, &self.feed);
    }
}

pub struct Camera {
    pub source: NativeVideoSource,
    feed: Arc<Feed>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CameraDevice {
    pub id: String,
    pub label: String,
}

/// CameraX picks the lens; these are the ids the app understands.
pub fn list_cameras() -> Result<Vec<CameraDevice>, String> {
    Ok(vec![
        CameraDevice { id: "front".into(), label: "Câmera frontal".into() },
        CameraDevice { id: "back".into(), label: "Câmera traseira".into() },
    ])
}

impl Camera {
    pub fn start(_device_id: Option<String>, generation: u64, runtime: &tokio::runtime::Runtime, events: SyncSender<CaptureEvent>) -> Self {
        let feed = Feed::new(true, false, 30, generation, runtime, events);
        *CAMERA.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(feed.clone());
        Self { source: feed.video.clone(), feed }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        clear(&CAMERA, &self.feed);
    }
}

/// Copies a `width`×`height` plane (row stride, pixel stride) into `dst` turned clockwise by `degrees`.
fn rotate_plane(src: &[u8], (width, height): (u32, u32), (stride, pixel): (u32, u32), degrees: i32, dst: &mut [u8], dst_stride: u32) {
    let (w, h, stride, pixel, dst_stride) = (width as usize, height as usize, stride as usize, pixel as usize, dst_stride as usize);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match degrees {
                90 => (h - 1 - y, x),
                180 => (w - 1 - x, h - 1 - y),
                270 => (y, w - 1 - x),
                _ => (x, y),
            };
            dst[dy * dst_stride + dx] = src[y * stride + x * pixel];
        }
    }
}

/// An ImageReader RGBA_8888 frame (R,G,B,A in memory, libyuv's "ABGR"). False when no share is running.
pub fn push_screen_rgba(data: &[u8], width: u32, height: u32, row_stride: u32, timestamp_us: i64) -> Result<bool, String> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 || row_stride < width * 4 {
        return Err("invalid frame size".into());
    }
    if (data.len() as u64) < u64::from(row_stride) * u64::from(height - 1) + u64::from(width) * 4 {
        return Err("frame buffer too small".into());
    }
    let Some(feed) = current(&SCREEN) else { return Ok(false) };
    if !feed.due(timestamp_us) {
        return Ok(true);
    }
    let mut buffer = I420Buffer::new(width, height);
    let (sy, su, sv) = buffer.strides();
    let (y, u, v) = buffer.data_mut();
    yuv_helper::abgr_to_i420(data, row_stride, y, sy, u, su, v, sv, width as i32, height as i32);
    feed.deliver(buffer, VideoRotation::VideoRotation0, timestamp_us);
    Ok(true)
}

/// 10 ms of 48 kHz stereo i16 PCM (960 samples) for the share's `screen-audio` track.
pub fn push_screen_audio(samples: &[i16]) -> Result<bool, String> {
    if samples.len() != 960 {
        return Err("expected 10 ms of 48 kHz stereo".into());
    }
    let Some(feed) = current(&SCREEN) else { return Ok(false) };
    let Some(audio) = &feed.audio else { return Ok(false) };
    let frame = AudioFrame { data: samples.into(), sample_rate: 48_000, num_channels: 2, samples_per_channel: 480 };
    let _ = feed.runtime.block_on(audio.capture_frame(&frame));
    Ok(true)
}

/// A YUV_420_888 camera image: any pixel stride on the chroma planes (1 for I420, 2 for NV12/NV21).
#[allow(clippy::too_many_arguments)]
pub fn push_camera_yuv(
    (y, u, v): (&[u8], &[u8], &[u8]),
    (y_stride, uv_stride, uv_pixel_stride): (u32, u32, u32),
    width: u32,
    height: u32,
    degrees: i32,
    timestamp_us: i64,
) -> Result<bool, String> {
    let (cw, ch) = (width.div_ceil(2), height.div_ceil(2));
    let fits = |plane: &[u8], stride: u32, rows: u32, row_bytes: u32| rows > 0 && stride >= row_bytes && (plane.len() as u64) >= u64::from(stride) * u64::from(rows - 1) + u64::from(row_bytes);
    if width == 0 || height == 0 || width > 4096 || height > 4096 || !(1..=2).contains(&uv_pixel_stride) {
        return Err("invalid camera frame".into());
    }
    let chroma_row = (cw - 1) * uv_pixel_stride + 1;
    if !fits(y, y_stride, height, width) || !fits(u, uv_stride, ch, chroma_row) || !fits(v, uv_stride, ch, chroma_row) {
        return Err("camera buffer too small".into());
    }
    let Some(feed) = current(&CAMERA) else { return Ok(false) };
    // Upright before encoding: the desktop's renderer ignores the frame's rotation, so turning the
    // pixels here is what makes a portrait phone show portrait everywhere.
    let degrees = degrees.rem_euclid(360);
    let (ow, oh) = if degrees % 180 == 90 { (height, width) } else { (width, height) };
    let mut buffer = I420Buffer::new(ow, oh);
    let (sy, su, sv) = buffer.strides();
    let (dy, du, dv) = buffer.data_mut();
    rotate_plane(y, (width, height), (y_stride, 1), degrees, dy, sy);
    rotate_plane(u, (cw, ch), (uv_stride, uv_pixel_stride), degrees, du, su);
    rotate_plane(v, (cw, ch), (uv_stride, uv_pixel_stride), degrees, dv, sv);
    feed.deliver(buffer, VideoRotation::VideoRotation0, timestamp_us);
    Ok(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn camera_planes_turn_clockwise() {
        // 3×2:  a b c / d e f, with a pixel stride of 2 (NV12-like) and row padding.
        let src = [b'a', 0, b'b', 0, b'c', 0, 9, b'd', 0, b'e', 0, b'f', 0, 9];
        let turned = |degrees, dst_w: usize, dst_h: usize| {
            let mut dst = vec![b'.'; dst_w * dst_h];
            super::rotate_plane(&src, (3, 2), (7, 2), degrees, &mut dst, dst_w as u32);
            String::from_utf8(dst).unwrap()
        };
        assert_eq!(turned(0, 3, 2), "abcdef");
        assert_eq!(turned(90, 2, 3), "daebfc");
        assert_eq!(turned(180, 3, 2), "fedcba");
        assert_eq!(turned(270, 2, 3), "cfbead");
    }

    #[test]
    fn frame_pacing_follows_the_share_rate() {
        let (events, _rx) = std::sync::mpsc::sync_channel(1);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let feed = super::Feed::new(false, false, 30, 1, &runtime, events);
        let kept = (0..60).filter(|frame| feed.due(frame * 16_667)).count();
        assert!((29..=31).contains(&kept), "60 Hz display at 30 fps kept {kept}");
        feed.fps.store(60, std::sync::atomic::Ordering::Relaxed);
        let kept = (60..120).filter(|frame| feed.due(frame * 16_667)).count();
        assert_eq!(kept, 60);
    }
}
