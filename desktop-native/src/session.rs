use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    RustEngine, ScreenAudioMode, ShareSource,
    share::{SharePreset, ShareProfile},
};

use serde::Deserialize;
use serde_json::{Value, json};

const MAX_FRAME: usize = 256 * 1024;
const MAX_CHAT: usize = 4 * 1024;
const MAX_PENDING: usize = 256;
// Same receipt deadline as WebsocketService.sendChatMessage.
const CHAT_RECEIPT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub socket_id: String,
    pub username: String,
    #[serde(default = "muted_by_default")]
    pub is_muted: bool,
}

// SocketUser.isMuted defaults to true on the server: everyone joins muted.
fn muted_by_default() -> bool {
    true
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    #[serde(default)]
    pub username: String,
    pub content: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub sender_socket_id: Option<String>,
    #[serde(default)]
    pub client_message_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenOptions {
    pub source: ShareSource,
    pub profile: ShareProfile,
    /// The share height the bitrate ceiling follows; the capture keeps the source resolution,
    /// like the Tauri native source.
    pub height: u32,
    pub audio_mode: ScreenAudioMode,
}

impl Default for ScreenOptions {
    fn default() -> Self {
        Self {
            source: ShareSource::Screen { left: 0, top: 0, width: 1280, height: 720 },
            profile: SharePreset::Video.profile(),
            height: 1080,
            audio_mode: ScreenAudioMode::None,
        }
    }
}

impl ScreenOptions {
    fn command(&self) -> Result<Value, String> {
        if !(1..=60).contains(&self.profile.fps) {
            return Err("Use de 1 a 60 quadros por segundo.".into());
        }
        if self.audio_mode == ScreenAudioMode::Application && !matches!(self.source, ShareSource::Window { .. }) {
            return Err("Escolha a janela para compartilhar o áudio do aplicativo.".into());
        }
        Ok(json!({"command":"startScreen","source":self.source,"profile":self.profile,"height":self.height,"audioMode":self.audio_mode}))
    }
}

#[derive(Clone, Debug)]
pub enum AudioControl {
    Microphone {
        enabled: bool,
        device_id: Option<String>,
    },
    Output {
        enabled: bool,
        device_id: Option<String>,
    },
    /// "Silenciar áudio para você": that peer's voice, on this client only.
    Peer {
        peer_id: String,
        muted: bool,
    },
}

/// WebRTC `audioLevel` (0..1) above which someone counts as speaking.
/// shortcut: a fixed level, tuned by ear against the web's analyser; make it relative if rooms
/// with loud background noise ring all the time.
const SPEAKING_LEVEL: f64 = 0.05;

/// The `audioLevels` event as the speaking set; the local voice only counts with the microphone on.
fn speaking(event: &Value, local: Option<&str>, microphone_enabled: bool) -> BTreeSet<String> {
    let loud = |level: &Value| level.as_f64().is_some_and(|level| level > SPEAKING_LEVEL);
    let mut speaking: BTreeSet<String> = event["peers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, level)| loud(level))
        .map(|(peer, _)| peer.clone())
        .collect();
    if let (Some(local), true, true) = (local, microphone_enabled, loud(&event["local"])) {
        speaking.insert(local.to_owned());
    }
    speaking
}

impl AudioControl {
    fn command(&self, view: &SessionView) -> Result<Value, String> {
        let (enabled, device_id, devices, command) = match self {
            Self::Peer { peer_id, muted } => return Ok(json!({"command":"setPeerMuted","peerId":peer_id,"muted":muted})),
            Self::Microphone { enabled, device_id } => {
                (*enabled, device_id, &view.audio_inputs, "setMicrophone")
            }
            Self::Output { enabled, device_id } => {
                (*enabled, device_id, &view.audio_outputs, "setOutput")
            }
        };
        if enabled
            && device_id
                .as_ref()
                .is_some_and(|id| !devices.iter().any(|device| &device.id == id))
        {
            return Err("Dispositivo não está na lista atual; atualize os dispositivos.".into());
        }
        let active = match self {
            Self::Microphone { .. } => view.microphone_enabled,
            Self::Output { .. } => view.output_enabled,
            Self::Peer { .. } => false,
        };
        if enabled && active && device_id.is_some() {
            return Err("Desligue o áudio antes de aplicar outro dispositivo.".into());
        }
        Ok(
            json!({"command":command,"enabled":enabled,"deviceId":if enabled {device_id.as_ref()} else {None},"volume":1.0}),
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionView {
    pub generation: u64,
    pub local_socket_id: Option<String>,
    pub participants: Vec<Participant>,
    pub screen_sharers: BTreeSet<String>,
    pub camera_sharers: BTreeSet<String>,
    // shortcut: keep the last 200 messages; persist/page if older history is needed.
    pub messages: VecDeque<ChatMessage>,
    pub message_count: u64,
    pub error: Option<String>,
    pub screen_state: String,
    pub screen_error: Option<String>,
    pub audio_inputs: Vec<AudioDevice>,
    pub audio_outputs: Vec<AudioDevice>,
    pub devices_error: Option<String>,
    pub devices_loaded: bool,
    pub microphone_enabled: bool,
    pub microphone_device: Option<String>,
    pub output_enabled: bool,
    pub output_device: Option<String>,
    pub audio_pending: usize,
    pub audio_error: Option<String>,
    /// Socket ids speaking now, the local one included (web setUserSpeaking).
    pub speaking: BTreeSet<String>,
}

impl SessionView {
    fn audio_devices(&mut self, event: &Value) -> Result<(), serde_json::Error> {
        let inputs = serde_json::from_value(event["inputs"].clone())?;
        let outputs = serde_json::from_value(event["outputs"].clone())?;
        self.audio_inputs = inputs;
        self.audio_outputs = outputs;
        self.devices_loaded = true;
        Ok(())
    }
    /// True when a message with `text` sent by this client arrived after the first `count` messages.
    pub fn echoed_after(&self, count: u64, text: &str) -> bool {
        let Some(local) = &self.local_socket_id else {
            return false;
        };
        let base = self
            .message_count
            .saturating_sub(self.messages.len() as u64);
        self.messages.iter().enumerate().any(|(index, msg)| {
            base + index as u64 + 1 > count
                && msg.sender_socket_id.as_ref() == Some(local)
                && msg.content == text
        })
    }
}

pub struct UiConnection {
    pub chat: std::sync::mpsc::Receiver<(u64, String)>,
    pub screen: std::sync::mpsc::Receiver<(u64, Option<ScreenOptions>)>,
    pub devices: std::sync::mpsc::Receiver<u64>,
    pub audio: std::sync::mpsc::Receiver<(u64, AudioControl)>,
    /// (generation, profile, share height): the running share follows it live.
    pub share_profile: std::sync::mpsc::Receiver<(u64, ShareProfile, u32)>,
    pub view: std::sync::Arc<std::sync::Mutex<SessionView>>,
    pub video_surface: crate::VideoSurface,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocketUser {
    socket_id: String,
    username: String,
    room_id: String,
    #[serde(default = "muted_by_default")]
    is_muted: bool,
}

/// Every server packet of `common/Packet.kt` flattened: `type` + `rid` plus the optional fields.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Packet {
    #[serde(rename = "type")]
    kind: String,
    rid: String,
    #[serde(default)]
    sid: Option<String>,
    #[serde(default)]
    users: Vec<SocketUser>,
    #[serde(default)]
    sdp: Option<Value>,
    #[serde(default)]
    ice: Option<String>,
    #[serde(default)]
    msg: Option<Value>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    screen_sharers: Vec<String>,
    #[serde(default)]
    camera_sharers: Vec<String>,
    #[serde(default)]
    ice_servers: Option<Value>,
}

#[derive(Debug, PartialEq)]
pub enum Action {
    EnsurePeer {
        peer_id: String,
        polite: bool,
        initiate: bool,
    },
    RemovePeer(String),
    RemoteDescription {
        peer_id: String,
        kind: String,
        sdp: String,
    },
    RemoteIce {
        peer_id: String,
        candidate: String,
        mid: Option<String>,
        line: Option<u16>,
        fragment: Option<String>,
    },
    /// STUN/TURN from the server's `ice-servers`, which arrives before any participant.
    SetIceServers(Value),
    /// A packet for the server, e.g. `request-media-state` right after `room-joined`.
    Send(Value),
}

pub struct Session {
    room: String,
    username: String,
    socket_id: Option<String>,
    peers: HashSet<String>,
    pending: VecDeque<Packet>,
    generation: u64,
    view: SessionView,
    unconfirmed_chat: HashMap<String, Instant>,
}

impl Session {
    pub fn new(room: String, username: String) -> Self {
        Self {
            room,
            username,
            socket_id: None,
            peers: HashSet::new(),
            pending: VecDeque::new(),
            generation: 1,
            view: SessionView::default(),
            unconfirmed_chat: HashMap::new(),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn identity(&self) -> Option<&str> {
        self.socket_id.as_deref()
    }
    pub fn join(&self) -> Value {
        json!({"type":"join-room","rid":self.room,"username":self.username,"clientVersion":3})
    }
    /// The packet to send and its `clientMessageId`; the server echoes the id back as a receipt.
    pub fn chat(&mut self, text: &str) -> Result<Value, String> {
        if text.len() > MAX_CHAT {
            return Err("chat exceeds 4 KiB".into());
        }
        let id = format!("{:016x}", rand::random::<u64>());
        self.unconfirmed_chat.insert(id.clone(), Instant::now());
        Ok(json!({"type":"send-message","rid":self.room,"msg":text,"clientMessageId":id}))
    }
    /// Drops receipts older than the deadline and reports whether any expired.
    fn chat_receipts_expired(&mut self) -> bool {
        let before = self.unconfirmed_chat.len();
        self.unconfirmed_chat
            .retain(|_, sent| sent.elapsed() < CHAT_RECEIPT);
        self.unconfirmed_chat.len() < before
    }
    pub fn description(&self, peer: &str, kind: &str, sdp: &str) -> Value {
        json!({"type":"send-description","rid":self.room,"tid":peer,"description":{"type":kind,"sdp":sdp}})
    }
    pub fn ice(
        &self,
        peer: &str,
        candidate: &str,
        mid: Option<&str>,
        line: Option<u16>,
        fragment: Option<&str>,
    ) -> Option<Value> {
        if mid.is_none() && line.is_none() {
            return None;
        }
        let ice = json!({"candidate":candidate,"sdpMid":mid,"sdpMLineIndex":line,"usernameFragment":fragment});
        Some(json!({"type":"send-ice-candidate","rid":self.room,"tid":peer,"ice":ice.to_string()}))
    }
    pub fn screen(&self, active: bool) -> Value {
        json!({"type":if active {"start-screen-share"} else {"stop-screen-share"},"rid":self.room})
    }
    pub fn invalidate(&mut self) -> Vec<Action> {
        self.generation += 1;
        self.socket_id = None;
        self.pending.clear();
        self.unconfirmed_chat.clear();
        self.view = SessionView::default();
        self.peers.drain().map(Action::RemovePeer).collect()
    }
    pub fn receive(&mut self, frame: &str, generation: u64) -> Result<Vec<Action>, String> {
        if generation != self.generation {
            return Ok(vec![]);
        }
        if frame.len() > MAX_FRAME {
            return Err("frame exceeds 256 KiB".into());
        }
        let packet: Packet = serde_json::from_str(frame).map_err(|e| e.to_string())?;
        self.handle(packet)
    }
    fn handle(&mut self, packet: Packet) -> Result<Vec<Action>, String> {
        if packet.rid != self.room {
            return Ok(vec![]);
        }
        // Peer packets need our own socket id for the polite/initiator roles; room-joined comes first,
        // so this only guards against reordering.
        if self.socket_id.is_none()
            && matches!(
                packet.kind.as_str(),
                "user-connected"
                    | "user-disconnected"
                    | "user-list"
                    | "description-received"
                    | "ice-candidate-received"
            )
        {
            if self.pending.len() == MAX_PENDING {
                return Err("identity buffer full".into());
            }
            self.pending.push_back(packet);
            return Ok(vec![]);
        }
        let mut actions = vec![];
        match packet.kind.as_str() {
            "room-joined" => {
                let Some(sid) = packet.sid else {
                    return Ok(vec![]);
                };
                if self.socket_id.is_none() {
                    self.socket_id = Some(sid.clone());
                    self.view.local_socket_id = Some(sid);
                    self.system(format!("Você entrou na sala {}", self.room));
                    actions.push(Action::Send(
                        json!({"type":"request-media-state","rid":self.room}),
                    ));
                    while let Some(early) = self.pending.pop_front() {
                        actions.extend(self.handle(early)?);
                    }
                }
            }
            "ice-servers" => actions.extend(packet.ice_servers.map(Action::SetIceServers)),
            "user-list" => {
                let local = self.socket_id.clone().unwrap();
                let users: Vec<_> = packet
                    .users
                    .into_iter()
                    .filter(|u| u.room_id == self.room)
                    .collect();
                self.view.participants = users
                    .iter()
                    .map(|u| Participant {
                        socket_id: u.socket_id.clone(),
                        username: u.username.clone(),
                        is_muted: u.is_muted,
                    })
                    .collect();
                self.view.participants.sort_by(|a, b| {
                    a.username
                        .cmp(&b.username)
                        .then(a.socket_id.cmp(&b.socket_id))
                });
                let remote: HashSet<_> = users
                    .into_iter()
                    .filter(|u| u.socket_id != local)
                    .map(|u| u.socket_id)
                    .collect();
                for peer in self.peers.difference(&remote) {
                    actions.push(Action::RemovePeer(peer.clone()));
                }
                self.peers.retain(|peer| remote.contains(peer));
                let present: HashSet<_> = self.peers.iter().cloned().collect();
                self.view.screen_sharers.retain(|id| present.contains(id) || *id == local);
                self.view.camera_sharers.retain(|id| present.contains(id) || *id == local);
                for peer in remote {
                    self.ensure(peer, false, &mut actions);
                }
            }
            "user-connected" => {
                if let Some(peer) = packet.sid {
                    if let Some(username) = packet.username {
                        if !self.view.participants.iter().any(|p| p.socket_id == peer) {
                            if self.socket_id.as_ref() != Some(&peer) {
                                self.system(format!("{username} entrou na sala"));
                            }
                            self.view.participants.push(Participant {
                                socket_id: peer.clone(),
                                username,
                                is_muted: true,
                            });
                        }
                    }
                    self.ensure(peer, true, &mut actions);
                }
            }
            "user-disconnected" => {
                if let Some(peer) = packet.sid {
                    if let Some(p) = self.view.participants.iter().find(|p| p.socket_id == peer) {
                        let name = packet.username.clone().unwrap_or_else(|| p.username.clone());
                        self.system(format!("{name} saiu da sala"));
                    }
                    self.view.participants.retain(|p| p.socket_id != peer);
                    self.view.screen_sharers.remove(&peer);
                    self.view.camera_sharers.remove(&peer);
                    if self.peers.remove(&peer) {
                        actions.push(Action::RemovePeer(peer));
                    }
                }
            }
            "room-media-state" => {
                self.view.screen_sharers = packet.screen_sharers.into_iter().collect();
                self.view.camera_sharers = packet.camera_sharers.into_iter().collect();
            }
            kind @ ("screen-share-started" | "screen-share-stopped" | "camera-share-started"
            | "camera-share-stopped") => {
                if let Some(peer) = packet.sid {
                    let who = self.display_name(&peer);
                    let sharers = if kind.starts_with("screen") {
                        &mut self.view.screen_sharers
                    } else {
                        &mut self.view.camera_sharers
                    };
                    // Sharers repeat "started" for newcomers; only a real change is announced.
                    let changed = if kind.ends_with("started") {
                        sharers.insert(peer)
                    } else {
                        sharers.remove(&peer)
                    };
                    if changed {
                        self.system(match kind {
                            "screen-share-started" => format!("{who} começou a compartilhar a tela"),
                            "screen-share-stopped" => format!("{who} parou de compartilhar a tela"),
                            "camera-share-started" => format!("{who} ativou a câmera"),
                            _ => format!("{who} desativou a câmera"),
                        });
                    }
                }
            }
            kind @ ("user-muted" | "user-unmuted") => {
                if let Some(peer) = packet.sid {
                    for participant in &mut self.view.participants {
                        if participant.socket_id == peer {
                            participant.is_muted = kind == "user-muted";
                        }
                    }
                }
            }
            "description-received" => {
                if let (Some(peer), Some(sdp)) = (packet.sid, packet.sdp) {
                    let kind = sdp.get("type").and_then(Value::as_str).unwrap_or("");
                    let text = sdp.get("sdp").and_then(Value::as_str).unwrap_or("");
                    if matches!(kind, "offer" | "answer") && !text.is_empty() {
                        self.ensure(peer.clone(), false, &mut actions);
                        actions.push(Action::RemoteDescription {
                            peer_id: peer,
                            kind: kind.into(),
                            sdp: text.into(),
                        });
                    }
                }
            }
            "ice-candidate-received" => {
                if let (Some(peer), Some(ice)) = (packet.sid, packet.ice) {
                    let value: Value = serde_json::from_str(&ice).map_err(|e| e.to_string())?;
                    let mid = value
                        .get("sdpMid")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let line = value
                        .get("sdpMLineIndex")
                        .and_then(Value::as_u64)
                        .and_then(|n| n.try_into().ok());
                    if let (Some(candidate), true) = (
                        value.get("candidate").and_then(Value::as_str),
                        mid.is_some() || line.is_some(),
                    ) {
                        self.ensure(peer.clone(), false, &mut actions);
                        actions.push(Action::RemoteIce {
                            peer_id: peer,
                            candidate: candidate.into(),
                            mid,
                            line,
                            fragment: value
                                .get("usernameFragment")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        });
                    }
                }
            }
            "chat-message" => {
                let Some(msg) = packet.msg else {
                    return Ok(vec![]);
                };
                let message: ChatMessage =
                    serde_json::from_value(msg).map_err(|e| e.to_string())?;
                if message.content.len() > MAX_CHAT {
                    return Err("received chat exceeds 4 KiB".into());
                }
                if message.sender_socket_id.is_some() && message.sender_socket_id == self.socket_id {
                    if let Some(id) = &message.client_message_id {
                        self.unconfirmed_chat.remove(id);
                    }
                }
                self.push_message(message);
            }
            _ => {}
        }
        Ok(actions)
    }
    fn push_message(&mut self, message: ChatMessage) {
        self.view.message_count += 1;
        self.view.messages.push_back(message);
        if self.view.messages.len() > 200 {
            self.view.messages.pop_front();
        }
    }
    /// Local-only notices in the chat, with the same texts as SessionHandler.kt.
    fn system(&mut self, content: String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        self.push_message(ChatMessage {
            username: "Sistema".into(),
            content,
            timestamp,
            sender_socket_id: None,
            client_message_id: None,
        });
    }
    fn display_name(&self, peer: &str) -> String {
        if self.socket_id.as_deref() == Some(peer) {
            return "Você".into();
        }
        self.view
            .participants
            .iter()
            .find(|p| p.socket_id == peer)
            .map_or_else(|| peer.get(peer.len().saturating_sub(6)..).unwrap_or(peer).to_owned(), |p| p.username.clone())
    }
    fn ensure(&mut self, peer: String, initiate: bool, actions: &mut Vec<Action>) {
        let Some(local) = &self.socket_id else {
            return;
        };
        if peer != *local && self.peers.insert(peer.clone()) {
            // Same roles as PeerConnections.kt: whoever was already in the room offers and is impolite.
            actions.push(Action::EnsurePeer {
                polite: !initiate,
                initiate,
                peer_id: peer,
            });
        }
    }
}

pub fn endpoint_allowed(endpoint: &str) -> bool {
    if endpoint.starts_with("wss://") {
        return true;
    }
    let Some(authority) = endpoint
        .strip_prefix("ws://")
        .and_then(|rest| rest.split('/').next())
    else {
        return false;
    };
    let host = authority.split(':').next().unwrap_or("");
    matches!(host, "127.0.0.1" | "localhost")
        || authority.starts_with("[::1]:")
        || authority == "[::1]"
}

fn engine_action(engine: &mut RustEngine, outgoing: &mut VecDeque<Value>, action: Action) -> Result<(), String> {
    let command = match action {
        Action::EnsurePeer {
            peer_id,
            polite,
            initiate,
        } => json!({"command":"ensurePeer","peerId":peer_id,"polite":polite,"initiate":initiate}),
        Action::RemovePeer(peer_id) => json!({"command":"removePeer","peerId":peer_id}),
        Action::SetIceServers(servers) => json!({"command":"setIceServers","iceServers":servers}),
        Action::RemoteDescription { peer_id, kind, sdp } => {
            json!({"command":"remoteDescription","peerId":peer_id,"type":kind,"sdp":sdp})
        }
        Action::RemoteIce {
            peer_id,
            candidate,
            mid,
            line,
            fragment,
        } => {
            json!({"command":"remoteIce","peerId":peer_id,"candidate":candidate,"sdpMid":mid,"sdpMLineIndex":line,"usernameFragment":fragment})
        }
        Action::Send(packet) => {
            outgoing.push_back(packet);
            return Ok(());
        }
    };
    engine.submit(command)
}

pub async fn run_controlled(
    endpoint: &str,
    room: &str,
    name: &str,
    config: &str,
    seconds: u64,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    status: Option<std::sync::mpsc::Sender<String>>,
) -> Result<(), String> {
    run_controlled_with_ui(
        endpoint, room, name, config, seconds, stop, status, None,
    )
    .await
}

pub async fn run_controlled_with_ui(
    endpoint: &str,
    room: &str,
    name: &str,
    config: &str,
    seconds: u64,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    status: Option<std::sync::mpsc::Sender<String>>,
    ui: Option<UiConnection>,
) -> Result<(), String> {
    let view = ui.as_ref().map(|ui| ui.view.clone());
    let result = tokio::select! {
        result = run_inner(endpoint, room, name, config, seconds, status, ui) => result,
        _ = async {
            loop {
                if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        } => Ok(()),
    };
    if let Some(view) = view {
        let mut view = view.lock().unwrap();
        *view = SessionView {
            generation: view.generation,
            error: result.as_ref().err().cloned(),
            ..Default::default()
        };
    }
    result
}

async fn run_inner(
    endpoint: &str,
    room: &str,
    name: &str,
    config: &str,
    seconds: u64,
    status: Option<std::sync::mpsc::Sender<String>>,
    ui: Option<UiConnection>,
) -> Result<(), String> {
    if !endpoint_allowed(endpoint) {
        return Err("endpoint requires wss or loopback ws".into());
    }
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut reconnect = 0;
    let mut ui_generation = 0;
    loop {
        if Instant::now() >= deadline {
            return Ok(());
        }
        ui_generation += 1;
        if let Some(ui) = &ui {
            *ui.view.lock().unwrap() = SessionView {
                generation: ui_generation,
                ..Default::default()
            };
            while ui.chat.try_recv().is_ok() {}
            while ui.screen.try_recv().is_ok() {}
            while ui.devices.try_recv().is_ok() {}
            while ui.audio.try_recv().is_ok() {}
            while ui.share_profile.try_recv().is_ok() {}
            if let Some(tx) = &status {
                let _ = tx.send(
                    if ui_generation == 1 {
                        "connecting"
                    } else {
                        "reconnecting"
                    }
                    .into(),
                );
            }
        }
        let mut engine = match &ui {
            Some(ui) => RustEngine::new_with_surface(config, ui.video_surface.clone())?,
            None => RustEngine::new(config)?,
        };
        let mut session = Session::new(room.to_owned(), name.to_owned());
        let handshake_timeout =
            Duration::from_secs(30).min(deadline.saturating_duration_since(Instant::now()));
        let connection = tokio::time::timeout(handshake_timeout, connect_async(endpoint))
            .await
            .map_err(|_| "websocket handshake timeout".to_owned())
            .and_then(|result| result.map_err(|error| error.to_string()));
        let (mut socket, _) = match connection {
            Ok(connection) => connection,
            Err(error) => {
                engine.close();
                if Instant::now() >= deadline {
                    return Ok(());
                }
                // Like the web client, the app retries forever; headless runs give up after 5.
                if reconnect == 5 && ui.is_none() {
                    return Err(format!("websocket connect: {error}"));
                }
                let delay = [1, 2, 4, 8, 15][reconnect.min(4)];
                reconnect += 1;
                tokio::time::sleep(
                    Duration::from_secs(delay)
                        .min(deadline.saturating_duration_since(Instant::now())),
                )
                .await;
                continue;
            }
        };
        socket
            .send(Message::Text(session.join().to_string().into()))
            .await
            .map_err(|e| e.to_string())?;
        let joined_by = Instant::now() + Duration::from_secs(15);
        let mut tick = tokio::time::interval(Duration::from_millis(20));
        let mut failed = None;
        let mut outgoing = VecDeque::new();
        let mut last_stats = Instant::now();
        let mut last_levels = Instant::now();
        let mut screen_request = None;
        let mut devices_request = None;
        let mut audio_requests = HashSet::new();
        let mut ice_retries: HashMap<String, (u8, Instant)> = HashMap::new();
        while Instant::now() < deadline {
            tokio::select! {
                incoming = socket.next() => {
                    match incoming {
                        Some(Ok(Message::Text(frame))) => {
                            let was_joined = session.identity().is_some();
                            match session.receive(&frame, session.generation()) {
                                Ok(actions) => {
                                    if !was_joined && session.identity().is_some() {
                                        reconnect = 0;
                                        if let Some(tx) = &status { let _ = tx.send(format!("joined as {name}")); }
                                    }
                                    for action in actions {
                                        engine_action(&mut engine, &mut outgoing, action)?;
                                    }
                                    if let Some(ui) = &ui {
                                        let mut view = session.view.clone();
                                        view.generation = ui_generation;
                                        let previous = ui.view.lock().unwrap();
                                        view.screen_state.clone_from(&previous.screen_state);
                                        view.screen_error.clone_from(&previous.screen_error);
                                        view.audio_inputs.clone_from(&previous.audio_inputs);
                                        view.audio_outputs.clone_from(&previous.audio_outputs);
                                        view.devices_error.clone_from(&previous.devices_error);
                                        view.devices_loaded = previous.devices_loaded;
                                        view.microphone_enabled = previous.microphone_enabled;
                                        view.microphone_device.clone_from(&previous.microphone_device);
                                        view.output_enabled = previous.output_enabled;
                                        view.output_device.clone_from(&previous.output_device);
                                        view.audio_pending = previous.audio_pending;
                                        view.audio_error.clone_from(&previous.audio_error);
                                        view.speaking.clone_from(&previous.speaking);
                                        drop(previous);
                                        *ui.view.lock().unwrap() = view;
                                    }
                                },
                                Err(error) => { failed = Some(error); break; }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => { failed = Some("websocket closed".into()); break; }
                        _ => {}
                    }
                }
                _ = tick.tick() => {
                    if session.identity().is_none() && Instant::now() >= joined_by { failed = Some("identity timeout".into()); break; }
                    if let Some(ui) = &ui {
                        while let Ok((generation, profile, height)) = ui.share_profile.try_recv() {
                            if generation == ui_generation {
                                if let Err(error) = engine.submit(json!({"command":"setShareProfile","profile":profile,"height":height})) {
                                    ui.view.lock().unwrap().screen_error = Some(error);
                                }
                            }
                        }
                        while let Ok((generation, control)) = ui.audio.try_recv() {
                            let mut view = ui.view.lock().unwrap();
                            let result = if generation != ui_generation || session.identity().is_none() {
                                Err("Sessão mudou ou ainda não está conectada.".into())
                            } else if audio_requests.len() >= 8 {
                                Err("Aguarde as operações de áudio pendentes.".into())
                            } else {
                                control.command(&view).and_then(|command| engine.submit_tracked(command))
                            };
                            match result { Ok(id) => { audio_requests.insert(id); view.audio_error = None; }, Err(error) => view.audio_error = Some(error) }
                            view.audio_pending = audio_requests.len();
                        }
                        while let Ok(generation) = ui.devices.try_recv() {
                            let result = if generation != ui_generation || session.identity().is_none() {
                                Err("Sessão mudou ou ainda não está conectada.".into())
                            } else {
                                engine.submit_tracked(json!({"command":"listAudioDevices"}))
                            };
                            match result {
                                Ok(id) => { devices_request = Some(id); ui.view.lock().unwrap().devices_error = None; },
                                Err(error) => ui.view.lock().unwrap().devices_error = Some(error),
                            }
                        }
                        while let Ok((generation, options)) = ui.screen.try_recv() {
                            let start = options.is_some();
                            let mut view = ui.view.lock().unwrap();
                            let result = if generation != ui_generation || session.identity().is_none() {
                                Err("Sessão mudou ou ainda não está conectada.".into())
                            } else if start && matches!(view.screen_state.as_str(), "pending" | "picking" | "active") {
                                Err("Compartilhamento já solicitado.".into())
                            } else {
                                match options { Some(options) => options.command(), None => Ok(json!({"command":"stopScreen"})) }.and_then(|command| engine.submit_tracked(command))
                            };
                            match result { Ok(id) => { screen_request = Some(id); view.screen_error = None; }, Err(error) => view.screen_error = Some(error) }
                            if view.screen_error.is_none() && start { view.screen_state = "pending".into(); }
                        }
                        while let Ok((generation, text)) = ui.chat.try_recv() {
                            let result = queue_ui_chat(&mut session, generation, ui_generation, &text, &mut outgoing);
                            session.view.error = result.err();
                            ui.view.lock().unwrap().error = session.view.error.clone();
                        }
                        if session.chat_receipts_expired() {
                            session.view.error = Some("O servidor não confirmou a mensagem; envie de novo.".into());
                            ui.view.lock().unwrap().error = session.view.error.clone();
                        }
                    }
                    // The speaking ring follows 4 reads a second, close to the web's per-frame analyser.
                    if ui.is_some() && session.identity().is_some() && last_levels.elapsed() >= Duration::from_millis(250) {
                        engine.submit(json!({"command":"getAudioLevels"}))?;
                        last_levels = Instant::now();
                    }
                    if session.identity().is_some() && last_stats.elapsed() >= Duration::from_secs(5) {
                        engine.submit(json!({"command":"getStats"}))?;
                        last_stats = Instant::now();
                    }
                    for (peer, (attempt, due)) in &mut ice_retries {
                        if *attempt < 3 && *due <= Instant::now() && session.peers.contains(peer) {
                            engine.submit(json!({"command":"restartIce","peerId":peer}))?;
                            *due = Instant::now() + Duration::from_secs([1, 2, 4][*attempt as usize]);
                            *attempt += 1;
                        }
                    }
                    while let Ok(event) = engine.events.try_recv() {
                        let kind = event.get("event").and_then(Value::as_str).unwrap_or("");
                        if let Some(ui) = &ui {
                            let mut view = ui.view.lock().unwrap();
                            if kind == "audioLevels" {
                                view.speaking = speaking(&event, session.identity(), view.microphone_enabled);
                            }
                            if let Some(enabled) = event.get("enabled").and_then(Value::as_bool) {
                                match kind {
                                    "microphoneState" => { view.microphone_enabled = enabled; view.microphone_device = event.get("deviceId").and_then(Value::as_str).map(str::to_owned); },
                                    "outputState" => { view.output_enabled = enabled; view.output_device = event.get("deviceId").and_then(Value::as_str).map(str::to_owned); },
                                    _ => {},
                                }
                            }
                            if matches!(kind, "completed" | "error") && event.get("requestId").and_then(Value::as_i64).is_some_and(|id| audio_requests.remove(&id)) {
                                view.audio_pending = audio_requests.len();
                                if kind == "error" { view.audio_error = Some(event.get("message").and_then(Value::as_str).unwrap_or("Falha na operação de áudio.").into()); }
                            }
                            if kind == "audioDevices" && devices_request.is_some() {
                                let parsed = view.audio_devices(&event);
                                view.devices_error = parsed.err().map(|_| "Resposta de dispositivos inválida.".into());
                            }
                            if kind == "error" && devices_request.is_some() && event.get("requestId").and_then(Value::as_i64) == devices_request {
                                view.devices_error = Some(event.get("message").and_then(Value::as_str).unwrap_or("Dispositivos indisponíveis.").into());
                            }
                            if matches!(kind, "error" | "completed") && event.get("requestId").and_then(Value::as_i64) == devices_request { devices_request = None; }
                            if kind == "screenState" {
                                view.screen_state = event.get("state").and_then(Value::as_str).unwrap_or("ended").into();
                                view.screen_error = event.get("reason").and_then(Value::as_str).map(str::to_owned);
                            }
                            if kind == "error" && event.get("requestId").and_then(Value::as_i64) == screen_request && screen_request.is_some() {
                                view.screen_error = Some(event.get("message").and_then(Value::as_str).unwrap_or("Falha na captura.").into());
                                if matches!(view.screen_state.as_str(), "pending" | "picking") { view.screen_state = "ended".into(); }
                            }
                        }
                        let peer = event.get("peerId").and_then(Value::as_str).unwrap_or("");
                        let packet = match kind {
                            "localDescription" => Some(session.description(peer, event.get("type").and_then(Value::as_str).unwrap_or(""), event.get("sdp").and_then(Value::as_str).unwrap_or(""))),
                            "localIce" => session.ice(peer, event.get("candidate").and_then(Value::as_str).unwrap_or(""), event.get("sdpMid").and_then(Value::as_str), event.get("sdpMLineIndex").and_then(Value::as_u64).and_then(|n| n.try_into().ok()), event.get("usernameFragment").and_then(Value::as_str)),
                            "screenState" if event.get("state").and_then(Value::as_str) == Some("active") => Some(session.screen(true)),
                            "screenState" if event.get("state").and_then(Value::as_str) == Some("ended") => Some(session.screen(false)),
                            "microphoneState" => event.get("enabled").and_then(Value::as_bool).map(|enabled| json!({"type":if enabled {"send-unmuted"} else {"send-muted"},"rid":room})),
                            "peerState" => {
                                match event.get("state").and_then(Value::as_str) {
                                    Some("connected") => {
                                        ice_retries.remove(peer);
                                        if session.peers.contains(peer) {
                                            engine.submit(json!({"command":"ensurePeer","peerId":peer,"polite":session.identity().is_some_and(|id| id > peer),"initiate":true}))?;
                                        }
                                    }
                                    Some("failed") => { ice_retries.insert(peer.into(), (0, Instant::now())); }
                                    Some("disconnected") => { ice_retries.entry(peer.into()).or_insert((0, Instant::now() + Duration::from_secs(5))); }
                                    _ => {}
                                }
                                None
                            },
                            "stats" => { eprintln!("Native engine: {event}"); if let Some(tx) = &status { let _ = tx.send("engine stats received".into()); } None },
                            "error" => { eprintln!("engine error: {}", event.get("code").and_then(Value::as_str).unwrap_or("unknown")); None },
                            _ => None,
                        };
                        if let Some(packet) = packet {
                            if outgoing.len() == MAX_PENDING { failed = Some("outgoing signaling queue full".into()); break; }
                            outgoing.push_back(packet);
                        }
                    }
                    if failed.is_some() { break; }
                    while let Some(packet) = outgoing.pop_front() {
                        if !matches!(tokio::time::timeout(Duration::from_secs(10), socket.send(Message::Text(packet.to_string().into()))).await, Ok(Ok(()))) {
                            failed = Some("websocket send failed".into()); break;
                        }
                    }
                    if failed.is_some() { break; }
                }
            }
        }
        let _ = socket.close(None).await;
        for action in session.invalidate() {
            engine_action(&mut engine, &mut VecDeque::new(), action)?;
        }
        engine.close();
        if let Some(ui) = &ui {
            *ui.view.lock().unwrap() = SessionView {
                generation: ui_generation,
                error: failed.clone(),
                ..Default::default()
            };
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
        if reconnect == 5 && ui.is_none() {
            return Err(failed.unwrap_or_else(|| "session ended".into()));
        }
        let delay = [1, 2, 4, 8, 15][reconnect.min(4)];
        reconnect += 1;
        tokio::time::sleep(
            Duration::from_secs(delay).min(deadline.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}

fn queue_ui_chat(
    session: &mut Session,
    generation: u64,
    current: u64,
    text: &str,
    outgoing: &mut VecDeque<Value>,
) -> Result<(), String> {
    if generation != current || session.identity().is_none() {
        return Err("A sessão mudou; envie novamente após conectar.".into());
    }
    if text.trim().is_empty() {
        return Err("Digite uma mensagem.".into());
    }
    if outgoing.len() == MAX_PENDING {
        return Err("Fila de envio cheia; tente novamente.".into());
    }
    outgoing.push_back(session.chat(text)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn session_deadline_bounds_stalled_handshake_and_retry_wait() {
        use std::sync::{Arc, atomic::AtomicBool};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (_tcp, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            run_controlled(
                &endpoint,
                "room",
                "alice",
                r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#,
                1,
                Arc::new(AtomicBool::new(false)),
                None,
            ),
        )
        .await;
        server.abort();
        assert_eq!(
            result.expect("session exceeded its deadline during handshake"),
            Ok(())
        );

        // A refused connection must not sleep past the same session deadline.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
        drop(listener);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            run_controlled(
                &endpoint,
                "room",
                "alice",
                r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#,
                1,
                Arc::new(AtomicBool::new(false)),
                None,
            ),
        )
        .await;
        assert_eq!(
            result.expect("retry wait exceeded session deadline"),
            Ok(())
        );
    }

    #[tokio::test]
    async fn successful_join_resets_reconnect_budget() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let joins = Arc::new(AtomicUsize::new(0));
        let server_stop = stop.clone();
        let server_joins = joins.clone();
        let server = tokio::spawn(async move {
            for index in 0..7 {
                let (tcp, _) = listener.accept().await.unwrap();
                let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
                let join = socket.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&join).unwrap(),
                    json!({"type":"join-room","rid":"room","username":"alice","clientVersion":3})
                );
                socket.send(Message::Text(r#"{"type":"room-joined","rid":"room","sid":"a"}"#.into())).await.unwrap();
                socket.send(Message::Text(r#"{"type":"user-list","rid":"room","users":[{"socketId":"a","username":"alice","roomId":"room"}]}"#.into())).await.unwrap();
                // Joining asks for the room media state, like Session.confirmRoomJoined.
                let request = socket.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&request).unwrap(),
                    json!({"type":"request-media-state","rid":"room"})
                );
                server_joins.fetch_add(1, Ordering::Relaxed);
                if index == 6 {
                    server_stop.store(true, Ordering::Relaxed);
                    let _ = socket.next().await;
                } else {
                    socket.close(None).await.unwrap();
                }
            }
        });
        let result = tokio::time::timeout(
            Duration::from_secs(45),
            run_controlled(
                &endpoint,
                "room",
                "alice",
                r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#,
                60,
                stop,
                None,
            ),
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap(), Ok(()));
        assert_eq!(joins.load(Ordering::Relaxed), 7);
    }

    #[test]
    fn server_ice_servers_reach_the_engine() {
        let mut session = Session::new("room".into(), "alice".into());
        session.receive(r#"{"type":"room-joined","rid":"room","sid":"a"}"#, 1).unwrap();
        // Fixture from MessageSpec.kt: STUN entries carry no username/credential.
        let servers = json!([{"urls":["stun:h:3478"]},{"urls":["turn:h:3478"],"username":"u","credential":"c"}]);
        let actions = session
            .receive(&json!({"type":"ice-servers","rid":"room","iceServers":servers}).to_string(), 1)
            .unwrap();
        assert_eq!(actions, vec![Action::SetIceServers(servers.clone())]);
        // The engine accepts what the server sends, defaults included.
        assert!(crate::parse_command(&json!({"command":"setIceServers","iceServers":servers}).to_string()).is_ok());
    }

    #[test]
    fn speaking_set_follows_levels_and_the_local_microphone() {
        let event = json!({"event":"audioLevels","peers":{"b":0.2,"c":0.01},"local":0.3});
        assert_eq!(speaking(&event, Some("a"), true), BTreeSet::from(["a".to_owned(), "b".to_owned()]));
        assert_eq!(speaking(&event, Some("a"), false), BTreeSet::from(["b".to_owned()]), "a muted mic never rings");
        assert!(speaking(&json!({"event":"audioLevels"}), None, true).is_empty());
        let mute = AudioControl::Peer { peer_id: "b".into(), muted: true }.command(&SessionView::default()).unwrap();
        assert_eq!(mute, json!({"command":"setPeerMuted","peerId":"b","muted":true}));
        assert!(crate::parse_command(&mute.to_string()).is_ok());
        assert!(crate::parse_command(r#"{"command":"getAudioLevels"}"#).is_ok());
    }
    #[test]
    fn room_events_become_system_messages_like_the_web_chat() {
        let mut s = Session::new("room".into(), "alice".into());
        for packet in [
            r#"{"type":"room-joined","rid":"room","sid":"a"}"#,
            r#"{"type":"user-connected","rid":"room","sid":"b","username":"bob"}"#,
            r#"{"type":"screen-share-started","rid":"room","sid":"b"}"#,
            r#"{"type":"screen-share-started","rid":"room","sid":"b"}"#,
            r#"{"type":"camera-share-started","rid":"room","sid":"a"}"#,
            r#"{"type":"screen-share-stopped","rid":"room","sid":"b"}"#,
            r#"{"type":"user-disconnected","rid":"room","sid":"b"}"#,
        ] {
            s.receive(packet, 1).unwrap();
        }
        let texts: Vec<_> = s.view.messages.iter().map(|m| (m.username.as_str(), m.content.as_str())).collect();
        assert_eq!(
            texts,
            [
                ("Sistema", "Você entrou na sala room"),
                ("Sistema", "bob entrou na sala"),
                ("Sistema", "bob começou a compartilhar a tela"),
                ("Sistema", "Você ativou a câmera"),
                ("Sistema", "bob parou de compartilhar a tela"),
                ("Sistema", "bob saiu da sala"),
            ]
        );
        assert!(s.view.messages.iter().all(|m| m.timestamp > 0 && m.sender_socket_id.is_none()));
    }
    #[test]
    fn ui_history_members_and_chat_queue_follow_identity_and_generation() {
        let mut s = Session::new("room".into(), "alice".into());
        let history = r#"{"type":"chat-message","rid":"room","msg":{"username":"bob","content":"olá","timestamp":1}}"#;
        s.receive(history, 1).unwrap();
        assert_eq!(s.view.messages[0].username, "bob");
        assert!(s.view.local_socket_id.is_none());
        s.receive(r#"{"type":"room-joined","rid":"room","sid":"a"}"#, 1).unwrap();
        s.receive(r#"{"type":"user-list","rid":"room","users":[{"socketId":"a","username":"alice","roomId":"room"},{"socketId":"b","username":"bob","roomId":"room"}]}"#, 1).unwrap();
        assert_eq!(s.view.participants.len(), 2);
        let mut queue = VecDeque::new();
        assert!(queue_ui_chat(&mut s, 1, 2, "old", &mut queue).is_err());
        assert!(queue.is_empty());
        queue_ui_chat(&mut s, 2, 2, "😀", &mut queue).unwrap();
        assert_eq!((queue[0]["type"].as_str(), queue[0]["msg"].as_str()), (Some("send-message"), Some("😀")));
        let receipt = queue[0]["clientMessageId"].as_str().unwrap().to_owned();
        assert_eq!(s.unconfirmed_chat.len(), 1);
        assert!(queue_ui_chat(&mut s, 2, 2, &"é".repeat(2049), &mut queue).is_err());
        let count = s.view.message_count;
        s.receive(&json!({"type":"chat-message","rid":"room","msg":{"username":"alice","content":"😀","timestamp":2,"senderSocketId":"a","clientMessageId":receipt}}).to_string(), 1).unwrap();
        assert!(s.unconfirmed_chat.is_empty(), "the echo with our clientMessageId is the receipt");
        assert!(s.view.echoed_after(count, "😀"));
        assert!(!s.view.echoed_after(s.view.message_count, "😀"));
        s.receive(r#"{"type":"user-disconnected","rid":"other","sid":"b"}"#, 1)
            .unwrap();
        assert_eq!(s.view.participants.len(), 2);
        s.receive(r#"{"type":"user-disconnected","rid":"room","sid":"b"}"#, 1)
            .unwrap();
        assert_eq!(s.view.participants.len(), 1);
        for _ in 0..205 {
            s.receive(history, 1).unwrap();
        }
        assert_eq!(s.view.messages.len(), 200);
        queue.resize(MAX_PENDING, json!({}));
        assert!(queue_ui_chat(&mut s, 2, 2, "full", &mut queue).is_err());
        s.invalidate();
        assert!(s.view.participants.is_empty() && s.view.messages.is_empty());
        assert!(queue_ui_chat(&mut s, 2, 2, "closed", &mut VecDeque::new()).is_err());
    }

    #[cfg(feature = "synthetic-test")]
    #[tokio::test]
    async fn interactive_chat_and_screen_use_existing_engine_and_cancel() {
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let join = socket.next().await.unwrap().unwrap();
            assert!(join.into_text().unwrap().contains("join-room"));
            socket.send(Message::Text(r#"{"type":"chat-message","rid":"room","msg":{"username":"bob","content":"history","timestamp":1}}"#.into())).await.unwrap();
            socket.send(Message::Text(r#"{"type":"room-joined","rid":"room","sid":"a"}"#.into())).await.unwrap();
            socket.send(Message::Text(r#"{"type":"user-list","rid":"room","users":[{"socketId":"a","username":"alice","roomId":"room"}]}"#.into())).await.unwrap();
            let request: Value = serde_json::from_str(&socket.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
            assert_eq!(request, json!({"type":"request-media-state","rid":"room"}));
            let sent: Value = serde_json::from_str(&socket.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
            assert_eq!((sent["type"].as_str(), sent["msg"].as_str()), (Some("send-message"), Some("olá 😀")));
            socket.send(Message::Text(json!({"type":"chat-message","rid":"room","msg":{"username":"alice","content":"olá 😀","timestamp":2,"senderSocketId":"a","clientMessageId":sent["clientMessageId"]}}).to_string().into())).await.unwrap();
            for expected in [
                "start-screen-share",
                "stop-screen-share",
                "send-unmuted",
                "send-muted",
            ] {
                let packet = socket.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&packet).unwrap(),
                    json!({"type":expected,"rid":"room"})
                );
            }
            let _ = socket.next().await;
        });
        let stop = Arc::new(AtomicBool::new(false));
        let view = Arc::new(Mutex::new(SessionView::default()));
        let (tx, chat) = mpsc::sync_channel(32);
        let (screen_tx, screen) = mpsc::sync_channel(8);
        let (devices_tx, devices) = mpsc::sync_channel(8);
        let (audio_tx, audio) = mpsc::sync_channel(8);
        let (_profile_tx, share_profile) = mpsc::sync_channel(8);
        let run_stop = stop.clone();
        let run_view = view.clone();
        let client = tokio::spawn(async move {
            run_controlled_with_ui(
                &endpoint,
                "room",
                "alice",
                r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#,
                60,
                run_stop,
                None,
                Some(UiConnection {
                    chat,
                    screen,
                    devices,
                    audio,
                    share_profile,
                    view: run_view,
                    video_surface: crate::VideoSurface::default(),
                }),
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while view.lock().unwrap().local_socket_id.is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let generation = view.lock().unwrap().generation;
        assert_eq!(view.lock().unwrap().messages[0].content, "history");
        tx.send((generation, "olá 😀".into())).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !view.lock().unwrap().echoed_after(1, "olá 😀") {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        screen_tx
            .send((generation + 1, Some(ScreenOptions::default())))
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while view.lock().unwrap().screen_error.is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(view.lock().unwrap().screen_state.is_empty());
        devices_tx.send(generation + 1).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while view.lock().unwrap().devices_error.is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(!view.lock().unwrap().devices_loaded);
        for (start, expected) in [(true, "active"), (false, "ended")] {
            screen_tx
                .send((generation, start.then(ScreenOptions::default)))
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while view.lock().unwrap().screen_state != expected {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert!(view.lock().unwrap().screen_error.is_none());
        }
        audio_tx
            .send((
                generation + 1,
                AudioControl::Microphone {
                    enabled: true,
                    device_id: None,
                },
            ))
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while view.lock().unwrap().audio_error.is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(!view.lock().unwrap().microphone_enabled);
        for enabled in [true, false] {
            audio_tx
                .send((
                    generation,
                    AudioControl::Microphone {
                        enabled,
                        device_id: None,
                    },
                ))
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let done = {
                        let view = view.lock().unwrap();
                        view.microphone_enabled == enabled
                            && view.audio_pending == 0
                            && view.audio_error.is_none()
                    };
                    if done {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        }
        audio_tx
            .send((
                generation,
                AudioControl::Output {
                    enabled: false,
                    device_id: None,
                },
            ))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!view.lock().unwrap().output_enabled);
        assert!(view.lock().unwrap().audio_error.is_none());
        stop.store(true, Ordering::Relaxed);
        tokio::time::timeout(Duration::from_secs(2), client)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(view.lock().unwrap().local_socket_id.is_none());
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
    }

    #[cfg(feature = "synthetic-test")]
    #[tokio::test]
    async fn ui_reconnect_discards_old_generation_and_requires_explicit_media_restart() {
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
        let wire = Arc::new(Mutex::new(Vec::<(u8, String)>::new()));
        let server_wire = wire.clone();
        let (disconnect, disconnected) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let generation = 1;
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let join: Value = serde_json::from_str(
                socket
                    .next()
                    .await
                    .unwrap()
                    .unwrap()
                    .into_text()
                    .unwrap()
                    .as_str(),
            )
            .unwrap();
            assert_eq!(join["type"], "join-room");
            socket.send(Message::Text(json!({"type":"room-joined","rid":"room","sid":format!("alice-{generation}")}).to_string().into())).await.unwrap();
            socket.send(Message::Text(json!({"type":"user-list","rid":"room","users":[{"socketId":format!("alice-{generation}"),"username":"alice","roomId":"room"}]}).to_string().into())).await.unwrap();
            let mut disconnected = Box::pin(disconnected);
            loop {
                tokio::select! {
                    _ = &mut disconnected => break,
                    frame = socket.next() => {
                        let frame: Value = serde_json::from_str(frame.unwrap().unwrap().into_text().unwrap().as_str()).unwrap();
                        if frame["type"] == "request-media-state" { continue; }
                        server_wire.lock().unwrap().push((generation, frame["type"].as_str().unwrap().into()));
                    }
                }
            }
            drop(socket);
            // The oneshot is consumed; use a separate receive loop for the final connection.
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let join: Value = serde_json::from_str(
                socket
                    .next()
                    .await
                    .unwrap()
                    .unwrap()
                    .into_text()
                    .unwrap()
                    .as_str(),
            )
            .unwrap();
            assert_eq!(join["type"], "join-room");
            socket.send(Message::Text(json!({"type":"room-joined","rid":"room","sid":"alice-2"}).to_string().into())).await.unwrap();
            socket.send(Message::Text(json!({"type":"user-list","rid":"room","users":[{"socketId":"alice-2","username":"alice","roomId":"room"}]}).to_string().into())).await.unwrap();
            while let Some(Ok(Message::Text(frame))) = socket.next().await {
                let frame: Value = serde_json::from_str(&frame).unwrap();
                if frame["type"] == "request-media-state" { continue; }
                server_wire
                    .lock()
                    .unwrap()
                    .push((2, frame["type"].as_str().unwrap().into()));
            }
        });
        let stop = Arc::new(AtomicBool::new(false));
        let view = Arc::new(Mutex::new(SessionView::default()));
        let (chat_tx, chat) = mpsc::sync_channel(8);
        let (screen_tx, screen) = mpsc::sync_channel(8);
        let (_devices_tx, devices) = mpsc::sync_channel(8);
        let (audio_tx, audio) = mpsc::sync_channel(8);
        let (_profile_tx, share_profile) = mpsc::sync_channel(8);
        let (run_stop, run_view) = (stop.clone(), view.clone());
        let client = tokio::spawn(async move {
            run_controlled_with_ui(
                &endpoint,
                "room",
                "alice",
                r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#,
                30,
                run_stop,
                None,
                Some(UiConnection {
                    chat,
                    screen,
                    devices,
                    audio,
                    share_profile,
                    view: run_view,
                    video_surface: crate::VideoSurface::default(),
                }),
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while view.lock().unwrap().local_socket_id.is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let first_generation = view.lock().unwrap().generation;
        chat_tx
            .send((first_generation, "rascunho sem confirmação".into()))
            .unwrap();
        screen_tx
            .send((first_generation, Some(ScreenOptions::default())))
            .unwrap();
        audio_tx
            .send((
                first_generation,
                AudioControl::Microphone {
                    enabled: true,
                    device_id: None,
                },
            ))
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while wire.lock().unwrap().len() < 3 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(view.lock().unwrap().screen_state, "active");
        assert!(view.lock().unwrap().microphone_enabled);
        disconnect.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let joined_again = {
                    let snapshot = view.lock().unwrap();
                    snapshot.generation > first_generation && snapshot.local_socket_id.is_some()
                };
                if joined_again {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        chat_tx
            .send((first_generation, "mensagem antiga".into()))
            .unwrap();
        screen_tx
            .send((first_generation, Some(ScreenOptions::default())))
            .unwrap();
        audio_tx
            .send((
                first_generation,
                AudioControl::Microphone {
                    enabled: true,
                    device_id: None,
                },
            ))
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let rejected = {
                    let snapshot = view.lock().unwrap();
                    snapshot.error.is_some()
                        && snapshot.screen_error.is_some()
                        && snapshot.audio_error.is_some()
                };
                if rejected {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        {
            let snapshot = view.lock().unwrap();
            assert!(snapshot.screen_state.is_empty());
            assert!(!snapshot.microphone_enabled && !snapshot.output_enabled);
            // Only the local "Você entrou na sala" notice; no chat from the old generation.
            assert!(snapshot.messages.iter().all(|m| m.username == "Sistema" && m.sender_socket_id.is_none()));
        }
        assert!(
            !wire
                .lock()
                .unwrap()
                .iter()
                .any(|(generation, _)| *generation == 2),
            "old commands or media replayed after reconnect"
        );
        let second_generation = view.lock().unwrap().generation;
        for (options, state) in [(Some(ScreenOptions::default()), "active"), (None, "ended")] {
            screen_tx.send((second_generation, options)).unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while view.lock().unwrap().screen_state != state {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        }
        assert!(!view.lock().unwrap().microphone_enabled);
        stop.store(true, Ordering::Relaxed);
        tokio::time::timeout(Duration::from_secs(2), client)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        assert!(view.lock().unwrap().local_socket_id.is_none());
    }

    #[tokio::test]
    async fn ui_cancel_closes_engine_during_stalled_handshake() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        let server = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel.store(true, Ordering::Relaxed);
            std::future::pending::<()>().await;
        });
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            run_controlled(
                &endpoint,
                "ui-smoke",
                "ui-smoke",
                r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#,
                60,
                stop,
                None,
            ),
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap(), Ok(()));
    }
    use super::*;
    #[test]
    fn screen_options_build_the_engine_command_and_audio_defaults_off() {
        assert_eq!(ScreenOptions::default().audio_mode, ScreenAudioMode::None);
        let window = ScreenOptions {
            source: ShareSource::Window { hwnd: 42, pid: 7 },
            profile: SharePreset::Game.profile(),
            height: 1440,
            audio_mode: ScreenAudioMode::Application,
        };
        let command = window.command().unwrap();
        assert_eq!(
            command,
            json!({"command":"startScreen","source":{"kind":"window","hwnd":42,"pid":7},"profile":{"fps":60,"priority":"SMOOTHNESS","maxBitrate":null},"height":1440,"audioMode":"application"})
        );
        // The engine parses exactly what the session sends.
        assert!(crate::parse_command(&command.to_string()).is_ok());
        assert!(crate::parse_command(&ScreenOptions::default().command().unwrap().to_string()).is_ok());
        assert!(ScreenOptions { audio_mode: ScreenAudioMode::Application, ..ScreenOptions::default() }.command().is_err());
        let mut fast = ScreenOptions::default();
        fast.profile.fps = 61;
        assert!(fast.command().is_err());
    }
    #[test]
    fn device_snapshot_requires_complete_engine_response_and_resets_on_disconnect() {
        let mut view = SessionView::default();
        assert!(!view.devices_loaded);
        view.audio_devices(&json!({"inputs":[{"id":"guid","label":"Entrada 😀"}],"outputs":[]}))
            .unwrap();
        let approved = view.clone();
        assert!(
            view.audio_devices(&json!({"inputs":[],"outputs":[{"label":"missing id"}]}))
                .is_err()
        );
        assert_eq!(view, approved);
        assert_eq!(view.audio_inputs[0].id, "guid");
        let mut session = Session::new("room".into(), "name".into());
        session.view = view;
        session.invalidate();
        assert!(session.view.audio_inputs.is_empty());
        assert!(!session.view.devices_loaded);
        let control = AudioControl::Microphone {
            enabled: true,
            device_id: Some("missing".into()),
        };
        assert!(control.command(&session.view).is_err());
        session.view.audio_inputs.push(AudioDevice {
            id: "missing".into(),
            label: "Input".into(),
        });
        assert!(control.command(&session.view).is_ok());
        session.view.microphone_enabled = true;
        assert!(control.command(&session.view).is_err());
        assert!(
            AudioControl::Microphone {
                enabled: false,
                device_id: Some("removed".into())
            }
            .command(&session.view)
            .is_ok()
        );
    }
    // JSON literals copied from common/src/commonTest/kotlin/screenshare/common/MessageSpec.kt:
    // if the Kotlin protocol changes a tag or field, these must change in the same PR.
    #[test]
    fn outgoing_packets_match_message_spec() {
        assert!(!endpoint_allowed("ws://localhost.evil/"));
        assert!(!endpoint_allowed("ws://10.0.0.1/"));
        assert!(endpoint_allowed("ws://127.0.0.1:8091/"));
        assert!(endpoint_allowed("wss://example.org/"));
        let spec = |text: &str| serde_json::from_str::<Value>(text).unwrap();
        let mut s = Session::new("room".into(), "same".into());
        assert_eq!(s.join(), spec(r#"{"type":"join-room","rid":"room","username":"same","clientVersion":3}"#));
        let mut chat = s.chat("Hello").unwrap();
        assert!(chat["clientMessageId"].as_str().is_some_and(|id| !id.is_empty()));
        chat["clientMessageId"] = json!("receipt");
        assert_eq!(chat, spec(r#"{"type":"send-message","rid":"room","msg":"Hello","clientMessageId":"receipt"}"#));
        assert!(s.chat(&"x".repeat(4097)).is_err());
        assert_eq!(
            s.description("b", "offer", "s"),
            json!({"type":"send-description","rid":"room","tid":"b","description":{"type":"offer","sdp":"s"}})
        );
        let ice = s.ice("b", "candidate:1", Some("0"), Some(0), Some("ufrag")).unwrap();
        assert_eq!((&ice["type"], &ice["rid"], &ice["tid"]), (&json!("send-ice-candidate"), &json!("room"), &json!("b")));
        assert_eq!(spec(ice["ice"].as_str().unwrap())["usernameFragment"], "ufrag");
        assert!(s.ice("b", "", None, None, None).is_none());
        assert_eq!(s.screen(true), json!({"type":"start-screen-share","rid":"room"}));
        assert_eq!(s.screen(false), json!({"type":"stop-screen-share","rid":"room"}));
    }

    #[test]
    fn identity_comes_from_room_joined_and_drives_peer_roles() {
        let mut s = Session::new("room".into(), "alice".into());
        // Peer packets wait for our identity; room-joined releases them in order.
        assert!(s.receive(r#"{"type":"description-received","rid":"room","sid":"b","sdp":{"type":"offer","sdp":"x"}}"#, 1).unwrap().is_empty());
        let actions = s.receive(r#"{"type":"room-joined","rid":"room","sid":"a"}"#, 1).unwrap();
        assert_eq!(s.identity(), Some("a"));
        assert_eq!(
            actions,
            vec![
                Action::Send(json!({"type":"request-media-state","rid":"room"})),
                Action::EnsurePeer { peer_id: "b".into(), polite: true, initiate: false },
                Action::RemoteDescription { peer_id: "b".into(), kind: "offer".into(), sdp: "x".into() },
            ]
        );
        // Two users with our name are fine now: identity is the socket id, not the name.
        let list = r#"{"type":"user-list","rid":"room","users":[{"socketId":"a","username":"alice","roomId":"room"},{"socketId":"b","username":"bob","roomId":"room","isMuted":false},{"socketId":"c","username":"alice","roomId":"room"}]}"#;
        assert_eq!(
            s.receive(list, 1).unwrap(),
            vec![Action::EnsurePeer { peer_id: "c".into(), polite: true, initiate: false }]
        );
        assert_eq!(s.view.participants.len(), 3);
        assert!(!s.view.participants.iter().find(|p| p.socket_id == "b").unwrap().is_muted);
        // Whoever was already in the room offers to newcomers (SessionHandler.handleUserConnected).
        assert_eq!(
            s.receive(r#"{"type":"user-connected","rid":"room","sid":"d","username":"dan"}"#, 1).unwrap(),
            vec![Action::EnsurePeer { peer_id: "d".into(), polite: false, initiate: true }]
        );
        let generation = s.generation();
        assert_eq!(s.invalidate().len(), 3);
        assert!(s.receive(r#"{"type":"user-connected","rid":"room","sid":"e","username":"eve"}"#, generation).unwrap().is_empty());
    }

    #[test]
    fn media_state_and_mute_follow_server_packets() {
        let mut s = Session::new("testRoomId".into(), "alice".into());
        s.receive(r#"{"type":"room-joined","rid":"testRoomId","sid":"a"}"#, 1).unwrap();
        s.receive(r#"{"type":"user-list","rid":"testRoomId","users":[{"socketId":"a","username":"alice","roomId":"testRoomId"},{"socketId":"screen","username":"s","roomId":"testRoomId"},{"socketId":"camera","username":"c","roomId":"testRoomId"}]}"#, 1).unwrap();
        s.receive(r#"{"type":"room-media-state","rid":"testRoomId","screenSharers":["screen"],"cameraSharers":["camera"]}"#, 1).unwrap();
        assert!(s.view.screen_sharers.contains("screen") && s.view.camera_sharers.contains("camera"));
        s.receive(r#"{"type":"screen-share-stopped","rid":"testRoomId","sid":"screen"}"#, 1).unwrap();
        s.receive(r#"{"type":"camera-share-started","rid":"testRoomId","sid":"screen"}"#, 1).unwrap();
        assert!(s.view.screen_sharers.is_empty());
        assert!(s.view.camera_sharers.contains("screen"));
        s.receive(r#"{"type":"user-unmuted","rid":"testRoomId","sid":"camera"}"#, 1).unwrap();
        assert!(!s.view.participants.iter().find(|p| p.socket_id == "camera").unwrap().is_muted);
        s.receive(r#"{"type":"user-disconnected","rid":"testRoomId","sid":"camera","username":"c"}"#, 1).unwrap();
        assert!(!s.view.camera_sharers.contains("camera"));
    }

    #[test]
    fn chat_receipts_confirm_only_our_echo_and_expire() {
        let mut s = Session::new("room".into(), "same".into());
        s.receive(r#"{"type":"room-joined","rid":"room","sid":"socket"}"#, 1).unwrap();
        s.chat("Hello").unwrap();
        let id = s.unconfirmed_chat.keys().next().unwrap().clone();
        // Someone else's message with the same id is not our receipt.
        s.receive(&json!({"type":"chat-message","rid":"room","msg":{"username":"same","content":"Hello","timestamp":123,"senderSocketId":"other","clientMessageId":id}}).to_string(), 1).unwrap();
        assert_eq!(s.unconfirmed_chat.len(), 1);
        s.receive(&json!({"type":"chat-message","rid":"room","msg":{"username":"same","content":"Hello","timestamp":123,"senderSocketId":"socket","clientMessageId":id}}).to_string(), 1).unwrap();
        assert!(s.unconfirmed_chat.is_empty());
        assert_eq!(s.view.messages.back().unwrap().sender_socket_id.as_deref(), Some("socket"));
        assert!(!s.chat_receipts_expired());
        s.unconfirmed_chat.insert("late".into(), Instant::now() - CHAT_RECEIPT);
        assert!(s.chat_receipts_expired());
        assert!(s.unconfirmed_chat.is_empty());
    }

    #[test]
    fn rejects_overflowed_identity_wrong_room_and_large_frame() {
        let mut overflow = Session::new("room".into(), "alice".into());
        let early = r#"{"type":"user-connected","rid":"room","sid":"b","username":"b"}"#;
        for _ in 0..MAX_PENDING {
            assert!(overflow.receive(early, 1).unwrap().is_empty());
        }
        assert_eq!(overflow.receive(early, 1).unwrap_err(), "identity buffer full");
        let mut s = Session::new("room".into(), "alice".into());
        s.receive(r#"{"type":"room-joined","rid":"room","sid":"b"}"#, 1).unwrap();
        s.receive(r#"{"type":"user-list","rid":"room","users":[{"socketId":"b","username":"alice","roomId":"room"},{"socketId":"a","username":"bob","roomId":"room"}]}"#, 1).unwrap();
        assert!(s.receive(r#"{"type":"user-disconnected","rid":"other","sid":"a"}"#, 1).unwrap().is_empty());
        assert_eq!(
            s.receive(r#"{"type":"user-disconnected","rid":"room","sid":"a"}"#, 1).unwrap(),
            vec![Action::RemovePeer("a".into())]
        );
        assert!(s.receive(&"x".repeat(MAX_FRAME + 1), 1).is_err());
    }
}
