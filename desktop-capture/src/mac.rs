//! macOS capture through ScreenCaptureKit: no picker, and the system's own
//! menu bar recording indicator is the sharing notice the OS requires.
use super::{drain_chunks, fit_within, AudioChunk, AudioClock};
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use screencapturekit::shareable_content::SCShareableContentInfo;
use screencapturekit::stream::ErrorHandler;
use serde::Serialize;
use std::sync::Mutex;

/// Video frames above this box are scaled down by the system: raw frames cross
/// the WKWebView IPC (there is no shared memory), so their size bounds the cost.
const MAX_W: u32 = 1920;
const MAX_H: u32 = 1080;
/// Frame header: width u32, height u32, timestamp f64 (µs), wall clock f64 (µs).
pub const FRAME_HEADER: usize = 24;
pub const THUMB_DIM: u32 = 320;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub title: String,
    pub hwnd: u64,
    pub pid: i32,
    pub rect: (i32, i32, i32, i32),
    pub kind: &'static str,
}

#[derive(Serialize)]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// A window id, or the display whose top-left corner is `(left, top)` (points).
pub enum Target {
    Window(u32),
    Display(u32),
    DisplayAt(i32, i32),
}

/// Keeps a stream running; dropping it stops the capture.
pub enum Capture {
    Stream(SCStream),
    // Keeps its configuration so the frame rate can change while it runs.
    Video(SCStream, SCStreamConfiguration),
    AppAudio {
        _capture: super::mac_audio::AppAudioCapture,
    },
}
impl Drop for Capture {
    fn drop(&mut self) {
        if let Self::Stream(stream) | Self::Video(stream, _) = self {
            let _ = stream.stop_capture();
        }
    }
}
impl Capture {
    /// Changes a running video capture's frame rate without restarting it.
    pub fn set_fps(&mut self, fps: u32) -> Result<(), String> {
        let Self::Video(stream, config) = self else {
            return Err("not-video".into());
        };
        config.set_minimum_frame_interval(&frame_interval(fps));
        stream
            .update_configuration(config)
            .map_err(|_| "capture-failed".into())
    }
}

fn frame_interval(fps: u32) -> CMTime {
    CMTime::new(1, fps.clamp(1, 60) as i32)
}

fn content() -> Result<SCShareableContent, String> {
    // Fails only without the Screen Recording permission (the first call asks for it).
    SCShareableContent::create()
        .with_on_screen_windows_only(true)
        .with_exclude_desktop_windows(true)
        .get()
        .map_err(|_| "screen-recording-denied".to_string())
}

fn rect(frame: CGRect) -> (i32, i32, i32, i32) {
    let (x, y) = (frame.origin.x as i32, frame.origin.y as i32);
    (
        x,
        y,
        x + frame.size.width as i32,
        y + frame.size.height as i32,
    )
}

pub fn enumerate_sources() -> Result<Vec<Source>, String> {
    let content = content()?;
    let own = std::process::id() as i32;
    let mut sources: Vec<Source> = content
        .displays()
        .iter()
        .enumerate()
        .map(|(index, display)| Source {
            id: format!("screen:{}", display.display_id()),
            title: format!("Tela {}", index + 1),
            hwnd: display.display_id() as u64,
            pid: 0,
            rect: rect(display.frame()),
            kind: "screen",
        })
        .collect();
    for window in content.windows() {
        let frame = window.frame();
        // Layer 0 holds normal app windows; menus, the Dock and overlays sit above it.
        if window.window_layer() != 0 || frame.size.width < 64.0 || frame.size.height < 64.0 {
            continue;
        }
        let Some(app) = window.owning_application() else {
            continue;
        };
        if app.process_id() == own {
            continue;
        }
        let name = app.application_name();
        let title = match window.title().filter(|title| !title.is_empty()) {
            Some(title) if title != name => format!("{name} — {title}"),
            _ => name,
        };
        sources.push(Source {
            id: format!("window:{}", window.window_id()),
            title,
            hwnd: window.window_id() as u64,
            pid: app.process_id(),
            rect: rect(frame),
            kind: "window",
        });
    }
    Ok(sources)
}

/// The filter for `target` and its size in pixels.
fn filter_for(target: &Target) -> Result<(SCContentFilter, (f64, f64)), String> {
    let content = content()?;
    let builder = SCContentFilter::create();
    let builder = match *target {
        Target::Window(id) => builder.with_window(
            &content
                .windows()
                .into_iter()
                .find(|w| w.window_id() == id)
                .ok_or("window-closed")?,
        ),
        Target::Display(_) | Target::DisplayAt(..) => {
            let display = content
                .displays()
                .into_iter()
                .find(|d| match *target {
                    Target::Display(id) => d.display_id() == id,
                    Target::DisplayAt(left, top) => {
                        let (x, y, _, _) = rect(d.frame());
                        (x, y) == (left, top)
                    }
                    Target::Window(_) => false,
                })
                .ok_or("display-not-found")?;
            builder.with_display(&display).with_excluding_windows(&[])
        }
    };
    let filter = builder.build().map_err(|_| "capture-failed")?;
    let (width, height) = SCShareableContentInfo::for_filter(&filter)
        .map(|info| info.pixel_size())
        .ok_or("capture-failed")?;
    Ok((filter, (width as f64, height as f64)))
}

pub fn capture_thumbnail(target: Target) -> Result<Option<Thumbnail>, String> {
    let (filter, (width, height)) = filter_for(&target)?;
    let (width, height) = fit_within(width, height, THUMB_DIM, THUMB_DIM);
    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height)
        .with_pixel_format(PixelFormat::BGRA);
    let Ok(sample) =
        screencapturekit::screenshot_manager::SCScreenshotManager::capture_sample_buffer(
            &filter, &config,
        )
    else {
        return Ok(None);
    };
    let Some(buffer) = sample.pixel_buffer() else {
        return Ok(None);
    };
    let guard = buffer
        .lock(CVPixelBufferLockFlags::READ_ONLY)
        .map_err(|_| "thumbnail-failed")?;
    let (w, h) = (guard.width(), guard.height());
    let mut data = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let Some(bytes) = (unsafe { guard.row(row) }) else {
            return Ok(None);
        };
        data.extend_from_slice(&bytes[..w * 4]);
    }
    Ok(Some(Thumbnail {
        width: w as u32,
        height: h as u32,
        data,
    }))
}

/// Streams NV12 frames (`FRAME_HEADER` + Y plane + interleaved UV plane).
/// `on_end` receives "window-closed" or "capture-failed" when the system stops the stream.
pub fn start_video(
    target: Target,
    fps: u32,
    on_frame: impl Fn(Vec<u8>) + Send + Sync + 'static,
    on_end: impl Fn(String) + Send + Sync + 'static,
) -> Result<Capture, String> {
    let is_window = matches!(target, Target::Window(_));
    let (filter, (width, height)) = filter_for(&target)?;
    let (width, height) = fit_within(width, height, MAX_W, MAX_H);
    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height)
        .with_pixel_format(PixelFormat::YCbCr_420v)
        .with_shows_cursor(true)
        .with_queue_depth(3)
        .with_minimum_frame_interval(&frame_interval(fps));
    let mut stream = SCStream::new_with_delegate(
        &filter,
        &config,
        ErrorHandler::new(move |_| {
            on_end(
                if is_window {
                    "window-closed"
                } else {
                    "capture-failed"
                }
                .into(),
            )
        }),
    )
    .map_err(|_| "capture-failed")?;
    let start = std::time::Instant::now();
    stream
        .add_output_handler(
            move |sample: CMSampleBuffer, _: SCStreamOutputType| {
                // Idle (unchanged) frames carry no pixel buffer: nothing to send.
                let Some(buffer) = sample.pixel_buffer() else {
                    return;
                };
                let Ok(guard) = buffer.lock(CVPixelBufferLockFlags::READ_ONLY) else {
                    return;
                };
                let (w, h) = (guard.width(), guard.height());
                let mut frame = Vec::with_capacity(FRAME_HEADER + w * h * 3 / 2);
                frame.extend_from_slice(&(w as u32).to_le_bytes());
                frame.extend_from_slice(&(h as u32).to_le_bytes());
                frame.extend_from_slice(&(start.elapsed().as_micros() as f64).to_le_bytes());
                let wall = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0.0, |d| d.as_micros() as f64);
                frame.extend_from_slice(&wall.to_le_bytes());
                // Rows are padded to bytes_per_row; VideoFrame expects them packed.
                for (plane, row_bytes, rows) in [(0, w, h), (1, w.div_ceil(2) * 2, h.div_ceil(2))] {
                    for row in 0..rows {
                        let Some(bytes) = (unsafe { guard.plane_row(plane, row) }) else {
                            return;
                        };
                        frame.extend_from_slice(&bytes[..row_bytes]);
                    }
                }
                on_frame(frame);
            },
            SCStreamOutputType::Screen,
        )
        .map_err(|_| "capture-failed")?;
    stream.start_capture().map_err(|_| "capture-failed")?;
    Ok(Capture::Video(stream, config))
}

/// System audio (every app but this one), or the audio of the app `pid`.
pub fn start_audio(
    pid: Option<i32>,
    on_audio: impl Fn(AudioChunk) + Send + Sync + 'static,
    on_end: impl Fn() + Send + Sync + 'static,
) -> Result<Capture, String> {
    let content = content()?;
    if let Some(pid) = pid {
        if !content
            .applications()
            .iter()
            .any(|app| app.process_id() == pid)
        {
            return Err("process-not-found".into());
        }
        return super::mac_audio::AppAudioCapture::start(pid, on_audio)
            .map(|capture| Capture::AppAudio { _capture: capture });
    }
    let display = content
        .displays()
        .into_iter()
        .next()
        .ok_or("no-default-render-device")?;
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build()
        .map_err(|_| "capture-failed")?;
    // Video is required by the stream but unused: keep it tiny and rare.
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_minimum_frame_interval(&CMTime::new(1, 1))
        .with_captures_audio(true)
        .with_excludes_current_process_audio(true)
        .with_sample_rate(48_000)
        .with_channel_count(2);
    let mut stream =
        SCStream::new_with_delegate(&filter, &config, ErrorHandler::new(move |_| on_end()))
            .map_err(|_| "capture-failed")?;
    let state = Mutex::new((Vec::new(), AudioClock::new()));
    stream
        .add_output_handler(
            move |sample: CMSampleBuffer, _: SCStreamOutputType| {
                let Ok(list) = sample.audio_buffer_list() else {
                    return;
                };
                // ScreenCaptureKit delivers planar f32: one buffer per channel.
                let pcm: Vec<u8> = match (list.get(0), list.get(1)) {
                    (Some(left), Some(right)) => left
                        .data()
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .zip(right.data().as_chunks::<4>().0.iter())
                        .flat_map(|(l, r)| l.iter().chain(r.iter()).copied())
                        .collect(),
                    (Some(mono), None) => mono
                        .data()
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .flat_map(|s| s.iter().chain(s.iter()).copied())
                        .collect(),
                    _ => return,
                };
                let Ok(mut state) = state.lock() else { return };
                let (pending, clock) = &mut *state;
                for data in drain_chunks(pending, &pcm) {
                    on_audio(AudioChunk {
                        timestamp_us: clock.timestamp_us(),
                        data,
                        ended: None,
                    });
                }
            },
            SCStreamOutputType::Audio,
        )
        .map_err(|_| "capture-failed")?;
    stream.start_capture().map_err(|_| "capture-failed")?;
    Ok(Capture::Stream(stream))
}
