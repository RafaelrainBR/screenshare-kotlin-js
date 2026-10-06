#[cfg(target_os = "windows")]
mod capture;
#[cfg(target_os = "macos")]
mod session_media;
#[cfg(target_os = "windows")]
use capture::{sources::Source, thumb::Thumbnail, AudioChunk};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use std::sync::Arc;
use tauri::{menu::{Menu, MenuItem}, tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent}, Emitter, Manager, State};
#[cfg(target_os = "windows")]
use tauri::ipc::Channel;

struct TrayUiState {
    tray: TrayIcon,
    mic: MenuItem<tauri::Wry>,
    camera: MenuItem<tauri::Wry>,
    screen: MenuItem<tauri::Wry>,
    participants: u32,
    mic_active: bool,
    camera_active: bool,
    screen_active: bool,
    remote_sharing: bool,
}

#[tauri::command]
fn set_tray_state(
    kind: String,
    active: bool,
    participants: Option<u32>,
    state: State<'_, Mutex<TrayUiState>>,
) -> Result<(), String> {
    let mut tray = state.lock().map_err(|_| "tray-state-unavailable")?;
    if let Some(count) = participants {
        tray.participants = count;
    }
    match kind.as_str() {
        "mic" => {
            tray.mic_active = active;
            tray.mic.set_text(if active { "Microfone ativo · Silenciar" } else { "Microfone silenciado · Ativar" })
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "camera" => {
            tray.camera_active = active;
            tray.camera.set_text(if active { "Câmera ativa · Parar" } else { "Câmera desligada · Ligar" })
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "screen" => {
            tray.screen_active = active;
            tray.screen.set_text(if active { "Tela compartilhada · Parar" } else { "Compartilhar tela ou janela…" })
                .map_err(|_| "tray-menu-unavailable")?;
        }
        "remote" => {
            tray.remote_sharing = active;
        }
        "participants" => {}
        _ => return Err("unknown-tray-state".into()),
    }
    let tooltip = if tray.remote_sharing {
        format!("Screenshare · {} participantes · compartilhamento disponível", tray.participants)
    } else {
        format!("Screenshare · {} participantes", tray.participants)
    };
    tray.tray.set_tooltip(Some(tooltip)).map_err(|_| "tray-icon-unavailable".to_string())
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
    install(&state, move |running| {
        let _ = capture::loopback::capture_loop(running, |chunk| on_audio.send(chunk).is_ok());
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
fn capture_thumbnail(hwnd: u64) -> Result<Option<Thumbnail>, String> {
    capture::thumb::capture_thumbnail(
        windows::Win32::Foundation::HWND(hwnd as usize as *mut _),
        capture::thumb::MAX_THUMB_DIM,
    )
}

#[tauri::command]
fn hide_main_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().and_then(|_| window.emit("viewer-mode", "hidden")).map_err(|_| "window-hide-failed".to_string())
}

#[tauri::command]
fn show_main_window(window: tauri::WebviewWindow) -> Result<(), String> {
    present_window(&window, false)
}

fn present_window(window: &tauri::WebviewWindow, compact: bool) -> Result<(), String> {
    let (width, height, min_width, min_height) = if compact { (400.0, 560.0, 360.0, 320.0) } else { (1280.0, 800.0, 960.0, 640.0) };
    window.emit("viewer-mode", if compact { "compact" } else { "room" })
        .and_then(|_| window.set_min_size(Some(tauri::LogicalSize::new(min_width, min_height))))
        .and_then(|_| window.set_size(tauri::LogicalSize::new(width, height)))
        .and_then(|_| window.show())
        .and_then(|_| window.set_focus())
        .map_err(|_| "window-show-failed".to_string())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = present_window(&window, false);
            }
        }));
    #[cfg(target_os = "windows")]
    let builder = builder
        .manage(CaptureState {
            active: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            start_system_audio,
            start_process_audio,
            stop_audio,
            enumerate_sources,
            capture_thumbnail,
            hide_main_window,
            show_main_window,
            quit_app,
            set_tray_state
        ]);
    #[cfg(not(target_os = "windows"))]
    let builder = builder.invoke_handler(tauri::generate_handler![hide_main_window, show_main_window, quit_app, set_tray_state]);
    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            session_media::install(app.handle());
            use tauri_plugin_autostart::MacosLauncher;
            app.handle().plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))?;
            let open = MenuItem::with_id(app, "open", "Abrir sala", true, None::<&str>)?;
            let mic = MenuItem::with_id(app, "mic", "Microfone silenciado · Ativar", true, None::<&str>)?;
            let camera = MenuItem::with_id(app, "camera", "Câmera desligada · Ligar", true, None::<&str>)?;
            let q720 = MenuItem::with_id(app, "quality720", "Qualidade: 720p", true, None::<&str>)?;
            let q1080 = MenuItem::with_id(app, "quality1080", "Qualidade: 1080p", true, None::<&str>)?;
            let q4k = MenuItem::with_id(app, "quality4k", "Qualidade: 4K", true, None::<&str>)?;
            let screen = MenuItem::with_id(app, "screen", "Compartilhar tela ou janela…", true, None::<&str>)?;
            let switch_room = MenuItem::with_id(app, "switch", "Trocar de sala…", true, None::<&str>)?;
            let leave = MenuItem::with_id(app, "leave", "Sair da sala", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Encerrar app", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &mic, &camera, &q720, &q1080, &q4k, &screen, &switch_room, &leave, &quit])?;
            let icon = app.default_window_icon().cloned().ok_or("missing app icon")?;
            let tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = present_window(&window, true);
                        }
                    }
                })
                .on_menu_event(|app, event| {
                    let action = event.id().as_ref();
                    if action == "quit" {
                        app.exit(0);
                    } else if let Some(window) = app.get_webview_window("main") {
                        if action == "open" || action == "switch" {
                            let _ = present_window(&window, false);
                        } else if action == "screen" || action == "camera" {
                            let _ = present_window(&window, true);
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
                participants: 0,
                mic_active: false,
                camera_active: false,
                screen_active: false,
                remote_sharing: false,
            }));

            if let Some(window) = app.get_webview_window("main") {
                let window_to_hide = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_to_hide.hide();
                        let _ = window_to_hide.emit("viewer-mode", "hidden");
                    }
                });
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
