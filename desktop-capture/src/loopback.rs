use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::{AudioChunk, AudioClock, BYTES_PER_CHUNK};
use wasapi::{
    AudioClient, DeviceEnumerator, Direction, SampleType, StreamMode, WasapiError, WaveFormat,
};

/// Captures the default render mix or the system mix excluding a process tree;
/// it knows nothing about rooms, signaling, or WebRTC.
pub fn preflight_default_render_device() -> Result<(), WasapiError> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new()?;
    let render = enumerator.get_default_device(&Direction::Render)?;
    let _ = render.get_iaudioclient()?;
    Ok(())
}

pub fn capture_loop<F>(
    excluded_pid: Option<u32>,
    running: Arc<AtomicBool>,
    mut on_chunk: F,
) -> Result<(), WasapiError>
where
    F: FnMut(AudioChunk) -> bool,
{
    let _ = wasapi::initialize_mta();
    let mut client = match excluded_pid {
        Some(pid) => AudioClient::new_application_loopback_client(pid, false)?,
        None => {
            let enumerator = DeviceEnumerator::new()?;
            enumerator
                .get_default_device(&Direction::Render)?
                .get_iaudioclient()?
        }
    };
    let format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
    client.initialize_client(
        &format,
        &Direction::Capture,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 100_000,
        },
    )?;
    let event = client.set_get_eventhandle()?;
    let capture = client.get_audiocaptureclient()?;
    client.start_stream()?;
    let mut clock = AudioClock::new();
    let mut bytes = VecDeque::new();
    while running.load(Ordering::Acquire) {
        if excluded_pid.is_some_and(|pid| !super::process::process_exists(pid)) {
            break;
        }
        match event.wait_for_event(100) {
            Ok(()) => {}
            Err(WasapiError::EventTimeout) => continue,
            Err(error) => {
                let _ = client.stop_stream();
                return Err(error);
            }
        }
        capture.read_from_device_to_deque(&mut bytes)?;
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
    let _ = client.stop_stream();
    Ok(())
}
