//! Core Audio process taps include app helpers that ScreenCaptureKit does not list.
use super::{drain_chunks, AudioChunk, AudioClock};
use std::{ffi::c_void, sync::Mutex};

type Output = Box<dyn Fn(AudioChunk) + Send + Sync>;
struct Context {
    state: Mutex<(Vec<u8>, AudioClock)>,
    output: Output,
}

extern "C" {
    fn screenshare_start_app_audio(
        pid: i32,
        output: extern "C" fn(*mut c_void, *const f32, u32),
        context: *mut c_void,
    ) -> *mut c_void;
    fn screenshare_stop_app_audio(capture: *mut c_void);
}

extern "C" fn receive(context: *mut c_void, samples: *const f32, count: u32) {
    if samples.is_null() || count == 0 || !count.is_multiple_of(2) {
        return;
    }
    // Swift owns the samples for this callback; the context lives until stop has joined it.
    let context = unsafe { &*(context as *const Context) };
    let pcm = unsafe { std::slice::from_raw_parts(samples as *const u8, count as usize * 4) };
    let Ok(mut state) = context.state.lock() else {
        return;
    };
    let (pending, clock) = &mut *state;
    for data in drain_chunks(pending, pcm) {
        (context.output)(AudioChunk {
            timestamp_us: clock.timestamp_us(),
            data,
            ended: None,
        });
    }
}

pub struct AppAudioCapture {
    capture: *mut c_void,
    _context: Box<Context>,
}
// The handle is opaque; Core Audio synchronizes its callbacks with stop.
unsafe impl Send for AppAudioCapture {}
impl Drop for AppAudioCapture {
    fn drop(&mut self) {
        unsafe { screenshare_stop_app_audio(self.capture) };
    }
}
impl AppAudioCapture {
    pub fn start(
        pid: i32,
        output: impl Fn(AudioChunk) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let mut context = Box::new(Context {
            state: Mutex::new((Vec::new(), AudioClock::new())),
            output: Box::new(output),
        });
        let capture = unsafe {
            screenshare_start_app_audio(pid, receive, &mut *context as *mut Context as *mut c_void)
        };
        if capture.is_null() {
            return Err("app-audio-unavailable".into());
        }
        Ok(Self {
            capture,
            _context: context,
        })
    }
}
