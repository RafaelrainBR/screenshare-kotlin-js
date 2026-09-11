use serde::Serialize;

pub mod loopback;
pub mod process;
pub mod sources;
pub mod thumb;

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

#[cfg(test)]
mod tests {
    use super::*;
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
