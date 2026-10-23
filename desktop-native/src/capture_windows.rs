//! Windows capture: desktop-capture's Graphics Capture video and WASAPI audio.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::SyncSender,
    },
    thread::{self, JoinHandle},
};

use desktop_capture::{
    AudioChunk, CHANNELS, loopback, process,
    video::{VideoMetrics, VideoSource, capture_video_loop},
};
use libwebrtc::{
    audio_source::{AudioSourceOptions, native::NativeAudioSource},
    native::yuv_helper,
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
};
use windows::Win32::Foundation::HWND;

use super::{CaptureEvent, ScreenAudioMode, ShareSource, audio_frame};

pub struct Capture {
    pub source: NativeVideoSource,
    pub audio: Option<NativeAudioSource>,
    /// Read by the capture loop every tick, so the rate changes without a restart.
    pub fps: Arc<AtomicU32>,
    /// The chosen share height: bigger sources are scaled down before the (software) encoder.
    max_height: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    metrics: Arc<VideoMetrics>,
    threads: Vec<JoinHandle<()>>,
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
        let (video, size) = match *target {
            ShareSource::Screen { left, top, width, height } => {
                (VideoSource::Screen { left, top, width, height }, (width, height))
            }
            ShareSource::Window { hwnd, .. } => (VideoSource::Window(HWND(hwnd as usize as *mut _)), (1280, 720)),
        };
        // Resolve audio before starting threads so a refusal leaves nothing running.
        let audio_route = match (audio_mode, target) {
            (ScreenAudioMode::None, _) => None,
            (ScreenAudioMode::Application, ShareSource::Window { pid, .. }) => {
                if !process::process_exists(*pid) {
                    return Err("CAPTURE_ERROR: o aplicativo da janela já fechou.".into());
                }
                Some(AudioRoute::Process(*pid))
            }
            (ScreenAudioMode::Application, ShareSource::Screen { .. }) => {
                return Err("INVALID_ARGUMENT: selecione a janela para capturar o áudio do aplicativo.".into());
            }
            (ScreenAudioMode::Computer, _) => {
                loopback::preflight_default_render_device()
                    .map_err(|_| "CAPTURE_ERROR: nenhuma saída de áudio padrão.".to_owned())?;
                // shortcut: exclusion is resolved at start, like Tauri; restart sharing after launching Discord.
                Some(AudioRoute::Computer(process::discord_process_id().map_err(|e| format!("CAPTURE_ERROR: {e}"))?))
            }
        };
        let _runtime = runtime.enter();
        let source = NativeVideoSource::new(VideoResolution { width: size.0, height: size.1 }, screencast);
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(VideoMetrics::default());
        let fps = Arc::new(AtomicU32::new(fps));
        let max_height = Arc::new(AtomicU32::new(u32::MAX));
        let mut threads = Vec::new();
        {
            let (source, running, metrics, fps) = (source.clone(), running.clone(), metrics.clone(), fps.clone());
            let max_height = max_height.clone();
            threads.push(thread::spawn(move || {
                let mut ready = false;
                let result = capture_video_loop(video, fps, running.clone(), metrics, |frame| {
                    let mut buffer = I420Buffer::new(frame.width, frame.height);
                    let (sy, su, sv) = buffer.strides();
                    let (y, u, v) = buffer.data_mut();
                    // libyuv's "ARGB" is B,G,R,A in memory: the layout desktop-capture produces.
                    yuv_helper::argb_to_i420(frame.data, frame.width * 4, y, sy, u, su, v, sv, frame.width as i32, frame.height as i32);
                    let buffer = match scaled_size(frame.width, frame.height, max_height.load(Ordering::Relaxed)) {
                        Some((width, height)) => buffer.scale(width as i32, height as i32),
                        None => buffer,
                    };
                    source.capture_frame(&VideoFrame {
                        rotation: VideoRotation::VideoRotation0,
                        timestamp_us: frame.timestamp_us as i64,
                        frame_metadata: None,
                        buffer,
                    });
                    if !ready {
                        ready = true;
                        let _ = events.try_send(CaptureEvent::Ready(generation));
                    }
                    true
                });
                if running.load(Ordering::Acquire) {
                    let reason = result.err().unwrap_or_else(|| "capture-ended".into());
                    let _ = events.send(CaptureEvent::Ended(generation, reason));
                }
            }));
        }
        let audio = audio_route.map(|route| {
            let audio = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, CHANNELS as u32, 0);
            let (sink, running, handle) = (audio.clone(), running.clone(), runtime.handle().clone());
            threads.push(thread::spawn(move || {
                let deliver = |chunk: AudioChunk| handle.block_on(sink.capture_frame(&audio_frame(&chunk))).is_ok();
                // An audio failure ends only the audio; the video keeps going, as in Tauri.
                let _ = match route {
                    AudioRoute::Process(pid) => process::capture_process_loop(pid, true, running, deliver),
                    AudioRoute::Computer(excluded) => {
                        loopback::capture_loop(excluded, running, deliver).map_err(|e| e.to_string())
                    }
                };
            }));
            audio
        });
        Ok(Self { source, audio, fps, max_height, running, metrics, threads })
    }

    /// Read by the capture loop every tick, so the rate changes without a restart.
    pub fn set_fps(&self, fps: u32) {
        self.fps.store(fps, Ordering::Relaxed);
    }

    /// Read by the capture loop every frame, like the rate.
    pub fn set_max_height(&self, height: u32) {
        self.max_height.store(height, Ordering::Relaxed);
    }

    pub fn frames(&self) -> u64 {
        self.metrics.delivered.load(Ordering::Relaxed)
    }
}

/// Even size with the source's aspect at `max_height`, or None when the source already fits.
fn scaled_size(width: u32, height: u32, max_height: u32) -> Option<(u32, u32)> {
    let even = |value: u64| ((value as u32) & !1).max(2);
    (height > max_height).then(|| (even(u64::from(width) * u64::from(max_height) / u64::from(height)), even(u64::from(max_height))))
}

enum AudioRoute {
    Process(u32),
    Computer(Option<u32>),
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scaled_size;

    #[test]
    fn scales_down_to_the_share_height_keeping_aspect() {
        assert_eq!(scaled_size(3840, 2160, 1080), Some((1920, 1080)));
        assert_eq!(scaled_size(2560, 1440, 1080), Some((1920, 1080)));
        assert_eq!(scaled_size(1282, 911, 720), Some((1012, 720)), "1013.2 wide rounds down to even");
        assert_eq!(scaled_size(1920, 1080, 1080), None);
        assert_eq!(scaled_size(1280, 720, u32::MAX), None);
    }
}
