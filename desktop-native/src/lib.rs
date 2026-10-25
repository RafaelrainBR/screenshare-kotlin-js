use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        mpsc::{
            Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel,
        },
    },
    thread::{self, JoinHandle},
};
#[cfg(feature = "synthetic-test")]
use std::sync::{Mutex, atomic::Ordering};

#[cfg(not(feature = "synthetic-test"))]
#[cfg(windows)]
mod camera;
#[cfg(not(feature = "synthetic-test"))]
#[cfg(target_os = "macos")]
#[path = "camera_macos.rs"]
mod camera;
#[cfg(windows)]
mod ducking;
mod capture;
pub use capture::{ScreenAudioMode, ShareSource};
mod render;
pub use render::{VideoFrameSnapshot, VideoFrames, VideoSurface};
/// Surface peer ids of this client's own screen and camera, the web's isLocal tiles.
pub const LOCAL_SCREEN: &str = "local:screen";
pub const LOCAL_CAMERA: &str = "local:camera";
pub mod quality;
pub mod session;
pub mod share;
pub mod sources;
mod voice;
use share::{ShareProfile, share_bitrate};

#[cfg(not(feature = "synthetic-test"))]
use capture::{Capture, CaptureEvent};
#[cfg(not(feature = "synthetic-test"))]
use libwebrtc::media_stream_track::MediaStreamTrack;
use libwebrtc::{
    ice_candidate::IceCandidate,
    peer_connection::{OfferOptions, PeerConnection},
    peer_connection_factory::native::PeerConnectionFactoryExt,
    peer_connection_factory::{
        IceServer, IceTransportsType, PeerConnectionFactory, RtcConfiguration,
    },
    rtp_sender::RtpSender,
    rtp_transceiver::{RtpTransceiverDirection, RtpTransceiverInit},
    session_description::{SdpType, SessionDescription},
    stats::RtcStats,
};
#[cfg(not(feature = "synthetic-test"))]
use render::Renderer;
use serde::Deserialize;
use serde_json::json;
use tokio::runtime::Runtime;

#[cfg(feature = "synthetic-test")]
use libwebrtc::{
    audio_frame::AudioFrame,
    audio_source::{AudioSourceOptions, native::NativeAudioSource},
    audio_stream::native::{NativeAudioStream, NativeAudioStreamOptions},
    audio_track::RtcAudioTrack,
    media_stream_track::MediaStreamTrack,
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
};
#[cfg(feature = "synthetic-test")]
use std::sync::atomic::AtomicBool;
#[cfg(feature = "synthetic-test")]
use tokio_stream::StreamExt;

const MAX_CONFIG: usize = 16 * 1024;
const MAX_COMMAND: usize = 256 * 1024;
const MAX_SDP: usize = 128 * 1024;
const MAX_CANDIDATE: usize = 8 * 1024;
const MAX_PEER_ID: usize = 128;
const MAX_PEERS: usize = 4;
const QUEUE_CAPACITY: usize = 256;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineConfig {
    schema_version: u8,
    ice_servers: Vec<IceServerConfig>,
    ice_transport_policy: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IceServerConfig {
    urls: Vec<String>,
    // The server leaves both out for STUN entries.
    #[serde(default)]
    username: String,
    #[serde(default)]
    credential: String,
}

#[derive(Deserialize)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Command {
    EnsurePeer {
        peer_id: String,
        polite: bool,
        initiate: bool,
    },
    RemovePeer {
        peer_id: String,
    },
    RemoteDescription {
        peer_id: String,
        r#type: String,
        sdp: String,
    },
    RemoteIce {
        peer_id: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<i32>,
        username_fragment: Option<String>,
    },
    SetIceServers {
        ice_servers: Vec<IceServerConfig>,
    },
    StartScreen {
        source: ShareSource,
        profile: ShareProfile,
        /// The share height the bitrate ceiling follows (share::share_bitrate).
        height: u32,
        #[serde(default)]
        audio_mode: ScreenAudioMode,
    },
    /// Live profile change: fps, bitrate and degradation without renegotiation.
    SetShareProfile {
        profile: ShareProfile,
        height: u32,
    },
    StopScreen,
    SetMicrophone {
        enabled: bool,
        device_id: Option<String>,
    },
    ListAudioDevices,
    SetOutput {
        device_id: Option<String>,
        volume: f64,
        #[serde(default = "enabled_by_default")]
        enabled: bool,
    },
    RestartIce {
        peer_id: String,
    },
    GetStats,
    /// The webcam (None: the first one), sent as the "camera" stream after the screen's.
    StartCamera {
        device_id: Option<String>,
    },
    StopCamera,
    /// Emits `cameraDevices`.
    ListCameras,
    /// Emits `audioLevels`: each peer's loudest voice and the local microphone, 0..1.
    GetAudioLevels,
    /// Emits `screenQuality`: per peer, our screen's encoder limits and every inbound video's loss (quality.rs).
    GetScreenQuality,
    /// "Silenciar áudio para você": only this client stops playing that peer's voice.
    SetPeerMuted {
        peer_id: String,
        muted: bool,
    },
    /// "Volume para você", 0..1: that peer's voice, or the audio of the screen it shares.
    SetPeerVolume {
        peer_id: String,
        volume: f64,
        #[serde(default)]
        screen: bool,
    },
}

fn enabled_by_default() -> bool {
    true
}

fn audio_device_index(id: &str, mut guids: impl Iterator<Item = String>) -> Option<u16> {
    guids
        .position(|guid| guid == id)
        .and_then(|index| index.try_into().ok())
}

enum Work {
    Command { request_id: i64, command: Command },
    Close,
}

#[cfg(feature = "synthetic-test")]
struct SyntheticScreen {
    source: NativeVideoSource,
    audio: Option<SyntheticAudio>,
    stopped: Arc<AtomicBool>,
}

#[cfg(feature = "synthetic-test")]
struct SyntheticAudio {
    source: NativeAudioSource,
    stopped: Arc<AtomicBool>,
}

#[cfg(feature = "synthetic-test")]
impl Drop for SyntheticScreen {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

#[cfg(feature = "synthetic-test")]
impl Drop for SyntheticAudio {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct PeerMedia {
    screen: Vec<RtpSender>,
    // Kept only to tear down a half-attached share; see detach_screen.
    screen_transceivers: Vec<libwebrtc::rtp_transceiver::RtpTransceiver>,
    microphone: Option<RtpSender>,
    camera: Option<libwebrtc::rtp_transceiver::RtpTransceiver>,
}

/// The camera goes after the screen, as the web sender adds it: receivers take the first video
/// track as the screen and the next as the camera (PeerConnections.onTrack).
#[cfg(not(feature = "synthetic-test"))]
fn attach_camera(peer: &PeerConnection, factory: &PeerConnectionFactory, camera: &camera::Camera, media: &mut PeerMedia) -> Result<(), String> {
    if media.camera.is_none() {
        let track = factory.create_video_track("camera", camera.source.clone());
        let init = RtpTransceiverInit { direction: RtpTransceiverDirection::SendRecv, stream_ids: vec!["camera".into()], send_encodings: Vec::new() };
        let transceiver = peer.add_transceiver(track.into(), init).map_err(|e| e.to_string())?;
        let codecs = share::preferred_video_codecs(factory.get_rtp_sender_capabilities(libwebrtc::MediaType::Video).codecs, false);
        if let Err(error) = transceiver.set_codec_preferences(codecs) {
            eprintln!("codec preference not applied: {error}");
        }
        media.camera = Some(transceiver);
    }
    Ok(())
}

/// stop(), as for the screen: only that frees the send stream and its encoder.
#[cfg(not(feature = "synthetic-test"))]
fn detach_camera(
    runtime: &Runtime,
    peers: &HashMap<String, PeerConnection>,
    peer_media: &mut HashMap<String, PeerMedia>,
    pending_renegotiation: &mut HashSet<String>,
    output: &EventSink,
) -> Result<(), String> {
    for media in peer_media.values_mut() {
        if let Some(transceiver) = media.camera.take() {
            let _ = transceiver.stop();
        }
    }
    for peer_id in peers.keys() {
        negotiate_or_defer(runtime, peers, peer_id, pending_renegotiation, output)?;
    }
    Ok(())
}

/// Shows a local capture on the surface, replacing the previous one; the UI hides it once the
/// capture is no longer active. A new id per generation keeps the old task's cleanup off the new one.
/// shortcut: the last preview's task idles until the next start or the session ends.
#[cfg(not(feature = "synthetic-test"))]
fn preview(renderer: &Renderer, factory: &PeerConnectionFactory, runtime: &Runtime, peer: &str, generation: u64, source: &libwebrtc::video_source::native::NativeVideoSource) {
    renderer.remove(peer);
    let track = factory.create_video_track(&format!("{peer}-{generation}"), source.clone());
    renderer.sink().add_track(peer.to_owned(), track.into(), runtime.handle().clone());
}

/// PeerConnections.applyShareEncoding: bitrate ceiling, fps and degradation, no renegotiation.
fn apply_share_encoding(sender: &RtpSender, profile: ShareProfile, height: u32) {
    let mut params = sender.parameters();
    if params.encodings.is_empty() {
        params.encodings.push(Default::default());
    }
    for encoding in &mut params.encodings {
        encoding.max_bitrate = Some(share_bitrate(profile, height));
        encoding.max_framerate = Some(f64::from(profile.fps));
    }
    params.set_degradation_preference(profile.priority.degradation());
    if let Err(error) = sender.set_parameters(params) {
        eprintln!("share encoding not applied: {error}");
    }
}

#[cfg(not(feature = "synthetic-test"))]
fn attach_real_screen(
    peer: &PeerConnection,
    factory: &PeerConnectionFactory,
    screen: &Capture,
    media: &mut PeerMedia,
    (profile, height): (ShareProfile, u32),
) -> Result<(), String> {
    if media.screen.is_empty() {
        let track = factory.create_video_track("screen", screen.source.clone());
        // add_transceiver instead of add_track: it hands back this transceiver, so nothing has to
        // walk transceivers and call sender().track(), which crashes (0xC0000005) on a sender whose
        // track was removed by an earlier share.
        let transceiver = peer
            .add_transceiver(
                track.into(),
                RtpTransceiverInit {
                    direction: RtpTransceiverDirection::SendRecv,
                    stream_ids: vec!["screen".into()],
                    send_encodings: Vec::new(),
                },
            )
            .map_err(|e| e.to_string())?;
        // Codec order and encoding go in before the offer, as in addTracksIfNotPresent.
        let codecs = share::preferred_video_codecs(
            factory.get_rtp_sender_capabilities(libwebrtc::MediaType::Video).codecs,
            profile.priority != share::SharePriority::Sharpness,
        );
        if let Err(error) = transceiver.set_codec_preferences(codecs) {
            eprintln!("codec preference not applied: {error}");
        }
        let sender = transceiver.sender();
        apply_share_encoding(&sender, profile, height);
        media.screen.push(sender);
        media.screen_transceivers.push(transceiver);
        if let Some(audio) = &screen.audio {
            let track = factory.create_audio_track("screen-audio", audio.clone());
            let init = RtpTransceiverInit {
                direction: RtpTransceiverDirection::SendRecv,
                stream_ids: vec!["screen".into()],
                send_encodings: Vec::new(),
            };
            match peer.add_transceiver(track.into(), init) {
                Ok(transceiver) => {
                    media.screen.push(transceiver.sender());
                    media.screen_transceivers.push(transceiver);
                }
                Err(error) => {
                    for transceiver in media.screen_transceivers.drain(..) {
                        let _ = transceiver.stop();
                    }
                    media.screen.clear();
                    return Err(error.to_string());
                }
            }
        }
    }
    Ok(())
}

fn detach_screen(
    runtime: &Runtime,
    peers: &HashMap<String, PeerConnection>,
    peer_media: &mut HashMap<String, PeerMedia>,
    pending_renegotiation: &mut HashSet<String>,
    output: &EventSink,
) -> Result<(), String> {
    for (peer_id, peer) in peers {
        if let Some(media) = peer_media.get_mut(peer_id) {
            for sender in media.screen.drain(..) {
                let _ = peer.remove_track(sender);
            }
            // Only stop() frees the send stream and its encoder: JSEP keeps a sender's MSID/SSRCs in
            // every offer until then (~67 MB leaked per share otherwise).
            for transceiver in media.screen_transceivers.drain(..) {
                let _ = transceiver.stop();
            }
        }
    }
    for peer_id in peers.keys() {
        negotiate_or_defer(runtime, peers, peer_id, pending_renegotiation, output)?;
    }
    Ok(())
}

struct PendingIce {
    queued_at: std::time::Instant,
    candidate: IceCandidate,
    username_fragment: Option<String>,
}

fn ice_generation_matches(sdp: &str, fragment: Option<&str>) -> bool {
    fragment.is_none_or(|fragment| {
        sdp.lines()
            .any(|line| line.strip_prefix("a=ice-ufrag:") == Some(fragment))
    })
}


fn parse_config(json: &str) -> Result<RtcConfiguration, String> {
    if json.len() > MAX_CONFIG {
        return Err("INVALID_ARGUMENT: config exceeds 16 KiB".into());
    }
    let config: EngineConfig =
        serde_json::from_str(json).map_err(|_| "INVALID_ARGUMENT: invalid config")?;
    if config.schema_version != 1 {
        return Err("INVALID_ARGUMENT: unsupported schema version".into());
    }
    let ice_transport_type = match config.ice_transport_policy.as_str() {
        "all" => IceTransportsType::All,
        "relay" => IceTransportsType::Relay,
        _ => return Err("INVALID_ARGUMENT: invalid ICE policy".into()),
    };
    let mut rtc_config = RtcConfiguration::default();
    rtc_config.ice_servers = config
        .ice_servers
        .into_iter()
        .map(|server| IceServer {
            urls: server.urls,
            username: server.username,
            password: server.credential,
        })
        .collect();
    rtc_config.ice_transport_type = ice_transport_type;
    Ok(rtc_config)
}

fn parse_command(json: &str) -> Result<Command, String> {
    if json.len() > MAX_COMMAND {
        return Err("INVALID_ARGUMENT: command exceeds 256 KiB".into());
    }
    let command: Command =
        serde_json::from_str(json).map_err(|_| "INVALID_ARGUMENT: invalid command")?;
    validate_command(&command)?;
    Ok(command)
}

fn validate_peer_id(peer_id: &str) -> Result<(), String> {
    if peer_id.is_empty() || peer_id.chars().count() > MAX_PEER_ID {
        return Err("INVALID_ARGUMENT: invalid peerId".into());
    }
    Ok(())
}

fn validate_ice_servers(servers: &[IceServerConfig]) -> Result<(), String> {
    if servers.len() > 8 || servers.iter().any(|server| {
        server.urls.is_empty() || server.urls.len() > 8 || server.urls.iter().any(|url| {
            url.len() > 2048 || !(url.starts_with("stun:") || url.starts_with("stuns:") || url.starts_with("turn:") || url.starts_with("turns:"))
        }) || (server.urls.iter().any(|url| url.starts_with("turn:") || url.starts_with("turns:"))
            != (!server.username.is_empty() && !server.credential.is_empty()))
    }) {
        return Err("INVALID_ARGUMENT: invalid ICE server configuration".into());
    }
    Ok(())
}

fn validate_command(command: &Command) -> Result<(), String> {
    if let Command::SetMicrophone {
        device_id: Some(id),
        ..
    }
    | Command::SetOutput {
        device_id: Some(id),
        ..
    } = command
    {
        if id.is_empty() || id.len() > 1024 || id.chars().any(char::is_control) {
            return Err("INVALID_ARGUMENT: invalid audio deviceId".into());
        }
    }
    match command {
        Command::SetIceServers { ice_servers } => validate_ice_servers(ice_servers),
        Command::RemoteDescription {
            peer_id,
            r#type,
            sdp,
        } => {
            validate_peer_id(peer_id)?;
            if !matches!(r#type.as_str(), "offer" | "answer") || sdp.len() > MAX_SDP {
                Err("INVALID_ARGUMENT: invalid remote description".into())
            } else {
                Ok(())
            }
        }
        Command::RemoteIce {
            peer_id, candidate, ..
        } => {
            validate_peer_id(peer_id)?;
            if candidate.len() > MAX_CANDIDATE {
                Err("INVALID_ARGUMENT: candidate exceeds 8 KiB".into())
            } else {
                Ok(())
            }
        }
        Command::SetPeerVolume { volume, .. } if !(0.0..=1.0).contains(volume) => Err("INVALID_ARGUMENT: invalid peer volume".into()),
        Command::SetPeerVolume { peer_id, .. } => validate_peer_id(peer_id),
        Command::EnsurePeer { peer_id, .. }
        | Command::SetPeerMuted { peer_id, .. }
        | Command::RemovePeer { peer_id }
        | Command::RestartIce { peer_id } => validate_peer_id(peer_id),
        Command::StartScreen { profile, .. } | Command::SetShareProfile { profile, .. }
            if !(1..=60).contains(&profile.fps) =>
        {
            Err("INVALID_ARGUMENT: fps must be 1..=60".into())
        }
        Command::StartScreen {
            source: ShareSource::Screen { width, height, .. },
            ..
        } if *width == 0 || *height == 0 => Err("INVALID_ARGUMENT: invalid capture dimensions".into()),
        Command::StartScreen {
            source: ShareSource::Window { hwnd, pid },
            ..
        } if *hwnd == 0 || *pid == 0 => Err("INVALID_ARGUMENT: identidade de janela inválida.".into()),
        Command::StartScreen {
            source: ShareSource::Screen { .. },
            audio_mode: ScreenAudioMode::Application,
            ..
        } => Err("INVALID_ARGUMENT: selecione a janela para capturar o áudio do aplicativo.".into()),
        Command::SetOutput { volume, .. } if !(0.0..=1.0).contains(volume) => {
            Err("INVALID_ARGUMENT: invalid output volume".into())
        }
        _ => Ok(()),
    }
}

type EventSink = Sender<serde_json::Value>;

pub struct RustEngine {
    sender: SyncSender<Work>,
    worker: Option<JoinHandle<()>>,
    pub events: Receiver<serde_json::Value>,
    request_id: i64,
}

impl RustEngine {
    pub fn new(config_json: &str) -> Result<Self, String> {
        Self::new_with_surface(config_json, VideoSurface::default())
    }

    pub fn new_with_surface(config_json: &str, surface: VideoSurface) -> Result<Self, String> {
        let config = parse_config(config_json)?;
        #[cfg(windows)]
        ducking::opt_out();
        let factory = PeerConnectionFactory::default();
        let (sender, receiver) = sync_channel(QUEUE_CAPACITY);
        let (event_sender, events) = channel();
        let output = Arc::new(event_sender);
        let worker = thread::spawn(move || run_worker(receiver, factory, config, output, surface));
        Ok(Self {
            sender,
            worker: Some(worker),
            events,
            request_id: 1,
        })
    }

    pub fn submit(&mut self, value: serde_json::Value) -> Result<(), String> {
        self.submit_tracked(value).map(|_| ())
    }

    pub fn submit_tracked(&mut self, value: serde_json::Value) -> Result<i64, String> {
        let command = parse_command(&value.to_string())?;
        self.sender
            .try_send(Work::Command {
                request_id: self.request_id,
                command,
            })
            .map_err(|_| "engine command queue full or closed".to_owned())?;
        self.request_id += 1;
        Ok(self.request_id - 1)
    }

    pub fn close(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.sender.send(Work::Close);
            let _ = worker.join();
        }
    }
}

impl Drop for RustEngine {
    fn drop(&mut self) {
        self.close();
    }
}

fn event(output: &EventSink, value: serde_json::Value) {
    let _ = output.send(value);
}

fn result_event(output: &EventSink, request_id: i64, result: Result<(), String>) {
    match result {
        Ok(()) => event(
            output,
            json!({ "event": "completed", "requestId": request_id }),
        ),
        Err(message) => {
            let (code, message) = message.split_once(": ").unwrap_or(("RTC_ERROR", &message));
            event(
                output,
                json!({ "event": "error", "requestId": request_id, "code": code, "message": message }),
            );
        }
    }
}

fn local_description(
    runtime: &Runtime,
    peer: &PeerConnection,
    peer_id: &str,
    answer: bool,
    output: &EventSink,
) -> Result<(), String> {
    let description = if answer {
        runtime
            .block_on(peer.create_answer(Default::default()))
            .map_err(|error| error.to_string())?
    } else {
        runtime
            .block_on(peer.create_offer(OfferOptions {
                offer_to_receive_audio: true,
                offer_to_receive_video: true,
                ..Default::default()
            }))
            .map_err(|error| error.to_string())?
    };
    // Same Opus upgrade the web client applies to every offer and answer.
    let description = SessionDescription::parse(
        &share::upgrade_audio_quality_sdp(&description.to_string()),
        description.sdp_type(),
    )
    .map_err(|error| error.to_string())?;
    runtime
        .block_on(peer.set_local_description(description.clone()))
        .map_err(|error| error.to_string())?;
    if std::env::var_os("SCREENSHARE_SDP_LOG").is_some() {
        let sdp = description.to_string();
        let lines: Vec<_> = sdp.lines().filter(|l| l.starts_with("m=") || matches!(*l, "a=sendrecv" | "a=sendonly" | "a=recvonly" | "a=inactive")).map(|l| l.split(' ').next().unwrap_or(l)).collect();
        eprintln!("local {} to {peer_id}: {}", description.sdp_type(), lines.join(" "));
    }
    event(
        output,
        json!({
            "event": "localDescription",
            "peerId": peer_id,
            "type": description.sdp_type().to_string(),
            "sdp": description.to_string(),
        }),
    );
    Ok(())
}

fn negotiate_or_defer(
    runtime: &Runtime,
    peers: &HashMap<String, PeerConnection>,
    peer_id: &str,
    pending: &mut HashSet<String>,
    output: &EventSink,
) -> Result<(), String> {
    let peer = peers
        .get(peer_id)
        .ok_or_else(|| "RTC_ERROR: unknown peer".to_owned())?;
    if ready_to_negotiate(peer) {
        local_description(runtime, peer, peer_id, false, output)
    } else {
        pending.insert(peer_id.to_owned());
        Ok(())
    }
}

/// Same guard as PeerConnections.onNegotiationNeeded: only a stable signaling state. Waiting for a
/// connected transport deadlocked when the first offer carried no media (a viewer with nothing to
/// send): nothing ever connected, so the share offer was deferred forever.
fn ready_to_negotiate(peer: &PeerConnection) -> bool {
    peer.signaling_state() == libwebrtc::peer_connection::SignalingState::Stable
}

fn stats_snapshot(runtime: &Runtime, peer: &PeerConnection) -> Result<serde_json::Value, String> {
    let stats = runtime
        .block_on(peer.get_stats())
        .map_err(|error| error.to_string())?;
    let mut codecs = Vec::new();
    let mut inbound = Vec::new();
    let mut outbound = Vec::new();
    let mut selected_pair = None;
    let mut candidates = HashMap::new();
    for stat in &stats {
        match stat {
            RtcStats::LocalCandidate(value) => {
                candidates.insert(
                    value.rtc.id.clone(),
                    value
                        .local_candidate
                        .candidate_type
                        .map(|kind| format!("{kind:?}")),
                );
            }
            RtcStats::RemoteCandidate(value) => {
                candidates.insert(
                    value.rtc.id.clone(),
                    value
                        .remote_candidate
                        .candidate_type
                        .map(|kind| format!("{kind:?}")),
                );
            }
            _ => {}
        }
    }
    for stat in stats {
        match stat {
            RtcStats::Codec(value) => codecs.push(json!({
                "mimeType": value.codec.mime_type,
                "clockRate": value.codec.clock_rate,
            })),
            RtcStats::InboundRtp(value) => inbound.push(json!({
                "kind": value.stream.kind,
                "codecId": value.stream.codec_id,
                "bytesReceived": value.inbound.bytes_received,
                "framesDecoded": value.inbound.frames_decoded,
                "framesRendered": value.inbound.frames_rendered,
                "framesReceived": value.inbound.frames_received,
                "frameWidth": value.inbound.frame_width,
                "frameHeight": value.inbound.frame_height,
                "totalAudioEnergy": value.inbound.total_audio_energy,
            })),
            RtcStats::OutboundRtp(value) => outbound.push(json!({
                "kind": value.stream.kind,
                "codecId": value.stream.codec_id,
                "bytesSent": value.sent.bytes_sent,
                "framesEncoded": value.outbound.frames_encoded,
                "frameWidth": value.outbound.frame_width,
                "frameHeight": value.outbound.frame_height,
            })),
            RtcStats::CandidatePair(value) if value.candidate_pair.nominated => {
                let local = candidates.get(&value.candidate_pair.local_candidate_id);
                let remote = candidates.get(&value.candidate_pair.remote_candidate_id);
                selected_pair = Some(json!({
                    "state": value.candidate_pair.state.map(|state| format!("{state:?}")),
                    "localType": local,
                    "remoteType": remote,
                    "bytesSent": value.candidate_pair.bytes_sent,
                    "bytesReceived": value.candidate_pair.bytes_received,
                }));
            }
            _ => {}
        }
    }
    Ok(json!({
        "codec": codecs,
        "inbound": inbound,
        "outbound": outbound,
        "selectedCandidatePair": selected_pair,
    }))
}

fn callbacks(peer: &PeerConnection, peer_id: String, output: Arc<EventSink>) {
    let candidate_peer = peer_id.clone();
    let candidate_output = output.clone();
    peer.on_ice_candidate(Some(Box::new(move |candidate| {
        event(
            &candidate_output,
            json!({
                "event": "localIce",
                "peerId": candidate_peer,
                "candidate": candidate.candidate(),
                "sdpMid": candidate.sdp_mid(),
                "sdpMLineIndex": candidate.sdp_mline_index(),
            }),
        );
    })));
    peer.on_connection_state_change(Some(Box::new(move |state| {
        event(
            &output,
            json!({ "event": "peerState", "peerId": peer_id, "state": format!("{state:?}").to_lowercase() }),
        );
    })));
}

#[cfg(feature = "synthetic-test")]
fn attach_screen(
    peer: &PeerConnection,
    factory: &PeerConnectionFactory,
    screen: &SyntheticScreen,
    media: &mut PeerMedia,
) -> Result<(), String> {
    if !media.screen.is_empty() {
        return Ok(());
    }
    let track = factory.create_video_track("screen", screen.source.clone());
    media.screen.push(
        peer.add_track(track.into(), &["screen"])
            .map_err(|error| error.to_string())?,
    );
    if let Some(audio) = &screen.audio {
        let track = factory.create_audio_track("screen-audio", audio.source.clone());
        match peer.add_track(track.into(), &["screen"]) {
            Ok(sender) => media.screen.push(sender),
            Err(error) => {
                for sender in media.screen.drain(..) {
                    let _ = peer.remove_track(sender);
                }
                return Err(error.to_string());
            }
        }
    }
    Ok(())
}

#[cfg(feature = "synthetic-test")]
fn start_synthetic_screen(
    runtime: &Runtime,
    width: i32,
    height: i32,
    fps: i32,
    system_audio: bool,
) -> SyntheticScreen {
    let source = {
        let _runtime = runtime.enter();
        NativeVideoSource::new(
            VideoResolution {
                width: width as u32,
                height: height as u32,
            },
            true,
        )
    };
    let stopped = Arc::new(AtomicBool::new(false));
    let source_for_task = source.clone();
    let stopped_for_task = stopped.clone();
    let started = std::time::Instant::now();
    runtime.spawn(async move {
        let mut frame = VideoFrame {
            rotation: VideoRotation::VideoRotation0,
            timestamp_us: 0,
            frame_metadata: None,
            buffer: I420Buffer::new_black(width as u32, height as u32),
        };
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(1_000 / fps as u64));
        while !stopped_for_task.load(Ordering::Relaxed) {
            tick.tick().await;
            frame.timestamp_us = started.elapsed().as_micros() as i64;
            let frame_number = frame.timestamp_us as u64 * fps as u64 / 1_000_000;
            let stride = frame.buffer.strides().0 as usize;
            let (luma, _, _) = frame.buffer.data_mut();
            paint_test_frame(
                luma,
                stride,
                width as usize,
                height as usize,
                frame_number,
                fps as u64,
            );
            let _ = source_for_task.capture_frame(&frame);
        }
    });
    SyntheticScreen {
        source,
        audio: system_audio
            .then(|| start_synthetic_audio(runtime, 880, Some((started, fps as u64)))),
        stopped,
    }
}

#[cfg(any(test, feature = "synthetic-test"))]
fn paint_test_frame(
    luma: &mut [u8],
    stride: usize,
    width: usize,
    height: usize,
    frame: u64,
    fps: u64,
) {
    for y in 0..height {
        for x in 0..width {
            // Binary frame counter along the top; moving bars below and a one-second flash.
            luma[y * stride + x] = if y < height / 8 {
                if frame & (1 << (x * 16 / width)) != 0 {
                    235
                } else {
                    16
                }
            } else if frame % fps == 0 {
                235
            } else {
                16 + (((x + frame as usize * 3) % width) * 219 / width) as u8
            };
        }
    }
}

#[cfg(any(test, feature = "synthetic-test"))]
fn flash_tone_gain(elapsed_us: u64, fps: u64) -> i16 {
    if (elapsed_us * fps / 1_000_000) % fps == 0 {
        2
    } else {
        1
    }
}

#[cfg(any(test, feature = "synthetic-test"))]
fn flash_tone_frequency(elapsed_us: u64, fps: u64) -> i32 {
    if flash_tone_gain(elapsed_us, fps) == 2 {
        1_200 + (elapsed_us / 1_000_000 % 16) as i32 * 100
    } else {
        880
    }
}

#[cfg(any(test, feature = "synthetic-test"))]
fn tone_sample(phase: &mut f32, frequency: i32) -> i16 {
    let sample = (phase.sin() * 8_000.0) as i16;
    *phase = (*phase + std::f32::consts::TAU * frequency as f32 / 48_000.0) % std::f32::consts::TAU;
    sample
}

#[cfg(feature = "synthetic-test")]
fn start_synthetic_audio(
    runtime: &Runtime,
    frequency: i32,
    flash_clock: Option<(std::time::Instant, u64)>,
) -> SyntheticAudio {
    let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
    let stopped = Arc::new(AtomicBool::new(false));
    let source_for_task = source.clone();
    let stopped_for_task = stopped.clone();
    runtime.spawn(async move {
        let mut phase = 0.0_f32;
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(10));
        while !stopped_for_task.load(Ordering::Relaxed) {
            tick.tick().await;
            let (gain, frequency) = flash_clock.map_or((1, frequency), |(started, fps)| {
                let elapsed_us = started.elapsed().as_micros() as u64;
                (
                    flash_tone_gain(elapsed_us, fps),
                    flash_tone_frequency(elapsed_us, fps),
                )
            });
            let data = (0..480)
                .map(|_| tone_sample(&mut phase, frequency) * gain)
                .collect::<Vec<_>>();
            if source_for_task
                .capture_frame(&AudioFrame {
                    data: data.into(),
                    sample_rate: 48_000,
                    num_channels: 1,
                    samples_per_channel: 480,
                })
                .await
                .is_err()
            {
                break;
            }
        }
    });
    SyntheticAudio { source, stopped }
}

fn run_worker(
    receiver: Receiver<Work>,
    factory: PeerConnectionFactory,
    mut config: RtcConfiguration,
    output: Arc<EventSink>,
    surface: VideoSurface,
) {
    factory.set_adm_recording_enabled(false);
    factory.set_adm_playout_enabled(false);
    let runtime = Runtime::new().expect("tokio runtime initialization");
    let mut peers: HashMap<String, PeerConnection> = HashMap::new();
    let mut polite_by_peer: HashMap<String, bool> = HashMap::new();
    let mut ignore_offer: HashMap<String, bool> = HashMap::new();
    let mut pending_ice: HashMap<String, Vec<PendingIce>> = HashMap::new();
    let mut remote_sdp: HashMap<String, String> = HashMap::new();
    let mut pending_renegotiation = HashSet::new();
    let mut peer_media: HashMap<String, PeerMedia> = HashMap::new();
    let mut adm_acquired = false;
    #[cfg(feature = "synthetic-test")]
    let mut screen: Option<SyntheticScreen> = None;
    #[cfg(not(feature = "synthetic-test"))]
    let mut screen: Option<Capture> = None;
    #[cfg(not(feature = "synthetic-test"))]
    let (capture_sender, capture_receiver) = sync_channel(8);
    #[cfg(not(feature = "synthetic-test"))]
    let mut pending_capture_request: Option<i64> = None;
    #[cfg(not(feature = "synthetic-test"))]
    let mut camera: Option<camera::Camera> = None;
    #[cfg(not(feature = "synthetic-test"))]
    let mut camera_generation: u64 = 0;
    #[cfg(not(feature = "synthetic-test"))]
    let mut pending_camera_request: Option<i64> = None;
    #[cfg(not(feature = "synthetic-test"))]
    let mut capture_generation: u64 = 0;
    #[cfg(not(feature = "synthetic-test"))]
    let mut share_encoding = (share::SharePreset::Video.profile(), 1080);
    #[cfg(not(feature = "synthetic-test"))]
    let renderer = Renderer::with_surface(surface.clone());
    #[cfg(feature = "synthetic-test")]
    let _ = surface;
    #[cfg(feature = "synthetic-test")]
    let mut microphone: Option<SyntheticAudio> = None;
    #[cfg(feature = "synthetic-test")]
    let remote_audio = Arc::new(Mutex::new(HashMap::<String, Vec<RtcAudioTrack>>::new()));
    let mut microphone_enabled = false;
    // Filled by on_track on libwebrtc's thread, read here for levels and mute.
    let voices = Arc::new(std::sync::Mutex::new(voice::Voices::default()));
    loop {
        #[cfg(not(feature = "synthetic-test"))]
        while let Ok(capture_event) = capture_receiver.try_recv() {
            match capture_event {
                CaptureEvent::Ready(generation)
                    if generation == capture_generation && screen.is_some() =>
                {
                    let attached = peers.iter().try_for_each(|(peer_id, peer)| {
                        attach_real_screen(
                            peer,
                            &factory,
                            screen.as_ref().unwrap(),
                            peer_media.get_mut(peer_id).unwrap(),
                            share_encoding,
                        )
                    });
                    let result = attached.and_then(|()| {
                        peers.keys().try_for_each(|peer_id| {
                            negotiate_or_defer(
                                &runtime,
                                &peers,
                                peer_id,
                                &mut pending_renegotiation,
                                &output,
                            )
                        })
                    });
                    if result.is_ok() {
                        preview(&renderer, &factory, &runtime, LOCAL_SCREEN, capture_generation, &screen.as_ref().unwrap().source);
                        event(
                            &output,
                            json!({ "event": "screenState", "state": "active" }),
                        );
                    } else {
                        let _ = detach_screen(
                            &runtime,
                            &peers,
                            &mut peer_media,
                            &mut pending_renegotiation,
                            &output,
                        );
                        screen.take();
                    }
                    if let Some(id) = pending_capture_request.take() {
                        result_event(&output, id, result);
                    }
                }
                CaptureEvent::Ended(generation, reason)
                    if generation == capture_generation && screen.is_some() =>
                {
                    screen.take();
                    let _ = detach_screen(
                        &runtime,
                        &peers,
                        &mut peer_media,
                        &mut pending_renegotiation,
                        &output,
                    );
                    event(
                        &output,
                        json!({ "event": "screenState", "state": "ended", "reason": reason }),
                    );
                    if let Some(id) = pending_capture_request.take() {
                        result_event(&output, id, Err(reason));
                    }
                }
                CaptureEvent::CameraReady(generation) if generation == camera_generation && camera.is_some() => {
                    let result = peers
                        .iter()
                        .try_for_each(|(peer_id, peer)| attach_camera(peer, &factory, camera.as_ref().unwrap(), peer_media.get_mut(peer_id).unwrap()))
                        .and_then(|()| peers.keys().try_for_each(|peer_id| negotiate_or_defer(&runtime, &peers, peer_id, &mut pending_renegotiation, &output)));
                    if result.is_ok() {
                        preview(&renderer, &factory, &runtime, LOCAL_CAMERA, camera_generation, &camera.as_ref().unwrap().source);
                        event(&output, json!({ "event": "cameraState", "state": "active" }));
                    } else {
                        camera.take();
                        let _ = detach_camera(&runtime, &peers, &mut peer_media, &mut pending_renegotiation, &output);
                    }
                    if let Some(id) = pending_camera_request.take() {
                        result_event(&output, id, result);
                    }
                }
                CaptureEvent::CameraEnded(generation, reason) if generation == camera_generation && camera.is_some() => {
                    camera.take();
                    let _ = detach_camera(&runtime, &peers, &mut peer_media, &mut pending_renegotiation, &output);
                    event(&output, json!({ "event": "cameraState", "state": "ended", "reason": reason }));
                    if let Some(id) = pending_camera_request.take() {
                        result_event(&output, id, Err(reason));
                    }
                }
                _ => {}
            }
        }
        let work = match receiver.recv_timeout(std::time::Duration::from_millis(25)) {
            Ok(work) => work,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match work {
            Work::Close => break,
            Work::Command {
                request_id,
                command,
            } => {
                let result = match command {
                    Command::SetIceServers { ice_servers } => {
                        if peers.is_empty() {
                            config.ice_servers = ice_servers.into_iter().map(|server| IceServer {
                                urls: server.urls,
                                username: server.username,
                                password: server.credential,
                            }).collect();
                            Ok(())
                        } else {
                            Err("RTC_ERROR: ICE servers must arrive before participants.".into())
                        }
                    }
                    Command::EnsurePeer {
                        peer_id,
                        polite,
                        initiate,
                    } => {
                        let created = !peers.contains_key(&peer_id);
                        let ensured = if peers.contains_key(&peer_id) {
                            Ok(())
                        } else if peers.len() >= MAX_PEERS {
                            Err("participant limit reached".into())
                        } else {
                            factory
                                .create_peer_connection(config.clone())
                                .map_err(|error| error.to_string())
                                .and_then(|peer| {
                                    let mut media = PeerMedia::default();
                                    #[cfg(feature = "synthetic-test")]
                                    if let Some(screen) = screen.as_ref() {
                                        attach_screen(&peer, &factory, screen, &mut media)?;
                                    }
                                    #[cfg(not(feature = "synthetic-test"))]
                                    if pending_capture_request.is_none() {
                                        if let Some(screen) = screen.as_ref() {
                                            attach_real_screen(&peer, &factory, screen, &mut media, share_encoding)?;
                                        }
                                    }
                                    // After the screen, so a newcomer's receiver sees the camera second.
                                    #[cfg(not(feature = "synthetic-test"))]
                                    if pending_camera_request.is_none() {
                                        if let Some(camera) = camera.as_ref() {
                                            attach_camera(&peer, &factory, camera, &mut media)?;
                                        }
                                    }
                                    #[cfg(feature = "synthetic-test")]
                                    if let Some(audio) = microphone.as_ref() {
                                        let track = factory
                                            .create_audio_track("microphone", audio.source.clone());
                                        media.microphone = Some(
                                            peer.add_track(track.into(), &["microphone"])
                                                .map_err(|error| error.to_string())?,
                                        );
                                    }
                                    if microphone_enabled {
                                        let track = factory.create_device_audio_track("microphone");
                                        media.microphone = Some(
                                            peer.add_track(track.into(), &["microphone"])
                                                .map_err(|error| error.to_string())?,
                                        );
                                    }
                                    callbacks(&peer, peer_id.clone(), output.clone());
                                    #[cfg(feature = "synthetic-test")]
                                    {
                                        let tracks = remote_audio.clone();
                                        let id = peer_id.clone();
                                        peer.on_track(Some(Box::new(move |event| {
                                            if let MediaStreamTrack::Audio(track) = event.track {
                                                let mut tracks = tracks.lock().unwrap();
                                                let tracks = tracks.entry(id.clone()).or_default();
                                                if tracks.len() < 8 {
                                                    tracks.push(track);
                                                }
                                            }
                                        })));
                                    }
                                    #[cfg(not(feature = "synthetic-test"))]
                                    {
                                        let render_sink = renderer.sink();
                                        let render_runtime = runtime.handle().clone();
                                        let id = peer_id.clone();
                                        let track_output = output.clone();
                                        let track_voices = voices.clone();
                                        peer.on_track(Some(Box::new(move |track_event| {
                                            let kind = if matches!(track_event.track, MediaStreamTrack::Video(_)) { "video" } else { "audio" };
                                            let stream_id = track_event.streams.first().map(|stream| stream.id()).unwrap_or_default();
                                            event(&track_output, json!({
                                                "event": "remoteTrack", "peerId": id, "streamId": stream_id,
                                                "trackId": track_event.track.id(), "kind": kind, "ended": false,
                                            }));
                                            match track_event.track {
                                                MediaStreamTrack::Video(_) => {
                                                    render_sink.add_track(id.clone(), track_event.track, render_runtime.clone());
                                                }
                                                MediaStreamTrack::Audio(track) => {
                                                    if track_event.streams.first().is_none_or(|stream| stream.video_tracks().is_empty()) {
                                                        track_voices.lock().unwrap().add(&id, track);
                                                    } else {
                                                        track_voices.lock().unwrap().add_screen(&id, track);
                                                    }
                                                }
                                            }
                                        })));
                                    }
                                    peers.insert(peer_id.clone(), peer);
                                    polite_by_peer.insert(peer_id.clone(), polite);
                                    ignore_offer.insert(peer_id.clone(), false);
                                    let has_local_tracks = media.microphone.is_some();
                                    let has_local_tracks = has_local_tracks || !media.screen.is_empty() || media.camera.is_some();
                                    if !initiate && has_local_tracks {
                                        pending_renegotiation.insert(peer_id.clone());
                                    }
                                    peer_media.insert(peer_id.clone(), media);
                                    Ok(())
                                })
                        };
                        ensured.and_then(|()| {
                            if initiate && created {
                                local_description(
                                    &runtime,
                                    peers.get(&peer_id).unwrap(),
                                    &peer_id,
                                    false,
                                    &output,
                                )
                            } else if initiate && ready_to_negotiate(peers.get(&peer_id).unwrap())
                                && pending_renegotiation.remove(&peer_id)
                            {
                                local_description(&runtime, peers.get(&peer_id).unwrap(), &peer_id, false, &output)
                            } else {
                                let _ = polite;
                                Ok(())
                            }
                        })
                    }
                    Command::RemovePeer { peer_id } => {
                        if let Some(peer) = peers.remove(&peer_id) {
                            peer.on_ice_candidate(None);
                            peer.on_connection_state_change(None);
                            peer.on_track(None);
                            peer.close();
                        }
                        #[cfg(feature = "synthetic-test")]
                        remote_audio.lock().unwrap().remove(&peer_id);
                        #[cfg(not(feature = "synthetic-test"))]
                        renderer.remove(&peer_id);
                        voices.lock().unwrap().remove(&peer_id);
                        peer_media.remove(&peer_id);
                        polite_by_peer.remove(&peer_id);
                        ignore_offer.remove(&peer_id);
                        pending_ice.remove(&peer_id);
                        remote_sdp.remove(&peer_id);
                        pending_renegotiation.remove(&peer_id);
                        Ok(())
                    }
                    Command::RestartIce { peer_id } => peers
                        .get(&peer_id)
                        .ok_or_else(|| "unknown peer".into())
                        .and_then(|peer| {
                            if peer.signaling_state()
                                != libwebrtc::peer_connection::SignalingState::Stable
                            {
                                return Err("RTC_ERROR: negotiation in progress".into());
                            }
                            peer.restart_ice();
                            local_description(&runtime, peer, &peer_id, false, &output)
                        }),
                    Command::RemoteDescription {
                        peer_id,
                        r#type,
                        sdp,
                    } => peers
                        .get(&peer_id)
                        .ok_or_else(|| "unknown peer".into())
                        .and_then(|peer| {
                            let sdp_type = r#type
                                .parse::<SdpType>()
                                .map_err(|error| error.to_string())?;
                            if sdp_type == SdpType::Offer
                                && peer.signaling_state()
                                    != libwebrtc::peer_connection::SignalingState::Stable
                            {
                                if !polite_by_peer.get(&peer_id).copied().unwrap_or(false) {
                                    ignore_offer.insert(peer_id.clone(), true);
                                    return Ok(());
                                }
                                let rollback = SessionDescription::parse("", SdpType::Rollback)
                                    .map_err(|error| error.to_string())?;
                                runtime
                                    .block_on(peer.set_local_description(rollback))
                                    .map_err(|error| error.to_string())?;
                                pending_renegotiation.insert(peer_id.clone());
                            }
                            let description = SessionDescription::parse(&share::improve_video_startup_sdp(&sdp), sdp_type)
                                .map_err(|error| error.to_string())?;
                            runtime
                                .block_on(peer.set_remote_description(description))
                                .map_err(|error| error.to_string())?;
                            ignore_offer.insert(peer_id.clone(), false);
                            remote_sdp.insert(peer_id.clone(), sdp.clone());
                            if let Some(candidates) = pending_ice.remove(&peer_id) {
                                for pending in candidates {
                                    if pending.queued_at.elapsed()
                                        > std::time::Duration::from_secs(10)
                                        || !ice_generation_matches(
                                            &sdp,
                                            pending.username_fragment.as_deref(),
                                        )
                                    {
                                        continue;
                                    }
                                    runtime
                                        .block_on(peer.add_ice_candidate(pending.candidate))
                                        .map_err(|error| error.to_string())?;
                                }
                            }
                            if sdp_type == SdpType::Offer {
                                local_description(&runtime, peer, &peer_id, true, &output)?;
                                if ready_to_negotiate(peer) && pending_renegotiation.remove(&peer_id) {
                                    local_description(&runtime, peer, &peer_id, false, &output)
                                } else {
                                    Ok(())
                                }
                            } else {
                                if ready_to_negotiate(peer) && pending_renegotiation.remove(&peer_id) {
                                    local_description(&runtime, peer, &peer_id, false, &output)
                                } else {
                                    Ok(())
                                }
                            }
                        }),
                    Command::RemoteIce {
                        peer_id,
                        candidate,
                        sdp_mid,
                        sdp_m_line_index,
                        username_fragment,
                    } => peers
                        .get(&peer_id)
                        .ok_or_else(|| "unknown peer".into())
                        .and_then(|peer| {
                            if ignore_offer.get(&peer_id).copied().unwrap_or(false) {
                                return Ok(());
                            }
                            let candidate = IceCandidate::parse(
                                sdp_mid.as_deref().ok_or("missing sdpMid")?,
                                sdp_m_line_index.ok_or("missing sdpMLineIndex")?,
                                &candidate,
                            )
                            .map_err(|error| error.to_string())?;
                            if remote_sdp.get(&peer_id).is_none_or(|sdp| {
                                !ice_generation_matches(sdp, username_fragment.as_deref())
                            }) {
                                let queue = pending_ice.entry(peer_id.clone()).or_default();
                                queue.retain(|pending| {
                                    pending.queued_at.elapsed()
                                        <= std::time::Duration::from_secs(10)
                                });
                                if queue.len() >= 128 {
                                    return Err("RTC_ERROR: ICE queue full".into());
                                }
                                queue.push(PendingIce {
                                    queued_at: std::time::Instant::now(),
                                    candidate,
                                    username_fragment,
                                });
                                return Ok(());
                            }
                            runtime
                                .block_on(peer.add_ice_candidate(candidate))
                                .map_err(|error| error.to_string())
                        }),
                    #[cfg(not(feature = "synthetic-test"))]
                    Command::StartCamera { device_id } => {
                        if camera.is_some() {
                            Err("RTC_ERROR: camera is already active".into())
                        } else {
                            camera_generation = camera_generation.wrapping_add(1);
                            camera = Some(camera::Camera::start(device_id, camera_generation, &runtime, capture_sender.clone()));
                            pending_camera_request = Some(request_id);
                            event(&output, json!({ "event": "cameraState", "state": "pending" }));
                            continue;
                        }
                    }
                    #[cfg(not(feature = "synthetic-test"))]
                    Command::StopCamera => {
                        camera_generation = camera_generation.wrapping_add(1);
                        camera.take();
                        if let Some(id) = pending_camera_request.take() {
                            result_event(&output, id, Err("CAPTURE_CANCELLED: camera stopped".into()));
                        }
                        let result = detach_camera(&runtime, &peers, &mut peer_media, &mut pending_renegotiation, &output);
                        event(&output, json!({ "event": "cameraState", "state": "ended" }));
                        result
                    }
                    #[cfg(not(feature = "synthetic-test"))]
                    Command::ListCameras => camera::list().map(|devices| event(&output, json!({ "event": "cameraDevices", "devices": devices }))),
                    #[cfg(feature = "synthetic-test")]
                    Command::StartCamera { .. } | Command::StopCamera | Command::ListCameras => Err("UNSUPPORTED: camera in synthetic tests".into()),
                    Command::GetAudioLevels => {
                        let voices = voices.lock().unwrap();
                        let targets: Vec<_> = peers.iter().map(|(peer_id, peer)| (peer_id.clone(), peer.clone(), voices.ids(peer_id))).collect();
                        let output = output.clone();
                        // Off the command loop: every peer's stats, four times a second, kept mute
                        // and share commands waiting behind them.
                        runtime.spawn(async move {
                            let mut local = 0.0_f64;
                            let mut remote = serde_json::Map::new();
                            for (peer_id, peer, ids) in targets {
                                let stats = peer.get_stats().await.unwrap_or_default();
                                let (level, microphone) = voice::levels(&stats, &ids);
                                remote.insert(peer_id, json!(level));
                                local = local.max(microphone);
                            }
                            event(&output, json!({ "event": "audioLevels", "peers": remote, "local": local }));
                        });
                        Ok(())
                    }
                    Command::GetScreenQuality => {
                        let targets: Vec<_> = peers.iter().map(|(peer_id, peer)| (peer_id.clone(), peer.clone())).collect();
                        let output = output.clone();
                        runtime.spawn(async move {
                            let mut samples = serde_json::Map::new();
                            for (peer_id, peer) in targets {
                                let stats = peer.get_stats().await.unwrap_or_default();
                                samples.insert(peer_id, quality::samples(&stats, "screen"));
                            }
                            event(&output, json!({ "event": "screenQuality", "peers": samples }));
                        });
                        Ok(())
                    }
                    Command::SetPeerMuted { peer_id, muted } => {
                        if peers.contains_key(&peer_id) {
                            voices.lock().unwrap().set_muted(&peer_id, muted);
                            Ok(())
                        } else {
                            Err("INVALID_ARGUMENT: unknown peer".into())
                        }
                    }
                    // Kept for a peer whose connection is not up yet: its tracks pick the volume up on arrival.
                    Command::SetPeerVolume { peer_id, volume, screen } => {
                        voices.lock().unwrap().set_volume(&peer_id, volume, screen);
                        Ok(())
                    }
                    Command::GetStats => peers
                        .iter()
                        .map(|(peer_id, peer)| {
                            stats_snapshot(&runtime, peer).map(|snapshot| {
                                #[cfg(feature = "synthetic-test")]
                                let snapshot = {
                                    let tracks = remote_audio.lock().unwrap().get(peer_id).cloned().unwrap_or_default();
                                    let mut snapshot = snapshot;
                                    let mut tones = Vec::new();
                                    for track in tracks {
                                        let mut stream = NativeAudioStream::with_options(track, 48_000, 1, NativeAudioStreamOptions { queue_size_frames: Some(10) });
                                        if let Ok(Some(frame)) = runtime.block_on(async { tokio::time::timeout(std::time::Duration::from_millis(200), stream.next()).await }) {
                                            if frame.data.iter().any(|sample| sample.unsigned_abs() > 100) {
                                                let crossings = frame.data.windows(2).filter(|pair| (pair[0] < 0) != (pair[1] < 0)).count();
                                                tones.push(crossings as f64 * 48_000.0 / (2.0 * frame.data.len() as f64));
                                            }
                                        }
                                    }
                                    snapshot["decodedAudioTonesHz"] = json!(tones);
                                    snapshot
                                };
                                json!({ "peerId": peer_id, "snapshot": snapshot })
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map(|snapshots| {
                            let resources = json!({
                                "peers": peers.len(),
                                "pendingIce": pending_ice.values().map(Vec::len).sum::<usize>(),
                                "microphoneSenders": peer_media.values().filter(|media| media.microphone.is_some()).count(),
                                "admReferences": usize::from(adm_acquired),
                            });
                            #[cfg(feature = "synthetic-test")]
                            let resources = {
                                let mut resources = resources;
                                resources["screenSources"] = json!(usize::from(screen.is_some()));
                                resources["microphoneSources"] = json!(usize::from(microphone.is_some() || microphone_enabled));
                                resources["screenSenders"] = json!(peer_media.values().map(|media| media.screen.len()).sum::<usize>());
                                resources
                            };
                            #[cfg(not(feature = "synthetic-test"))]
                            let resources = {
                                let mut resources = resources;
                                let (received, presented) = renderer.counts();
                                resources["screenSources"] = json!(usize::from(screen.is_some()));
                                resources["screenSenders"] = json!(peer_media.values().map(|media| media.screen.len()).sum::<usize>());
                                resources["screenFramesCaptured"] = json!(screen.as_ref().map_or(0, Capture::frames));
                                resources["remoteFramesReceived"] = json!(received);
                                resources["remoteFramesPresented"] = json!(presented);
                                resources
                            };
                            event(&output, json!({ "event": "stats", "peers": snapshots, "resourceCounts": resources }));
                        }),
                    #[cfg(feature = "synthetic-test")]
                    Command::StartScreen { source, profile, audio_mode, .. } => {
                        let (width, height) = match source {
                            ShareSource::Screen { width, height, .. } => (width as i32, height as i32),
                            ShareSource::Window { .. } => (1280, 720),
                        };
                        let fps = profile.fps as i32;
                        let system_audio = audio_mode != ScreenAudioMode::None;
                        if screen.is_some() {
                            Err("RTC_ERROR: screen is already active".into())
                        } else {
                            let candidate =
                                start_synthetic_screen(&runtime, width, height, fps, system_audio);
                            let attached = peers.iter().try_for_each(|(peer_id, peer)| {
                                attach_screen(
                                    peer,
                                    &factory,
                                    &candidate,
                                    peer_media.get_mut(peer_id).unwrap(),
                                )
                            });
                            if attached.is_err() {
                                for (peer_id, peer) in &peers {
                                    for sender in peer_media.get_mut(peer_id).unwrap().screen.drain(..) {
                                        let _ = peer.remove_track(sender);
                                    }
                                }
                            }
                            attached.and_then(|()| {
                                screen = Some(candidate);
                                peers
                                    .keys()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .try_for_each(|peer_id| {
                                        negotiate_or_defer(
                                            &runtime,
                                            &peers,
                                            &peer_id,
                                            &mut pending_renegotiation,
                                            &output,
                                        )
                                    })?;
                                event(
                                    &output,
                                    json!({ "event": "screenState", "state": "active" }),
                                );
                                Ok(())
                            })
                        }
                    }
                    #[cfg(not(feature = "synthetic-test"))]
                    Command::StartScreen { source, profile, height, audio_mode } => {
                        if screen.is_some() { Err("RTC_ERROR: screen is already active".into()) }
                        else {
                            capture_generation = capture_generation.wrapping_add(1);
                            match Capture::start(&source, profile.fps, audio_mode, profile.priority.screencast(), capture_generation, &runtime, capture_sender.clone()) {
                                Ok(capture) => {
                                    #[cfg(windows)]
                                    capture.set_max_height(height);
                                    screen = Some(capture);
                                    share_encoding = (profile, height);
                                    pending_capture_request = Some(request_id);
                                    event(&output, json!({ "event": "screenState", "state": "picking" }));
                                    continue;
                                }
                                Err(error) => Err(error),
                            }
                        }
                    }
                    #[cfg(not(feature = "synthetic-test"))]
                    Command::SetShareProfile { profile, height } => {
                        share_encoding = (profile, height);
                        if let Some(screen) = &screen {
                            screen.set_fps(profile.fps);
                            #[cfg(windows)]
                            screen.set_max_height(height);
                        }
                        for sender in peer_media.values().filter_map(|media| media.screen.first()) {
                            apply_share_encoding(sender, profile, height);
                        }
                        Ok(())
                    }
                    #[cfg(feature = "synthetic-test")]
                    Command::SetShareProfile { .. } => Ok(()),
                    #[cfg(feature = "synthetic-test")]
                    Command::StopScreen => {
                        if let Some(screen) = screen.take() {
                            drop(screen);
                            for (peer_id, peer) in &peers {
                                if let Some(media) = peer_media.get_mut(peer_id) {
                                    for sender in media.screen.drain(..) {
                                        let _ = peer.remove_track(sender);
                                    }
                                }
                            }
                            let result = peers
                                .keys()
                                .cloned()
                                .collect::<Vec<_>>()
                                .into_iter()
                                .try_for_each(|peer_id| {
                                    negotiate_or_defer(
                                        &runtime,
                                        &peers,
                                        &peer_id,
                                        &mut pending_renegotiation,
                                        &output,
                                    )
                                });
                            result.and_then(|()| {
                                event(
                                    &output,
                                    json!({ "event": "screenState", "state": "ended" }),
                                );
                                Ok(())
                            })
                        } else {
                            event(
                                &output,
                                json!({ "event": "screenState", "state": "ended" }),
                            );
                            Ok(())
                        }
                    }
                    #[cfg(not(feature = "synthetic-test"))]
                    Command::StopScreen => {
                        capture_generation = capture_generation.wrapping_add(1);
                        screen.take();
                        if let Some(id) = pending_capture_request.take() {
                            result_event(&output, id, Err("CAPTURE_CANCELLED: picker stopped".into()));
                        }
                        let result = detach_screen(&runtime, &peers, &mut peer_media, &mut pending_renegotiation, &output);
                        event(&output, json!({ "event": "screenState", "state": "ended" }));
                        result
                    }
                    Command::ListAudioDevices => {
                        if !adm_acquired && !factory.acquire_platform_adm() {
                            Err("DEVICE_LOST: platform ADM unavailable".into())
                        } else {
                            adm_acquired = true;
                            let inputs = (0..factory.recording_devices().max(0) as u16)
                                .map(|index| {
                                    json!({
                                        "id": factory.recording_device_guid(index),
                                        "label": factory.recording_device_name(index),
                                    })
                                })
                                .collect::<Vec<_>>();
                            let outputs = (0..factory.playout_devices().max(0) as u16)
                                .map(|index| {
                                    json!({
                                        "id": factory.playout_device_guid(index),
                                        "label": factory.playout_device_name(index),
                                    })
                                })
                                .collect::<Vec<_>>();
                            event(
                                &output,
                                json!({ "event": "audioDevices", "inputs": inputs, "outputs": outputs }),
                            );
                            Ok(())
                        }
                    }
                    Command::SetMicrophone { enabled, device_id } => {
                        #[cfg(feature = "synthetic-test")]
                        if device_id.is_none() && !microphone_enabled {
                            let result = (|| -> Result<(), String> {
                                if enabled && microphone.is_none() {
                                    let candidate = start_synthetic_audio(&runtime, 440, None);
                                    for (peer_id, peer) in &peers {
                                        let media = peer_media.get_mut(peer_id).unwrap();
                                        let track = factory.create_audio_track(
                                            "microphone",
                                            candidate.source.clone(),
                                        );
                                        match peer.add_track(track.into(), &["microphone"]) {
                                            Ok(sender) => media.microphone = Some(sender),
                                            Err(error) => {
                                                for (attached_id, attached_peer) in &peers {
                                                    if let Some(sender) = peer_media
                                                        .get_mut(attached_id)
                                                        .and_then(|media| media.microphone.take())
                                                    {
                                                        let _ = attached_peer.remove_track(sender);
                                                    }
                                                }
                                                return Err(error.to_string());
                                            }
                                        }
                                    }
                                    microphone = Some(candidate);
                                } else if !enabled {
                                    if let Some(audio) = microphone.take() {
                                        audio.stopped.store(true, Ordering::Relaxed);
                                    }
                                    for (peer_id, peer) in &peers {
                                        if let Some(sender) = peer_media
                                            .get_mut(peer_id)
                                            .and_then(|media| media.microphone.take())
                                        {
                                            let _ = peer.remove_track(sender);
                                        }
                                    }
                                }
                                event(&output, json!({"event":"microphoneState","enabled":enabled,"deviceId":null,"requestId":request_id}));
                                peers
                                    .keys()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .try_for_each(|peer_id| {
                                        negotiate_or_defer(
                                            &runtime,
                                            &peers,
                                            &peer_id,
                                            &mut pending_renegotiation,
                                            &output,
                                        )
                                    })?;
                                Ok(())
                            })();
                            result_event(&output, request_id, result);
                            continue;
                        }
                        {
                            #[cfg(feature = "synthetic-test")]
                            if microphone.is_some() {
                                result_event(&output, request_id, Err("RTC_ERROR: stop synthetic microphone before selecting a device".into()));
                                continue;
                            }
                            let result = if enabled {
                                if !adm_acquired && !factory.acquire_platform_adm() {
                                    Err("DEVICE_LOST: platform ADM unavailable".into())
                                } else {
                                    // Acquisition owns an ADM reference even if device selection fails.
                                    adm_acquired = true;
                                    if device_id.as_deref().is_none_or(|id| {
                                        audio_device_index(id, (0..factory.recording_devices().max(0) as u16).map(|index| factory.recording_device_guid(index)))
                                            .is_some_and(|index| factory.set_recording_device(index))
                                    }) {
                                        Ok(())
                                    } else {
                                        Err("DEVICE_LOST: recording device unavailable".into())
                                    }
                                }
                            } else {
                                Ok(())
                            };
                            result.and_then(|()| {
                                adm_acquired = enabled || adm_acquired;
                                factory.set_adm_recording_enabled(enabled);
                                if factory.adm_recording_enabled() != enabled {
                                    return Err("DEVICE_LOST: recording state was not applied".into());
                                }
                                let mut attached = Vec::new();
                                for (peer_id, peer) in &peers {
                                    let media = peer_media.get_mut(peer_id).unwrap();
                                    if enabled && media.microphone.is_none() {
                                        let track = factory.create_device_audio_track("microphone");
                                        match peer.add_track(track.into(), &["microphone"]) {
                                            Ok(sender) => {
                                                media.microphone = Some(sender);
                                                attached.push(peer_id.clone());
                                            }
                                            Err(error) => {
                                                for attached_id in attached {
                                                    if let Some(sender) = peer_media
                                                        .get_mut(&attached_id)
                                                        .and_then(|media| media.microphone.take())
                                                    {
                                                        let _ = peers[&attached_id].remove_track(sender);
                                                    }
                                                }
                                                factory.set_adm_recording_enabled(microphone_enabled);
                                                return Err(error.to_string());
                                            }
                                        }
                                    } else if !enabled {
                                        if let Some(sender) = media.microphone.take() {
                                            let _ = peer.remove_track(sender);
                                        }
                                    }
                                }
                                microphone_enabled = enabled;
                                event(&output, json!({"event":"microphoneState","enabled":enabled,"deviceId":device_id,"requestId":request_id}));
                                peers
                                    .keys()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .try_for_each(|peer_id| {
                                        negotiate_or_defer(
                                            &runtime,
                                            &peers,
                                            &peer_id,
                                            &mut pending_renegotiation,
                                            &output,
                                        )
                                    })?;
                                Ok(())
                            })
                        }
                    }
                    // ponytail: the fixed libwebrtc binding exposes device selection but no volume setter; wire volume when upstream exposes one.
                    Command::SetOutput {
                        device_id,
                        volume: _,
                        enabled,
                    } => {
                        if !enabled {
                            factory.set_adm_playout_enabled(false);
                            if factory.adm_playout_enabled() { result_event(&output, request_id, Err("DEVICE_LOST: playout did not stop".into())); continue; }
                            event(&output, json!({"event":"outputState","enabled":false,"deviceId":null,"requestId":request_id}));
                            Ok(())
                        } else if !adm_acquired && !factory.acquire_platform_adm() {
                            Err("DEVICE_LOST: platform ADM unavailable".into())
                        } else {
                            adm_acquired = true;
                            if device_id.as_deref().is_none_or(|id| {
                                audio_device_index(id, (0..factory.playout_devices().max(0) as u16).map(|index| factory.playout_device_guid(index)))
                                    .is_some_and(|index| factory.set_playout_device(index))
                            }) {
                                factory.set_adm_playout_enabled(true);
                                if !factory.adm_playout_enabled() { result_event(&output, request_id, Err("DEVICE_LOST: playout state was not applied".into())); continue; }
                                event(&output, json!({"event":"outputState","enabled":true,"deviceId":device_id,"requestId":request_id}));
                                Ok(())
                            } else {
                                Err("DEVICE_LOST: playout device unavailable".into())
                            }
                        }
                    }
                };
                result_event(&output, request_id, result);
            }
        }
    }
    for peer in peers.into_values() {
        peer.on_ice_candidate(None);
        peer.on_connection_state_change(None);
        peer.on_track(None);
        peer.close();
    }
    if adm_acquired {
        factory.release_platform_adm();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wss_has_a_rustls_crypto_provider() {
        // Panics like the 0.1.0 and 0.1.1 betas did on their first wss:// connect when no provider is built in.
        let _ = rustls::ClientConfig::builder();
    }

    #[test]
    fn this_build_offers_the_codecs_the_master_prefers() {
        use libwebrtc::{MediaType, peer_connection_factory::PeerConnectionFactory};
        let factory = PeerConnectionFactory::default();
        let names = |caps: libwebrtc::rtp_parameters::RtpCapabilities| {
            caps.codecs.iter().map(|c| format!("{} {}", c.mime_type, c.sdp_fmtp_line.as_deref().unwrap_or(""))).collect::<Vec<_>>()
        };
        let send = names(factory.get_rtp_sender_capabilities(MediaType::Video));
        println!("video send: {send:#?}");
        println!("video receive: {:#?}", names(factory.get_rtp_receiver_capabilities(MediaType::Video)));
        // PeerConnections.kt orders VP9 > H264 > VP8 > AV1; all four must exist to port it 1:1.
        for codec in ["video/VP9", "video/H264", "video/VP8", "video/AV1"] {
            assert!(send.iter().any(|c| c.starts_with(codec)), "{codec} missing from this libwebrtc build");
        }
    }

    #[cfg(not(feature = "synthetic-test"))]
    #[test]
    fn a_new_local_preview_replaces_the_old_one() {
        use libwebrtc::{
            peer_connection_factory::PeerConnectionFactory,
            video_frame::{I420Buffer, VideoFrame, VideoRotation},
            video_source::{VideoResolution, native::NativeVideoSource},
        };
        let runtime = super::Runtime::new().unwrap();
        let _context = runtime.enter();
        let factory = PeerConnectionFactory::default();
        let surface = super::VideoSurface::default();
        let renderer = super::Renderer::with_surface(surface.clone());
        let [old, new] = [8, 16].map(|size| NativeVideoSource::new(VideoResolution { width: size, height: size }, true));
        super::preview(&renderer, &factory, &runtime, super::LOCAL_SCREEN, 1, &old);
        super::preview(&renderer, &factory, &runtime, super::LOCAL_SCREEN, 2, &new);
        let frame = |size| VideoFrame { rotation: VideoRotation::VideoRotation0, timestamp_us: 0, frame_metadata: None, buffer: I420Buffer::new_black(size, size) };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while surface.lock().unwrap().latest.is_empty() && std::time::Instant::now() < deadline {
            old.capture_frame(&frame(8));
            new.capture_frame(&frame(16));
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // The old task's cleanup runs after the new one started: it must not take the new frames away.
        std::thread::sleep(std::time::Duration::from_millis(200));
        new.capture_frame(&frame(16));
        std::thread::sleep(std::time::Duration::from_millis(100));
        let frames = surface.lock().unwrap();
        assert_eq!(frames.order[super::LOCAL_SCREEN], vec!["local:screen-2".to_owned()]);
        assert!(frames.latest.values().all(|f| f.width == 16 && f.track_id == "local:screen-2"));
        assert!(!frames.latest.is_empty());
    }

    #[cfg(feature = "synthetic-test")]
    #[test]
    fn rust_engine_stops_events_after_close() {
        let mut engine = super::RustEngine::new(
            r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#,
        )
        .unwrap();
        engine
            .submit(serde_json::json!({"command":"ensurePeer","peerId":"peer","polite":false,"initiate":true}))
            .unwrap();
        engine.close();
        while engine.events.try_recv().is_ok() {}
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(engine.events.try_recv().is_err());
    }

    #[cfg(feature = "synthetic-test")]
    #[test]
    fn dropping_synthetic_sources_cancels_their_producers() {
        let runtime = super::Runtime::new().unwrap();
        let screen = super::start_synthetic_screen(&runtime, 320, 180, 30, true);
        let video_stopped = screen.stopped.clone();
        let audio_stopped = screen.audio.as_ref().unwrap().stopped.clone();
        drop(screen);
        assert!(video_stopped.load(super::Ordering::Relaxed));
        assert!(audio_stopped.load(super::Ordering::Relaxed));
    }

    #[test]
    fn synthetic_video_moves_and_preserves_stride_padding() {
        let mut first = vec![0; 12 * 16];
        super::paint_test_frame(&mut first, 12, 8, 16, 1, 10);
        let mut next = vec![0; 12 * 16];
        super::paint_test_frame(&mut next, 12, 8, 16, 2, 10);
        assert_ne!(first, next);
        assert!(
            first
                .chunks(12)
                .all(|row| row[8..].iter().all(|value| *value == 0))
        );
        super::paint_test_frame(&mut next, 12, 8, 16, 10, 10);
        assert!(next[2 * 12..3 * 12][..8].iter().all(|value| *value == 235));
        for elapsed_us in [0, 99_999, 100_000, 999_999, 1_000_000, 1_100_000] {
            let frame = elapsed_us * 10 / 1_000_000;
            super::paint_test_frame(&mut next, 12, 8, 16, frame, 10);
            assert_eq!(super::flash_tone_gain(elapsed_us, 10) == 2, next[24] == 235);
            assert_eq!(
                super::flash_tone_frequency(elapsed_us, 10),
                if next[24] == 235 {
                    1_200 + (frame / 10 % 16) as i32 * 100
                } else {
                    880
                }
            );
        }
        assert_eq!(super::flash_tone_frequency(15_000_000, 10), 2_700);
        assert_eq!(super::flash_tone_frequency(16_000_000, 10), 1_200);
    }
    #[test]
    fn synthetic_tone_keeps_frequency_after_thirty_minutes() {
        for frequency in [440, 880] {
            let mut phase = 0.0;
            for _ in 0..48_000 * 1_800 {
                super::tone_sample(&mut phase, frequency);
            }
            let samples = (0..480)
                .map(|_| super::tone_sample(&mut phase, frequency))
                .collect::<Vec<_>>();
            let crossings = samples
                .windows(2)
                .filter(|s| (s[0] < 0) != (s[1] < 0))
                .count();
            let expected = frequency as usize / 50;
            assert!(
                crossings.abs_diff(expected) <= 1,
                "{frequency} Hz: {crossings} crossings"
            );
            assert!(phase < std::f32::consts::TAU);
        }
    }
    #[test]
    fn ice_candidates_match_only_their_sdp_generation() {
        let sdp = "v=0\r\na=ice-ufrag:new\r\n";
        assert!(super::ice_generation_matches(sdp, Some("new")));
        assert!(super::ice_generation_matches(sdp, None));
        assert!(!super::ice_generation_matches(sdp, Some("old")));
        assert!(!super::ice_generation_matches(sdp, Some("ne")));
    }
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };

    use super::{parse_command, parse_config};
    use libwebrtc::{
        audio_frame::AudioFrame,
        audio_source::{AudioSourceOptions, native::NativeAudioSource},
        audio_stream::native::{NativeAudioStream, NativeAudioStreamOptions},
        audio_track::RtcAudioTrack,
        ice_candidate::IceCandidate,
        media_stream_track::MediaStreamTrack,
        peer_connection::{OfferOptions, PeerConnectionState, SignalingState},
        peer_connection_factory::{
            PeerConnectionFactory, RtcConfiguration, native::PeerConnectionFactoryExt,
        },
        session_description::{SdpType, SessionDescription},
        video_frame::{I420Buffer, VideoFrame, VideoRotation},
        video_source::{VideoResolution, native::NativeVideoSource},
        video_stream::native::NativeVideoStream,
    };
    use tokio::runtime::Runtime;
    use tokio_stream::StreamExt;

    #[test]
    fn accepts_relay_configuration() {
        assert!(
            parse_config(r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"relay"}"#)
                .is_ok()
        );
    }

    #[test]
    fn applies_server_ice_configuration_before_peer_creation() {
        let command = parse_command(r#"{"command":"setIceServers","iceServers":[{"urls":["stun:stun.example:3478"],"username":"","credential":""},{"urls":["turn:turn.example:3478?transport=udp"],"username":"fixture","credential":"fixture-secret"}]}"#).unwrap();
        assert!(matches!(command, crate::Command::SetIceServers { ice_servers } if ice_servers.len() == 2));
        assert!(parse_command(r#"{"command":"setIceServers","iceServers":[{"urls":["turn:turn.example:3478"],"username":"","credential":""}]}"#).is_err());
    }

    #[test]
    fn rejects_unknown_policy() {
        assert!(
            parse_config(r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"direct"}"#)
                .is_err()
        );
    }

    #[test]
    fn enforces_command_limits_before_queueing() {
        use crate::{Command, audio_device_index};
        assert_eq!(
            audio_device_index("missing", ["real".to_owned()].into_iter()),
            None
        );
        assert_eq!(
            audio_device_index(
                "second",
                ["first".to_owned(), "second".to_owned()].into_iter()
            ),
            Some(1)
        );
        assert!(matches!(
            parse_command(r#"{"command":"setOutput","volume":1.0}"#).unwrap(),
            Command::SetOutput { enabled: true, .. }
        ));
        assert!(matches!(
            parse_command(r#"{"command":"setOutput","volume":1.0,"enabled":false}"#).unwrap(),
            Command::SetOutput { enabled: false, .. }
        ));
        assert!(
            parse_command(r#"{"command":"setMicrophone","enabled":true,"deviceId":"bad\n"}"#)
                .is_err()
        );
        let too_long_peer = "x".repeat(129);
        assert!(parse_command(&format!(
            r#"{{"command":"ensurePeer","peerId":"{too_long_peer}","polite":false,"initiate":false}}"#
        ))
        .is_err());
    }

    #[test]
    fn rolls_back_the_polite_side_of_a_simultaneous_offer() {
        Runtime::new().unwrap().block_on(async {
            let factory = PeerConnectionFactory::default();
            let impolite = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let polite = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let impolite_offer = impolite.create_offer(Default::default()).await.unwrap();
            let polite_offer = polite.create_offer(Default::default()).await.unwrap();

            impolite
                .set_local_description(impolite_offer.clone())
                .await
                .unwrap();
            polite.set_local_description(polite_offer).await.unwrap();
            assert_eq!(polite.signaling_state(), SignalingState::HaveLocalOffer);

            polite
                .set_local_description(SessionDescription::parse("", SdpType::Rollback).unwrap())
                .await
                .unwrap();
            polite.set_remote_description(impolite_offer).await.unwrap();
            let answer = polite.create_answer(Default::default()).await.unwrap();
            polite.set_local_description(answer.clone()).await.unwrap();
            impolite.set_remote_description(answer).await.unwrap();
            assert_eq!(polite.signaling_state(), SignalingState::Stable);
            assert_eq!(impolite.signaling_state(), SignalingState::Stable);

            let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            polite
                .add_track(
                    factory.create_audio_track("microphone", source).into(),
                    &["microphone"],
                )
                .unwrap();
            let pending_offer = polite.create_offer(Default::default()).await.unwrap();
            assert!(pending_offer.to_string().contains("microphone"));
            polite
                .set_local_description(pending_offer.clone())
                .await
                .unwrap();
            impolite
                .set_remote_description(pending_offer)
                .await
                .unwrap();
            let answer = impolite.create_answer(Default::default()).await.unwrap();
            impolite
                .set_local_description(answer.clone())
                .await
                .unwrap();
            polite.set_remote_description(answer).await.unwrap();
            assert_eq!(polite.signaling_state(), SignalingState::Stable);
            assert_eq!(impolite.signaling_state(), SignalingState::Stable);

            impolite.close();
            polite.close();
        });
    }

    #[test]
    fn retains_senders_added_during_a_rolled_back_offer() {
        Runtime::new().unwrap().block_on(async {
            let factory = PeerConnectionFactory::default();
            let polite = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let remote = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let options = OfferOptions {
                offer_to_receive_audio: true,
                offer_to_receive_video: true,
                ..Default::default()
            };
            let first = polite.create_offer(options.clone()).await.unwrap();
            polite.set_local_description(first).await.unwrap();
            let video = NativeVideoSource::new(
                VideoResolution {
                    width: 320,
                    height: 180,
                },
                true,
            );
            polite
                .add_track(
                    factory.create_video_track("screen", video).into(),
                    &["screen"],
                )
                .unwrap();
            let audio = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            polite
                .add_track(
                    factory.create_audio_track("screen-audio", audio).into(),
                    &["screen"],
                )
                .unwrap();
            let microphone = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            remote
                .add_track(
                    factory.create_audio_track("microphone", microphone).into(),
                    &["microphone"],
                )
                .unwrap();
            let offer = remote.create_offer(options.clone()).await.unwrap();
            remote.set_local_description(offer.clone()).await.unwrap();
            polite
                .set_local_description(SessionDescription::parse("", SdpType::Rollback).unwrap())
                .await
                .unwrap();
            polite.set_remote_description(offer).await.unwrap();
            let answer = polite.create_answer(Default::default()).await.unwrap();
            polite.set_local_description(answer.clone()).await.unwrap();
            remote.set_remote_description(answer).await.unwrap();
            let pending = polite.create_offer(options).await.unwrap().to_string();
            let screen_sections = pending
                .split("m=")
                .filter(|section| section.contains("a=msid:screen "))
                .collect::<Vec<_>>();
            assert_eq!(screen_sections.len(), 2, "screen stream senders were lost");
            assert!(
                screen_sections
                    .iter()
                    .all(|section| section.contains("a=sendrecv")),
                "screen stream senders are inactive"
            );
            polite.close();
            remote.close();
        });
    }

    #[test]
    fn submits_a_synthetic_i420_frame_without_user_capture() {
        Runtime::new().unwrap().block_on(async {
            let source = NativeVideoSource::new(
                VideoResolution {
                    width: 320,
                    height: 180,
                },
                true,
            );
            let factory = PeerConnectionFactory::default();
            let track = factory.create_video_track("synthetic", source.clone());
            let peer = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            peer.add_track(track.into(), &["screen"]).unwrap();
            let frame = VideoFrame {
                rotation: VideoRotation::VideoRotation0,
                timestamp_us: 1,
                frame_metadata: None,
                buffer: I420Buffer::new_black(320, 180),
            };
            let _ = source.capture_frame(&frame);
            peer.close();
        });
    }

    #[test]
    fn offers_opus_from_a_synthetic_audio_source() {
        Runtime::new().unwrap().block_on(async {
            let factory = PeerConnectionFactory::default();
            let peer = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            peer.add_track(
                factory
                    .create_audio_track("synthetic-audio", source.clone())
                    .into(),
                &["audio"],
            )
            .unwrap();
            let offer = peer.create_offer(Default::default()).await.unwrap();
            assert!(offer.to_string().contains("opus/48000"));
            source
                .capture_frame(&AudioFrame {
                    data: vec![0i16; 480].as_slice().into(),
                    sample_rate: 48_000,
                    num_channels: 1,
                    samples_per_channel: 480,
                })
                .await
                .unwrap();
            peer.close();
        });
    }

    #[test]
    fn native_offer_receives_browser_audio_and_video() {
        Runtime::new().unwrap().block_on(async {
            let factory = PeerConnectionFactory::default();
            let peer = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            peer.add_track(factory.create_audio_track("mic", source).into(), &["mic"])
                .unwrap();
            let offer = peer
                .create_offer(OfferOptions {
                    offer_to_receive_audio: true,
                    offer_to_receive_video: true,
                    ..Default::default()
                })
                .await
                .unwrap()
                .to_string();
            let audio_section = offer
                .split("m=audio")
                .nth(1)
                .unwrap()
                .split("m=")
                .next()
                .unwrap();
            assert!(
                audio_section.contains("a=sendrecv"),
                "microphone must send and receive"
            );
            let video_section = offer
                .split("m=video")
                .nth(1)
                .unwrap()
                .split("m=")
                .next()
                .unwrap();
            assert!(video_section.contains("a=recvonly"), "video must receive");
            peer.close();
        });
    }

    #[test]
    fn receives_decoded_opus_audio_and_keeps_screen_and_microphone_streams_separate() {
        Runtime::new().unwrap().block_on(async {
            let factory = PeerConnectionFactory::default();
            let alice = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let bob = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let microphone = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            let screen_audio = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            let return_audio = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
            bob.add_track(
                factory
                    .create_audio_track("return-microphone", return_audio.clone())
                    .into(),
                &["microphone"],
            )
            .unwrap();
            alice
                .add_track(
                    factory
                        .create_audio_track("microphone", microphone.clone())
                        .into(),
                    &["microphone"],
                )
                .unwrap();
            alice
                .add_track(
                    factory
                        .create_audio_track("screen-audio", screen_audio.clone())
                        .into(),
                    &["screen"],
                )
                .unwrap();

            let (to_alice, from_bob) = mpsc::channel();
            let (to_bob, from_alice) = mpsc::channel();
            alice.on_ice_candidate(Some(Box::new(move |candidate| {
                let _ = to_bob.send((
                    candidate.sdp_mid(),
                    candidate.sdp_mline_index(),
                    candidate.candidate(),
                ));
            })));
            bob.on_ice_candidate(Some(Box::new(move |candidate| {
                let _ = to_alice.send((
                    candidate.sdp_mid(),
                    candidate.sdp_mline_index(),
                    candidate.candidate(),
                ));
            })));
            let connected = Arc::new(AtomicBool::new(false));
            let connected_for_callback = connected.clone();
            bob.on_connection_state_change(Some(Box::new(move |state| {
                if state == PeerConnectionState::Connected {
                    connected_for_callback.store(true, Ordering::Relaxed);
                }
            })));
            let remote_audio = Arc::new(Mutex::new(Vec::<RtcAudioTrack>::new()));
            let remote_audio_for_callback = remote_audio.clone();
            let remote_stream_ids = Arc::new(Mutex::new(Vec::new()));
            let stream_ids_for_callback = remote_stream_ids.clone();
            bob.on_track(Some(Box::new(move |event| {
                stream_ids_for_callback
                    .lock()
                    .unwrap()
                    .extend(event.receiver.stream_ids());
                if let MediaStreamTrack::Audio(track) = event.track {
                    let mut tracks = remote_audio_for_callback.lock().unwrap();
                    if event
                        .receiver
                        .stream_ids()
                        .iter()
                        .any(|id| id == "microphone")
                    {
                        tracks.insert(0, track);
                    } else {
                        tracks.push(track);
                    }
                }
            })));
            let return_tracks = Arc::new(Mutex::new(Vec::<RtcAudioTrack>::new()));
            let return_tracks_for_callback = return_tracks.clone();
            alice.on_track(Some(Box::new(move |event| {
                if let MediaStreamTrack::Audio(track) = event.track {
                    return_tracks_for_callback.lock().unwrap().push(track);
                }
            })));

            let offer = alice
                .create_offer(OfferOptions {
                    offer_to_receive_audio: true,
                    ..Default::default()
                })
                .await
                .unwrap();
            let offer_sdp = offer.to_string();
            assert!(offer_sdp.contains("opus/48000"));
            assert!(offer_sdp.contains("a=msid:microphone"));
            assert!(offer_sdp.contains("a=msid:screen"));
            alice.set_local_description(offer.clone()).await.unwrap();
            bob.set_remote_description(offer).await.unwrap();
            let answer = bob.create_answer(Default::default()).await.unwrap();
            bob.set_local_description(answer.clone()).await.unwrap();
            alice.set_remote_description(answer).await.unwrap();

            let mut phase = 0.0_f32;
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline
                && !(connected.load(Ordering::Relaxed)
                    && remote_audio.lock().unwrap().len() == 2
                    && return_tracks.lock().unwrap().len() == 1)
            {
                while let Ok((mid, index, candidate)) = from_alice.try_recv() {
                    bob.add_ice_candidate(IceCandidate::parse(&mid, index, &candidate).unwrap())
                        .await
                        .unwrap();
                }
                while let Ok((mid, index, candidate)) = from_bob.try_recv() {
                    alice
                        .add_ice_candidate(IceCandidate::parse(&mid, index, &candidate).unwrap())
                        .await
                        .unwrap();
                }
                let data = (0..480)
                    .map(|_| {
                        let sample = (phase.sin() * 8_000.0) as i16;
                        phase = (phase + std::f32::consts::TAU * 440.0 / 48_000.0)
                            % std::f32::consts::TAU;
                        sample
                    })
                    .collect::<Vec<_>>();
                microphone
                    .capture_frame(&AudioFrame {
                        data: data.into(),
                        sample_rate: 48_000,
                        num_channels: 1,
                        samples_per_channel: 480,
                    })
                    .await
                    .unwrap();
                screen_audio
                    .capture_frame(&AudioFrame {
                        data: vec![2_000; 480].into(),
                        sample_rate: 48_000,
                        num_channels: 1,
                        samples_per_channel: 480,
                    })
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(connected.load(Ordering::Relaxed));
            let tracks = remote_audio.lock().unwrap().clone();
            assert_eq!(tracks.len(), 2);
            let mut ids = remote_stream_ids.lock().unwrap().clone();
            ids.sort();
            assert_eq!(ids, vec!["microphone", "screen"]);
            let microphone_track = &tracks[0];
            assert_eq!(return_tracks.lock().unwrap().len(), 1);
            let return_track = return_tracks.lock().unwrap()[0].clone();
            let mut return_stream = NativeAudioStream::with_options(
                return_track,
                48_000,
                1,
                NativeAudioStreamOptions {
                    queue_size_frames: Some(10),
                },
            );
            let mut stream = NativeAudioStream::with_options(
                microphone_track.clone(),
                48_000,
                1,
                NativeAudioStreamOptions {
                    queue_size_frames: Some(10),
                },
            );
            let mut forward_received = false;
            let mut reverse_received = false;
            let received = tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    while let Ok((mid, index, candidate)) = from_alice.try_recv() {
                        bob.add_ice_candidate(
                            IceCandidate::parse(&mid, index, &candidate).unwrap(),
                        )
                        .await
                        .unwrap();
                    }
                    let data = (0..480)
                        .map(|_| {
                            let sample = (phase.sin() * 8_000.0) as i16;
                            phase = (phase + std::f32::consts::TAU * 440.0 / 48_000.0)
                                % std::f32::consts::TAU;
                            sample
                        })
                        .collect::<Vec<_>>();
                    let frame = AudioFrame {
                        data: data.into(),
                        sample_rate: 48_000,
                        num_channels: 1,
                        samples_per_channel: 480,
                    };
                    microphone.capture_frame(&frame).await.unwrap();
                    return_audio.capture_frame(&frame).await.unwrap();
                    for (audio_stream, received) in [
                        (&mut stream, &mut forward_received),
                        (&mut return_stream, &mut reverse_received),
                    ] {
                        if let Ok(Some(frame)) =
                            tokio::time::timeout(Duration::from_millis(100), audio_stream.next())
                                .await
                        {
                            let crossings = frame
                                .data
                                .windows(2)
                                .filter(|pair| (pair[0] < 0) != (pair[1] < 0))
                                .count();
                            if frame.data.iter().any(|sample| sample.abs() > 100)
                                && (7..=11).contains(&crossings)
                            {
                                *received = true;
                            }
                        }
                    }
                    if forward_received && reverse_received {
                        break frame;
                    }
                }
            })
            .await
            .expect("both receivers must decode the 440 Hz Opus tone");
            assert_eq!(
                (
                    received.sample_rate,
                    received.num_channels,
                    received.samples_per_channel
                ),
                (48_000, 1, 480)
            );
            assert!(received.data.iter().any(|sample| sample.abs() > 100));

            alice.close();
            bob.close();
        });
    }

    #[test]
    fn connects_two_peers_with_sdp_ice_and_a_synthetic_video_track() {
        Runtime::new().unwrap().block_on(async {
            let factory = PeerConnectionFactory::default();
            let alice = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let bob = factory
                .create_peer_connection(RtcConfiguration::default())
                .unwrap();
            let source = NativeVideoSource::new(
                VideoResolution {
                    width: 320,
                    height: 180,
                },
                true,
            );
            let track = factory.create_video_track("synthetic", source.clone());
            alice.add_track(track.into(), &["screen"]).unwrap();
            let reverse_source = NativeVideoSource::new(
                VideoResolution {
                    width: 320,
                    height: 180,
                },
                true,
            );
            let reverse_track = factory.create_video_track("return-screen", reverse_source.clone());
            bob.add_track(reverse_track.into(), &["screen"]).unwrap();

            let (to_alice, from_bob) = mpsc::channel();
            let (to_bob, from_alice) = mpsc::channel();
            alice.on_ice_candidate(Some(Box::new(move |candidate| {
                let _ = to_bob.send((
                    candidate.sdp_mid(),
                    candidate.sdp_mline_index(),
                    candidate.candidate(),
                ));
            })));
            bob.on_ice_candidate(Some(Box::new(move |candidate| {
                let _ = to_alice.send((
                    candidate.sdp_mid(),
                    candidate.sdp_mline_index(),
                    candidate.candidate(),
                ));
            })));
            let alice_connected = Arc::new(AtomicBool::new(false));
            let alice_state = alice_connected.clone();
            alice.on_connection_state_change(Some(Box::new(move |state| {
                if state == PeerConnectionState::Connected {
                    alice_state.store(true, Ordering::Relaxed);
                }
            })));
            let bob_connected = Arc::new(AtomicBool::new(false));
            let bob_state = bob_connected.clone();
            bob.on_connection_state_change(Some(Box::new(move |state| {
                if state == PeerConnectionState::Connected {
                    bob_state.store(true, Ordering::Relaxed);
                }
            })));
            let remote_track = Arc::new(Mutex::new(None));
            let bob_track = remote_track.clone();
            bob.on_track(Some(Box::new(move |event| {
                if let MediaStreamTrack::Video(track) = event.track {
                    *bob_track.lock().unwrap() = Some(track);
                }
            })));

            let return_track = Arc::new(Mutex::new(None));
            let alice_track = return_track.clone();
            alice.on_track(Some(Box::new(move |event| {
                if let MediaStreamTrack::Video(track) = event.track {
                    *alice_track.lock().unwrap() = Some(track);
                }
            })));
            let offer = alice
                .create_offer(OfferOptions {
                    offer_to_receive_video: true,
                    ..Default::default()
                })
                .await
                .unwrap();
            assert!(offer.to_string().contains("VP8"));
            alice.set_local_description(offer.clone()).await.unwrap();
            bob.set_remote_description(offer).await.unwrap();
            let answer = bob.create_answer(Default::default()).await.unwrap();
            bob.set_local_description(answer.clone()).await.unwrap();
            alice.set_remote_description(answer).await.unwrap();

            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline
                && !(alice_connected.load(Ordering::Relaxed)
                    && bob_connected.load(Ordering::Relaxed)
                    && remote_track.lock().unwrap().is_some()
                    && return_track.lock().unwrap().is_some())
            {
                while let Ok((mid, index, candidate)) = from_alice.try_recv() {
                    bob.add_ice_candidate(IceCandidate::parse(&mid, index, &candidate).unwrap())
                        .await
                        .unwrap();
                }
                while let Ok((mid, index, candidate)) = from_bob.try_recv() {
                    alice
                        .add_ice_candidate(IceCandidate::parse(&mid, index, &candidate).unwrap())
                        .await
                        .unwrap();
                }
                let frame = VideoFrame {
                    rotation: VideoRotation::VideoRotation0,
                    timestamp_us: 0,
                    frame_metadata: None,
                    buffer: I420Buffer::new_black(320, 180),
                };
                let _ = source.capture_frame(&frame);
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(alice_connected.load(Ordering::Relaxed));
            assert!(bob_connected.load(Ordering::Relaxed));
            let mut stream = NativeVideoStream::new(remote_track.lock().unwrap().take().unwrap());
            let mut reverse_stream =
                NativeVideoStream::new(return_track.lock().unwrap().take().unwrap());
            let received = tokio::time::timeout(Duration::from_secs(10), async {
                let mut forward_received = false;
                loop {
                    let frame = VideoFrame {
                        rotation: VideoRotation::VideoRotation0,
                        timestamp_us: 0,
                        frame_metadata: None,
                        buffer: I420Buffer::new_black(320, 180),
                    };
                    let _ = source.capture_frame(&frame);
                    let _ = reverse_source.capture_frame(&frame);
                    if let Ok(Some(frame)) =
                        tokio::time::timeout(Duration::from_millis(100), stream.next()).await
                    {
                        assert_eq!((frame.buffer.width(), frame.buffer.height()), (320, 180));
                        forward_received = true;
                    }
                    if let Ok(Some(frame)) =
                        tokio::time::timeout(Duration::from_millis(100), reverse_stream.next())
                            .await
                    {
                        assert_eq!((frame.buffer.width(), frame.buffer.height()), (320, 180));
                        if forward_received {
                            break frame;
                        }
                    }
                }
            })
            .await
            .expect("receiver did not decode a video frame");
            assert_eq!(
                (received.buffer.width(), received.buffer.height()),
                (320, 180)
            );
            // quality::samples finds our screen's encoder by its track id, and the viewer's inbound video.
            let sent = crate::quality::samples(&alice.get_stats().await.unwrap(), "synthetic");
            assert!(crate::quality::send_sample(&sent["send"], 0.0).is_some(), "{sent}");
            let seen = crate::quality::samples(&bob.get_stats().await.unwrap(), "synthetic");
            let decoded = seen["receive"].as_array().unwrap().iter().filter_map(crate::quality::receive_sample).any(|sample| sample.frames_decoded > 0.0);
            assert!(decoded, "{seen}");
            alice.close();
            bob.close();
        });
    }

    /// A 1080p60 H264 share between two local peers, as the game preset sends it: which encoder
    /// libwebrtc picked and how much CPU the process spent. Ignored: it needs real hardware to mean
    /// anything. `cargo test --release --lib h264_1080p60 -- --ignored --nocapture`, with and
    /// without CUDA_PATH at build time, compares NVENC with OpenH264.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn h264_1080p60_encoder_and_cpu() {
        use libwebrtc::{
            MediaType,
            rtp_parameters::RtpEncodingParameters,
            rtp_transceiver::{RtpTransceiverDirection, RtpTransceiverInit},
            stats::RtcStats,
        };
        use windows::Win32::{Foundation::FILETIME, System::Threading::{GetCurrentProcess, GetProcessTimes}};
        let cpu = || {
            let (mut created, mut exited, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
            // SAFETY: four out-pointers to FILETIMEs on the stack.
            unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user).unwrap() };
            let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32 | u64::from(t.dwLowDateTime)) as f64 / 1e7;
            ticks(kernel) + ticks(user)
        };
        let (width, height) = (1920u32, 1080u32);
        // Moving diagonal bars: every frame changes, like a game, so the encoder does real work.
        let mut frames: Vec<_> = (0..60u32)
            .map(|i| {
                let mut buffer = I420Buffer::new(width, height);
                let (y, u, v) = buffer.data_mut();
                for (n, px) in y.iter_mut().enumerate() {
                    let (x, row) = (n as u32 % width, n as u32 / width);
                    *px = ((x + row + i * 16) % 256) as u8;
                }
                u.fill(100);
                v.fill(160);
                VideoFrame { rotation: VideoRotation::VideoRotation0, timestamp_us: 0, frame_metadata: None, buffer }
            })
            .collect();
        Runtime::new().unwrap().block_on(async {
            let factory = PeerConnectionFactory::default();
            let alice = factory.create_peer_connection(RtcConfiguration::default()).unwrap();
            let bob = factory.create_peer_connection(RtcConfiguration::default()).unwrap();
            let source = NativeVideoSource::new(VideoResolution { width, height }, true);
            let track = factory.create_video_track("game", source.clone());
            let init = RtpTransceiverInit {
                direction: RtpTransceiverDirection::SendOnly,
                stream_ids: vec!["screen".into()],
                send_encodings: vec![RtpEncodingParameters { max_bitrate: Some(8_000_000), max_framerate: Some(60.0), ..Default::default() }],
            };
            let transceiver = alice.add_transceiver(track.into(), init).unwrap();
            let h264: Vec<_> = crate::share::preferred_video_codecs(factory.get_rtp_sender_capabilities(MediaType::Video).codecs, true)
                .into_iter()
                .filter(|codec| codec.mime_type.eq_ignore_ascii_case("video/H264") || codec.mime_type.eq_ignore_ascii_case("video/rtx"))
                .collect();
            transceiver.set_codec_preferences(h264).unwrap();
            let (to_bob, from_alice) = mpsc::channel();
            let (to_alice, from_bob) = mpsc::channel();
            alice.on_ice_candidate(Some(Box::new(move |c| drop(to_bob.send((c.sdp_mid(), c.sdp_mline_index(), c.candidate()))))));
            bob.on_ice_candidate(Some(Box::new(move |c| drop(to_alice.send((c.sdp_mid(), c.sdp_mline_index(), c.candidate()))))));
            let offer = alice.create_offer(OfferOptions::default()).await.unwrap();
            alice.set_local_description(offer.clone()).await.unwrap();
            let offer = SessionDescription::parse(&crate::share::improve_video_startup_sdp(&offer.to_string()), SdpType::Offer).unwrap();
            bob.set_remote_description(offer).await.unwrap();
            let answer = bob.create_answer(Default::default()).await.unwrap();
            bob.set_local_description(answer.clone()).await.unwrap();
            let answer = SessionDescription::parse(&crate::share::improve_video_startup_sdp(&answer.to_string()), SdpType::Answer).unwrap();
            alice.set_remote_description(answer).await.unwrap();

            // Sleep's default 15.6 ms granularity would cap the pacing below at ~30 fps.
            // SAFETY: plain call; the matching timeEndPeriod is the process exit.
            unsafe { windows::Win32::Media::timeBeginPeriod(1) };
            let start = Instant::now();
            let (warmup, measured) = (Duration::from_secs(4), Duration::from_secs(12));
            let mut cpu_at_warmup = None;
            for n in 0u64.. {
                let at = start + Duration::from_micros(n * 16_667);
                if at > start + warmup + measured {
                    break;
                }
                while let Ok((mid, index, c)) = from_alice.try_recv() {
                    bob.add_ice_candidate(IceCandidate::parse(&mid, index, &c).unwrap()).await.unwrap();
                }
                while let Ok((mid, index, c)) = from_bob.try_recv() {
                    alice.add_ice_candidate(IceCandidate::parse(&mid, index, &c).unwrap()).await.unwrap();
                }
                if cpu_at_warmup.is_none() && at >= start + warmup {
                    cpu_at_warmup = Some((cpu(), Instant::now()));
                }
                let frame = &mut frames[(n % 60) as usize];
                frame.timestamp_us = (n * 16_667) as i64;
                source.capture_frame(frame);
                if let Some(wait) = at.checked_duration_since(Instant::now()) {
                    std::thread::sleep(wait);
                }
            }
            let (cpu_start, wall_start) = cpu_at_warmup.unwrap();
            let cores = (cpu() - cpu_start) / wall_start.elapsed().as_secs_f64();
            let stats = alice.get_stats().await.unwrap();
            let outbound = stats.iter().find_map(|s| match s { RtcStats::OutboundRtp(o) if o.stream.kind == "video" => Some(o.clone()), _ => None }).unwrap();
            println!(
                "encoder={} power_efficient={} sent {}x{} at {:.0} fps (limited by {:?}), {} frames encoded, {:.0} kbps; process CPU {:.2} cores ({:.0}% of one)",
                outbound.outbound.encoder_implementation,
                outbound.outbound.power_efficient_encoder,
                outbound.outbound.frame_width,
                outbound.outbound.frame_height,
                outbound.outbound.frames_per_second,
                outbound.outbound.quality_limitation_reason,
                outbound.outbound.frames_encoded,
                outbound.sent.bytes_sent as f64 * 8.0 / start.elapsed().as_secs_f64() / 1000.0,
                cores,
                cores * 100.0,
            );
            assert!(outbound.outbound.frames_encoded > 0);
            alice.close();
            bob.close();
        });
    }
}
