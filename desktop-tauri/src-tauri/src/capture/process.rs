//! WASAPI application loopback.  It deliberately has no knowledge of Tauri,
//! rooms, signaling, or WebRTC.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wasapi::{AudioClient, Direction, SampleType, StreamMode, WasapiError, WaveFormat};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use super::{AudioChunk, AudioClock, BYTES_PER_CHUNK};

pub(crate) fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let _ = unsafe { CloseHandle(handle) };
    true
}

/// Captures an application and, optionally, its child-process tree.  A target
/// that is alive but currently silent remains a valid capture: zero PCM chunks
/// keep the WebCodecs track alive without blocking the UI.
pub fn capture_process_loop<F>(
    pid: u32,
    include_process_tree: bool,
    running: Arc<AtomicBool>,
    mut on_chunk: F,
) -> Result<(), String>
where
    F: FnMut(AudioChunk) -> bool,
{
    if !process_exists(pid) {
        return Err("process-not-found".into());
    }
    let _ = wasapi::initialize_mta();
    let mut client = AudioClient::new_application_loopback_client(pid, include_process_tree)
        .map_err(|e| format!("process-loopback-activation: {e}"))?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
    client
        .initialize_client(
            &format,
            &Direction::Capture,
            &StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 100_000,
            },
        )
        .map_err(|e| format!("process-loopback-initialize: {e}"))?;
    let event = client
        .set_get_eventhandle()
        .map_err(|e| format!("process-loopback-event: {e}"))?;
    let capture = client
        .get_audiocaptureclient()
        .map_err(|e| format!("process-loopback-client: {e}"))?;
    client
        .start_stream()
        .map_err(|e| format!("process-loopback-start: {e}"))?;
    let mut clock = AudioClock::new();
    let mut bytes = VecDeque::new();
    while running.load(Ordering::Acquire) {
        match event.wait_for_event(100) {
            Ok(()) => {
                capture
                    .read_from_device_to_deque(&mut bytes)
                    .map_err(|e| format!("process-loopback-read: {e}"))?;
                while bytes.len() >= BYTES_PER_CHUNK && running.load(Ordering::Acquire) {
                    let data = bytes.drain(..BYTES_PER_CHUNK).collect();
                    if !on_chunk(AudioChunk {
                        timestamp_us: clock.timestamp_us(),
                        data,
                        ended: None,
                    }) {
                        running.store(false, Ordering::Release);
                    }
                }
            }
            Err(WasapiError::EventTimeout) => {
                if !process_exists(pid) {
                    break;
                }
                // 100 ms elapsed: provide ten 10-ms silence chunks, preserving cadence.
                for _ in 0..10 {
                    if !running.load(Ordering::Acquire)
                        || !on_chunk(AudioChunk {
                            timestamp_us: clock.timestamp_us(),
                            data: vec![0; BYTES_PER_CHUNK],
                            ended: None,
                        })
                    {
                        running.store(false, Ordering::Release);
                        break;
                    }
                }
            }
            Err(e) => {
                let _ = client.stop_stream();
                return Err(format!("process-loopback-event: {e}"));
            }
        }
    }
    let _ = client.stop_stream();
    // `lib.rs` emits the terminal marker after every worker exit, including a
    // vanished target and capture errors. Keeping it at that boundary prevents
    // duplicate end markers and keeps this module independent of IPC policy.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonexistent_pid_is_rejected_immediately() {
        assert!(!process_exists(u32::MAX));
    }
    #[test]
    fn stopped_flag_ends_without_chunks() {
        let running = Arc::new(AtomicBool::new(false));
        assert!(
            capture_process_loop(std::process::id(), true, running, |_| panic!(
                "no chunks expected"
            ))
            .is_ok()
        );
    }
}
