//! JNI bridge for `com.rafaelrain.screenshare.engine.Engine`: the desktop's session and engine
//! (desktop-native) run as they do under the iced UI, and Kotlin only sends JSON, reads the
//! SessionView as JSON, pushes captured frames and takes decoded ones.
//! No Rust panic crosses into the JVM: every export runs in `guard`.
use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Sender, channel},
    },
    thread::JoinHandle,
};

use desktop_native::{
    LOCAL_CAMERA, LOCAL_SCREEN, ScreenAudioMode, ShareSource, VideoFrameSnapshot, VideoSurface,
    session::{AudioControl, ScreenOptions, SessionView, UiConnection, run_controlled_with_ui},
    share::ShareProfile,
};
use jni::{
    JNIEnv, JavaVM,
    objects::{JByteBuffer, JClass, JObject, JString},
    sys::{jboolean, jint, jlong, jstring},
};
use serde_json::Value;

struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    chat: Sender<(u64, String)>,
    screen: Sender<(u64, Option<ScreenOptions>)>,
    camera: Sender<(u64, Option<Option<String>>)>,
    devices: Sender<u64>,
    audio: Sender<(u64, AudioControl)>,
    share_profile: Sender<(u64, ShareProfile, u32)>,
    view: Arc<Mutex<SessionView>>,
    surface: VideoSurface,
    last_view: String,
    /// Newest frame per tile (socket id, camera), and whether Kotlin already took it.
    tiles: HashMap<(String, bool), (Arc<VideoFrameSnapshot>, bool)>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);
static SENTRY: Mutex<Option<sentry::ClientInitGuard>> = Mutex::new(None);

fn running() -> std::sync::MutexGuard<'static, Option<Running>> {
    RUNNING.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

fn string(env: &mut JNIEnv, value: &JString) -> Option<String> {
    if value.is_null() {
        return None;
    }
    env.get_string(value).ok().map(Into::into)
}

fn jstr(env: &mut JNIEnv, value: Option<String>) -> jstring {
    value.and_then(|value| env.new_string(value).ok()).map_or(std::ptr::null_mut(), |s| s.into_raw())
}

/// A direct ByteBuffer as a slice; None for a heap buffer or a null one.
fn bytes<'a>(env: &JNIEnv, buffer: &JByteBuffer) -> Option<&'a mut [u8]> {
    if buffer.is_null() {
        return None;
    }
    let address = env.get_direct_buffer_address(buffer).ok()?;
    let capacity = env.get_direct_buffer_capacity(buffer).ok()?;
    // SAFETY: a direct buffer's memory stays valid while Kotlin holds it, which spans this call.
    (!address.is_null()).then(|| unsafe { std::slice::from_raw_parts_mut(address, capacity) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_init(mut env: JNIEnv, _class: JClass, context: JObject, dsn: JString, release: JString) -> jboolean {
    let dsn = string(&mut env, &dsn).filter(|dsn| !dsn.is_empty());
    let release = string(&mut env, &release);
    guard(0, || {
        let Ok(vm) = env.get_java_vm() else { return 0 };
        let ok = libwebrtc::android::initialize_android_context(&vm, &context);
        // Rust errors and the metrics' logs; Kotlin and native crashes go through sentry-android.
        if let Some(dsn) = dsn {
            let mut sentry = SENTRY.lock().unwrap_or_else(|poison| poison.into_inner());
            if sentry.is_none() {
                let mut options = sentry::ClientOptions::default();
                options.release = release.map(Into::into);
                *sentry = Some(sentry::init((dsn, options)));
                sentry::configure_scope(|scope| scope.set_tag("platform", "native-android"));
            }
        }
        jboolean::from(ok)
    })
}

/// Starts the session; the error text, or null when it started.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_start(mut env: JNIEnv, _class: JClass, endpoint: JString, room: JString, name: JString) -> jstring {
    let (Some(endpoint), Some(room), Some(name)) = (string(&mut env, &endpoint), string(&mut env, &room), string(&mut env, &name)) else {
        return jstr(&mut env, Some("argumentos inválidos".into()));
    };
    let error = guard(Some("falha interna".into()), || start(endpoint, room, name).err());
    jstr(&mut env, error)
}

fn start(endpoint: String, room: String, name: String) -> Result<(), String> {
    if !desktop_native::session::endpoint_allowed(&endpoint) {
        return Err("servidor precisa ser wss://".into());
    }
    stop();
    let stop = Arc::new(AtomicBool::new(false));
    let view = Arc::new(Mutex::new(SessionView::default()));
    let surface = VideoSurface::default();
    let (chat, chat_rx) = channel();
    let (screen, screen_rx) = channel();
    let (camera, camera_rx) = channel();
    let (devices, devices_rx) = channel();
    let (audio, audio_rx) = channel();
    let (share_profile, share_profile_rx) = channel();
    let ui = UiConnection {
        chat: chat_rx,
        screen: screen_rx,
        camera: camera_rx,
        devices: devices_rx,
        audio: audio_rx,
        share_profile: share_profile_rx,
        view: view.clone(),
        video_surface: surface.clone(),
    };
    let config = r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#;
    let thread_stop = stop.clone();
    let thread = std::thread::Builder::new()
        .name("session".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Runtime::new() else { return };
            let result = runtime.block_on(run_controlled_with_ui(&endpoint, &room, &name, config, 24 * 3600, thread_stop, None, Some(ui)));
            if let Err(error) = result {
                sentry::capture_message(&format!("session ended: {error}"), sentry::Level::Error);
            }
        })
        .map_err(|error| error.to_string())?;
    *running() = Some(Running {
        stop,
        thread: Some(thread),
        chat,
        screen,
        camera,
        devices,
        audio,
        share_profile,
        view,
        surface,
        last_view: String::new(),
        tiles: HashMap::new(),
    });
    Ok(())
}

fn stop() {
    let taken = running().take();
    if let Some(mut session) = taken {
        session.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = session.thread.take() {
            let _ = thread.join();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_stop(_env: JNIEnv, _class: JClass) {
    guard((), stop);
}

/// One UI request as JSON (see `request`); the error text, or null when it was queued.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_send(mut env: JNIEnv, _class: JClass, json: JString) -> jstring {
    let json = string(&mut env, &json);
    let error = guard(Some("falha interna".into()), || {
        let json = json?;
        let value: Value = match serde_json::from_str(&json) {
            Ok(value) if json.len() <= 64 * 1024 => value,
            _ => return Some("pedido inválido".into()),
        };
        request(&value).err()
    });
    jstr(&mut env, error)
}

fn closed<T>(_: T) -> String {
    "sessão encerrada".into()
}

fn field<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(value[key].clone()).map_err(|_| format!("campo inválido: {key}"))
}

/// The same channels the desktop UI drives, tagged with the current connection's generation.
fn request(value: &Value) -> Result<(), String> {
    if let Some(on) = value["metrics"].as_bool() {
        desktop_native::metrics::set_enabled(on);
        return Ok(());
    }
    let guard = running();
    let session = guard.as_ref().ok_or("fora da sala")?;
    let generation = session.view.lock().unwrap_or_else(|poison| poison.into_inner()).generation;
    match value["type"].as_str().unwrap_or_default() {
        "chat" => session.chat.send((generation, field(value, "text")?)).map_err(closed),
        "startScreen" => {
            let options = ScreenOptions {
                source: ShareSource::Screen { left: 0, top: 0, width: field(value, "width")?, height: field(value, "height")? },
                profile: field(value, "profile")?,
                height: field(value, "shareHeight")?,
                audio_mode: if field(value, "audio")? { ScreenAudioMode::Computer } else { ScreenAudioMode::None },
            };
            session.screen.send((generation, Some(options))).map_err(closed)
        }
        "stopScreen" => session.screen.send((generation, None)).map_err(closed),
        "startCamera" => session.camera.send((generation, Some(field(value, "device")?))).map_err(closed),
        "stopCamera" => session.camera.send((generation, None)).map_err(closed),
        "devices" => session.devices.send(generation).map_err(closed),
        "shareProfile" => session.share_profile.send((generation, field(value, "profile")?, field(value, "shareHeight")?)).map_err(closed),
        "microphone" => session.audio.send((generation, AudioControl::Microphone { enabled: field(value, "enabled")?, device_id: None })).map_err(closed),
        "output" => session.audio.send((generation, AudioControl::Output { enabled: field(value, "enabled")?, device_id: None })).map_err(closed),
        "peerMuted" => session.audio.send((generation, AudioControl::Peer { peer_id: field(value, "peerId")?, muted: field(value, "muted")? })).map_err(closed),
        "peerVolume" => {
            let volume: f64 = field(value, "volume")?;
            if !(0.0..=1.0).contains(&volume) {
                return Err("volume inválido".into());
            }
            session.audio.send((generation, AudioControl::Volume { peer_id: field(value, "peerId")?, volume, screen: field(value, "screen")? })).map_err(closed)
        }
        _ => Err("pedido desconhecido".into()),
    }
}

/// The SessionView as JSON when it changed since the last call, else null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_view(mut env: JNIEnv, _class: JClass) -> jstring {
    let json = guard(None, || {
        let mut guard = running();
        let session = guard.as_mut()?;
        let json = serde_json::to_string(&*session.view.lock().unwrap_or_else(|poison| poison.into_inner())).ok()?;
        (json != session.last_view).then(|| {
            session.last_view = json.clone();
            json
        })
    });
    jstr(&mut env, json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_pushScreenFrame(env: JNIEnv, _class: JClass, buffer: JByteBuffer, width: jint, height: jint, row_stride: jint, timestamp_ns: jlong) -> jboolean {
    guard(0, || {
        let Some(data) = bytes(&env, &buffer) else { return 0 };
        let (Ok(width), Ok(height), Ok(stride)) = (u32::try_from(width), u32::try_from(height), u32::try_from(row_stride)) else { return 0 };
        jboolean::from(desktop_native::push_screen_rgba(data, width, height, stride, timestamp_ns / 1000).unwrap_or(false))
    })
}

/// 10 ms of 48 kHz stereo 16-bit PCM, native byte order (1920 bytes).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_pushScreenAudio(env: JNIEnv, _class: JClass, buffer: JByteBuffer) -> jboolean {
    guard(0, || {
        let Some(data) = bytes(&env, &buffer) else { return 0 };
        let Some(data) = data.get(..1920) else { return 0 };
        let samples: Vec<i16> = data.chunks_exact(2).map(|pair| i16::from_ne_bytes([pair[0], pair[1]])).collect();
        jboolean::from(desktop_native::push_screen_audio(&samples).unwrap_or(false))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_pushCameraFrame(
    env: JNIEnv,
    _class: JClass,
    y: JByteBuffer,
    u: JByteBuffer,
    v: JByteBuffer,
    y_stride: jint,
    uv_stride: jint,
    uv_pixel_stride: jint,
    width: jint,
    height: jint,
    rotation: jint,
    timestamp_ns: jlong,
) -> jboolean {
    guard(0, || {
        let (Some(y), Some(u), Some(v)) = (bytes(&env, &y), bytes(&env, &u), bytes(&env, &v)) else { return 0 };
        let size = |value: jint| u32::try_from(value).unwrap_or(0);
        jboolean::from(
            desktop_native::push_camera_yuv((y, u, v), (size(y_stride), size(uv_stride), size(uv_pixel_stride)), size(width), size(height), rotation, timestamp_ns / 1000)
                .unwrap_or(false),
        )
    })
}

/// Copies the newest frame of a tile (socket id, camera) as packed I420 into `buffer`.
/// Returns width << 32 | height, 0 when there is no newer frame, or -(bytes needed) when `buffer` is too small.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rafaelrain_screenshare_engine_Engine_takeFrame(mut env: JNIEnv, _class: JClass, peer: JString, camera: jboolean, buffer: JByteBuffer) -> jlong {
    let Some(peer) = string(&mut env, &peer) else { return 0 };
    guard(0, || {
        let mut guard = running();
        let Some(session) = guard.as_mut() else { return 0 };
        collect_frames(session);
        let Some((frame, taken)) = session.tiles.get_mut(&(peer, camera != 0)) else { return 0 };
        if *taken {
            return 0;
        }
        let needed = frame.y.len() + frame.u.len() + frame.v.len();
        let Some(out) = bytes(&env, &buffer) else { return 0 };
        if out.len() < needed {
            return -(needed as jlong);
        }
        out[..frame.y.len()].copy_from_slice(&frame.y);
        out[frame.y.len()..][..frame.u.len()].copy_from_slice(&frame.u);
        out[frame.y.len() + frame.u.len()..][..frame.v.len()].copy_from_slice(&frame.v);
        *taken = true;
        (jlong::from(frame.width) << 32) | jlong::from(frame.height)
    })
}

/// main.rs's tick: our own capture under our socket id, a peer's track as its screen or camera.
fn collect_frames(session: &mut Running) {
    let view = session.view.lock().unwrap_or_else(|poison| poison.into_inner()).clone();
    let mut surface = session.surface.lock().unwrap_or_else(|poison| poison.into_inner());
    for ((peer, track), snapshot) in std::mem::take(&mut surface.latest) {
        let key = match peer.as_str() {
            LOCAL_SCREEN => view.local_socket_id.clone().map(|id| (id, false)),
            LOCAL_CAMERA => view.local_socket_id.clone().map(|id| (id, true)),
            _ => surface.kind(&peer, &track, view.screen_sharers.contains(&peer), view.camera_sharers.contains(&peer)).map(|camera| (peer, camera)),
        };
        if let Some(key) = key {
            session.tiles.insert(key, (snapshot, false));
        }
    }
    session.tiles.retain(|(peer, _), _| view.participants.iter().any(|p| &p.socket_id == peer));
}

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut std::ffi::c_void) -> jint {
    guard((), || libwebrtc::android::initialize_android(&vm));
    jni::sys::JNI_VERSION_1_6
}
