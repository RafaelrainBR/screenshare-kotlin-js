//! The Windows capture commands, implemented with ScreenCaptureKit. Same names
//! and payloads, so the bridge only differs in how script receives frames.
use crate::capture::mac::{self, Capture, Target};
use crate::capture::AudioChunk;
use std::sync::Mutex;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

#[derive(Default)]
pub struct MacCaptureState {
    video: Mutex<Option<Capture>>,
    audio: Mutex<Option<Capture>>,
}

fn replace(slot: &Mutex<Option<Capture>>, capture: Option<Capture>) -> Result<(), String> {
    // Dropping the previous capture stops it.
    let previous = std::mem::replace(
        &mut *slot.lock().map_err(|_| "capture-state-unavailable")?,
        capture,
    );
    drop(previous);
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRequest {
    hwnd: Option<u64>,
    rect: Option<(i32, i32, u32, u32)>,
    fps: u32,
    /// The chosen share height; the capture scales taller sources down to it.
    #[serde(default)]
    max_height: Option<u32>,
}

#[tauri::command]
pub async fn enumerate_sources() -> Result<Vec<mac::Source>, String> {
    mac::enumerate_sources()
}

/// `rect` marks a screen: then `hwnd` is its display id.
#[tauri::command]
pub async fn capture_thumbnail(
    hwnd: u64,
    rect: Option<(i32, i32, i32, i32)>,
) -> Result<Option<mac::Thumbnail>, String> {
    mac::capture_thumbnail(if rect.is_some() {
        Target::Display(hwnd as u32)
    } else {
        Target::Window(hwnd as u32)
    })
}

/// Frames go out as raw channel messages; the end is a JSON `{ended, error}`.
#[tauri::command]
pub async fn start_video_capture(
    request: VideoRequest,
    on_frame: Channel,
    state: State<'_, MacCaptureState>,
) -> Result<(), String> {
    let target = match (request.hwnd, request.rect) {
        (Some(id), _) => Target::Window(id as u32),
        (None, Some((left, top, _, _))) => Target::DisplayAt(left, top),
        _ => return Err("invalid-source".into()),
    };
    replace(&state.video, None)?;
    let ended = on_frame.clone();
    let capture = mac::start_video(
        target,
        request.fps,
        request.max_height.unwrap_or(u32::MAX),
        move |frame| {
            let _ = on_frame.send(InvokeResponseBody::Raw(frame));
        },
        move |error| {
            let _ = ended.send(InvokeResponseBody::Json(format!(
                r#"{{"ended":true,"error":{}}}"#,
                serde_json::Value::String(error)
            )));
        },
    )?;
    replace(&state.video, Some(capture))
}

#[tauri::command]
pub async fn stop_video_capture(state: State<'_, MacCaptureState>) -> Result<(), String> {
    replace(&state.video, None)
}

#[tauri::command]
pub async fn set_video_capture_fps(
    fps: u32,
    state: State<'_, MacCaptureState>,
) -> Result<(), String> {
    match &mut *state
        .video
        .lock()
        .map_err(|_| "capture-state-unavailable")?
    {
        Some(capture) => capture.set_fps(fps),
        None => Ok(()),
    }
}

fn start_audio(
    pid: Option<i32>,
    on_audio: Channel<AudioChunk>,
    state: State<'_, MacCaptureState>,
) -> Result<(), String> {
    replace(&state.audio, None)?;
    let ended = on_audio.clone();
    let capture = mac::start_audio(
        pid,
        move |chunk| {
            let _ = on_audio.send(chunk);
        },
        move || {
            let _ = ended.send(AudioChunk::terminal());
        },
    )?;
    replace(&state.audio, Some(capture))
}

#[tauri::command]
pub async fn start_system_audio(
    on_audio: Channel<AudioChunk>,
    state: State<'_, MacCaptureState>,
) -> Result<(), String> {
    start_audio(None, on_audio, state)
}

/// Core Audio taps include the chosen app's descendant audio processes.
#[tauri::command]
pub async fn start_process_audio(
    process_id: u32,
    #[allow(unused_variables)] include_process_tree: bool,
    on_audio: Channel<AudioChunk>,
    state: State<'_, MacCaptureState>,
) -> Result<(), String> {
    start_audio(Some(process_id as i32), on_audio, state)
}

#[tauri::command]
pub async fn stop_audio(state: State<'_, MacCaptureState>) -> Result<(), String> {
    replace(&state.audio, None)
}
