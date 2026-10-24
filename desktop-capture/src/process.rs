//! WASAPI application loopback.  It deliberately has no knowledge of Tauri,
//! rooms, signaling, or WebRTC.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wasapi::{
    AudioCaptureClient, AudioClient, Direction, Handle, SampleType, StreamMode, WasapiError, WaveFormat,
};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::ERROR_NO_MORE_FILES;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use super::{AudioChunk, AudioClock, BYTES_PER_CHUNK};

/// Call and chat apps: their sound is the conversation itself, which viewers either hear
/// already or must not hear. Steam stays out on purpose: it is the parent of its games.
const EXCLUDED_APPS: &[&str] = &[
    "discord.exe",
    "discordptb.exe",
    "discordcanary.exe",
    "discorddevelopment.exe",
    "ms-teams.exe",
    "teams.exe",
    "zoom.exe",
    "skype.exe",
    "slack.exe",
    "ts3client_win64.exe",
    "ts3client_win32.exe",
    "teamspeak.exe",
    "mumble.exe",
    "whatsapp.exe",
    "whatsapp.root.exe",
    "telegram.exe",
    "signal.exe",
    "element.exe",
    "guilded.exe",
    // Another Screenshare build plays the room too; installed and portable names start with "screenshare".
    "desktop-native.exe",
];

/// A browser opened from a link in a call app is that app's child, but it is its own app.
const BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
];

/// (pid, parent pid, exe name) of every running process.
pub type ProcessList = Vec<(u32, u32, String)>;

/// `pid` and its parents, nearest first. Parent ids can be stale or reused, so the walk is bounded.
fn lineage(pid: u32, processes: &[(u32, u32, String)]) -> impl Iterator<Item = &(u32, u32, String)> {
    std::iter::successors(processes.iter().find(|p| p.0 == pid), move |(_, parent, _)| {
        processes.iter().find(|p| p.0 == *parent && p.0 != 0)
    })
    .take(64)
}

fn is_excluded(pid: u32, processes: &[(u32, u32, String)], own_pid: u32) -> bool {
    for (pid, _, name) in lineage(pid, processes) {
        let name = name.to_ascii_lowercase();
        // Our own tree plays the call and the other screens (WebView2 runs as our child).
        if *pid == own_pid || name.starts_with("screenshare") || EXCLUDED_APPS.contains(&name.as_str()) {
            return true;
        }
        if BROWSERS.contains(&name.as_str()) {
            return false;
        }
    }
    false
}

/// The process trees to capture for "the computer's sound": every audio session owner that
/// is not excluded, minus the ones already inside another captured tree. WASAPI can exclude
/// a single tree only, so the share is built from the apps that are kept instead.
pub fn capture_targets(session_pids: &[u32], processes: &[(u32, u32, String)], own_pid: u32) -> Vec<u32> {
    let mut kept: Vec<u32> = session_pids
        .iter()
        .copied()
        .filter(|&pid| pid != 0 && !is_excluded(pid, processes, own_pid))
        .collect();
    kept.sort_unstable();
    kept.dedup();
    kept.iter()
        .copied()
        .filter(|&pid| !lineage(pid, processes).skip(1).any(|(parent, _, _)| kept.contains(parent)))
        .collect()
}

pub fn process_list() -> Result<ProcessList, String> {
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
        Ok(processes)
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

/// Starts a 48 kHz stereo f32 loopback of `pid`'s tree, or of everything but it.
pub fn open_process_loopback(
    pid: u32,
    include_process_tree: bool,
) -> Result<(AudioClient, Handle, AudioCaptureClient), String> {
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
    Ok((client, event, capture))
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
    let (client, event, capture) = open_process_loopback(pid, include_process_tree)?;
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
    fn tree() -> ProcessList {
        vec![
            (1, 0, "explorer.exe".into()),
            (10, 1, "Discord.exe".into()),
            (11, 10, "Discord.exe".into()),
            (12, 10, "chrome.exe".into()),
            (13, 12, "chrome.exe".into()),
            (20, 1, "ms-teams.exe".into()),
            (21, 20, "msedgewebview2.exe".into()),
            (30, 1, "Spotify.exe".into()),
            (40, 1, "steam.exe".into()),
            (41, 40, "game.exe".into()),
            (50, 1, "Screenshare.exe".into()),
            (51, 50, "msedgewebview2.exe".into()),
            (60, 1, "desktop-native.exe".into()),
        ]
    }
    #[test]
    fn call_apps_and_screenshare_are_left_out_with_their_helpers() {
        assert_eq!(capture_targets(&[11, 21, 51, 60, 30], &tree(), 99), vec![30]);
        // The running app is excluded by pid, whatever its exe is called.
        assert_eq!(capture_targets(&[30], &tree(), 30), Vec::<u32>::new());
    }
    #[test]
    fn a_browser_opened_from_discord_and_steam_games_still_play() {
        assert_eq!(capture_targets(&[13, 41], &tree(), 99), vec![13, 41]);
    }
    #[test]
    fn a_tree_already_captured_is_not_captured_twice() {
        assert_eq!(capture_targets(&[13, 12, 12, 0], &tree(), 99), vec![12]);
    }
    #[test]
    fn an_unknown_session_owner_is_still_captured() {
        assert_eq!(capture_targets(&[777], &tree(), 99), vec![777]);
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
