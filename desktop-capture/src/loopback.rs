use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{process, AudioChunk, AudioClock, BYTES_PER_CHUNK, SAMPLE_BYTES};
use wasapi::{AudioSessionManager, DeviceEnumerator, Direction, SessionState, WasapiError};

/// New apps join the share within this long after they open their audio.
const SCAN_EVERY: Duration = Duration::from_secs(1);
const CHUNK: Duration = Duration::from_millis(10);
/// Each app's jitter buffer: two event periods before it plays, trimmed back past ten.
const PRIMED_BYTES: usize = BYTES_PER_CHUNK * 2;
const MAX_BYTES: usize = BYTES_PER_CHUNK * 10;

/// The computer's sound for a screen share; it knows nothing about rooms, signaling, or WebRTC.
pub fn preflight_default_render_device() -> Result<(), WasapiError> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new()?;
    let render = enumerator.get_default_device(&Direction::Render)?;
    let _ = render.get_iaudioclient()?;
    Ok(())
}

/// One app's samples waiting to be mixed.
#[derive(Default)]
struct Pending {
    bytes: VecDeque<u8>,
    primed: bool,
}

impl Pending {
    /// Adds up to one chunk of this app into `mix`; an app that runs dry waits to refill.
    fn mix_into(&mut self, mix: &mut [f32]) {
        if self.bytes.len() > MAX_BYTES {
            self.bytes.drain(..self.bytes.len() - PRIMED_BYTES);
        }
        self.primed |= self.bytes.len() >= PRIMED_BYTES;
        if !self.primed {
            return;
        }
        let take = self.bytes.len().min(BYTES_PER_CHUNK) / SAMPLE_BYTES * SAMPLE_BYTES;
        self.primed = take == BYTES_PER_CHUNK;
        let bytes: Vec<u8> = self.bytes.drain(..take).collect();
        for (sample, b) in mix.iter_mut().zip(bytes.chunks_exact(SAMPLE_BYTES)) {
            *sample += f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
    }
}

struct Source {
    pid: u32,
    running: Arc<AtomicBool>,
    pending: Arc<Mutex<Pending>>,
    thread: JoinHandle<Result<(), String>>,
}

impl Source {
    fn start(pid: u32) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let pending = Arc::new(Mutex::new(Pending::default()));
        let (flag, sink) = (running.clone(), pending.clone());
        let thread = thread::spawn(move || {
            let (client, event, capture) = process::open_process_loopback(pid, true)?;
            let mut bytes = VecDeque::new();
            while flag.load(Ordering::Acquire) {
                match event.wait_for_event(100) {
                    Ok(()) => {
                        if capture.read_from_device_to_deque(&mut bytes).is_err() {
                            break;
                        }
                        sink.lock().unwrap().bytes.extend(bytes.drain(..));
                    }
                    // Silence is the mixer's job; a quiet app only has to still exist.
                    Err(WasapiError::EventTimeout) if process::process_exists(pid) => {}
                    Err(_) => break,
                }
            }
            let _ = client.stop_stream();
            Ok(())
        });
        Self { pid, running, pending, thread }
    }

    fn stop(self) -> Result<(), String> {
        self.running.store(false, Ordering::Release);
        self.thread.join().unwrap_or(Ok(()))
    }
}

fn session_pids(manager: &AudioSessionManager) -> Vec<u32> {
    let Ok(sessions) = manager.get_audiosessionenumerator() else {
        return Vec::new();
    };
    (0..sessions.get_count().unwrap_or(0))
        .filter_map(|i| sessions.get_session(i).ok())
        .filter(|session| !matches!(session.get_state(), Ok(SessionState::Expired)))
        .filter_map(|session| session.get_process_id().ok())
        .collect()
}

/// Keeps one loopback per app that should be heard. A target that failed to open is not retried.
fn rescan(manager: &AudioSessionManager, sources: &mut Vec<Source>, failed: &mut HashSet<u32>) {
    let Ok(processes) = process::process_list() else {
        return;
    };
    let targets = process::capture_targets(&session_pids(manager), &processes, std::process::id());
    let (keep, gone): (Vec<_>, Vec<_>) = std::mem::take(sources)
        .into_iter()
        .partition(|source| targets.contains(&source.pid) && !source.thread.is_finished());
    for source in gone {
        let pid = source.pid;
        if source.stop().is_err() {
            failed.insert(pid);
        }
    }
    *sources = keep;
    for pid in targets {
        if !failed.contains(&pid) && !sources.iter().any(|source| source.pid == pid) {
            sources.push(Source::start(pid));
        }
    }
}

/// The computer's sound without call apps or Screenshare itself, mixed from one loopback per
/// app and paced by the wall clock: silence keeps flowing while nothing plays.
pub fn capture_loop<F>(running: Arc<AtomicBool>, mut on_chunk: F) -> Result<(), String>
where
    F: FnMut(AudioChunk) -> bool,
{
    let _ = wasapi::initialize_mta();
    let manager = DeviceEnumerator::new()
        .and_then(|enumerator| enumerator.get_default_device(&Direction::Render))
        .and_then(|device| device.get_iaudiosessionmanager())
        .map_err(|e| e.to_string())?;
    // Without application loopback every app would fail to open and the share would go silent.
    let (probe, _, _) = process::open_process_loopback(std::process::id(), true)?;
    let _ = probe.stop_stream();
    let mut sources = Vec::new();
    let mut failed = HashSet::new();
    let mut last_scan: Option<Instant> = None;
    let mut clock = AudioClock::new();
    let start = Instant::now();
    let mut emitted = 0u128;
    while running.load(Ordering::Acquire) {
        if last_scan.is_none_or(|at| at.elapsed() >= SCAN_EVERY) {
            last_scan = Some(Instant::now());
            rescan(&manager, &mut sources, &mut failed);
        }
        let due = start.elapsed().as_micros() / CHUNK.as_micros();
        // After a stall, skip ahead instead of bursting the backlog.
        emitted = emitted.max(due.saturating_sub(10));
        while emitted < due && running.load(Ordering::Acquire) {
            emitted += 1;
            let mut mix = [0f32; BYTES_PER_CHUNK / SAMPLE_BYTES];
            for source in &sources {
                source.pending.lock().unwrap().mix_into(&mut mix);
            }
            let data = mix.iter().flat_map(|s| s.clamp(-1.0, 1.0).to_le_bytes()).collect();
            if !on_chunk(AudioChunk {
                timestamp_us: clock.timestamp_us(),
                data,
                ended: None,
            }) {
                running.store(false, Ordering::Release);
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    for source in sources {
        let _ = source.stop();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(value: f32, count: usize) -> Vec<u8> {
        (0..BYTES_PER_CHUNK / SAMPLE_BYTES * count).flat_map(|_| value.to_le_bytes()).collect()
    }
    /// Live check: `cargo test live_mix -- --ignored --nocapture` while something plays.
    #[test]
    #[ignore]
    fn live_mix() {
        let _ = wasapi::initialize_mta();
        let manager = DeviceEnumerator::new()
            .and_then(|e| e.get_default_device(&Direction::Render))
            .and_then(|d| d.get_iaudiosessionmanager())
            .unwrap();
        let processes = process::process_list().unwrap();
        let name = |pid: u32| processes.iter().find(|p| p.0 == pid).map(|p| p.2.clone()).unwrap_or_default();
        let sessions = session_pids(&manager);
        let targets = process::capture_targets(&sessions, &processes, std::process::id());
        println!("sessions: {:?}", sessions.iter().map(|&p| (p, name(p))).collect::<Vec<_>>());
        println!("captured: {:?}", targets.iter().map(|&p| (p, name(p))).collect::<Vec<_>>());
        let running = Arc::new(AtomicBool::new(true));
        let (stop, started) = (running.clone(), Instant::now());
        let (mut chunks, mut peak) = (0, 0f32);
        capture_loop(running, |chunk| {
            chunks += 1;
            for b in chunk.data.chunks_exact(SAMPLE_BYTES) {
                peak = peak.max(f32::from_le_bytes([b[0], b[1], b[2], b[3]]).abs());
            }
            if started.elapsed() > Duration::from_secs(3) {
                stop.store(false, Ordering::Release);
            }
            true
        })
        .unwrap();
        println!("chunks in 3 s: {chunks}, peak: {peak}");
        assert!((280..=320).contains(&chunks));
    }
    #[test]
    fn an_app_plays_only_after_its_buffer_fills_and_apps_add_up() {
        let mut a = Pending::default();
        let mut b = Pending::default();
        a.bytes.extend(chunk(0.25, 1));
        b.bytes.extend(chunk(0.5, 2));
        let mut mix = [0f32; BYTES_PER_CHUNK / SAMPLE_BYTES];
        a.mix_into(&mut mix);
        b.mix_into(&mut mix);
        assert!(mix.iter().all(|s| *s == 0.5), "a has one chunk only, so it waits");
        a.bytes.extend(chunk(0.25, 1));
        a.mix_into(&mut mix);
        assert!(mix.iter().all(|s| *s == 0.75));
    }
    #[test]
    fn a_running_dry_app_waits_and_a_late_one_is_trimmed() {
        let mut dry = Pending::default();
        dry.bytes.extend(chunk(0.1, 2));
        let mut mix = [0f32; BYTES_PER_CHUNK / SAMPLE_BYTES];
        dry.mix_into(&mut mix);
        dry.mix_into(&mut mix);
        dry.mix_into(&mut mix);
        dry.bytes.extend(chunk(0.1, 1));
        dry.mix_into(&mut mix);
        assert_eq!(dry.bytes.len(), BYTES_PER_CHUNK, "refilling, not played");
        let mut late = Pending::default();
        late.bytes.extend(chunk(0.1, 11));
        late.mix_into(&mut mix);
        assert_eq!(late.bytes.len(), PRIMED_BYTES - BYTES_PER_CHUNK);
    }
}
