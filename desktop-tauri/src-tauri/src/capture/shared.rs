//! WebView2 shared-buffer transport for video frames. A small ring of shared
//! memory slots is posted to script once; afterwards the capture thread fills a
//! free slot and only announces it with a small message. Script copies the pixels
//! into a VideoFrame and clears the slot's state byte. A frame with no free slot
//! is dropped, so nothing queues up.
//!
//! Posting a buffer per frame was measured at ~110% of a core in the renderer
//! (one 11 MB mapping per post at 60 fps); posting once avoids that cost.
use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, Ordering};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Environment12, ICoreWebView2SharedBuffer, ICoreWebView2_17,
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE,
};
use windows::core::{Interface, HSTRING};

pub const SLOTS: usize = 3;
/// Byte 0: 1 while the slot holds a frame script has not consumed yet.
pub const HEADER: usize = 16;

type Ring = (u64, Vec<ICoreWebView2SharedBuffer>);

thread_local! {
    // COM objects live on the UI thread only; the capture thread gets raw pointers.
    static RING: RefCell<Option<Ring>> = const { RefCell::new(None) };
}

/// Shared memory addresses usable from any thread while the ring is alive.
pub struct SlotPointers {
    pub ptrs: Vec<usize>,
    pub capacity: usize,
}

/// Must run on the UI thread (inside `with_webview`). Script receives each slot
/// as a `sharedbufferreceived` event tagged with `{generation, slot}`.
pub fn create_ring(
    webview: &tauri::webview::PlatformWebview,
    generation: u64,
    frame_bytes: usize,
) -> Result<SlotPointers, String> {
    unsafe {
        let environment: ICoreWebView2Environment12 = webview
            .environment()
            .cast()
            .map_err(|_| "shared-buffer-unsupported")?;
        let core: ICoreWebView2_17 = webview
            .controller()
            .CoreWebView2()
            .and_then(|core| core.cast())
            .map_err(|_| "shared-buffer-unsupported")?;
        let capacity = HEADER + frame_bytes;
        let mut buffers = Vec::with_capacity(SLOTS);
        let mut ptrs = Vec::with_capacity(SLOTS);
        for slot in 0..SLOTS {
            let buffer = environment
                .CreateSharedBuffer(capacity as u64)
                .map_err(|_| "shared-buffer-failed")?;
            let mut ptr = std::ptr::null_mut();
            buffer
                .Buffer(&mut ptr)
                .map_err(|_| "shared-buffer-failed")?;
            core.PostSharedBufferToScript(
                &buffer,
                COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE,
                &HSTRING::from(format!(r#"{{"generation":{generation},"slot":{slot}}}"#)),
            )
            .map_err(|_| "shared-buffer-failed")?;
            ptrs.push(ptr as usize);
            buffers.push(buffer);
        }
        close(RING.with(|ring| ring.borrow_mut().replace((generation, buffers))));
        Ok(SlotPointers { ptrs, capacity })
    }
}

/// UI thread: closes the shared memory unless a newer capture already replaced it.
pub fn release_ring(generation: u64) {
    let owned = RING.with(|ring| {
        let mut ring = ring.borrow_mut();
        if ring
            .as_ref()
            .is_some_and(|(current, _)| *current == generation)
        {
            ring.take()
        } else {
            None
        }
    });
    close(owned);
}

fn close(ring: Option<Ring>) {
    for buffer in ring.into_iter().flat_map(|(_, buffers)| buffers) {
        let _ = unsafe { buffer.Close() };
    }
}

impl SlotPointers {
    /// Capture thread: copies a frame into a free slot, or returns None (dropped).
    pub fn fill(&self, data: &[u8]) -> Option<usize> {
        if HEADER + data.len() > self.capacity {
            return None;
        }
        self.ptrs.iter().position(|&ptr| {
            let state = unsafe { &*(ptr as *const AtomicU8) };
            if state.load(Ordering::Acquire) != 0 {
                return false;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), (ptr + HEADER) as *mut u8, data.len());
            }
            state.store(1, Ordering::Release);
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_uses_free_slots_and_drops_when_all_are_busy() {
        let mut memory = vec![vec![0u8; HEADER + 8]; SLOTS];
        let slots = SlotPointers {
            ptrs: memory.iter_mut().map(|m| m.as_mut_ptr() as usize).collect(),
            capacity: HEADER + 8,
        };
        assert_eq!(slots.fill(&[7; 9]), None, "oversized frame");
        for expected in 0..SLOTS {
            assert_eq!(slots.fill(&[expected as u8; 8]), Some(expected));
        }
        assert_eq!(
            slots.fill(&[9; 8]),
            None,
            "no free slot: dropped, not queued"
        );
        memory[1][0] = 0; // script consumed slot 1
        assert_eq!(slots.fill(&[5; 8]), Some(1));
        assert_eq!(&memory[1][HEADER..], &[5; 8]);
    }
}
