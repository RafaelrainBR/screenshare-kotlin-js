#[cfg(any(target_os = "windows", target_os = "macos"))]
mod capture;
#[cfg(target_os = "macos")]
mod mac_commands;
#[cfg(target_os = "macos")]
mod session_media;
#[cfg(target_os = "windows")]
use capture::{sources::Source, thumb::Thumbnail, AudioChunk};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "windows")]
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
#[cfg(target_os = "windows")]
use tauri::ipc::Channel;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};

struct TrayUiState {
    tray: TrayIcon,
    mic: MenuItem<tauri::Wry>,
    camera: MenuItem<tauri::Wry>,
    screen: MenuItem<tauri::Wry>,
    output: MenuItem<tauri::Wry>,
    leave: MenuItem<tauri::Wry>,
    qualities: [CheckMenuItem<tauri::Wry>; 3],
    participants: u32,
    remote_sharing: bool,
    /// Title of what this user shares, while sharing.
    sharing: Option<String>,
}

/// Window titles can be very long; the menu and tooltip only fit a short name.
fn short_title(title: &str) -> String {
    const MAX: usize = 40;
    if title.chars().count() <= MAX {
        title.to_string()
    } else {
        format!("{}…", title.chars().take(MAX - 1).collect::<String>())
    }
}

#[tauri::command]
fn set_tray_state(
    kind: String,
    active: bool,
    participants: Option<u32>,
    detail: Option<String>,
    state: State<'_, Mutex<TrayUiState>>,
) -> Result<(), String> {
    let mut tray = state.lock().map_err(|_| "tray-state-unavailable")?;
    if let Some(count) = participants {
        tray.participants = count;
    }
    match kind.as_str() {
        "mic" => {
            tray.mic
                .set_text(if active {
                    "Microfone ativo · Silenciar"
                } else {
                    "Microfone silenciado · Ativar"
                })
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "camera" => {
            tray.camera
                .set_text(if active {
                    "Câmera ativa · Parar"
                } else {
                    "Câmera desligada · Ligar"
                })
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "screen" => {
            tray.sharing = active.then(|| short_title(detail.as_deref().unwrap_or("a tela")));
            let text = match &tray.sharing {
                Some(title) => format!("Compartilhando {title} · Parar"),
                None => "Compartilhar tela ou janela…".into(),
            };
            tray.screen
                .set_text(text)
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "remote" => {
            tray.remote_sharing = active;
        }
        "room" => {
            tray.leave
                .set_enabled(active)
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "connection" => {
            for item in [&tray.mic, &tray.camera, &tray.screen] {
                item.set_enabled(active)
                    .map_err(|_| "tray-menu-unavailable")?;
            }
        }
        "output" => {
            tray.output
                .set_text(if active {
                    "Áudio silenciado · Ativar"
                } else {
                    "Silenciar áudio"
                })
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "quality720" | "quality1080" | "quality4k" | "quality-source" => {
            for (item, id) in tray
                .qualities
                .iter()
                .zip(["quality720", "quality1080", "quality4k"])
            {
                item.set_checked(kind == id)
                    .map_err(|_| "tray-menu-unavailable")?;
            }
        }
        "participants" => {}
        _ => return Err("unknown-tray-state".into()),
    }
    let mut tooltip = format!("Screenshare · {} participantes", tray.participants);
    if let Some(title) = &tray.sharing {
        tooltip += &format!(" · Compartilhando {title}");
    } else if tray.remote_sharing {
        tooltip += " · compartilhamento disponível";
    }
    tray.tray
        .set_tooltip(Some(tooltip))
        .map_err(|_| "tray-icon-unavailable".to_string())
}
#[cfg(target_os = "windows")]
struct CaptureHandle {
    running: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<()>,
}
#[cfg(target_os = "windows")]
struct CaptureState {
    active: Mutex<Option<CaptureHandle>>,
}

#[cfg(target_os = "windows")]
fn stop_capture(capture: CaptureHandle) {
    capture.running.store(false, Ordering::Release);
    let _ = capture.worker.join();
}

#[cfg(target_os = "windows")]
fn stop_active(state: &CaptureState) -> Result<(), String> {
    let active = state
        .active
        .lock()
        .map_err(|_| "capture-state-unavailable")?
        .take();
    if let Some(capture) = active {
        stop_capture(capture);
    }
    Ok(())
}
#[cfg(target_os = "windows")]
fn install<F>(state: &CaptureState, body: F) -> Result<(), String>
where
    F: FnOnce(Arc<AtomicBool>) + Send + 'static,
{
    // Keep the state lock through stop + install. Without this, two concurrent
    // start commands can both observe no active worker and the later one can
    // overwrite the former handle, orphaning a WASAPI thread.
    let mut active = state
        .active
        .lock()
        .map_err(|_| "capture-state-unavailable")?;
    if let Some(capture) = active.take() {
        stop_capture(capture);
    }
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = running.clone();
    let worker = std::thread::spawn(move || body(thread_running));
    *active = Some(CaptureHandle { running, worker });
    Ok(())
}

#[cfg(target_os = "windows")]
impl Drop for CaptureState {
    fn drop(&mut self) {
        if let Ok(active) = self.active.get_mut() {
            if let Some(capture) = active.take() {
                stop_capture(capture);
            }
        }
    }
}

#[tauri::command]
#[cfg(target_os = "windows")]
fn start_system_audio(
    on_audio: Channel<AudioChunk>,
    state: State<'_, CaptureState>,
) -> Result<(), String> {
    capture::loopback::preflight_default_render_device().map_err(|_| "no-default-render-device")?;
    // shortcut: exclusion is resolved at start; restart sharing after launching/restarting Discord.
    let excluded_pid = capture::process::discord_process_id()?;
    install(&state, move |running| {
        let _ = capture::loopback::capture_loop(excluded_pid, running, |chunk| {
            on_audio.send(chunk).is_ok()
        });
        // The worker cannot return an error through a command that has already
        // completed.  Always close the local WebCodecs track instead of leaving
        // a stale native-audio sender after a device/capture failure.
        let _ = on_audio.send(AudioChunk::terminal());
    })
}

#[tauri::command]
#[cfg(target_os = "windows")]
fn start_process_audio(
    process_id: u32,
    include_process_tree: bool,
    on_audio: Channel<AudioChunk>,
    state: State<'_, CaptureState>,
) -> Result<(), String> {
    if !capture::process::process_exists(process_id) {
        return Err("process-not-found".into());
    }
    install(&state, move |running| {
        let _ = capture::process::capture_process_loop(
            process_id,
            include_process_tree,
            running,
            |chunk| on_audio.send(chunk).is_ok(),
        );
        // As above, report completion without exposing native error details.
        // This is a control marker only; it contains no captured samples.
        let _ = on_audio.send(AudioChunk::terminal());
    })
}
#[tauri::command]
#[cfg(target_os = "windows")]
fn stop_audio(state: State<'_, CaptureState>) -> Result<(), String> {
    stop_active(&state)
}
#[tauri::command]
#[cfg(target_os = "windows")]
fn enumerate_sources() -> Vec<Source> {
    capture::sources::enumerate_sources()
}
#[tauri::command]
#[cfg(target_os = "windows")]
fn capture_thumbnail(
    hwnd: u64,
    rect: Option<(i32, i32, i32, i32)>,
) -> Result<Option<Thumbnail>, String> {
    capture::thumb::capture_thumbnail(
        windows::Win32::Foundation::HWND(hwnd as usize as *mut _),
        rect.map(
            |(left, top, right, bottom)| windows::Win32::Foundation::RECT {
                left,
                top,
                right,
                bottom,
            },
        ),
        capture::thumb::MAX_THUMB_DIM,
    )
}

/// Which mode the single window is in. Only the compact tray panel hides on blur.
#[derive(Default)]
struct PanelState {
    compact: bool,
    shown_at: Option<Instant>,
    blurred_at: Option<Instant>,
}

// A blur queued before the panel finished showing must not close it again.
const PANEL_SHOW_GRACE: Duration = Duration::from_millis(250);
// Pressing the tray icon blurs the panel first; the click that follows must not reopen it.
const PANEL_REOPEN_GUARD: Duration = Duration::from_millis(400);

// Passed by the login item so a login launch stays in the tray.
const AUTOSTART_ARG: &str = "--autostart";

#[tauri::command]
fn launched_at_login() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

/// The dock icon (and Cmd+Tab entry) follows the full window: an app living in the tray has neither.
fn set_dock_visible(app: &tauri::AppHandle, visible: bool) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(if visible {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    });
    #[cfg(not(target_os = "macos"))]
    let _ = (app, visible);
}

fn hide_window(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    window.hide()?;
    set_dock_visible(window.app_handle(), false);
    window.emit("viewer-mode", "hidden")
}

#[cfg(target_os = "windows")]
struct VideoCaptureState {
    capture: CaptureState,
    metrics: Mutex<Option<Arc<capture::video::VideoMetrics>>>,
    generation: std::sync::atomic::AtomicU64,
    // Shared with the running capture loop, which reads it every tick.
    fps: Arc<std::sync::atomic::AtomicU32>,
}

#[cfg(target_os = "windows")]
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoRequest {
    hwnd: Option<u64>,
    rect: Option<(i32, i32, u32, u32)>,
    fps: u32,
}

/// Frames travel through WebView2 shared memory; the channel only announces them
/// (raw frame bytes over IPC were measured at ~8 fps for 1080p and rejected).
#[tauri::command]
#[cfg(target_os = "windows")]
async fn start_video_capture(
    window: tauri::WebviewWindow,
    request: VideoRequest,
    on_frame: Channel,
    state: State<'_, VideoCaptureState>,
) -> Result<(), String> {
    use capture::video::{capture_video_loop, VideoMetrics, VideoSource};
    use tauri::ipc::InvokeResponseBody;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    };
    let source = match (request.hwnd, request.rect) {
        (Some(hwnd), _) => {
            VideoSource::Window(windows::Win32::Foundation::HWND(hwnd as usize as *mut _))
        }
        (None, Some((left, top, width, height))) => VideoSource::Screen {
            left,
            top,
            width,
            height,
        },
        _ => return Err("invalid-source".into()),
    };
    // The previous worker must stop before its shared memory is replaced.
    stop_active(&state.capture)?;
    let generation = state.generation.fetch_add(1, Ordering::AcqRel) + 1;
    // A window can grow up to the virtual screen while it is captured.
    let max_bytes = unsafe {
        GetSystemMetrics(SM_CXVIRTUALSCREEN) as usize
            * GetSystemMetrics(SM_CYVIRTUALSCREEN) as usize
            * 4
    };
    let (tx, rx) = std::sync::mpsc::channel();
    window
        .with_webview(move |webview| {
            let _ = tx.send(capture::shared::create_ring(
                &webview, generation, max_bytes,
            ));
        })
        .map_err(|_| "shared-buffer-unsupported")?;
    let slots = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "shared-buffer-unsupported")??;
    let metrics = Arc::new(VideoMetrics::default());
    *state
        .metrics
        .lock()
        .map_err(|_| "capture-state-unavailable")? = Some(metrics.clone());
    let app = window.app_handle().clone();
    state.fps.store(request.fps, Ordering::Relaxed);
    let fps = state.fps.clone();
    install(&state.capture, move |running| {
        let result = capture_video_loop(source, fps, running, metrics, |frame| {
            let Some(slot) = slots.fill(frame.data) else {
                return false;
            };
            on_frame
                .send(InvokeResponseBody::Json(format!(
                    r#"{{"generation":{generation},"slot":{slot},"width":{},"height":{},"timestampUs":{},"wallUs":{}}}"#,
                    frame.width, frame.height, frame.timestamp_us, frame.wall_us
                )))
                .is_ok()
        });
        let _ = app.run_on_main_thread(move || capture::shared::release_ring(generation));
        let error = result.err().unwrap_or_default();
        let _ = on_frame.send(InvokeResponseBody::Json(format!(
            r#"{{"ended":true,"error":{}}}"#,
            serde_json::Value::String(error)
        )));
    })
}

#[tauri::command]
#[cfg(target_os = "windows")]
fn stop_video_capture(state: State<'_, VideoCaptureState>) -> Result<(), String> {
    stop_active(&state.capture)
}

/// Changes the running capture's frame rate without restarting it.
#[tauri::command]
#[cfg(target_os = "windows")]
fn set_video_capture_fps(fps: u32, state: State<'_, VideoCaptureState>) {
    state.fps.store(fps, Ordering::Relaxed);
}

#[tauri::command]
#[cfg(target_os = "windows")]
fn video_capture_metrics(state: State<'_, VideoCaptureState>) -> serde_json::Value {
    let Some(m) = state.metrics.lock().ok().and_then(|m| m.clone()) else {
        return serde_json::Value::Null;
    };
    let get = |value: &std::sync::atomic::AtomicU64| value.load(Ordering::Relaxed);
    serde_json::json!({
        "ticks": get(&m.ticks),
        "newFrames": get(&m.new_frames),
        "delivered": get(&m.delivered),
        "droppedBusy": get(&m.dropped_busy),
        "lateTicks": get(&m.late_ticks),
        "captureUsTotal": get(&m.capture_us_total),
        "captureUsMax": get(&m.capture_us_max),
        "minimizedTicks": get(&m.minimized_ticks),
    })
}

#[tauri::command]
fn hide_main_window(window: tauri::WebviewWindow) -> Result<(), String> {
    hide_window(&window).map_err(|_| "window-hide-failed".to_string())
}

/// Apps cannot grant themselves camera/mic access; the best we can do is open the OS page.
#[tauri::command]
fn open_privacy_settings(kind: String) -> Result<(), String> {
    let url = match (kind.as_str(), cfg!(target_os = "macos")) {
        ("camera", false) => "ms-settings:privacy-webcam",
        ("microphone", false) => "ms-settings:privacy-microphone",
        ("camera", true) => "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera",
        ("microphone", true) => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        _ => return Err("unsupported-kind".into()),
    };
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        return Err("unsupported-platform".into());
    };
    std::process::Command::new(opener)
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|_| "open-settings-failed".to_string())
}

#[tauri::command]
fn show_main_window(window: tauri::WebviewWindow) -> Result<(), String> {
    present_window(&window, false, None)
}

fn hide_panel_on_blur(window: &tauri::WebviewWindow) {
    let state = window.state::<Mutex<PanelState>>();
    let Ok(mut panel) = state.lock() else { return };
    let settled = panel
        .shown_at
        .is_some_and(|shown| shown.elapsed() >= PANEL_SHOW_GRACE);
    if !panel.compact || !settled || !window.is_visible().unwrap_or(false) {
        return;
    }
    panel.blurred_at = Some(Instant::now());
    drop(panel);
    let _ = hide_window(window);
}

fn toggle_tray_panel(window: &tauri::WebviewWindow, rect: tauri::Rect) -> Result<(), String> {
    let visible = window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false);
    let (compact, just_blurred) = window
        .state::<Mutex<PanelState>>()
        .lock()
        .map(|panel| {
            (
                panel.compact,
                panel
                    .blurred_at
                    .is_some_and(|blurred| blurred.elapsed() < PANEL_REOPEN_GUARD),
            )
        })
        .map_err(|_| "panel-state-unavailable")?;
    match tray_click_action(visible, compact, just_blurred) {
        TrayClick::FocusFull => window.set_focus().map_err(|_| "window-show-failed".into()),
        TrayClick::HidePanel => hide_window(window).map_err(|_| "window-hide-failed".into()),
        TrayClick::Ignore => Ok(()),
        TrayClick::OpenPanel => present_window(window, true, Some(rect)),
    }
}

#[derive(Debug, PartialEq)]
enum TrayClick {
    FocusFull,
    HidePanel,
    Ignore,
    OpenPanel,
}

fn tray_click_action(visible: bool, compact: bool, just_blurred: bool) -> TrayClick {
    match (visible, compact) {
        (true, false) => TrayClick::FocusFull,
        (true, true) => TrayClick::HidePanel,
        // This click already closed the panel through its blur.
        _ if just_blurred => TrayClick::Ignore,
        _ => TrayClick::OpenPanel,
    }
}

fn present_window(
    window: &tauri::WebviewWindow,
    compact: bool,
    tray_rect: Option<tauri::Rect>,
) -> Result<(), String> {
    // Switch the mode before touching the window so blur events raised by the
    // transition itself are judged against the new mode.
    if let Ok(mut panel) = window.state::<Mutex<PanelState>>().lock() {
        panel.compact = compact;
        panel.shown_at = Some(Instant::now());
    }
    // Each mode owns these properties; restore them on every switch.
    window
        .set_decorations(!compact)
        .and_then(|_| window.set_resizable(!compact))
        .and_then(|_| window.set_skip_taskbar(compact))
        .and_then(|_| window.set_always_on_top(compact))
        .map_err(|_| "window-mode-failed")?;
    let (mut width, mut height, min_width, min_height): (f64, f64, f64, f64) = if compact {
        (430.0, 660.0, 390.0, 520.0)
    } else {
        (1280.0, 800.0, 960.0, 640.0)
    };
    let scale = window
        .scale_factor()
        .map_err(|_| "window-monitor-unavailable")?;
    let monitor = if let Some(rect) = tray_rect {
        let position = rect.position.to_physical::<f64>(scale);
        window.monitor_from_point(position.x, position.y)
    } else {
        window.current_monitor()
    }
    .map_err(|_| "window-monitor-unavailable")?;
    let mut frame = (0.0, 0.0);
    if let Some(monitor) = monitor {
        let available = monitor
            .work_area()
            .size
            .to_logical::<f64>(monitor.scale_factor());
        let outer = window
            .outer_size()
            .map_err(|_| "window-size-unavailable")?
            .to_logical::<f64>(scale);
        let inner = window
            .inner_size()
            .map_err(|_| "window-size-unavailable")?
            .to_logical::<f64>(scale);
        // The undecorated panel has no frame; its outer size may still report the old decorations.
        if !compact {
            frame = (outer.width - inner.width, outer.height - inner.height);
        }
        width = width.min((available.width - frame.0).max(1.0));
        height = height.min((available.height - frame.1).max(1.0));
    }
    // outer_size() still reports the old size right after set_size on macOS, so anchor with the requested one.
    let target =
        tauri::LogicalSize::new(width + frame.0, height + frame.1).to_physical::<f64>(scale);
    window
        .unminimize()
        .and_then(|_| window.unmaximize())
        .and_then(|_| window.emit("viewer-mode", if compact { "compact" } else { "room" }))
        .and_then(|_| {
            window.set_min_size(Some(tauri::LogicalSize::new(
                min_width.min(width),
                min_height.min(height),
            )))
        })
        .and_then(|_| window.set_size(tauri::LogicalSize::new(width, height)))
        .and_then(|_| if compact { Ok(()) } else { window.center() })
        .and_then(|_| {
            if let Some(rect) = tray_rect {
                position_window_near_tray(window, rect, target)
            } else {
                Ok(())
            }
        })
        .and_then(|_| {
            set_dock_visible(window.app_handle(), !compact);
            window.show()
        })
        .and_then(|_| window.set_focus())
        .map_err(|_| "window-show-failed".to_string())
}

fn position_window_near_tray(
    window: &tauri::WebviewWindow,
    rect: tauri::Rect,
    window_size: tauri::PhysicalSize<f64>,
) -> tauri::Result<()> {
    use tauri::PhysicalPosition;

    let scale = window.scale_factor()?;
    let tray_position = rect.position.to_physical::<f64>(scale);
    let tray_size = rect.size.to_physical::<f64>(scale);
    let tray_left = tray_position.x;
    let tray_top = tray_position.y;
    let tray_width = tray_size.width;
    let tray_height = tray_size.height;
    let monitor = window
        .monitor_from_point(tray_left, tray_top)?
        .or(window.primary_monitor()?);

    let (x, y) = if let Some(monitor) = monitor {
        let area = monitor.work_area();
        tray_window_position(
            (tray_left, tray_top, tray_width, tray_height),
            (window_size.width, window_size.height),
            (
                area.position.x as f64,
                area.position.y as f64,
                area.size.width as f64,
                area.size.height as f64,
            ),
        )
    } else {
        (
            tray_left + tray_width - window_size.width,
            tray_top + tray_height,
        )
    };

    window.set_position(PhysicalPosition::new(x as i32, y as i32))
}

fn tray_window_position(
    tray: (f64, f64, f64, f64),
    window: (f64, f64),
    area: (f64, f64, f64, f64),
) -> (f64, f64) {
    let x = (tray.0 + tray.2 - window.0).clamp(area.0, area.0 + (area.2 - window.0).max(0.0));
    let y = if tray.1 >= area.1 + area.3 / 2.0 {
        tray.1 - window.1
    } else {
        tray.1 + tray.3
    }
    .clamp(area.1, area.1 + (area.3 - window.1).max(0.0));
    (x, y)
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(Mutex::new(PanelState::default()))
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = present_window(&window, false, None);
            }
        }));
    #[cfg(target_os = "windows")]
    let builder = builder
        .manage(CaptureState {
            active: Mutex::new(None),
        })
        .manage(VideoCaptureState {
            capture: CaptureState {
                active: Mutex::new(None),
            },
            metrics: Mutex::new(None),
            generation: Default::default(),
            fps: Default::default(),
        })
        .invoke_handler(tauri::generate_handler![
            start_system_audio,
            start_process_audio,
            stop_audio,
            start_video_capture,
            stop_video_capture,
            set_video_capture_fps,
            video_capture_metrics,
            enumerate_sources,
            capture_thumbnail,
            hide_main_window,
            show_main_window,
            open_privacy_settings,
            launched_at_login,
            quit_app,
            set_tray_state
        ]);
    #[cfg(target_os = "macos")]
    let builder = builder
        .manage(mac_commands::MacCaptureState::default())
        .invoke_handler(tauri::generate_handler![
            mac_commands::start_system_audio,
            mac_commands::start_process_audio,
            mac_commands::stop_audio,
            mac_commands::start_video_capture,
            mac_commands::stop_video_capture,
            mac_commands::set_video_capture_fps,
            mac_commands::enumerate_sources,
            mac_commands::capture_thumbnail,
            hide_main_window,
            show_main_window,
            open_privacy_settings,
            launched_at_login,
            quit_app,
            set_tray_state
        ]);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        hide_main_window,
        show_main_window,
        open_privacy_settings,
        launched_at_login,
        quit_app,
        set_tray_state
    ]);
    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            session_media::install(app.handle());
            use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
            app.handle().plugin(tauri_plugin_autostart::init(
                MacosLauncher::LaunchAgent,
                Some(vec![AUTOSTART_ARG]),
            ))?;
            // Login items registered by older versions lack the flag; rewrite them.
            let autolaunch = app.autolaunch();
            if autolaunch.is_enabled().unwrap_or(false) {
                let _ = autolaunch.enable();
            }
            let open = MenuItem::with_id(app, "open", "Abrir sala", true, None::<&str>)?;
            let mic = MenuItem::with_id(
                app,
                "mic",
                "Microfone silenciado · Ativar",
                false,
                None::<&str>,
            )?;
            let camera = MenuItem::with_id(
                app,
                "camera",
                "Câmera desligada · Ligar",
                false,
                None::<&str>,
            )?;
            let output = MenuItem::with_id(app, "output", "Silenciar áudio", true, None::<&str>)?;
            let q720 =
                CheckMenuItem::with_id(app, "quality720", "720p", true, false, None::<&str>)?;
            let q1080 =
                CheckMenuItem::with_id(app, "quality1080", "1080p", true, true, None::<&str>)?;
            let q4k = CheckMenuItem::with_id(app, "quality4k", "4K", true, false, None::<&str>)?;
            let screen = MenuItem::with_id(
                app,
                "screen",
                "Compartilhar tela ou janela…",
                false,
                None::<&str>,
            )?;
            let leave = MenuItem::with_id(app, "leave", "Sair da sala", false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Encerrar app", true, None::<&str>)?;
            let quality = Submenu::with_items(
                app,
                "Qualidade da próxima tela",
                true,
                &[&q720, &q1080, &q4k],
            )?;
            let media_separator = PredefinedMenuItem::separator(app)?;
            let room_separator = PredefinedMenuItem::separator(app)?;
            let quit_separator = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(
                app,
                &[
                    &open,
                    &media_separator,
                    &mic,
                    &camera,
                    &output,
                    &screen,
                    &quality,
                    &room_separator,
                    &leave,
                    &quit_separator,
                    &quit,
                ],
            )?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or("missing app icon")?;
            let tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = toggle_tray_panel(&window, rect);
                        }
                    }
                })
                // Flows that need a form open the full window from the frontend,
                // which knows whether to hide it again afterwards.
                .on_menu_event(|app, event| {
                    let action = event.id().as_ref();
                    if action == "quit" {
                        app.exit(0);
                    } else if let Some(window) = app.get_webview_window("main") {
                        if action == "open" {
                            let _ = present_window(&window, false, None);
                        }
                        let _ = app.emit("tray-action", action);
                    }
                })
                .build(app)?;
            app.manage(Mutex::new(TrayUiState {
                tray,
                mic,
                camera,
                screen,
                output,
                leave,
                qualities: [q720, q1080, q4k],
                participants: 0,
                remote_sharing: false,
                sharing: None,
            }));

            if let Some(window) = app.get_webview_window("main") {
                let handle = window.clone();
                window.on_window_event(move |event| match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = hide_window(&handle);
                    }
                    tauri::WindowEvent::Focused(false) => hide_panel_on_blur(&handle),
                    _ => {}
                });
                // The window starts hidden so a login launch never flashes it; any other launch opens it.
                if launched_at_login() {
                    set_dock_visible(app.handle(), false);
                } else {
                    let _ = present_window(&window, false, None);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Screenshare Desktop");
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Barrier;

    #[test]
    fn tray_panel_on_small_monitor_keeps_top_left_accessible() {
        assert_eq!(
            tray_window_position(
                (600.0, 450.0, 20.0, 20.0),
                (700.0, 680.0),
                (0.0, 0.0, 640.0, 480.0)
            ),
            (0.0, 0.0)
        );
    }

    #[test]
    fn tray_panel_stays_in_work_area_on_negative_monitor() {
        assert_eq!(
            tray_window_position(
                (-30.0, 1040.0, 20.0, 40.0),
                (430.0, 660.0),
                (-1920.0, 0.0, 1920.0, 1040.0)
            ),
            (-440.0, 380.0)
        );
        assert_eq!(
            tray_window_position(
                (-1900.0, 0.0, 20.0, 30.0),
                (430.0, 660.0),
                (-1920.0, 30.0, 1920.0, 1050.0)
            ),
            (-1920.0, 30.0)
        );
    }

    #[test]
    fn long_shared_titles_are_shortened_on_character_boundaries() {
        assert_eq!(short_title("Spotify"), "Spotify");
        let long = "á".repeat(50);
        let short = short_title(&long);
        assert_eq!(short.chars().count(), 40);
        assert!(short.ends_with('…'));
    }

    #[test]
    fn tray_click_focuses_full_window_and_toggles_panel() {
        assert_eq!(tray_click_action(true, false, false), TrayClick::FocusFull);
        assert_eq!(tray_click_action(true, true, false), TrayClick::HidePanel);
        assert_eq!(tray_click_action(false, true, true), TrayClick::Ignore);
        assert_eq!(tray_click_action(false, true, false), TrayClick::OpenPanel);
        // A hidden full window opens as the panel.
        assert_eq!(tray_click_action(false, false, false), TrayClick::OpenPanel);
    }

    #[test]
    fn installing_a_capture_joins_the_previous_worker() {
        let state = CaptureState {
            active: Mutex::new(None),
        };
        let started = Arc::new(AtomicUsize::new(0));
        let stopped = Arc::new(AtomicUsize::new(0));
        let first_started = started.clone();
        let first_stopped = stopped.clone();
        install(&state, move |running| {
            first_started.fetch_add(1, Ordering::Release);
            while running.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            first_stopped.fetch_add(1, Ordering::Release);
        })
        .unwrap();
        while started.load(Ordering::Acquire) == 0 {
            std::thread::yield_now();
        }

        install(&state, |_| {}).unwrap();

        assert_eq!(stopped.load(Ordering::Acquire), 1);
        stop_active(&state).unwrap();
    }

    #[test]
    fn concurrent_installs_leave_no_worker_orphaned() {
        let state = Arc::new(CaptureState {
            active: Mutex::new(None),
        });
        let gate = Arc::new(Barrier::new(3));
        let started = Arc::new(AtomicUsize::new(0));
        let mut callers = Vec::new();

        for _ in 0..2 {
            let state = state.clone();
            let gate = gate.clone();
            let started = started.clone();
            callers.push(std::thread::spawn(move || {
                gate.wait();
                install(&state, move |running| {
                    started.fetch_add(1, Ordering::Release);
                    while running.load(Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                })
                .unwrap();
            }));
        }

        gate.wait();
        callers
            .into_iter()
            .for_each(|caller| caller.join().unwrap());
        stop_active(&state).unwrap();
        assert_eq!(started.load(Ordering::Acquire), 2);
    }
}
