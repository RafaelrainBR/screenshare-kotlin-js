use serde::Serialize;

#[cfg(target_os = "windows")]
pub mod loopback;
#[cfg(target_os = "macos")]
pub mod mac;
#[cfg(target_os = "macos")]
pub mod mac_audio;
#[cfg(target_os = "windows")]
pub mod process;
#[cfg(target_os = "windows")]
pub mod shared;
#[cfg(target_os = "windows")]
pub mod sources;
#[cfg(target_os = "windows")]
pub mod thumb;
#[cfg(target_os = "windows")]
pub mod video;

pub const FRAMES_PER_CHUNK: usize = 480;
pub const CHANNELS: usize = 2;
pub const SAMPLE_BYTES: usize = 4;
pub const BYTES_PER_CHUNK: usize = FRAMES_PER_CHUNK * CHANNELS * SAMPLE_BYTES;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioChunk {
    pub timestamp_us: u64,
    pub data: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<bool>,
}

impl AudioChunk {
    /// IPC control marker: deliberately carries no captured PCM or metadata.
    pub fn terminal() -> Self {
        Self {
            timestamp_us: 0,
            data: Vec::new(),
            ended: Some(true),
        }
    }
}

#[derive(Debug)]
pub struct AudioClock {
    start: std::time::Instant,
    last_us: u64,
}
impl AudioClock {
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
            last_us: 0,
        }
    }
    pub fn timestamp_us(&mut self) -> u64 {
        let value = self.start.elapsed().as_micros() as u64;
        self.last_us = value.max(self.last_us.saturating_add(1));
        self.last_us
    }
}

/// Largest even size with the source aspect that fits `max_w`×`max_h`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn fit_within(width: f64, height: f64, max_w: u32, max_h: u32) -> (u32, u32) {
    let scale = (max_w as f64 / width).min(max_h as f64 / height).min(1.0);
    let even = |v: f64| ((v * scale).round() as u32 / 2 * 2).max(2);
    (even(width), even(height))
}

/// Appends interleaved f32 PCM bytes and returns every complete IPC chunk.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn drain_chunks(pending: &mut Vec<u8>, pcm: &[u8]) -> Vec<Vec<u8>> {
    pending.extend_from_slice(pcm);
    let complete = pending.len() / BYTES_PER_CHUNK * BYTES_PER_CHUNK;
    let ready: Vec<u8> = pending.drain(..complete).collect();
    ready.chunks(BYTES_PER_CHUNK).map(<[u8]>::to_vec).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fit_keeps_aspect_and_even_sizes() {
        assert_eq!(fit_within(3024.0, 1964.0, 1920, 1080), (1662, 1080));
        assert_eq!(fit_within(801.0, 601.0, 1920, 1080), (800, 600));
        assert_eq!(fit_within(1.0, 1.0, 1920, 1080), (2, 2));
    }
    #[test]
    fn drain_emits_whole_chunks_and_keeps_the_rest() {
        let mut pending = Vec::new();
        assert!(drain_chunks(&mut pending, &[1; 1000]).is_empty());
        let chunks = drain_chunks(&mut pending, &[2; BYTES_PER_CHUNK * 2]);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|c| c.len() == BYTES_PER_CHUNK));
        assert_eq!(&chunks[0][..1000], &[1; 1000]);
        assert_eq!(pending.len(), 1000);
    }
    #[test]
    fn pcm_contract_is_stable() {
        assert_eq!(BYTES_PER_CHUNK, 3840);
        assert_eq!(FRAMES_PER_CHUNK, 480);
        assert_eq!(CHANNELS, 2);
    }
    #[test]
    fn timestamps_are_strictly_monotonic() {
        let mut c = AudioClock::new();
        let mut last = c.timestamp_us();
        for _ in 0..1000 {
            let now = c.timestamp_us();
            assert!(now > last);
            last = now;
        }
    }
    #[test]
    fn terminal_marker_has_no_captured_content() {
        let marker = AudioChunk::terminal();
        assert_eq!(marker.timestamp_us, 0);
        assert!(marker.data.is_empty());
        assert_eq!(marker.ended, Some(true));
    }
}
