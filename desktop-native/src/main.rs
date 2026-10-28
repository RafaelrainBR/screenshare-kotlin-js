// Screenshare native desktop: iced UI over the session and libwebrtc engine.
// Release builds are a GUI app on Windows: no console window behind it. Debug builds keep it for the logs.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod sounds;
mod startup;
mod telemetry;
mod tray;
mod ui;
mod updater;

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Sender, channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use desktop_native::{
    LOCAL_CAMERA, LOCAL_SCREEN, VideoSurface,
    quality,
    session::{AudioControl, ScreenOptions, SessionView, UiConnection, run_controlled_with_ui},
    share::{SharePreset, ShareProfile},
};
use iced::{
    Element, Point, Size, Subscription, Task, keyboard,
    widget::{image, operation},
    window,
};
use ui::{room, theme, wizard::Wizard};

#[derive(Debug, Clone)]
pub enum Message {
    Room(String),
    Name(String),
    Join,
    /// With a draft in the chat it asks first, like the web confirm(); again from the dialog it leaves.
    Leave,
    CancelLeave,
    /// #permission-recovery: "Tentar novamente", "Abrir configurações" and "Depois".
    PermissionRetry,
    PermissionSettings,
    PermissionClose,
    Draft(String),
    Send,
    ToggleCamera,
    CameraDevice(Option<String>),
    ToggleMic,
    ToggleOutput,
    MicDevice(Option<String>),
    OutputDevice(Option<String>),
    OpenWizard,
    CloseWizard,
    Sources(Vec<desktop_native::sources::Source>),
    /// A recent room on the join screen: fill it in and join.
    PickRoom(String),
    /// "Criar sala privada": a 128-bit code nobody guesses, only shared by link.
    PrivateRoom,
    ToggleFavoriteRoom(String),
    /// The tray's "Compartilhar de novo": a fresh listing, to find the same source again.
    ShareAgain(Vec<desktop_native::sources::Source>),
    /// Following a shared window that closed by itself (Follow).
    FollowPoll,
    FollowSources(Vec<desktop_native::sources::Source>),
    FollowAccept,
    FollowDismiss,
    Thumbnail(String, Option<image::Handle>),
    Pick(String),
    ToggleShareAudio,
    /// The 2 s refresh (false) or the "Atualizar" button (true, which also retakes the thumbnails).
    RefreshSources(bool),
    /// A preset or one field from the wizard or the dock menu; reaches a running share live.
    SetProfile(ShareProfile),
    /// Opens a page of the dock's transmission menu, or closes it.
    ShareMenu(Option<ui::profile::Menu>),
    /// Opens the device list above a dock chevron, or closes it.
    DevicePicker(Option<ui::dock::Picker>),
    ToggleAdvanced,
    /// "Sons ao entrar, sair e compartilhar" in the output picker.
    ToggleRoomSounds,
    /// "Enviar estatísticas de uso anônimas" in the output picker.
    ToggleMetrics,
    ToggleShareBorder,
    /// The ⋯ menu of a participant (and of the clicked video, if any), or None to close it.
    PeerOptions(Option<(String, Option<bool>)>),
    Cursor(Point),
    /// A click on a tile's quality chip: its explanation in the notice.
    Explain(String),
    /// "Algo deu errado?": opens the form (Some("")), edits it, or closes it (None).
    Feedback(Option<String>),
    SendFeedback,
    /// "Silenciar áudio para você": volume to 0 and back; true for the audio of that peer's screen.
    TogglePeerMute(String, bool),
    /// "Volume para você", 0..=100: (peer, screen audio, percent).
    Volume(String, bool, u8),
    ToggleHideVideo(TileKey),
    StartShare,
    StopShare,
    Key(keyboard::Key, keyboard::Modifiers),
    Tick,
    /// Click on a video: show it alone, or back to the grid.
    Focus(TileKey),
    /// The tile's "Tela cheia" button: that video alone over the whole monitor, or back.
    Fullscreen(Option<TileKey>),
    /// "Janela flutuante": the window shrinks to that video, on top of the game; None goes back.
    Float(Option<TileKey>),
    /// "Ocultar conversa" / "Mostrar conversa".
    ToggleChat,
    /// .strip-toggle: hide or show the rail under the stage.
    ToggleRail,
    /// A tray action from the compact panel's buttons.
    Tray(tray::Action),
    /// The panel lost focus: it hides, like the Tauri panel.
    PanelBlur,
    /// "Mais opções": the full window with the share settings.
    PanelOptions,
    /// "Visualizar tela" / "Visualizar câmera": the full window on that video.
    ViewTile(TileKey),
    CloseRequested(window::Id),
    CopyRoom,
    /// "Iniciar com o sistema e voltar a esta sala" on the join form.
    ToggleAutostart(bool),
    /// Reads the tray clicks and refreshes its menu.
    TrayPoll,
    /// At launch and every 6 h: a newer native release on GitHub.
    CheckUpdate,
    UpdateFound(Option<updater::Release>),
    /// The tray item or the join screen's link: install it, or open its page.
    Update,
    /// The new version is in place (or its setup is running) and started: quit.
    UpdateStarted(Result<(), String>),
}

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

/// (peer, camera): one remote screen or camera.
pub type TileKey = (String, bool);

/// The newest frame of one video; the window uploads it when it draws (ui::video).
#[derive(Default)]
pub struct Tile {
    pub frame: Option<Arc<desktop_native::VideoFrameSnapshot>>,
    pub version: u64,
    /// When the first frame arrived: it fades in over the tile.
    pub shown_at: Option<Instant>,
    /// The video stopped: its last frame fades out in its slot before the tile goes.
    pub leaving_since: Option<Instant>,
}

/// A shared window closed by itself (a game restarting): for FOLLOW_FOR a window of the same
/// executable may take its place, only when the person says so; it could be a launcher or a splash.
pub struct Follow {
    exe: String,
    since: Instant,
    pub found: Option<desktop_native::sources::Source>,
}

const FOLLOW_FOR: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct App {
    room: String,
    /// Recent and favorite rooms (rooms.json).
    rooms: Vec<startup::RecentRoom>,
    /// The source and audio choice of the last share, this run only: window ids change between runs.
    last_share: Option<(desktop_native::sources::Source, bool)>,
    name: String,
    draft: String,
    running: Option<Running>,
    view: SessionView,
    wizard: Option<Wizard>,
    /// Newest frame per video, ours included, like the web screen tiles.
    tiles: HashMap<TileKey, Tile>,
    /// The stage's videos in the order they appeared, the leaving ones still in their slot.
    video_order: Vec<TileKey>,
    /// When each peer started or stopped speaking, for the ring to ease between the two.
    speaking_since: HashMap<String, (bool, Instant)>,
    /// When the open popover (share menu or device list), the participant menu or a modal opened.
    popover_at: Option<Instant>,
    menu_opened: Option<Instant>,
    modal_at: Option<Instant>,
    /// When the chat or the rail was last hidden or shown, for their slide.
    chat_toggled: Option<Instant>,
    rail_toggled: Option<Instant>,
    /// When the room screen appeared, for its fade in.
    entered: Option<Instant>,
    /// Sharing, camera and microphone as the dock last showed them, and when each changed: the new icon pops in.
    dock_icons: [(bool, Option<Instant>); 3],
    /// Where each video tile and rail avatar was drawn, for the slide when the stage changes (ui::room).
    flips: ui::motion::Flips<(String, Option<bool>)>,
    /// The video shown alone (web focus mode); the others go to the rail.
    focused: Option<TileKey>,
    sent: u64,
    mic_device: Option<String>,
    output_device: Option<String>,
    /// A device picked while that audio was on: turn it off, then back on with the new device.
    mic_restart: bool,
    output_restart: bool,
    /// The room sound was muted (saved like the web screenshare.outputMuted).
    output_muted: bool,
    /// Room tones on (saved like the web screenshare.roomSounds), the last one heard and when.
    room_sounds: bool,
    /// "Enviar estatísticas de uso anônimas" (desktop_native::metrics), saved in audio.json.
    metrics_on: bool,
    share_border: bool,
    /// Usage metrics: when our share began, whether the stop button ended it, when the socket id
    /// was lost (a reconnection when it comes back), and whether this room's size was sent.
    share_started_at: Option<Instant>,
    stop_requested: bool,
    lost_at: Option<Instant>,
    /// The executable of the window being shared, read when it started: the process is gone once it closes.
    shared_exe: Option<String>,
    follow: Option<Follow>,
    room_reported: bool,
    /// feature_used already sent in this room: once per feature.
    features_used: HashSet<&'static str>,
    heard_sound: u64,
    sound_at: Option<Instant>,
    /// The session generation the saved sound preference was applied to.
    output_generation: u64,
    devices_requested: bool,
    /// Toast like the web ui-notice: the text and when it appeared; it hides after 6 s.
    notice: Option<(String, Instant)>,
    /// The open "Algo deu errado?" form and its text.
    feedback: Option<String>,
    /// "Por que está travando?": what each screen tile shows (desktop_native::quality).
    quality: Quality,
    /// Errors already toasted, so the same ones do not come back every tick.
    shown_errors: String,
    /// Joined at least once: losing the socket id after that means reconnecting.
    joined: bool,
    /// Kept across rooms and runs (share-profile.json), like the web localStorage.
    profile: ShareProfile,
    share_menu: Option<ui::profile::Menu>,
    device_picker: Option<ui::dock::Picker>,
    /// The wizard's "Ajustes avançados" is open.
    advanced: bool,
    /// Height of the running share, for live bitrate changes.
    share_height: u32,
    /// A new source picked while sharing: started once the old share ends.
    pending_share: Option<ScreenOptions>,
    /// The share last started, so it can start again after a reconnection.
    share_options: Option<ScreenOptions>,
    /// Share, camera and microphone as they were in `media_generation`, turned back on in the next one.
    media_before: (Option<ScreenOptions>, bool, bool),
    media_generation: u64,
    autostart: bool,
    /// Without a tray (Explorer not running) closing the window quits.
    has_tray: bool,
    camera_device: Option<String>,
    /// A camera picked while on: started again once the old one has ended.
    camera_restart: bool,
    /// Last cursor position in the window, where the participant menu opens (like the web contextmenu).
    cursor: Point,
    menu_at: Option<Point>,
    /// The camera stepped out for a new share and returns once the share is up.
    camera_after_share: bool,
    peer_options: Option<(String, Option<bool>)>,
    /// "Silenciar áudio para você" and "Ocultar vídeo para você": this client only, this session only.
    /// Voice volume by name, kept across runs like the web's screenshare.volume.user.<name>.
    voice_volumes: HashMap<String, u8>,
    /// Shared screen audio volume by peer, this session only (the web's screenVolumes).
    screen_volumes: HashMap<String, u8>,
    /// What "Reativar" goes back to.
    volume_before_mute: HashMap<(String, bool), u8>,
    /// The volumes the engine has, per (peer, screen); reset on each new connection.
    sent_volumes: HashMap<(String, bool), u8>,
    volumes_generation: u64,
    hidden_videos: HashSet<TileKey>,
    fullscreen: Option<TileKey>,
    /// The video the window floats with, small and on top (ui::room::floating).
    floating: Option<TileKey>,
    chat_hidden: bool,
    rail_hidden: bool,
    /// "Há uma mensagem não enviada…" is open.
    confirm_leave: bool,
    /// The camera or microphone failed: the help dialog, with its text.
    permission: Option<(ui::permission::Media, &'static str)>,
    /// The last audio request turned the microphone on, so an audio error is the microphone's.
    mic_attempt: bool,
    /// Errors already turned into the dialog, so each opens it once.
    seen_camera_error: Option<String>,
    seen_audio_error: Option<String>,
    /// The window is the tray panel (Tauri's compact mode), not the room.
    compact: bool,
    /// Shown, as far as the tray toggle is concerned.
    visible: bool,
    panel_shown: Option<Instant>,
    panel_blurred: Option<Instant>,
    /// A release newer than this build, and whether its setup is downloading.
    update: Option<updater::Release>,
    updating: bool,
}

pub const NAME_INPUT: &str = "name-input";

fn boot() -> (App, Task<Message>) {
    let last = startup::last_room();
    let prefs = startup::audio_prefs();
    let mut app = App {
        visible: !startup::launched_at_login(),
        name: if last.name.trim().is_empty() { whoami() } else { last.name },
        room: last.room,
        rooms: startup::recent_rooms(),
        profile: ui::profile::load(),
        autostart: startup::autostart_enabled(),
        has_tray: tray::install(),
        output_muted: prefs.output_muted,
        room_sounds: !prefs.room_sounds_off,
        metrics_on: !prefs.usage_metrics_off,
        share_border: prefs.share_border,
        voice_volumes: startup::voice_volumes(),
        output_device: prefs.output_device,
        ..Default::default()
    };
    desktop_native::metrics::set_enabled(app.metrics_on);
    set_share_border(app.share_border);
    startup::register_link_protocol();
    let link = startup::link_arg().map(|link| room_code(&link).to_owned()).filter(|code| !code.is_empty());
    if let Some(code) = &link {
        app.room = code.clone();
    }
    // Like the desktop web client: with autostart on and a saved room, every launch rejoins it.
    let rejoin = link.is_some() || app.autostart && !app.room.trim().is_empty();
    updater::clean_up();
    let mut tasks = vec![operation::focus(NAME_INPUT), Task::done(Message::CheckUpdate)];
    if rejoin {
        tasks.push(Task::done(Message::Join));
    } else if startup::launched_at_login() {
        // Nothing to rejoin: a tray icon alone would look like the app failed to start.
        tasks.push(show_window(&mut app));
    }
    (app, Task::batch(tasks))
}

/// Brings the (possibly hidden) window back as the room window, out of the panel's mode, and focuses it.
fn show_window(app: &mut App) -> Task<Message> {
    app.visible = true;
    let unfloat = if app.compact { Task::none() } else { set_floating(app, None) };
    let was_compact = std::mem::take(&mut app.compact);
    // Tauri centers it on its monitor; the cursor is on the one the user just clicked.
    let (cx, cy) = ui::panel::cursor();
    let area = was_compact.then(|| ui::panel::work_area(cx, cy)).flatten();
    unfloat.chain(with_window(move |id| {
        let full = if was_compact {
            let frame = window::toggle_decorations(id)
                .chain(window::set_resizable(id, true))
                .chain(window::set_level(id, window::Level::Normal));
            frame.chain(window::scale_factor(id).then(move |scale| {
                let Some((x, y, width, height)) = area else { return window::resize(id, Size::new(1280.0, 780.0)) };
                let s = f64::from(scale);
                let (w, h) = (1280.0_f64.min(width / s), 780.0_f64.min(height / s));
                window::resize(id, Size::new(w as f32, h as f32))
                    .chain(window::move_to(id, Point::new(((x + (width - w * s) / 2.0) / s) as f32, ((y + (height - h * s) / 2.0) / s) as f32)))
            }))
        } else {
            Task::none()
        };
        let button = if was_compact { taskbar(id, true) } else { Task::none() };
        full.chain(window::minimize(id, false)).chain(window::set_mode(id, window::Mode::Windowed)).chain(button).chain(window::gain_focus(id))
    }))
}

/// Runs a window task on the app's only window.
fn with_window(task: impl Fn(window::Id) -> Task<Message> + Send + 'static) -> Task<Message> {
    window::oldest().then(move |id| id.map_or_else(Task::none, &task))
}

/// Tauri's present_window(compact): frameless, fixed, on top, 430×660 by the tray icon.
fn open_panel(app: &mut App, (x, y, w, h): (f64, f64, f64, f64)) -> Task<Message> {
    // The panel sizes and lifts the window itself; leaving it restores the room window.
    app.floating = None;
    let windowed = set_fullscreen(app, None);
    let framed = !app.compact;
    app.compact = true;
    app.visible = true;
    app.panel_shown = Some(Instant::now());
    let area = ui::panel::work_area(x + w / 2.0, y + h / 2.0);
    let (width, height) = ui::panel::SIZE;
    windowed.chain(with_window(move |id| {
        let frame = if framed { window::toggle_decorations(id) } else { Task::none() };
        // shortcut: positions use this window's scale; a tray on a monitor with another DPI lands slightly off.
        let place = window::scale_factor(id).then(move |scale| {
            let s = f64::from(scale);
            let size = (f64::from(width) * s, f64::from(height) * s);
            // Without a work area, right under the icon, as Tauri falls back.
            let (px, py) = area.map_or((x + w - size.0, y + h), |area| ui::panel::position((x, y, w, h), size, area));
            window::move_to(id, Point::new((px / s) as f32, (py / s) as f32))
        });
        frame
            .chain(window::set_resizable(id, false))
            .chain(window::set_level(id, window::Level::AlwaysOnTop))
            .chain(window::resize(id, Size::new(width, height)))
            .chain(place)
            .chain(window::set_mode(id, window::Mode::Windowed))
            .chain(taskbar(id, false))
            .chain(window::gain_focus(id))
    }))
}

/// Tauri's set_skip_taskbar, done as winit does it (ITaskbarList): the panel has no taskbar button,
/// the room window has. Called after the window is shown, since showing it adds the button back.
#[cfg(target_os = "macos")]
fn taskbar(_id: window::Id, _shown: bool) -> Task<Message> {
    // The panel is a menu bar window; the Dock icon follows the app, not this window.
    Task::none()
}

#[cfg(windows)]
fn taskbar(id: window::Id, shown: bool) -> Task<Message> {
    window::run(id, move |handle| {
        use window::raw_window_handle::RawWindowHandle;
        use windows::Win32::{
            Foundation::HWND,
            System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            UI::Shell::{ITaskbarList, TaskbarList},
        };
        let Ok(raw) = handle.window_handle() else { return };
        let RawWindowHandle::Win32(win32) = raw.as_raw() else { return };
        let hwnd = HWND(win32.hwnd.get() as *mut _);
        // SAFETY: runs on the window's thread, where winit has initialized COM; the window is alive.
        unsafe {
            let Ok(list) = CoCreateInstance::<_, ITaskbarList>(&TaskbarList, None, CLSCTX_INPROC_SERVER) else { return };
            if list.HrInit().is_ok() {
                let _ = if shown { list.AddTab(hwnd) } else { list.DeleteTab(hwnd) };
            }
        }
    })
    .discard()
}

fn hide_panel(app: &mut App) -> Task<Message> {
    app.visible = false;
    app.panel_blurred = Some(Instant::now());
    with_window(|id| window::set_mode(id, window::Mode::Hidden))
}

/// The tray item a menu pick or a shortcut-free click maps to.
fn tray_action(app: &mut App, action: tray::Action) -> Task<Message> {
    match action {
        tray::Action::Open => show_window(app),
        // tray_click_action in the Tauri app.
        tray::Action::Panel(x, y, w, h) => match (app.visible, app.compact) {
            (true, false) => show_window(app),
            (true, true) => hide_panel(app),
            // This click already closed the panel through its blur.
            _ if app.panel_blurred.is_some_and(|at| at.elapsed() < Duration::from_millis(400)) => Task::none(),
            _ => open_panel(app, (x, y, w, h)),
        },
        tray::Action::Mic => update(app, Message::ToggleMic),
        tray::Action::Camera => update(app, Message::ToggleCamera),
        tray::Action::Output => update(app, Message::ToggleOutput),
        tray::Action::Screen if sharing(&app.view) => update(app, Message::StopShare),
        // Forms open in the full window, as in the Tauri app.
        tray::Action::Screen => show_window(app).chain(update(app, Message::OpenWizard)),
        tray::Action::ShareAgain => ui::wizard::load_sources_into(Message::ShareAgain),
        tray::Action::Leave => update(app, Message::Leave),
        tray::Action::Update => update(app, Message::Update),
        tray::Action::Logs => {
            telemetry::open_logs();
            Task::none()
        }
        tray::Action::Feedback => show_window(app).chain(update(app, Message::Feedback(Some(String::new())))),
        tray::Action::Quit => {
            app.running = None;
            iced::exit()
        }
    }
}

/// The update's tray item and join screen link.
pub fn update_label(app: &App) -> Option<String> {
    let release = app.update.as_ref()?;
    Some(match (app.updating, updater::can_install(release)) {
        (true, _) => format!("Baixando a versão {}…", release.label()),
        (false, true) => format!("Atualizar para a versão {}", release.label()),
        (false, false) => format!("Baixar a versão {}", release.label()),
    })
}

fn tray_state(app: &App) -> tray::State {
    let view = &app.view;
    let local = view.local_socket_id.as_deref();
    tray::State {
        in_room: app.running.is_some(),
        connected: local.is_some(),
        mic: view.microphone_enabled,
        camera: camera_on(view),
        output_muted: !view.output_enabled,
        sharing: sharing(view),
        participants: view.participants.len(),
        remote_sharing: view.screen_sharers.iter().chain(&view.camera_sharers).any(|id| Some(id.as_str()) != local),
        // Checked again on click against a fresh listing (Message::ShareAgain).
        share_again: app.last_share.as_ref().filter(|_| local.is_some() && !sharing(view)).map(|(source, _)| source.title.clone()),
        update: update_label(app),
    }
}

/// A room link opened while the app runs: join it, unless we are in another room already.
fn open_link(app: &mut App, link: &str) -> Task<Message> {
    let code = room_code(link).to_owned();
    if code.is_empty() || app.running.is_some() && code == app.room {
        return Task::none();
    }
    if app.running.is_some() {
        app.notice = Some((format!("Saia da sala para entrar em {code}"), Instant::now()));
        return Task::none();
    }
    app.room = code;
    update(app, Message::Join)
}

/// A pasted room link (…/?roomId=abc) joins that room, as the web join form takes ?roomId=.
fn room_code(input: &str) -> &str {
    let code = input.split_once("roomId=").map_or(input, |(_, rest)| rest.split(['&', '#']).next().unwrap_or_default());
    code.trim()
}

#[test]
fn a_room_link_joins_its_room() {
    assert_eq!(room_code(" https://screen-share.fly.dev/?roomId=4f8a2c1e "), "4f8a2c1e");
    assert_eq!(room_code("http://localhost:8080/?x=1&roomId=ab12&y=2#top"), "ab12");
    assert_eq!(room_code(" 4f8a2c1e "), "4f8a2c1e");
    assert_eq!(room_code("https://screen-share.fly.dev/?roomId="), "");
    assert_eq!(room_code("screenshare://join?roomId=4f8a2c1e"), "4f8a2c1e");
}

#[test]
fn a_reconnection_turns_the_share_camera_and_mic_back_on() {
    let (chat, _chat) = channel();
    let (screen, screens) = channel();
    let (camera, cameras) = channel();
    let (devices, _devices) = channel();
    let (audio, audios) = channel();
    let (share_profile, _profiles) = channel();
    let view = Arc::new(Mutex::new(SessionView {
        generation: 1,
        local_socket_id: Some("a".into()),
        screen_state: "active".into(),
        camera_state: "active".into(),
        microphone_enabled: true,
        ..Default::default()
    }));
    let running = Running {
        stop: Arc::default(),
        thread: None,
        chat,
        screen,
        camera,
        devices,
        audio,
        share_profile,
        view: view.clone(),
        surface: VideoSurface::default(),
    };
    let options = ScreenOptions::default();
    let mut app = App { running: Some(running), share_options: Some(options.clone()), ..Default::default() };
    let _ = tick(&mut app);
    let _ = tick(&mut app);
    assert!(screens.try_recv().is_err(), "nothing replayed on the first connection");
    // Dropped: a fresh view until the new socket joins, then generation 2 joined.
    *view.lock().unwrap() = SessionView { generation: 2, ..Default::default() };
    let _ = tick(&mut app);
    view.lock().unwrap().local_socket_id = Some("b".into());
    let _ = tick(&mut app);
    assert_eq!(screens.try_recv().unwrap(), (2, Some(options)));
    assert!(cameras.try_recv().is_err() && app.camera_after_share, "the camera waits for the share");
    assert!(audios.try_iter().any(|(generation, control)| generation == 2
        && matches!(control, AudioControl::Microphone { enabled: true, .. })));
}

fn whoami() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "Nativo".into())
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    // What the user did, in Sentry's breadcrumbs and logs, so an error comes with the steps before it.
    let action = match &message {
        Message::Join => Some("join room"),
        Message::Leave => Some("leave room"),
        Message::StartShare => Some("start share"),
        Message::StopShare => Some("stop share"),
        Message::ToggleCamera => Some("toggle camera"),
        Message::ToggleMic => Some("toggle microphone"),
        Message::ToggleOutput => Some("toggle sound"),
        _ => None,
    };
    if let Some(action) = action {
        let breadcrumb = sentry::Breadcrumb { category: Some("ui".into()), message: Some(action.into()), ..Default::default() };
        // The crash reporter process keeps its own copy, for the steps before a C++ crash.
        sentry::with_integration(|minidump: &sentry::integrations::minidump::MinidumpIntegration, _| minidump.add_breadcrumb(breadcrumb.clone()));
        sentry::add_breadcrumb(breadcrumb);
        telemetry::info(action);
    }
    match message {
        Message::Room(value) => app.room = value,
        Message::PrivateRoom => {
            app.room = format!("{:032x}", rand::random::<u128>());
            return update(app, Message::Join);
        }
        Message::PickRoom(id) => {
            app.room = id;
            feature_used(app, "recent_room");
            return update(app, Message::Join);
        }
        Message::ToggleFavoriteRoom(id) => {
            app.rooms = startup::toggle_favorite(&app.rooms, &id);
            startup::save_recent_rooms(&app.rooms);
        }
        Message::Name(value) => app.name = value,
        Message::Join if app.running.is_none() && !app.name.trim().is_empty() => {
            // Empty code creates a room, like the web join form.
            app.room = match room_code(&app.room) {
                "" => format!("{:08x}", rand::random::<u32>()),
                code => code.into(),
            };
            startup::save_last_room(app.name.trim(), &app.room);
            app.rooms = startup::remember_room(&app.rooms, &app.room);
            startup::save_recent_rooms(&app.rooms);
            app.running = Some(start(app.room.clone(), app.name.trim().into()));
            app.entered = Some(Instant::now());
            return operation::focus(room::CHAT_INPUT);
        }
        Message::Join => {}
        Message::Leave if !app.draft.trim().is_empty() && !app.confirm_leave => {
            app.confirm_leave = true;
            app.modal_at = Some(Instant::now());
            // The tray's "Sair da sala" may come with the window hidden; unfocusing keeps Enter off the chat.
            return set_fullscreen(app, None).chain(show_window(app)).chain(operation::focus("leave-confirm"));
        }
        Message::PermissionRetry => match app.permission.take() {
            Some((ui::permission::Media::Camera, _)) => return update(app, Message::ToggleCamera),
            Some((ui::permission::Media::Microphone, _)) => audio(app, AudioControl::Microphone { enabled: true, device_id: app.mic_device.clone() }),
            None => {}
        },
        Message::PermissionSettings => {
            if let Some((media, _)) = app.permission {
                ui::permission::open_settings(media);
            }
        }
        Message::PermissionClose => app.permission = None,
        Message::CancelLeave => {
            app.confirm_leave = false;
            return operation::focus(room::CHAT_INPUT);
        }
        Message::Leave => {
            // Leaving turns the rejoin off, as in the web client: the next launch must not return here.
            if app.autostart {
                if let Err(error) = startup::set_autostart(false) {
                    app.notice = Some((error, Instant::now()));
                }
            }
            if let Some(at) = app.share_started_at.take() {
                share_ended(app, at, "left");
            }
            if let Some(at) = app.entered {
                desktop_native::metrics::send("room_left", &[("minutes", desktop_native::metrics::minutes_bucket(at.elapsed()).into())]);
            }
            // Dropping Running joins the session thread: the engine stops the share and closes peers.
            let windowed = set_floating(app, None);
            let App { name, rooms, last_share, mic_device, output_device, output_muted, room_sounds, metrics_on, share_border, camera_device, profile, advanced, has_tray, notice, chat_hidden, rail_hidden, compact, visible, voice_volumes, update, updating, .. } = std::mem::take(app);
            *app = App { name, rooms, last_share, mic_device, output_device, output_muted, room_sounds, metrics_on, share_border, camera_device, profile, advanced, has_tray, notice, chat_hidden, rail_hidden, compact, visible, voice_volumes, update, updating, ..Default::default() };
            tray::update(tray_state(app));
            return windowed.chain(operation::focus(NAME_INPUT));
        }
        Message::ToggleAutostart(enabled) => match startup::set_autostart(enabled) {
            Ok(()) => app.autostart = enabled,
            Err(error) => app.notice = Some((error, Instant::now())),
        },
        Message::CheckUpdate => return Task::perform(updater::check(), Message::UpdateFound),
        Message::UpdateFound(Some(release)) => {
            if app.update.as_ref().map(|known| known.version) != Some(release.version) {
                app.notice = Some((format!("Nova versão {} disponível no menu da bandeja", release.label()), Instant::now()));
                app.update = Some(release);
            }
        }
        Message::UpdateFound(None) => {}
        Message::Update => {
            let Some(release) = app.update.clone().filter(|_| !app.updating) else { return Task::none() };
            if !updater::can_install(&release) {
                updater::open_page(&release);
                return Task::none();
            }
            app.updating = true;
            telemetry::info(&format!("update: installing {}", release.label()));
            return Task::perform(updater::install(release), Message::UpdateStarted);
        }
        Message::UpdateStarted(Ok(())) => {
            app.running = None;
            return iced::exit();
        }
        Message::UpdateStarted(Err(error)) => {
            app.updating = false;
            telemetry::warn(&format!("update failed: {error}"));
            app.notice = Some((format!("Não foi possível atualizar: {error}"), Instant::now()));
            // The page still has the setup to run by hand.
            if let Some(release) = &app.update {
                updater::open_page(release);
            }
        }
        Message::TrayPoll => {
            let mut tasks: Vec<_> = tray::actions().into_iter().map(|action| tray_action(app, action)).collect();
            if startup::show_requested() {
                tasks.push(show_window(app));
                if let Some(link) = startup::take_link() {
                    tasks.push(open_link(app, &link));
                }
            }
            tray::update(tray_state(app));
            return Task::batch(tasks);
        }
        Message::Draft(value) => app.draft = value,
        Message::Send => {
            if let Some(running) = &app.running {
                if !app.draft.trim().is_empty() {
                    let _ = running.chat.send((app.view.generation, std::mem::take(&mut app.draft)));
                }
            }
        }
        Message::ToggleCamera => {
            if let Some(running) = &app.running {
                let start = !camera_on(&app.view);
                let _ = running.camera.send((app.view.generation, start.then(|| app.camera_device.clone())));
            }
        }
        Message::CameraDevice(id) => {
            app.device_picker = None;
            app.camera_device = id;
            // Like the microphone: a camera picked while on restarts with it.
            if let (Some(running), true) = (&app.running, camera_on(&app.view)) {
                let _ = running.camera.send((app.view.generation, None));
                app.camera_restart = true;
            }
        }
        Message::ToggleMic => audio(app, AudioControl::Microphone { enabled: !app.view.microphone_enabled, device_id: app.mic_device.clone() }),
        Message::ToggleOutput => {
            app.output_muted = app.view.output_enabled;
            save_audio(app);
            audio(app, AudioControl::Output { enabled: !app.view.output_enabled, device_id: app.output_device.clone() });
        }
        Message::MicDevice(id) => {
            app.device_picker = None;
            app.mic_device = id;
            if app.view.microphone_enabled {
                app.mic_restart = true;
                audio(app, AudioControl::Microphone { enabled: false, device_id: None });
            }
        }
        Message::OutputDevice(id) => {
            app.device_picker = None;
            app.output_device = id;
            save_audio(app);
            if app.view.output_enabled {
                app.output_restart = true;
                audio(app, AudioControl::Output { enabled: false, device_id: None });
            }
        }
        Message::OpenWizard => {
            app.share_menu = None;
            let mut wizard = Wizard::open();
            // The last source comes back selected, so Enter shares it again; a closed window drops out
            // with the first listing (Wizard::set_sources).
            if let Some((source, audio)) = &app.last_share {
                wizard.selected = Some(source.clone());
                wizard.audio = *audio;
            }
            app.wizard = Some(wizard);
            app.modal_at = Some(Instant::now());
            // Focusing an id nobody has unfocuses the chat behind the modal, so A, P and Enter reach the wizard.
            return Task::batch([ui::wizard::load_sources(), operation::focus("share-wizard")]);
        }
        Message::CloseWizard => {
            app.wizard = None;
            return operation::focus(room::CHAT_INPUT);
        }
        Message::Sources(sources) => {
            if let Some(wizard) = &mut app.wizard {
                return ui::wizard::load_thumbnails(&wizard.set_sources(sources));
            }
        }
        Message::RefreshSources(force) => {
            if let Some(wizard) = &mut app.wizard {
                if force {
                    // The listing in flight, if any, reloads every thumbnail when it lands.
                    wizard.thumbs.clear();
                }
                if !wizard.loading {
                    wizard.loading = true;
                    return ui::wizard::load_sources();
                }
            }
        }
        Message::Thumbnail(id, handle) => {
            if let Some(wizard) = &mut app.wizard {
                wizard.set_thumbnail(id, handle);
            }
        }
        Message::Pick(id) => {
            if let Some(wizard) = &mut app.wizard {
                wizard.select(&id);
            }
        }
        Message::ToggleShareAudio => {
            if let Some(wizard) = &mut app.wizard {
                wizard.audio = !wizard.audio;
            }
        }
        Message::SetProfile(profile) => {
            app.profile = profile;
            ui::profile::save(profile);
            if let (Some(running), "active") = (&app.running, app.view.screen_state.as_str()) {
                let _ = running.share_profile.send((app.view.generation, profile, app.share_height));
            }
        }
        // One popover at a time, like the web <details> pickers.
        Message::ShareMenu(page) => {
            // A page change inside the open menu does not rise again.
            if app.share_menu.is_none() {
                app.popover_at = Some(Instant::now());
            }
            app.share_menu = page;
            app.device_picker = None;
        }
        Message::DevicePicker(picker) => {
            if picker.is_some() && picker != app.device_picker {
                app.popover_at = Some(Instant::now());
            }
            app.device_picker = picker;
            app.share_menu = None;
        }
        Message::ToggleAdvanced => app.advanced = !app.advanced,
        Message::ToggleRoomSounds => {
            app.room_sounds = !app.room_sounds;
            save_audio(app);
        }
        Message::ToggleShareBorder => {
            app.share_border = !app.share_border;
            set_share_border(app.share_border);
            save_audio(app);
        }
        Message::ToggleMetrics => {
            app.metrics_on = !app.metrics_on;
            desktop_native::metrics::set_enabled(app.metrics_on);
            save_audio(app);
        }
        Message::PeerOptions(target) => {
            app.menu_at = target.is_some().then_some(app.cursor);
            app.menu_opened = target.is_some().then(Instant::now);
            app.peer_options = target;
        }
        Message::Cursor(position) => app.cursor = position,
        Message::Explain(text) => app.notice = Some((text, Instant::now())),
        Message::Feedback(draft) => {
            let opening = app.feedback.is_none() && draft.is_some();
            app.feedback = draft;
            if opening {
                return operation::focus(ui::feedback::INPUT);
            }
        }
        Message::SendFeedback => {
            let Some(text) = app.feedback.as_ref().map(|draft| draft.trim().to_owned()).filter(|text| !text.is_empty()) else { return operation::focus(ui::feedback::INPUT) };
            app.feedback = None;
            let sent = telemetry::send_feedback(&text);
            app.notice = Some((if sent { "Obrigado! Recebemos seu relato." } else { "Esta versão não envia relatos. Use a pasta de logs." }.into(), Instant::now()));
        }
        Message::TogglePeerMute(peer, screen) => {
            let current = volume(app, &peer, screen);
            let next = if current > 0 {
                app.volume_before_mute.insert((peer.clone(), screen), current);
                0
            } else {
                app.volume_before_mute.remove(&(peer.clone(), screen)).filter(|v| *v > 0).unwrap_or(100)
            };
            return update(app, Message::Volume(peer, screen, next));
        }
        Message::Volume(peer, screen, percent) => {
            if screen {
                app.screen_volumes.insert(peer.clone(), percent);
            } else if let Some(name) = app.view.participants.iter().find(|p| p.socket_id == peer).map(|p| p.username.clone()) {
                app.voice_volumes.insert(name, percent);
                startup::save_voice_volumes(&app.voice_volumes);
            }
            send_volume(app, peer, screen, percent);
        }
        Message::ToggleHideVideo(key) => {
            if !app.hidden_videos.insert(key.clone()) {
                app.hidden_videos.remove(&key);
            }
        }
        Message::StartShare => {
            if let (Some(running), Some(options)) = (&app.running, app.wizard.as_ref().and_then(|w| w.options(app.profile))) {
                app.share_height = options.height;
                app.last_share = app.wizard.as_ref().and_then(|wizard| Some((wizard.selected.clone()?, wizard.audio)));
                if sharing(&app.view) {
                    // "Trocar tela ou janela": the engine takes one share at a time, so stop first
                    // and start the new source once it has ended (tick).
                    // shortcut: viewers see the share stop and start; swap the track in place if that bothers.
                    let _ = running.screen.send((app.view.generation, None));
                    app.pending_share = Some(options);
                } else {
                    // Receivers take a peer's first video track as the screen only while it shares no
                    // camera (PeerConnections.onTrack), so the camera steps out and returns after it.
                    // shortcut: viewers see the camera blink; the web recreates the peer connections instead.
                    if camera_on(&app.view) {
                        let _ = running.camera.send((app.view.generation, None));
                        app.camera_after_share = true;
                    }
                    app.share_options = Some(options.clone());
                    let _ = running.screen.send((app.view.generation, Some(options)));
                }
                app.wizard = None;
                return operation::focus(room::CHAT_INPUT);
            }
        }
        Message::ShareAgain(sources) => {
            let Some((last, audio)) = app.last_share.clone() else { return Task::none() };
            if app.running.is_none() || app.view.local_socket_id.is_none() || sharing(&app.view) {
                return Task::none();
            }
            // Only that same window (id, process and title) still open: never a guess at what goes out.
            let Some(source) = sources.into_iter().find(|s| s.id == last.id && s.pid == last.pid && s.title == last.title) else {
                app.notice = Some(("A janela de antes não está mais aberta. Escolha o que compartilhar.".into(), Instant::now()));
                return show_window(app).chain(update(app, Message::OpenWizard));
            };
            app.notice = Some((format!("Compartilhando de novo: {}", source.title), Instant::now()));
            feature_used(app, "share_again");
            let mut wizard = Wizard::open();
            wizard.selected = Some(source);
            wizard.audio = audio;
            app.wizard = Some(wizard);
            return update(app, Message::StartShare);
        }
        Message::FollowPoll => {
            let Some(follow) = &app.follow else { return Task::none() };
            if follow.since.elapsed() >= FOLLOW_FOR {
                app.follow = None;
            } else if follow.found.is_none() {
                return ui::wizard::load_sources_into(Message::FollowSources);
            }
        }
        Message::FollowSources(sources) => {
            if sharing(&app.view) || app.view.local_socket_id.is_none() {
                app.follow = None;
            }
            let Some(follow) = app.follow.as_mut().filter(|follow| follow.found.is_none()) else { return Task::none() };
            if let Some(source) = desktop_native::sources::reopened(sources, &desktop_native::sources::exe_names(), &follow.exe) {
                // The tray's "Compartilhar de novo" now names it too: the app is often hidden mid-game.
                let audio = app.last_share.as_ref().is_some_and(|(_, audio)| *audio);
                app.last_share = Some((source.clone(), audio));
                follow.found = Some(source);
                follow.since = Instant::now();
            }
        }
        Message::FollowAccept => {
            app.follow = None;
            feature_used(app, "follow_window");
            return ui::wizard::load_sources_into(Message::ShareAgain);
        }
        Message::FollowDismiss => app.follow = None,
        Message::StopShare => {
            app.stop_requested = true;
            if let Some(running) = &app.running {
                let _ = running.screen.send((app.view.generation, None));
            }
        }
        Message::Key(key, modifiers) => return shortcut(app, key, modifiers),
        Message::Tick => {
            let task = tick(app);
            usage_metrics(app);
            return task;
        }
        Message::Focus(key) => app.focused = (app.focused.as_ref() != Some(&key)).then_some(key),
        Message::Fullscreen(key) => return set_fullscreen(app, key),
        Message::Float(key) => {
            if key.is_some() {
                feature_used(app, "floating");
            }
            return set_floating(app, key);
        }
        Message::ToggleChat => {
            app.chat_hidden = !app.chat_hidden;
            app.chat_toggled = Some(Instant::now());
        }
        Message::ToggleRail => {
            app.rail_hidden = !app.rail_hidden;
            app.rail_toggled = Some(Instant::now());
        }
        Message::Tray(action) => return tray_action(app, action),
        // hide_panel_on_blur: not while it is still opening.
        Message::PanelBlur if app.compact && app.visible && app.panel_shown.is_some_and(|at| at.elapsed() >= Duration::from_millis(250)) => {
            return hide_panel(app);
        }
        Message::PanelBlur => {}
        Message::PanelOptions => {
            let next = if sharing(&app.view) { Message::ShareMenu(Some(ui::profile::Menu::Main)) } else { Message::OpenWizard };
            return show_window(app).chain(update(app, next));
        }
        Message::ViewTile(key) => {
            app.focused = Some(key);
            return show_window(app);
        }
        Message::CopyRoom => {
            // The one link for the web and the app (the web's Abrir no app hands it over).
            app.notice = Some(("Link da sala copiado".into(), Instant::now()));
            return iced::clipboard::write(format!("https://screen-share.fly.dev/?roomId={}", app.room));
        }
        // Closing keeps the app in the tray, in the room, like the Tauri app; "Encerrar app" quits.
        Message::CloseRequested(id) if app.has_tray => {
            app.visible = false;
            return window::set_mode(id, window::Mode::Hidden);
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
        (Key::Named(Named::Escape), _) if app.feedback.is_some() => Message::Feedback(None),
        (Key::Named(Named::Escape), _) if app.confirm_leave => Message::CancelLeave,
        (Key::Named(Named::Escape), _) if app.permission.is_some() => Message::PermissionClose,
        (Key::Named(Named::Enter), _) if app.permission.is_some() => Message::PermissionRetry,
        (Key::Named(Named::Enter), _) if app.confirm_leave => Message::Leave,
        (Key::Named(Named::Escape), Some(_)) => Message::CloseWizard,
        (Key::Named(Named::Escape), None) if app.peer_options.is_some() => Message::PeerOptions(None),
        (Key::Named(Named::Escape), None) if app.fullscreen.is_some() => Message::Fullscreen(None),
        (Key::Named(Named::Escape), None) if app.floating.is_some() => Message::Float(None),
        (Key::Named(Named::Escape), None) if app.device_picker.is_some() => Message::DevicePicker(None),
        // Like the web menu: Esc leaves a field's options first, then closes the menu.
        (Key::Named(Named::Escape), None) if app.share_menu.is_some() => {
            Message::ShareMenu((app.share_menu != Some(ui::profile::Menu::Main)).then_some(ui::profile::Menu::Main))
        }
        (Key::Named(Named::ArrowRight | Named::ArrowDown), Some(wizard)) => {
            wizard.step(1);
            return Task::none();
        }
        (Key::Named(Named::ArrowLeft | Named::ArrowUp), Some(wizard)) => {
            wizard.step(-1);
            return Task::none();
        }
        (Key::Character("a"), Some(_)) => Message::ToggleShareAudio,
        (Key::Character("p"), Some(_)) => {
            let next = ui::profile::matching(app.profile).and_then(|p| SharePreset::ALL.iter().position(|q| *q == p)).map_or(0, |i| i + 1);
            Message::SetProfile(SharePreset::ALL[next % SharePreset::ALL.len()].profile())
        }
        (Key::Named(Named::Enter), Some(_)) => Message::StartShare,
        (Key::Named(Named::F2), _) => Message::ToggleMic,
        (Key::Named(Named::F3), _) => Message::ToggleOutput,
        (Key::Named(Named::F6), _) => Message::ToggleCamera,
        (Key::Named(Named::F4), None) if sharing(&app.view) => Message::StopShare,
        (Key::Named(Named::F4), None) => Message::OpenWizard,
        (Key::Named(Named::F5), None) => Message::ShareMenu(app.share_menu.is_none().then_some(ui::profile::Menu::Main)),
        _ => return Task::none(),
    };
    update(app, message)
}

/// The local camera is on or starting.
pub fn camera_on(view: &SessionView) -> bool {
    matches!(view.camera_state.as_str(), "pending" | "active")
}

/// The local share is on or starting.
pub fn sharing(view: &SessionView) -> bool {
    matches!(view.screen_state.as_str(), "active" | "pending" | "picking")
}

fn save_audio(app: &App) {
    startup::save_audio_prefs(&startup::AudioPrefs { output_muted: app.output_muted, output_device: app.output_device.clone(), room_sounds_off: !app.room_sounds, usage_metrics_off: !app.metrics_on, share_border: app.share_border });
}

/// "Volume para você" as the engine has it: the saved one for voices, 100 when never set.
pub fn volume(app: &App, peer: &str, screen: bool) -> u8 {
    if screen {
        return app.screen_volumes.get(peer).copied().unwrap_or(100);
    }
    let name = app.view.participants.iter().find(|p| p.socket_id == peer).map(|p| p.username.as_str());
    name.and_then(|name| app.voice_volumes.get(name)).copied().unwrap_or(100)
}

fn send_volume(app: &mut App, peer: String, screen: bool, percent: u8) {
    app.sent_volumes.insert((peer.clone(), screen), percent);
    audio(app, AudioControl::Volume { peer_id: peer, volume: f64::from(percent) / 100.0, screen });
}

fn audio(app: &mut App, control: AudioControl) {
    app.mic_attempt = matches!(control, AudioControl::Microphone { enabled: true, .. });
    if let Some(running) = &app.running {
        let _ = running.audio.send((app.view.generation, control));
    }
}

fn tick(app: &mut App) -> Task<Message> {
    let Some(running) = &app.running else { return Task::none() };
    app.view = running.view.lock().unwrap().clone();
    if app.view.room_sound.0 != app.heard_sound {
        app.heard_sound = app.view.room_sound.0;
        // After a server deploy everyone rejoins at once: one tone stands for the burst.
        let quiet = app.sound_at.is_none_or(|at| at.elapsed() >= Duration::from_millis(1_500));
        if let Some(sound) = app.view.room_sound.1.filter(|_| app.room_sounds && !app.output_muted && quiet) {
            app.sound_at = Some(Instant::now());
            sounds::play(sound, app.output_device.as_deref());
        }
    }
    app.joined |= app.view.local_socket_id.is_some();
    // A new camera error, or an audio error right after turning the microphone on, opens the help.
    let had_dialog = app.permission.is_some();
    let mut unfocus = Task::none();
    if app.view.camera_error != app.seen_camera_error {
        app.seen_camera_error.clone_from(&app.view.camera_error);
        if let Some(error) = &app.view.camera_error {
            app.permission = Some((ui::permission::Media::Camera, ui::permission::help(ui::permission::Media::Camera, error)));
        }
    }
    if app.view.audio_error != app.seen_audio_error {
        app.seen_audio_error.clone_from(&app.view.audio_error);
        if let Some(error) = app.view.audio_error.as_ref().filter(|_| app.mic_attempt) {
            app.permission = Some((ui::permission::Media::Microphone, ui::permission::help(ui::permission::Media::Microphone, error)));
        }
    }
    if !had_dialog && app.permission.is_some() {
        // Like the wizard: Enter and Esc reach the dialog instead of the chat field.
        unfocus = operation::focus("permission-recovery");
        app.modal_at = Some(Instant::now());
    }
    let now = Instant::now();
    for p in &app.view.participants {
        let talking = app.view.speaking.contains(&p.socket_id);
        if app.speaking_since.get(&p.socket_id).is_none_or(|(was, _)| *was != talking) {
            app.speaking_since.insert(p.socket_id.clone(), (talking, now));
        }
    }
    let icons = [sharing(&app.view), camera_on(&app.view), app.view.microphone_enabled];
    for ((was, at), on) in app.dock_icons.iter_mut().zip(icons) {
        if *was != on {
            *was = on;
            *at = Some(now);
        }
    }
    let errors = room::errors(app).join(" · ");
    if errors != app.shown_errors {
        if !errors.is_empty() {
            app.notice = Some((errors.clone(), Instant::now()));
            // What the user saw go wrong (camera, audio, share, connection), with the session around it.
            telemetry::warn(&format!("toast: {errors}"));
        }
        app.shown_errors = errors;
    }
    if app.view.local_socket_id.is_some() && !app.devices_requested {
        app.devices_requested = true;
        let _ = running.devices.send(app.view.generation);
    }
    if !sharing(&app.view) {
        if let Some(options) = app.pending_share.take() {
            app.share_options = Some(options.clone());
            let _ = running.screen.send((app.view.generation, Some(options)));
        }
    }
    let camera_off = matches!(app.view.camera_state.as_str(), "" | "ended");
    let screen_settled = app.view.screen_state == "active" || (app.view.screen_state == "ended" && app.view.screen_error.is_some());
    if camera_off && ((app.camera_after_share && screen_settled) || app.camera_restart) {
        app.camera_after_share = false;
        app.camera_restart = false;
        let _ = running.camera.send((app.view.generation, Some(app.camera_device.clone())));
    }
    // Saved volumes reach each peer's tracks, again after a reconnection (the engine forgets them).
    if app.volumes_generation != app.view.generation {
        app.volumes_generation = app.view.generation;
        app.sent_volumes.clear();
    }
    let local = app.view.local_socket_id.clone();
    let pending: Vec<(String, bool, u8)> = app
        .view
        .participants
        .iter()
        .filter(|p| Some(&p.socket_id) != local.as_ref())
        .flat_map(|p| [(p.socket_id.clone(), false), (p.socket_id.clone(), true)])
        .map(|(peer, screen)| {
            let want = volume(app, &peer, screen);
            (peer, screen, want)
        })
        .filter(|(peer, screen, want)| app.sent_volumes.get(&(peer.clone(), *screen)).copied().unwrap_or(100) != *want)
        .collect();
    for (peer, screen, percent) in pending {
        send_volume(app, peer, screen, percent);
    }
    // The web plays the room sound unless it was muted last time; again after each reconnection.
    if app.view.local_socket_id.is_some() && app.output_generation != app.view.generation {
        app.output_generation = app.view.generation;
        if !app.output_muted && !app.view.output_enabled {
            audio(app, AudioControl::Output { enabled: true, device_id: app.output_device.clone() });
        }
    }
    // A server deploy or a dropped connection starts a new session without the local media;
    // what was on comes back on, so nobody has to share or unmute again.
    if app.view.local_socket_id.is_some() {
        if app.media_generation != app.view.generation {
            app.media_generation = app.view.generation;
            let (share, camera, mic) = std::mem::take(&mut app.media_before);
            if let Some(running) = &app.running {
                if let Some(options) = share {
                    app.share_height = options.height;
                    let _ = running.screen.send((app.view.generation, Some(options)));
                    // As in StartShare, the camera returns once the share is up.
                    app.camera_after_share = camera;
                } else if camera {
                    let _ = running.camera.send((app.view.generation, Some(app.camera_device.clone())));
                }
            }
            if mic {
                audio(app, AudioControl::Microphone { enabled: true, device_id: app.mic_device.clone() });
            }
        } else {
            let share = app.share_options.clone().filter(|_| sharing(&app.view));
            app.media_before = (share, camera_on(&app.view), app.view.microphone_enabled);
        }
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
    let Some(running) = &app.running else { return Task::none() };
    let view = &app.view;
    // Take the newest frames off the surface; the tiles keep them until a newer one arrives.
    let frames: Vec<_> = {
        let mut surface = running.surface.lock().unwrap();
        let latest = std::mem::take(&mut surface.latest);
        latest
            .into_iter()
            .filter_map(|((peer, track), snapshot)| {
                // Our own capture shows under our socket id, like the web local tile.
                let local = match peer.as_str() {
                    LOCAL_SCREEN => Some(false),
                    LOCAL_CAMERA => Some(true),
                    _ => None,
                };
                if let Some(camera) = local {
                    return Some(((view.local_socket_id.clone()?, camera), snapshot));
                }
                let camera = surface.kind(&peer, &track, view.screen_sharers.contains(&peer), view.camera_sharers.contains(&peer))?;
                Some(((peer, camera), snapshot))
            })
            .collect()
    };
    // A video that stopped keeps its last frame a moment, fading out in its slot (ui::motion::LEAVE).
    app.tiles.retain(|key, tile| {
        if is_shared(view, key) {
            tile.leaving_since = None;
            return true;
        }
        let since = *tile.leaving_since.get_or_insert(now);
        tile.frame.is_some() && ui::motion::running(Some(since), ui::motion::LEAVE)
    });
    // New videos join the end, like the web .screen-tile list; a leaving one keeps its slot.
    let shared: Vec<TileKey> = videos(view);
    let tiles = &app.tiles;
    app.video_order.retain(|key| shared.contains(key) || tiles.contains_key(key));
    for key in shared {
        if !app.video_order.contains(&key) {
            app.video_order.push(key);
        }
    }
    if app.focused.as_ref().is_some_and(|key| !is_shared(view, key)) {
        app.focused = None;
    }
    if app.quality.seen != view.screen_quality_count {
        app.quality.seen = view.screen_quality_count;
        app.quality.read(view, &running.surface.lock().unwrap());
    }
    app.quality.refresh(view);
    // Like the browser leaving fullscreen when the video element goes away.
    let windowed = if app.fullscreen.as_ref().is_some_and(|key| !is_shared(view, key)) { set_fullscreen(app, None) } else { Task::none() };
    // A share that ended takes its floating window back to the room.
    let windowed = if app.floating.as_ref().is_some_and(|key| !is_shared(&app.view, key)) { windowed.chain(set_floating(app, None)) } else { windowed };
    // Only the newest frame per tile is kept; the window uploads it when it draws.
    for (key, snapshot) in frames {
        app.sent += 1;
        let tile = app.tiles.entry(key).or_default();
        tile.shown_at.get_or_insert(now);
        tile.frame = Some(snapshot);
        tile.version = app.sent;
    }
    unfocus.chain(windowed)
}

/// The web's Picture-in-Picture: the room window itself shrinks to `key`'s video, on top of other
/// windows, and comes back to the room's size with None.
fn set_floating(app: &mut App, key: Option<TileKey>) -> Task<Message> {
    let was = app.floating.is_some();
    let windowed = set_fullscreen(app, None);
    app.floating = key;
    match (was, app.floating.is_some()) {
        (false, true) => windowed.chain(with_window(|id| {
            window::set_level(id, window::Level::AlwaysOnTop).chain(window::resize(id, Size::new(480.0, 270.0)))
        })),
        (true, false) => windowed.chain(with_window(|id| {
            window::set_level(id, window::Level::Normal).chain(window::resize(id, Size::new(1280.0, 780.0))).chain(window::gain_focus(id))
        })),
        _ => windowed,
    }
}

/// The window follows `app.fullscreen`; leaving it only touches a window that was fullscreen.
fn set_fullscreen(app: &mut App, key: Option<TileKey>) -> Task<Message> {
    let was = app.fullscreen.is_some();
    let mode = if key.is_some() { window::Mode::Fullscreen } else { window::Mode::Windowed };
    app.fullscreen = key;
    if was == app.fullscreen.is_some() {
        return Task::none();
    }
    window::oldest().then(move |id| id.map_or_else(Task::none, |id| window::set_mode(id, mode)))
}

/// The web's ConnectionQuality: what each screen tile shows. Our own share's limit comes from the
/// session (`local_share`, which the room hears of too); a remote screen shows its sharer's report
/// (`share_reports`) or else what our inbound counters say.
#[derive(Default)]
struct Quality {
    seen: u64,
    /// Keyed by (peer, track), so a new receiver starts from fresh counters.
    receives: HashMap<(String, String), quality::ReceiveSample>,
    signals: HashMap<TileKey, quality::Signal>,
    /// The problem a tile shows, its label and the explanation its chip gives.
    shown: HashMap<TileKey, (quality::Problem, String, String)>,
    /// The encoder our screen last went out on (outbound-rtp encoderImplementation), for share_ended.
    encoder: Option<String>,
}

impl Quality {
    /// A new `screenQuality`: our inbound counters move each remote screen's signal along.
    fn read(&mut self, view: &SessionView, surface: &desktop_native::VideoFrames) {
        let empty = serde_json::Map::new();
        let peers = view.screen_quality.as_object().unwrap_or(&empty);
        let mut measured: HashMap<TileKey, Option<quality::Problem>> = HashMap::new();
        let mut receives = HashMap::new();
        for value in peers.values() {
            if let Some(encoder) = value.get("send").and_then(|send| send.get("encoder")).and_then(serde_json::Value::as_str).filter(|name| !name.is_empty()) {
                self.encoder = Some(encoder.to_owned());
            }
        }
        for (peer, value) in peers {
            for inbound in value.get("receive").and_then(serde_json::Value::as_array).into_iter().flatten() {
                let (Some(track), Some(sample)) = (inbound.get("track").and_then(serde_json::Value::as_str), quality::receive_sample(inbound)) else { continue };
                if surface.kind(peer, track, view.screen_sharers.contains(peer), view.camera_sharers.contains(peer)) != Some(false) {
                    continue;
                }
                let key = (peer.clone(), track.to_owned());
                let problem = self.receives.get(&key).and_then(|previous| quality::receive_problem(*previous, sample));
                receives.insert(key, sample);
                // A re-share leaves the old receiver idle next to the new one: the worst counts.
                let entry = measured.entry((peer.clone(), false)).or_default();
                *entry = [*entry, problem].into_iter().flatten().min();
            }
        }
        self.receives = receives;
        self.signals.retain(|key, _| measured.contains_key(key));
        for (key, problem) in measured {
            self.signals.entry(key).or_default().update(problem);
        }
    }

    /// Every tick: a sharer's report or our own share's limit can change between reads.
    fn refresh(&mut self, view: &SessionView) {
        let mut wanted: HashMap<TileKey, (quality::Problem, String, usize)> = HashMap::new();
        if let (Some(local), Some((problem, viewers))) = (&view.local_socket_id, view.local_share) {
            wanted.insert((local.clone(), false), (problem, String::new(), viewers));
        }
        for peer in &view.screen_sharers {
            let report = view.share_reports.get(peer).and_then(|problem| quality::Problem::from_peer(problem));
            let Some(problem) = report.or_else(|| self.signals.get(&(peer.clone(), false)).and_then(|signal| signal.shown)) else { continue };
            let name = view.participants.iter().find(|p| &p.socket_id == peer).map_or(peer.as_str(), |p| p.username.as_str());
            // Everyone but the sharer watches: the participants without them, us included.
            wanted.insert((peer.clone(), false), (problem, name.to_owned(), view.participants.len().saturating_sub(1)));
        }
        // A tile that stopped reporting must not keep a stale chip.
        self.shown.retain(|key, _| wanted.contains_key(key));
        for (key, (problem, name, viewers)) in wanted {
            if self.shown.get(&key).is_some_and(|(was, ..)| *was == problem) {
                continue;
            }
            // quality_problem: which problem, never who or where.
            desktop_native::metrics::send("quality_problem", &[("problem", format!("{problem:?}"))]);
            self.shown.insert(key, (problem, problem.label(&name), problem.explanation(&name, viewers)));
        }
    }
}

/// The web's share_ended: how long, why, and which encoder carried it.
fn share_ended(app: &mut App, started: Instant, reason: &str) {
    let encoder = app.quality.encoder.clone().unwrap_or_else(|| "unknown".into());
    desktop_native::metrics::send("share_ended", &[("minutes", desktop_native::metrics::minutes_bucket(started.elapsed()).into()), ("reason", reason.into()), ("encoder", encoder)]);
}

/// Usage metrics read off the view each tick (desktop_native::metrics): share_started/ended,
/// room_joined with the room's size, and reconnected after a lost socket id.
fn usage_metrics(app: &mut App) {
    use desktop_native::metrics::{people_bucket, seconds_bucket, send};
    match (app.view.screen_state == "active", app.share_started_at) {
        (true, None) => {
            app.share_started_at = Some(Instant::now());
            let (source, audio) = app.last_share.as_ref().map_or(("unknown", "NONE"), |(source, audio)| {
                let window = source.kind == desktop_native::sources::SourceKind::Window;
                (if window { "window" } else { "screen" }, if !*audio { "NONE" } else if window { "PROCESS" } else { "SYSTEM" })
            });
            let preset = SharePreset::ALL.into_iter().find(|preset| preset.profile() == app.profile).map_or("CUSTOM".to_owned(), |preset| format!("{preset:?}").to_uppercase());
            send("share_started", &[("source", source.into()), ("preset", preset), ("audio", audio.into())]);
            app.shared_exe = app.last_share.as_ref().filter(|(source, _)| source.kind == desktop_native::sources::SourceKind::Window).and_then(|(source, _)| desktop_native::sources::exe_names().remove(&source.pid));
            app.follow = None;
        }
        (false, Some(started)) => {
            app.share_started_at = None;
            let reason = if std::mem::take(&mut app.stop_requested) { "user" } else { "ended" };
            share_ended(app, started, reason);
            let closed = reason == "ended" && app.view.screen_error.as_deref() == Some("window-closed");
            app.follow = app.shared_exe.take().filter(|_| closed).map(|exe| Follow { exe, since: Instant::now(), found: None });
        }
        _ => {}
    }
    if app.view.local_socket_id.is_some() {
        if let Some(lost) = app.lost_at.take() {
            send("reconnected", &[("seconds", seconds_bucket(lost.elapsed()).into())]);
        }
        if app.view.camera_state == "active" {
            feature_used(app, "camera");
        }
        if !app.room_sounds {
            feature_used(app, "sounds_off");
        }
        if !app.room_reported && !app.view.participants.is_empty() {
            app.room_reported = true;
            send("room_joined", &[("people", people_bucket(app.view.participants.len()).into())]);
        }
    } else if app.joined && app.lost_at.is_none() {
        app.lost_at = Some(Instant::now());
    }
}

fn set_share_border(on: bool) {
    #[cfg(windows)]
    desktop_capture::video::WINDOW_BORDER.store(on, std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(windows))]
    let _ = on;
}

/// The web's feature_used: the first use of a feature in this room, by its name only.
fn feature_used(app: &mut App, feature: &'static str) {
    if app.features_used.insert(feature) {
        desktop_native::metrics::send("feature_used", &[("feature", feature.into())]);
    }
}

/// Screens and cameras that are on, like the web .screen-tile list, ours (isLocal) included.
fn videos(view: &SessionView) -> Vec<TileKey> {
    let local = view.local_socket_id.as_deref();
    let own = local.into_iter().flat_map(|id| [(id.to_owned(), false), (id.to_owned(), true)]);
    view.screen_sharers
        .iter()
        .map(|peer| (peer.clone(), false))
        .chain(view.camera_sharers.iter().map(|peer| (peer.clone(), true)))
        .filter(|(peer, _)| Some(peer.as_str()) != local)
        .chain(own)
        .filter(|key| is_shared(view, key))
        .collect()
}

/// How far `peer`'s ring is toward "speaking", 0 to 1: it eases instead of blinking with each read.
pub fn speaking(app: &App, peer: &str) -> f32 {
    app.speaking_since.get(peer).map_or(0.0, |(on, at)| {
        let t = ui::motion::progress(*at, ui::motion::POP);
        if *on { t } else { 1.0 - t }
    })
}

/// Something is still moving, so the window redraws at 60 Hz (ui::motion).
fn animating(app: &App) -> bool {
    use ui::motion::{ENTER, ICON, LEAVE, POP, ROOM, SLIDE, running};
    let moving = app.flips.running() || ui::motion::hovers_running() || running(app.entered, ROOM) || app.dock_icons.iter().any(|(_, at)| running(*at, ICON)) || app.wizard.as_ref().is_some_and(Wizard::animating);
    if moving {
        return true;
    }
    let tiles = app.tiles.values().any(|tile| running(tile.shown_at, ENTER) || tile.leaving_since.is_some());
    let speaking = app.speaking_since.values().any(|(_, at)| running(Some(*at), POP));
    // The notice slides in, then fades out at the end of its 6 s.
    let notice = app.notice.as_ref().is_some_and(|(_, at)| running(Some(*at), ENTER) || (at.elapsed() > Duration::from_secs(6) - LEAVE && at.elapsed() < Duration::from_secs(6)));
    tiles || speaking || notice || [app.popover_at, app.menu_opened].into_iter().any(|at| running(at, POP)) || running(app.modal_at, ENTER) || [app.chat_toggled, app.rail_toggled].into_iter().any(|at| running(at, SLIDE))
}

/// A screen or camera that is still on: ours by the engine state, the others by the room's sharers.
pub fn is_shared(view: &SessionView, (peer, camera): &TileKey) -> bool {
    if Some(peer.as_str()) == view.local_socket_id.as_deref() {
        (if *camera { &view.camera_state } else { &view.screen_state }) == "active"
    } else if *camera {
        view.camera_sharers.contains(peer)
    } else {
        view.screen_sharers.contains(peer)
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
    let thread_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = runtime.block_on(run_controlled_with_ui(
            &endpoint, &room, &name, &config, 24 * 3600, thread_stop, None, Some(ui),
        ));
        telemetry::info(&format!("session ended: {result:?}"));
        if let Err(error) = result {
            sentry::capture_message(&format!("session ended: {error}"), sentry::Level::Error);
        }
    });
    Running { stop, thread: Some(thread), chat, screen, camera, devices, audio, share_profile, view, surface }
}

fn view(app: &App) -> Element<'_, Message> {
    if app.compact {
        return ui::panel::view(app);
    }
    if let Some(key) = app.floating.as_ref().filter(|_| app.running.is_some()) {
        return room::floating(app, key);
    }
    let screen = if app.running.is_some() {
        // Like the web #app-screen fade-in: the room comes up from the window's background.
        let veil = 1.0 - app.entered.map_or(1.0, |at| ui::motion::progress(at, ui::motion::ROOM));
        ui::motion::veil(room::view(app), ui::theme::BG, veil, 0.0)
    } else {
        ui::join::view(app)
    };
    match &app.feedback {
        Some(draft) => iced::widget::stack![screen, ui::feedback::view(draft)].into(),
        None => screen,
    }
}

fn subscription(app: &App) -> Subscription<Message> {
    let close = window::close_requests().map(Message::CloseRequested);
    let keys = keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } => Some(Message::Key(key, modifiers)),
        _ => None,
    });
    let mut subscriptions = vec![close, keys];
    if app.running.is_some() {
        subscriptions.push(iced::event::listen_with(|event, _, _| match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(Message::Cursor(position)),
            _ => None,
        }));
    }
    // Tray clicks and a second launch's "show" request.
    subscriptions.push(iced::time::every(Duration::from_millis(100)).map(|_| Message::TrayPoll));
    subscriptions.push(iced::time::every(Duration::from_secs(6 * 3600)).map(|_| Message::CheckUpdate));
    if app.compact && app.visible {
        subscriptions.push(window::events().filter_map(|(_, event)| matches!(event, window::Event::Unfocused).then_some(Message::PanelBlur)));
    }
    if app.running.is_some() {
        // Every update redraws the window: 60 a second only while a video plays in a shown room.
        let video = !app.tiles.is_empty() && app.visible && !app.compact;
        let fast = video || (app.visible && animating(app));
        subscriptions.push(iced::time::every(Duration::from_millis(if fast { 16 } else { 100 })).map(|_| Message::Tick));
    }
    if app.follow.is_some() {
        subscriptions.push(iced::time::every(Duration::from_secs(2)).map(|_| Message::FollowPoll));
    }
    if app.wizard.is_some() {
        // Like the web wizard: windows that open or close show up while it is open.
        subscriptions.push(iced::time::every(Duration::from_secs(2)).map(|_| Message::RefreshSources(false)));
    }
    Subscription::batch(subscriptions)
}

fn main() -> iced::Result {
    // Flushes what is queued when main returns.
    telemetry::redirect_stderr();
    let _telemetry = telemetry::init();
    telemetry::info(&format!("started {}", option_env!("SENTRY_RELEASE").unwrap_or(concat!("dev ", env!("CARGO_PKG_VERSION")))));
    updater::wait_for_replaced();
    if !startup::first_instance() {
        return Ok(());
    }
    iced::application(boot, update, view)
        .title("Screenshare")
        .theme(|_: &App| theme::theme())
        // body { font: 14px Barlow }
        .settings(iced::Settings { default_font: theme::FONT, default_text_size: 14.into(), ..Default::default() })
        .font(theme::BARLOW)
        .font(theme::BARLOW_BOLD)
        .subscription(subscription)
        .window(window::Settings {
            size: iced::Size::new(1280.0, 780.0),
            // A login launch starts in the tray; boot shows the window when there is nothing to rejoin.
            visible: !startup::launched_at_login(),
            icon: tray::icon_rgba().and_then(|(rgba, width, height)| window::icon::from_rgba(rgba, width, height).ok()),
            exit_on_close_request: false,
            ..Default::default()
        })
        .run()
}
