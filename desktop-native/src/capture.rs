//! Shared desktop-capture feeding libwebrtc: BGRA frames become I420 on a NativeVideoSource,
//! 10 ms f32 stereo chunks become i16 on a NativeAudioSource.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::SyncSender,
    },
    thread::{self, JoinHandle},
};

use desktop_capture::{
    AudioChunk, CHANNELS, FRAMES_PER_CHUNK, loopback, process,
    video::{VideoMetrics, VideoSource, capture_video_loop},
};
use libwebrtc::{
    audio_frame::AudioFrame,
    audio_source::{AudioSourceOptions, native::NativeAudioSource},
    native::yuv_helper,
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
};
use windows::Win32::Foundation::HWND;

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
}

pub struct Capture {
    pub source: NativeVideoSource,
    pub audio: Option<NativeAudioSource>,
    /// Read by the capture loop every tick, so the rate changes without a restart.
    pub fps: Arc<AtomicU32>,
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
        let mut threads = Vec::new();
        {
            let (source, running, metrics, fps) = (source.clone(), running.clone(), metrics.clone(), fps.clone());
            threads.push(thread::spawn(move || {
                let mut ready = false;
                let result = capture_video_loop(video, fps, running.clone(), metrics, |frame| {
                    let mut buffer = I420Buffer::new(frame.width, frame.height);
                    let (sy, su, sv) = buffer.strides();
                    let (y, u, v) = buffer.data_mut();
                    // libyuv's "ARGB" is B,G,R,A in memory: the layout desktop-capture produces.
                    yuv_helper::argb_to_i420(frame.data, frame.width * 4, y, sy, u, su, v, sv, frame.width as i32, frame.height as i32);
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
                let deliver = |chunk: AudioChunk| {
                    let data: Vec<i16> = chunk
                        .data
                        .chunks_exact(4)
                        .map(|b| (f32::from_le_bytes([b[0], b[1], b[2], b[3]]).clamp(-1.0, 1.0) * 32767.0) as i16)
                        .collect();
                    let frame = AudioFrame {
                        data: data.into(),
                        sample_rate: 48_000,
                        num_channels: CHANNELS as u32,
                        samples_per_channel: FRAMES_PER_CHUNK as u32,
                    };
                    handle.block_on(sink.capture_frame(&frame)).is_ok()
                };
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
        Ok(Self { source, audio, fps, running, metrics, threads })
    }

    pub fn frames(&self) -> u64 {
        self.metrics.delivered.load(Ordering::Relaxed)
    }
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
