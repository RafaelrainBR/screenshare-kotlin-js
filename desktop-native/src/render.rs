use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use libwebrtc::{
    media_stream_track::MediaStreamTrack,
    video_frame::VideoBuffer,
    video_stream::native::{NativeVideoStream, NativeVideoStreamOptions},
};
use tokio_stream::StreamExt;

/// Latest decoded frame of one video track as tightly packed I420 planes; the UI converts to RGB
/// on the GPU, so frames nobody draws cost a copy, not a conversion.
#[derive(Clone)]
pub struct VideoFrameSnapshot {
    pub width: u32,
    pub height: u32,
    /// `width`×`height` luma, then the two (`width`/2 rounded up)×(`height`/2 rounded up) chroma planes.
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
    pub peer_id: String,
    pub track_id: String,
}

/// Every remote video at once, like the web stage, which plays all screens and cameras.
#[derive(Default)]
pub struct VideoFrames {
    /// Live video track ids per peer in arrival order: the sender adds the screen before the camera.
    pub order: HashMap<String, Vec<String>>,
    /// Newest frame per (peer, track) not yet taken by the UI.
    pub latest: HashMap<(String, String), Arc<VideoFrameSnapshot>>,
    /// When each track last published a frame.
    seen: HashMap<(String, String), Instant>,
}

impl VideoFrames {
    /// Whether `track` is `peer`'s camera (true) or screen (false), given what the peer shares.
    /// A browser that stops and shares again sends on a new track while the old receiver never
    /// ends, so with one kind shared any track is that kind. With both, PeerConnections.onTrack's
    /// rule applies to the tracks sending lately: screen first, camera after it.
    pub fn kind(&self, peer: &str, track: &str, screen: bool, camera: bool) -> Option<bool> {
        match (screen, camera) {
            (false, false) => None,
            (true, false) => Some(false),
            (false, true) => Some(true),
            (true, true) => {
                // shortcut: a screen idle for 10 s while the camera sends shows the camera as the screen.
                let live = self.order.get(peer)?.iter().find(|id| {
                    self.seen.get(&(peer.to_owned(), (*id).clone())).is_some_and(|at| at.elapsed() < Duration::from_secs(10))
                });
                Some(live.is_some_and(|first| first != track))
            }
        }
    }
}

pub type VideoSurface = Arc<Mutex<VideoFrames>>;

#[derive(Clone)]
pub struct RenderSink {
    tracks: Arc<Mutex<HashMap<(String, String), Arc<AtomicBool>>>>,
    surface: VideoSurface,
    received: Arc<AtomicU64>,
    presented: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
}

pub struct Renderer {
    sink: RenderSink,
}

impl Renderer {
    pub fn with_surface(surface: VideoSurface) -> Self {
        Self {
            sink: RenderSink {
                tracks: Arc::default(),
                surface,
                received: Arc::default(),
                presented: Arc::default(),
                closed: Arc::default(),
            },
        }
    }

    pub fn sink(&self) -> RenderSink {
        self.sink.clone()
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
        let mut surface = self.sink.surface.lock().unwrap();
        surface.order.remove(peer);
        surface.latest.retain(|(id, _), _| id != peer);
        surface.seen.retain(|(id, _), _| id != peer);
    }

    /// (frames decoded, frames published to the surface)
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
        *self.sink.surface.lock().unwrap() = VideoFrames::default();
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
            // The native app sends every share as "screen", so a new receiver (a re-share, a rolled
            // back offer) can reuse a live id: the newest track takes over instead of being ignored.
            match tracks.insert(key, stop.clone()) {
                Some(previous) => previous.store(true, Ordering::Release),
                None => self.surface.lock().unwrap().order.entry(peer.clone()).or_default().push(track.id()),
            }
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
                let i420 = frame.buffer.to_i420();
                let (width, height) = (i420.width(), i420.height());
                if width == 0 || height == 0 || width > 8192 || height > 8192 {
                    continue;
                }
                let (y, u, v) = i420.data();
                let (sy, su, sv) = i420.strides();
                let (chroma_width, chroma_height) = (width.div_ceil(2), height.div_ceil(2));
                let (y, u, v) = (pack(y, sy, width, height), pack(u, su, chroma_width, chroma_height), pack(v, sv, chroma_width, chroma_height));
                // Checked under the surface lock, so a frame copied while the peer left never lands.
                let mut surface = sink.surface.lock().unwrap();
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let snapshot = VideoFrameSnapshot { width, height, y, u, v, peer_id: peer.clone(), track_id: track_id.clone() };
                surface.latest.insert((peer.clone(), track_id.clone()), Arc::new(snapshot));
                surface.seen.insert((peer.clone(), track_id.clone()), Instant::now());
                sink.presented.fetch_add(1, Ordering::Relaxed);
            }
            // A track that took this id over keeps its entry and frames.
            let key = (peer, track_id);
            let mut tracks = sink.tracks.lock().unwrap();
            if !tracks.get(&key).is_some_and(|current| Arc::ptr_eq(current, &stop)) {
                return;
            }
            tracks.remove(&key);
            let mut surface = sink.surface.lock().unwrap();
            if let Some(ids) = surface.order.get_mut(&key.0) {
                ids.retain(|id| *id != key.1);
            }
            surface.latest.remove(&key);
            surface.seen.remove(&key);
        });
    }
}

/// The `width`×`height` pixels of a plane whose rows are `stride` bytes apart, without the padding.
fn pack(plane: &[u8], stride: u32, width: u32, height: u32) -> Vec<u8> {
    let mut packed = Vec::with_capacity(width as usize * height as usize);
    for row in plane.chunks(stride as usize).take(height as usize) {
        packed.extend_from_slice(&row[..width as usize]);
    }
    packed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_drops_the_row_padding() {
        let plane = [1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9];
        assert_eq!(pack(&plane, 4, 3, 3), [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
    use libwebrtc::{
        peer_connection_factory::{PeerConnectionFactory, native::PeerConnectionFactoryExt},
        video_frame::{I420Buffer, VideoFrame, VideoRotation},
        video_source::{VideoResolution, native::NativeVideoSource},
    };
    use std::{thread, time::Instant};

    fn source(size: u32) -> NativeVideoSource {
        NativeVideoSource::new(VideoResolution { width: size, height: size }, true)
    }

    fn frame(size: u32) -> VideoFrame<I420Buffer> {
        VideoFrame {
            rotation: VideoRotation::VideoRotation0,
            timestamp_us: 0,
            frame_metadata: None,
            buffer: I420Buffer::new_black(size, size),
        }
    }

    /// Feeds every source until the surface holds `want` frames.
    fn pump(surface: &VideoSurface, sources: &[(&NativeVideoSource, u32)], want: usize) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while surface.lock().unwrap().latest.len() < want && Instant::now() < deadline {
            for (source, size) in sources {
                source.capture_frame(&frame(*size));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn every_peer_reaches_the_surface_and_leaves_with_its_peer() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let (b, c) = (source(16), source(32));
        let surface = VideoSurface::default();
        let renderer = Renderer::with_surface(surface.clone());
        renderer.sink().add_track("b".into(), factory.create_video_track("b-screen", b.clone()).into(), runtime.handle().clone());
        renderer.sink().add_track("c".into(), factory.create_video_track("c-screen", c.clone()).into(), runtime.handle().clone());

        pump(&surface, &[(&b, 16), (&c, 32)], 2);
        {
            let frames = surface.lock().unwrap();
            let width = |peer: &str| frames.latest[&(peer.to_owned(), format!("{peer}-screen"))].width;
            assert_eq!((width("b"), width("c")), (16, 32), "two sharers at once");
            let b = &frames.latest[&("b".into(), "b-screen".into())];
            assert_eq!((b.y.len(), b.u.len(), b.v.len()), (16 * 16, 8 * 8, 8 * 8), "packed planes");
        }

        renderer.remove("b");
        b.capture_frame(&frame(16));
        thread::sleep(Duration::from_millis(150));
        let frames = surface.lock().unwrap();
        assert!(!frames.order.contains_key("b"));
        assert!(frames.latest.keys().all(|(peer, _)| peer != "b"), "a removed peer never publishes again");
    }

    #[test]
    fn screen_is_the_first_video_track_and_camera_the_one_after_it() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let (screen, camera) = (source(16), source(32));
        let surface = VideoSurface::default();
        let renderer = Renderer::with_surface(surface.clone());
        let sink = renderer.sink();
        sink.add_track("b".into(), factory.create_video_track("screen", screen.clone()).into(), runtime.handle().clone());
        sink.add_track("b".into(), factory.create_video_track("camera", camera.clone()).into(), runtime.handle().clone());
        pump(&surface, &[(&screen, 16), (&camera, 32)], 2);
        let frames = surface.lock().unwrap();
        assert_eq!((frames.kind("b", "screen", true, true), frames.kind("b", "camera", true, true)), (Some(false), Some(true)));
        // Sharing one kind, any track is that kind: a new track after stop and share again included.
        assert_eq!((frames.kind("b", "camera", true, false), frames.kind("b", "screen", false, true)), (Some(false), Some(true)));
        assert_eq!(frames.kind("b", "screen", false, false), None);
        assert_eq!(frames.latest[&("b".into(), "camera".into())].width, 32);
    }

    #[test]
    fn a_new_track_reusing_the_id_of_an_old_one_is_shown() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let (old, new) = (source(16), source(32));
        let surface = VideoSurface::default();
        let renderer = Renderer::with_surface(surface.clone());
        let sink = renderer.sink();
        // The native app shares as "screen" every time: a second share is a new track with the same id.
        sink.add_track("b".into(), factory.create_video_track("screen", old.clone()).into(), runtime.handle().clone());
        pump(&surface, &[(&old, 16)], 1);
        sink.add_track("b".into(), factory.create_video_track("screen", new.clone()).into(), runtime.handle().clone());
        // Past the old sink's 100 ms poll, so its exit has run.
        thread::sleep(Duration::from_millis(250));
        surface.lock().unwrap().latest.clear();
        pump(&surface, &[(&new, 32)], 1);
        let frames = surface.lock().unwrap();
        assert_eq!(frames.latest[&("b".into(), "screen".into())].width, 32);
        assert_eq!(frames.order["b"], ["screen"]);
    }

    #[test]
    fn idle_receiver_does_not_replace_the_active_video_sink_for_a_peer() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let source = source(16);
        let renderer = Renderer::with_surface(VideoSurface::default());
        let sink = renderer.sink();
        let track = factory.create_video_track("active", source.clone());
        sink.add_track("b".into(), track.clone().into(), runtime.handle().clone());
        let receive_after = |previous| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while renderer.counts().0 <= previous && Instant::now() < deadline {
                source.capture_frame(&frame(16));
                thread::sleep(Duration::from_millis(20));
            }
            assert!(renderer.counts().0 > previous, "active sink stalled after idle receiver arrived");
        };
        receive_after(0);
        sink.add_track("b".into(), factory.create_video_track("idle", self::source(16)).into(), runtime.handle().clone());
        thread::sleep(Duration::from_millis(150));
        receive_after(renderer.counts().0);
        sink.add_track("b".into(), track.into(), runtime.handle().clone());
        receive_after(renderer.counts().0);
        renderer.remove("b");
        thread::sleep(Duration::from_millis(150));
        let stopped = renderer.counts().0;
        source.capture_frame(&frame(16));
        thread::sleep(Duration::from_millis(100));
        assert_eq!(renderer.counts().0, stopped);
    }
}
