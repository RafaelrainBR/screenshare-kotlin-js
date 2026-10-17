// Screenshare native desktop: iced UI over the session and libwebrtc engine.
mod ui;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Sender, channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use desktop_native::{
    ScreenAudioMode, VideoSurface,
    session::{AudioControl, ScreenOptions, SessionView, UiConnection, run_controlled_with_ui},
    share::SharePreset,
};
use iced::{
    Element, Length, Subscription, Task, keyboard,
    widget::{button, column, container, image, operation, text, text_input},
    window,
};
use ui::{room, theme, wizard::Wizard};

#[derive(Debug, Clone)]
pub enum Message {
    Room(String),
    Name(String),
    Join,
    Leave,
    Draft(String),
    Send,
    Watch(String, bool),
    ToggleMic,
    ToggleOutput,
    MicDevice(Option<String>),
    OutputDevice(Option<String>),
    OpenWizard,
    CloseWizard,
    Sources(Vec<desktop_capture::sources::Source>),
    Thumbnail(String, Option<image::Handle>),
    Pick(String),
    AudioMode(ScreenAudioMode),
    Preset(SharePreset),
    StartShare,
    StopShare,
    Key(keyboard::Key, keyboard::Modifiers),
    Tick,
    Allocated(u64, Option<image::Allocation>),
    CloseRequested(window::Id),
}

struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    chat: Sender<(u64, String)>,
    screen: Sender<(u64, Option<ScreenOptions>)>,
    devices: Sender<u64>,
    audio: Sender<(u64, AudioControl)>,
    video: Sender<(u64, Option<String>, bool)>,
    view: Arc<Mutex<SessionView>>,
    surface: VideoSurface,
}

impl Running {
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Default)]
pub struct App {
    room: String,
    name: String,
    draft: String,
    running: Option<Running>,
    view: SessionView,
    wizard: Option<Wizard>,
    frame: Option<image::Allocation>,
    sent: u64,
    shown: u64,
    frame_size: (u32, u32),
    frames: u64,
    fps: f64,
    fps_window: Option<(Instant, u64)>,
    watching_camera: bool,
    /// Last screen sharer shown automatically, so a manual choice is not overridden every tick.
    auto_watched: Option<String>,
    mic_device: Option<String>,
    output_device: Option<String>,
    /// A device picked while that audio was on: turn it off, then back on with the new device.
    mic_restart: bool,
    output_restart: bool,
    devices_requested: bool,
    notice: Option<String>,
}

const NAME_INPUT: &str = "name-input";

fn boot() -> (App, Task<Message>) {
    (App { name: whoami(), ..Default::default() }, operation::focus(NAME_INPUT))
}

fn whoami() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "Nativo".into())
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Room(value) => app.room = value,
        Message::Name(value) => app.name = value,
        Message::Join if app.running.is_none() && !app.name.trim().is_empty() => {
            // Empty code creates a room, like the web join form.
            app.room = match app.room.trim() {
                "" => format!("{:08x}", rand::random::<u32>()),
                code => code.into(),
            };
            app.running = Some(start(app.room.clone(), app.name.trim().into()));
            return operation::focus(room::CHAT_INPUT);
        }
        Message::Join => {}
        Message::Leave => {
            // Dropping Running joins the session thread: the engine stops the share and closes peers.
            let App { room, name, mic_device, output_device, .. } = std::mem::take(app);
            *app = App { room, name, mic_device, output_device, ..Default::default() };
        }
        Message::Draft(value) => app.draft = value,
        Message::Send => {
            if let Some(running) = &app.running {
                if !app.draft.trim().is_empty() {
                    let _ = running.chat.send((app.view.generation, std::mem::take(&mut app.draft)));
                }
            }
        }
        Message::Watch(peer, camera) => watch(app, Some(peer), camera),
        Message::ToggleMic => audio(app, AudioControl::Microphone { enabled: !app.view.microphone_enabled, device_id: app.mic_device.clone() }),
        Message::ToggleOutput => audio(app, AudioControl::Output { enabled: !app.view.output_enabled, device_id: app.output_device.clone() }),
        Message::MicDevice(id) => {
            app.mic_device = id;
            if app.view.microphone_enabled {
                app.mic_restart = true;
                audio(app, AudioControl::Microphone { enabled: false, device_id: None });
            }
        }
        Message::OutputDevice(id) => {
            app.output_device = id;
            if app.view.output_enabled {
                app.output_restart = true;
                audio(app, AudioControl::Output { enabled: false, device_id: None });
            }
        }
        Message::OpenWizard => {
            app.wizard = Some(Wizard::default());
            return ui::wizard::load_sources();
        }
        Message::CloseWizard => app.wizard = None,
        Message::Sources(sources) => {
            if let Some(wizard) = &mut app.wizard {
                let task = ui::wizard::load_thumbnails(&sources);
                wizard.sources = Some(sources);
                return task;
            }
        }
        Message::Thumbnail(id, handle) => {
            if let (Some(wizard), Some(handle)) = (&mut app.wizard, handle) {
                wizard.thumbs.insert(id, handle);
            }
        }
        Message::Pick(id) => {
            if let Some(wizard) = &mut app.wizard {
                wizard.select(id);
            }
        }
        Message::AudioMode(mode) => {
            if let Some(wizard) = &mut app.wizard {
                wizard.audio = mode;
            }
        }
        Message::Preset(preset) => {
            if let Some(wizard) = &mut app.wizard {
                wizard.preset = Some(preset);
            }
        }
        Message::StartShare => {
            if let (Some(running), Some(options)) = (&app.running, app.wizard.as_ref().and_then(Wizard::options)) {
                let _ = running.screen.send((app.view.generation, Some(options)));
                app.wizard = None;
            }
        }
        Message::StopShare => {
            if let Some(running) = &app.running {
                let _ = running.screen.send((app.view.generation, None));
            }
        }
        Message::Key(key, modifiers) => return shortcut(app, key, modifiers),
        Message::Tick => return tick(app),
        Message::Allocated(seq, allocation) => {
            if seq > app.shown && allocation.is_some() {
                app.shown = seq;
                app.frame = allocation;
                app.frames += 1;
            }
        }
        Message::CloseRequested(id) => {
            app.running = None;
            return window::close(id).chain(iced::exit());
        }
    }
    Task::none()
}

/// Keys no focused widget took: focus moves, dock shortcuts and the wizard without a mouse.
fn shortcut(app: &mut App, key: keyboard::Key, modifiers: keyboard::Modifiers) -> Task<Message> {
    use keyboard::{Key, key::Named};
    let message = match (key.as_ref(), &mut app.wizard) {
        (Key::Named(Named::Tab), _) if modifiers.shift() => return operation::focus_previous(),
        (Key::Named(Named::Tab), _) => return operation::focus_next(),
        (_, _) if app.running.is_none() => return Task::none(),
        (Key::Named(Named::Escape), Some(_)) => Message::CloseWizard,
        (Key::Named(Named::ArrowRight | Named::ArrowDown), Some(wizard)) => {
            wizard.step(1);
            return Task::none();
        }
        (Key::Named(Named::ArrowLeft | Named::ArrowUp), Some(wizard)) => {
            wizard.step(-1);
            return Task::none();
        }
        (Key::Character("a"), Some(wizard)) => {
            wizard.cycle_audio();
            return Task::none();
        }
        (Key::Character("p"), Some(wizard)) => {
            wizard.cycle_preset();
            return Task::none();
        }
        (Key::Named(Named::Enter), Some(_)) => Message::StartShare,
        (Key::Named(Named::F2), _) => Message::ToggleMic,
        (Key::Named(Named::F3), _) => Message::ToggleOutput,
        (Key::Named(Named::F4), None) if matches!(app.view.screen_state.as_str(), "active" | "pending" | "picking") => Message::StopShare,
        (Key::Named(Named::F4), None) => Message::OpenWizard,
        _ => return Task::none(),
    };
    update(app, message)
}

fn audio(app: &mut App, control: AudioControl) {
    if let Some(running) = &app.running {
        let _ = running.audio.send((app.view.generation, control));
    }
}

fn watch(app: &mut App, peer: Option<String>, camera: bool) {
    if let Some(running) = &app.running {
        app.watching_camera = camera;
        app.frame = None;
        let _ = running.video.send((app.view.generation, peer, camera));
    }
}

fn tick(app: &mut App) -> Task<Message> {
    let Some(running) = &app.running else { return Task::none() };
    app.view = running.view.lock().unwrap().clone();
    if app.view.local_socket_id.is_some() && !app.devices_requested {
        app.devices_requested = true;
        let _ = running.devices.send(app.view.generation);
    }
    if app.view.audio_pending == 0 {
        if app.mic_restart && !app.view.microphone_enabled {
            app.mic_restart = false;
            audio(app, AudioControl::Microphone { enabled: true, device_id: app.mic_device.clone() });
        }
        if app.output_restart && !app.view.output_enabled {
            app.output_restart = false;
            audio(app, AudioControl::Output { enabled: true, device_id: app.output_device.clone() });
        }
    }
    auto_watch(app);
    if app.view.video_peer.is_none() {
        app.frame = None;
    }
    let Some(running) = &app.running else { return Task::none() };
    // Take ownership of the newest frame so its pixels move into the image handle without a copy.
    // iced uploads images asynchronously: swap the shown frame once its GPU allocation is done,
    // letting uploads overlap and dropping any that finish after a newer one.
    let snapshot = running.surface.lock().unwrap().take();
    fps_update(app);
    let Some(snapshot) = snapshot else { return Task::none() };
    let snapshot = Arc::try_unwrap(snapshot).unwrap_or_else(|shared| (*shared).clone());
    app.frame_size = (snapshot.width, snapshot.height);
    app.sent += 1;
    let seq = app.sent;
    image::allocate(image::Handle::from_rgba(snapshot.width, snapshot.height, snapshot.rgba)).map(move |r| Message::Allocated(seq, r.ok()))
}

/// Like the web client: show a remote screen as soon as someone shares, and fall back when it ends.
fn auto_watch(app: &mut App) {
    let local = app.view.local_socket_id.clone();
    let watched_gone = app.view.video_peer.as_ref().is_some_and(|peer| {
        if app.watching_camera { !app.view.camera_sharers.contains(peer) } else { !app.view.screen_sharers.contains(peer) }
    });
    if watched_gone {
        watch(app, None, false);
        app.auto_watched = None;
    }
    if app.view.video_peer.is_some() || app.view.video_pending > 0 {
        return;
    }
    let sharer = app.view.screen_sharers.iter().find(|id| Some(*id) != local.as_ref()).cloned();
    if sharer.is_some() && sharer != app.auto_watched {
        app.auto_watched = sharer.clone();
        watch(app, sharer, false);
    }
}

fn fps_update(app: &mut App) {
    let (since, frames) = *app.fps_window.get_or_insert((Instant::now(), app.frames));
    let elapsed = since.elapsed();
    if elapsed >= Duration::from_secs(1) {
        app.fps = (app.frames - frames) as f64 / elapsed.as_secs_f64();
        app.fps_window = Some((Instant::now(), app.frames));
    }
}

fn start(room: String, name: String) -> Running {
    let endpoint = std::env::var("SCREENSHARE_WS").unwrap_or_else(|_| "wss://screen-share.fly.dev/".into());
    // STUN/TURN come from the server (ice-servers); SCREENSHARE_ENGINE_CONFIG, a local JSON file,
    // only matters for servers that do not send them.
    let config = std::env::var("SCREENSHARE_ENGINE_CONFIG")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#.into());
    let stop = Arc::new(AtomicBool::new(false));
    let view = Arc::new(Mutex::new(SessionView::default()));
    let surface = VideoSurface::default();
    let (chat, chat_rx) = channel();
    let (video, video_rx) = channel();
    let (screen, screen_rx) = channel();
    let (devices, devices_rx) = channel();
    let (audio, audio_rx) = channel();
    let ui = UiConnection {
        chat: chat_rx,
        screen: screen_rx,
        devices: devices_rx,
        audio: audio_rx,
        video: video_rx,
        view: view.clone(),
        video_surface: surface.clone(),
    };
    let thread_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = runtime.block_on(run_controlled_with_ui(
            &endpoint, &room, &name, &config, 24 * 3600, thread_stop, None, Some(ui),
        ));
        eprintln!("session ended: {result:?}");
    });
    Running { stop, thread: Some(thread), chat, screen, devices, audio, video, view, surface }
}

fn view(app: &App) -> Element<'_, Message> {
    if app.running.is_some() {
        return room::view(app);
    }
    let join = button(text("Entrar na sala").font(theme::BOLD)).padding([8, 16]).style(button::primary);
    container(
        column![
            text("Participar de uma sala").size(26).font(theme::BOLD),
            text("Seu nome").color(theme::DIM),
            text_input("Como você aparece na sala", &app.name).id(NAME_INPUT).on_input(Message::Name).on_submit(Message::Join).padding(8),
            text("Código da sala").color(theme::DIM),
            text("Use um código recebido ou deixe vazio para criar uma sala.").size(12).color(theme::FAINT),
            text_input("ex: 4f8a2c1e", &app.room).on_input(Message::Room).on_submit(Message::Join).padding(8),
            if app.name.trim().is_empty() { join } else { join.on_press(Message::Join) },
        ]
        .spacing(10)
        .max_width(380),
    )
    .center(Length::Fill)
    .into()
}

fn subscription(app: &App) -> Subscription<Message> {
    let close = window::close_requests().map(Message::CloseRequested);
    let keys = keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } => Some(Message::Key(key, modifiers)),
        _ => None,
    });
    let mut subscriptions = vec![close, keys];
    if app.running.is_some() {
        subscriptions.push(iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick));
    }
    Subscription::batch(subscriptions)
}

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title("Screenshare")
        .theme(|_: &App| theme::theme())
        .font(theme::BARLOW)
        .font(theme::BARLOW_BOLD)
        .default_font(theme::FONT)
        .subscription(subscription)
        .exit_on_close_request(false)
        .window_size((1280.0, 780.0))
        .run()
}
