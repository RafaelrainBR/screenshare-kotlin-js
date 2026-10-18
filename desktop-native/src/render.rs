use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

use libwebrtc::{
    media_stream_track::MediaStreamTrack,
    native::yuv_helper,
    video_frame::VideoBuffer,
    video_stream::native::{NativeVideoStream, NativeVideoStreamOptions},
};
use tokio_stream::StreamExt;

/// Latest decoded frame of the selected peer, RGBA, ready for the UI to upload.
#[derive(Clone)]
pub struct VideoFrameSnapshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub peer_id: String,
}
pub type VideoSurface = Arc<Mutex<Option<Arc<VideoFrameSnapshot>>>>;

#[derive(Default)]
struct Selection {
    peer: Option<String>,
    camera: bool,
    epoch: u64,
    ready: bool,
}

#[derive(Clone)]
pub struct RenderSink {
    selection: Arc<Mutex<Selection>>,
    tracks: Arc<Mutex<HashMap<(String, String), Arc<AtomicBool>>>>,
    // Live video track ids per peer in arrival order: the sender adds the screen before the camera.
    order: Arc<Mutex<HashMap<String, Vec<String>>>>,
    surface: VideoSurface,
    events: Sender<serde_json::Value>,
    received: Arc<AtomicU64>,
    presented: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
}

pub struct Renderer {
    sink: RenderSink,
    pub events: Receiver<serde_json::Value>,
}

fn notify(events: &Sender<serde_json::Value>, state: &str, peer: Option<&str>) {
    let _ = events.send(serde_json::json!({"event":"videoState","state":state,"peerId":peer}));
}

impl Renderer {
    pub fn with_surface(surface: VideoSurface) -> Self {
        let (events_sender, events) = channel();
        Self {
            sink: RenderSink {
                selection: Arc::default(),
                tracks: Arc::default(),
                order: Arc::default(),
                surface,
                events: events_sender,
                received: Arc::default(),
                presented: Arc::default(),
                closed: Arc::default(),
            },
            events,
        }
    }

    pub fn sink(&self) -> RenderSink {
        self.sink.clone()
    }

    /// Shows `peer`'s screen (or camera) on the surface, or clears it with `None`.
    pub fn show(&self, peer: Option<String>, camera: bool) {
        let mut selection = self.sink.selection.lock().unwrap();
        *self.sink.surface.lock().unwrap() = None;
        selection.epoch += 1;
        selection.ready = false;
        selection.peer = peer;
        selection.camera = camera;
        let state = if selection.peer.is_some() { "waiting" } else { "hidden" };
        notify(&self.sink.events, state, selection.peer.as_deref());
    }

    pub fn remove(&self, peer: &str) {
        self.sink.tracks.lock().unwrap().retain(|(id, _), stop| {
            if id == peer {
                stop.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
        self.sink.order.lock().unwrap().remove(peer);
        if self.sink.selection.lock().unwrap().peer.as_deref() == Some(peer) {
            self.show(None, false);
        }
    }

    /// (frames decoded for any peer, frames published to the surface)
    pub fn counts(&self) -> (u64, u64) {
        (
            self.sink.received.load(Ordering::Relaxed),
            self.sink.presented.load(Ordering::Relaxed),
        )
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.sink.closed.store(true, Ordering::Release);
        for (_, stop) in self.sink.tracks.lock().unwrap().drain() {
            stop.store(true, Ordering::Release);
        }
        *self.sink.surface.lock().unwrap() = None;
    }
}

impl RenderSink {
    pub fn add_track(&self, peer: String, track: MediaStreamTrack, runtime: tokio::runtime::Handle) {
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        let MediaStreamTrack::Video(track) = track else {
            return;
        };
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut tracks = self.tracks.lock().unwrap();
            let key = (peer.clone(), track.id());
            if tracks.contains_key(&key) {
                return;
            }
            tracks.insert(key, stop.clone());
            self.order.lock().unwrap().entry(peer.clone()).or_default().push(track.id());
        }
        let track_id = track.id();
        let sink = self.clone();
        runtime.spawn(async move {
            let mut stream = NativeVideoStream::with_options(
                track,
                NativeVideoStreamOptions {
                    queue_size_frames: Some(2),
                },
            );
            while !stop.load(Ordering::Acquire) && !sink.closed.load(Ordering::Acquire) {
                let frame = tokio::select! {
                    frame = stream.next() => frame,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
                };
                let Some(frame) = frame else {
                    break;
                };
                sink.received.fetch_add(1, Ordering::Relaxed);
                let Some(epoch) = sink.selected(&peer, &track_id) else {
                    continue;
                };
                let i420 = frame.buffer.to_i420();
                let (width, height) = (i420.width(), i420.height());
                if width == 0 || height == 0 || width > 8192 || height > 8192 {
                    continue;
                }
                let mut rgba = vec![0; width as usize * height as usize * 4];
                let (y, u, v) = i420.data();
                let (sy, su, sv) = i420.strides();
                // libyuv's "ABGR" is R,G,B,A in memory: what iced's RGBA images expect.
                yuv_helper::i420_to_abgr(y, sy, u, su, v, sv, &mut rgba, width * 4, width as i32, height as i32);
                sink.publish(VideoFrameSnapshot { width, height, rgba, peer_id: peer.clone() }, epoch);
            }
            if let Some(ids) = sink.order.lock().unwrap().get_mut(&peer) {
                ids.retain(|id| *id != track_id);
            }
            sink.tracks.lock().unwrap().remove(&(peer, track_id));
        });
    }

    /// The selection epoch when `track_id` is what the UI asked to see for `peer`.
    fn selected(&self, peer: &str, track_id: &str) -> Option<u64> {
        let selection = self.selection.lock().unwrap();
        if selection.peer.as_deref() != Some(peer) {
            return None;
        }
        let order = self.order.lock().unwrap();
        let ids = order.get(peer)?;
        // Same rule as PeerConnections.onTrack: screen first, camera after it (or alone).
        let chosen = if selection.camera { ids.last() } else { ids.first() };
        (chosen.map(String::as_str) == Some(track_id)).then_some(selection.epoch)
    }

    fn publish(&self, frame: VideoFrameSnapshot, epoch: u64) {
        let mut selection = self.selection.lock().unwrap();
        // The selection may have changed while this frame was converting.
        if selection.epoch != epoch || selection.peer.as_deref() != Some(&frame.peer_id) {
            return;
        }
        *self.surface.lock().unwrap() = Some(Arc::new(frame));
        self.presented.fetch_add(1, Ordering::Relaxed);
        if !selection.ready {
            selection.ready = true;
            notify(&self.events, "ready", selection.peer.as_deref());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libwebrtc::{
        peer_connection_factory::{PeerConnectionFactory, native::PeerConnectionFactoryExt},
        video_frame::{I420Buffer, VideoFrame, VideoRotation},
        video_source::{VideoResolution, native::NativeVideoSource},
    };
    use std::{thread, time::Instant};

    fn source() -> NativeVideoSource {
        NativeVideoSource::new(VideoResolution { width: 16, height: 16 }, true)
    }

    fn frame() -> VideoFrame<I420Buffer> {
        VideoFrame {
            rotation: VideoRotation::VideoRotation0,
            timestamp_us: 0,
            frame_metadata: None,
            buffer: I420Buffer::new_black(16, 16),
        }
    }

    fn state(renderer: &Renderer) -> String {
        renderer.events.recv_timeout(Duration::from_secs(5)).unwrap()["state"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn surface_shows_only_the_selected_peer_and_clears_on_departure() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let source = source();
        let surface = VideoSurface::default();
        let renderer = Renderer::with_surface(surface.clone());
        let track = factory.create_video_track("b", source.clone());
        renderer.sink().add_track("b".into(), track.into(), runtime.handle().clone());

        let pump_until = |done: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !done() && Instant::now() < deadline {
                source.capture_frame(&frame());
                thread::sleep(Duration::from_millis(20));
            }
        };

        renderer.show(Some("c".into()), false);
        assert_eq!(state(&renderer), "waiting");
        pump_until(&|| renderer.counts().0 >= 5);
        assert!(surface.lock().unwrap().is_none(), "unselected peers never reach the surface");

        renderer.show(Some("b".into()), false);
        assert_eq!(state(&renderer), "waiting");
        pump_until(&|| surface.lock().unwrap().is_some());
        assert_eq!(state(&renderer), "ready");
        let shown = surface.lock().unwrap().clone().unwrap();
        assert_eq!((shown.width, shown.height, shown.rgba.len()), (16, 16, 16 * 16 * 4));
        assert_eq!(shown.peer_id, "b");
        assert_eq!(shown.rgba[3], 255, "alpha is opaque");

        renderer.remove("b");
        assert_eq!(state(&renderer), "hidden");
        assert!(surface.lock().unwrap().is_none());
        let presented = renderer.counts().1;
        source.capture_frame(&frame());
        thread::sleep(Duration::from_millis(100));
        assert_eq!(renderer.counts().1, presented);
    }

    #[test]
    fn screen_is_the_first_video_track_and_camera_the_one_after_it() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let screen = source();
        let camera = NativeVideoSource::new(VideoResolution { width: 32, height: 32 }, false);
        let surface = VideoSurface::default();
        let renderer = Renderer::with_surface(surface.clone());
        let sink = renderer.sink();
        sink.add_track("b".into(), factory.create_video_track("screen", screen.clone()).into(), runtime.handle().clone());
        sink.add_track("b".into(), factory.create_video_track("camera", camera.clone()).into(), runtime.handle().clone());
        let camera_frame = VideoFrame {
            rotation: VideoRotation::VideoRotation0,
            timestamp_us: 0,
            frame_metadata: None,
            buffer: I420Buffer::new_black(32, 32),
        };
        let shown_width = |want_camera: bool| {
            renderer.show(Some("b".into()), want_camera);
            let deadline = Instant::now() + Duration::from_secs(3);
            while surface.lock().unwrap().is_none() && Instant::now() < deadline {
                screen.capture_frame(&frame());
                camera.capture_frame(&camera_frame);
                thread::sleep(Duration::from_millis(20));
            }
            let width = surface.lock().unwrap().as_ref().map(|f| f.width);
            // A few more frames from both tracks must not flip the surface to the other one.
            for _ in 0..5 {
                screen.capture_frame(&frame());
                camera.capture_frame(&camera_frame);
                thread::sleep(Duration::from_millis(20));
            }
            assert_eq!(surface.lock().unwrap().as_ref().map(|f| f.width), width);
            width
        };
        assert_eq!(shown_width(false), Some(16));
        assert_eq!(shown_width(true), Some(32));
    }

    #[test]
    fn idle_receiver_does_not_replace_the_active_video_sink_for_a_peer() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let source = source();
        let renderer = Renderer::with_surface(VideoSurface::default());
        let sink = renderer.sink();
        let track = factory.create_video_track("active", source.clone());
        sink.add_track("b".into(), track.clone().into(), runtime.handle().clone());
        let receive_after = |previous| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while renderer.counts().0 <= previous && Instant::now() < deadline {
                source.capture_frame(&frame());
                thread::sleep(Duration::from_millis(20));
            }
            assert!(renderer.counts().0 > previous, "active sink stalled after idle receiver arrived");
        };
        receive_after(0);
        sink.add_track(
            "b".into(),
            factory.create_video_track("idle", self::source()).into(),
            runtime.handle().clone(),
        );
        thread::sleep(Duration::from_millis(150));
        receive_after(renderer.counts().0);
        sink.add_track("b".into(), track.into(), runtime.handle().clone());
        receive_after(renderer.counts().0);
        renderer.remove("b");
        thread::sleep(Duration::from_millis(150));
        let stopped = renderer.counts().0;
        source.capture_frame(&frame());
        thread::sleep(Duration::from_millis(100));
        assert_eq!(renderer.counts().0, stopped);
    }
}
