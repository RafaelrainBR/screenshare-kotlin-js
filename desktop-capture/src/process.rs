//! WASAPI application loopback.  It deliberately has no knowledge of Tauri,
//! rooms, signaling, or WebRTC.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wasapi::{AudioClient, Direction, SampleType, StreamMode, WasapiError, WaveFormat};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::ERROR_NO_MORE_FILES;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use super::{AudioChunk, AudioClock, BYTES_PER_CHUNK};

fn discord_root(processes: &[(u32, u32, String)]) -> Result<Option<u32>, String> {
    let discord: Vec<_> = processes
        .iter()
        .filter(|(_, _, name)| {
            [
                "discord.exe",
                "discordptb.exe",
                "discordcanary.exe",
                "discorddevelopment.exe",
            ]
            .iter()
            .any(|expected| name.eq_ignore_ascii_case(expected))
        })
        .collect();
    let roots: Vec<_> = discord
        .iter()
        .filter(|(_, parent, _)| !discord.iter().any(|(pid, _, _)| pid == parent))
        .map(|(pid, _, _)| *pid)
        .collect();
    match roots.as_slice() {
        [] if discord.is_empty() => Ok(None),
        [pid] => Ok(Some(*pid)),
        // WASAPI can exclude one tree only. Never pick one and leak another call.
        _ => Err("discord-multiple-process-trees".into()),
    }
}

pub fn discord_process_id() -> Result<Option<u32>, String> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|_| "capture-process-list-unavailable")?;
    let result = (|| {
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        unsafe { Process32FirstW(snapshot, &mut entry) }
            .map_err(|_| "capture-process-list-unavailable")?;
        let mut processes = Vec::new();
        loop {
            let len = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            processes.push((
                entry.th32ProcessID,
                entry.th32ParentProcessID,
                String::from_utf16_lossy(&entry.szExeFile[..len]),
            ));
            match unsafe { Process32NextW(snapshot, &mut entry) } {
                Ok(()) => {}
                Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(_) => return Err("capture-process-list-unavailable".into()),
            }
        }
        discord_root(&processes)
    })();
    let _ = unsafe { CloseHandle(snapshot) };
    result
}

pub fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let _ = unsafe { CloseHandle(handle) };
    true
}

/// Includes the target process tree when true, excludes it when false. A target
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
    fn discord_detection_selects_root_not_audio_helper() {
        let processes = vec![
            (22, 11, "Discord.exe".into()),
            (11, 1, "DISCORD.EXE".into()),
            (33, 22, "Discord.exe".into()),
            (1, 0, "explorer.exe".into()),
        ];
        assert_eq!(discord_root(&processes), Ok(Some(11)));
        for name in [
            "DiscordPTB.exe",
            "DiscordCanary.exe",
            "DiscordDevelopment.exe",
        ] {
            assert_eq!(discord_root(&[(7, 1, name.into())]), Ok(Some(7)));
        }
    }
    #[test]
    fn discord_detection_does_not_match_browser_or_similar_names() {
        assert_eq!(
            discord_root(&[
                (1, 0, "chrome.exe".into()),
                (2, 0, "not-discord.exe".into())
            ]),
            Ok(None)
        );
    }
    #[test]
    fn discord_detection_rejects_independent_instances() {
        assert_eq!(
            discord_root(&[
                (1, 0, "Discord.exe".into()),
                (2, 0, "DiscordCanary.exe".into())
            ]),
            Err("discord-multiple-process-trees".into())
        );
    }
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
