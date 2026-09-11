mod capture;
use capture::{sources::Source, thumb::Thumbnail, AudioChunk};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{ipc::Channel, State};
struct CaptureHandle {
    running: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<()>,
}
struct CaptureState {
    active: Mutex<Option<CaptureHandle>>,
}

fn stop_capture(capture: CaptureHandle) {
    capture.running.store(false, Ordering::Release);
    let _ = capture.worker.join();
}

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
fn stop_audio(state: State<'_, CaptureState>) -> Result<(), String> {
    stop_active(&state)
}
#[tauri::command]
fn enumerate_sources() -> Vec<Source> {
    capture::sources::enumerate_sources()
}
#[tauri::command]
fn capture_thumbnail(hwnd: u64) -> Result<Option<Thumbnail>, String> {
    capture::thumb::capture_thumbnail(
        windows::Win32::Foundation::HWND(hwnd as usize as *mut _),
        capture::thumb::MAX_THUMB_DIM,
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(CaptureState {
            active: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            start_system_audio,
            start_process_audio,
            stop_audio,
            enumerate_sources,
            capture_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running Screenshare Desktop");
}

#[cfg(test)]
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
