// Screenshare native desktop: iced UI over the session and libwebrtc engine.
mod startup;
mod tray;
mod ui;

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
    session::{AudioControl, ScreenOptions, SessionView, UiConnection, run_controlled_with_ui},
    share::{SharePreset, ShareProfile},
};
use iced::{
    Element, Length, Point, Size, Subscription, Task, keyboard,
    widget::{button, checkbox, column, container, image, operation, text, text_input},
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
    Sources(Vec<desktop_capture::sources::Source>),
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
    /// The ⋯ menu of a participant (and of the clicked video, if any), or None to close it.
    PeerOptions(Option<(String, Option<bool>)>),
    TogglePeerMute(String),
    ToggleHideVideo(TileKey),
    StartShare,
    StopShare,
    Key(keyboard::Key, keyboard::Modifiers),
    Tick,
    Allocated(TileKey, u64, Option<image::Allocation>),
    /// Click on a video: show it alone, or back to the grid.
    Focus(TileKey),
    /// The tile's "Tela cheia" button: that video alone over the whole monitor, or back.
    Fullscreen(Option<TileKey>),
    /// "Ocultar conversa" / "Mostrar conversa".
    ToggleChat,
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

#[derive(Default)]
pub struct Tile {
    pub frame: Option<image::Allocation>,
    shown: u64,
}

#[derive(Default)]
pub struct App {
    room: String,
    name: String,
    draft: String,
    running: Option<Running>,
    view: SessionView,
    wizard: Option<Wizard>,
    /// Newest uploaded frame per remote video, like the web screen tiles.
    tiles: HashMap<TileKey, Tile>,
    /// The video shown alone (web focus mode); the others go to the rail.
    focused: Option<TileKey>,
    sent: u64,
    mic_device: Option<String>,
    output_device: Option<String>,
    /// A device picked while that audio was on: turn it off, then back on with the new device.
    mic_restart: bool,
    output_restart: bool,
    devices_requested: bool,
    /// Toast like the web ui-notice: the text and when it appeared; it hides after 6 s.
    notice: Option<(String, Instant)>,
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
    autostart: bool,
    /// Without a tray (Explorer not running) closing the window quits.
    has_tray: bool,
    camera_device: Option<String>,
    /// A camera picked while on: started again once the old one has ended.
    camera_restart: bool,
    /// The camera stepped out for a new share and returns once the share is up.
    camera_after_share: bool,
    peer_options: Option<(String, Option<bool>)>,
    /// "Silenciar áudio para você" and "Ocultar vídeo para você": this client only, this session only.
    muted_peers: HashSet<String>,
    hidden_videos: HashSet<TileKey>,
    fullscreen: Option<TileKey>,
    chat_hidden: bool,
    /// "Há uma mensagem não enviada…" is open.
    confirm_leave: bool,
    /// The window is the tray panel (Tauri's compact mode), not the room.
    compact: bool,
    /// Shown, as far as the tray toggle is concerned.
    visible: bool,
    panel_shown: Option<Instant>,
    panel_blurred: Option<Instant>,
}

const NAME_INPUT: &str = "name-input";

fn boot() -> (App, Task<Message>) {
    let last = startup::last_room();
    let mut app = App {
        visible: !startup::launched_at_login(),
        name: if last.name.trim().is_empty() { whoami() } else { last.name },
        room: last.room,
        profile: ui::profile::load(),
        autostart: startup::autostart_enabled(),
        has_tray: tray::install(),
        ..Default::default()
    };
    // Like the desktop web client: with autostart on and a saved room, every launch rejoins it.
    let rejoin = app.autostart && !app.room.trim().is_empty();
    let mut tasks = vec![operation::focus(NAME_INPUT)];
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
    let was_compact = std::mem::take(&mut app.compact);
    // Tauri centers it on its monitor; the cursor is on the one the user just clicked.
    let (cx, cy) = ui::panel::cursor();
    let area = was_compact.then(|| ui::panel::work_area(cx, cy)).flatten();
    with_window(move |id| {
        let full = if was_compact {
            let frame = window::toggle_decorations(id).chain(window::set_resizable(id, true)).chain(window::set_level(id, window::Level::Normal));
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
        full.chain(window::minimize(id, false)).chain(window::set_mode(id, window::Mode::Windowed)).chain(window::gain_focus(id))
    })
}

/// Runs a window task on the app's only window.
fn with_window(task: impl Fn(window::Id) -> Task<Message> + Send + 'static) -> Task<Message> {
    window::oldest().then(move |id| id.map_or_else(Task::none, &task))
}

/// Tauri's present_window(compact): frameless, fixed, on top, 430×660 by the tray icon.
/// shortcut: the panel still shows on the taskbar (iced has no skip-taskbar); set WS_EX_TOOLWINDOW if it bothers.
fn open_panel(app: &mut App, (x, y, w, h): (f64, f64, f64, f64)) -> Task<Message> {
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
            let Some(area) = area else { return Task::none() };
            let s = f64::from(scale);
            let (px, py) = ui::panel::position((x, y, w, h), (f64::from(width) * s, f64::from(height) * s), area);
            window::move_to(id, Point::new((px / s) as f32, (py / s) as f32))
        });
        frame
            .chain(window::set_resizable(id, false))
            .chain(window::set_level(id, window::Level::AlwaysOnTop))
            .chain(window::resize(id, Size::new(width, height)))
            .chain(place)
            .chain(window::set_mode(id, window::Mode::Windowed))
            .chain(window::gain_focus(id))
    }))
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
        tray::Action::Leave => update(app, Message::Leave),
        tray::Action::Quit => {
            app.running = None;
            iced::exit()
        }
    }
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
    }
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
            app.room = match room_code(&app.room) {
                "" => format!("{:08x}", rand::random::<u32>()),
                code => code.into(),
            };
            startup::save_last_room(app.name.trim(), &app.room);
            app.running = Some(start(app.room.clone(), app.name.trim().into()));
            return operation::focus(room::CHAT_INPUT);
        }
        Message::Join => {}
        Message::Leave if !app.draft.trim().is_empty() && !app.confirm_leave => {
            app.confirm_leave = true;
            // The tray's "Sair da sala" may come with the window hidden; unfocusing keeps Enter off the chat.
            return set_fullscreen(app, None).chain(show_window(app)).chain(operation::focus("leave-confirm"));
        }
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
            // Dropping Running joins the session thread: the engine stops the share and closes peers.
            let windowed = set_fullscreen(app, None);
            let App { name, mic_device, output_device, camera_device, profile, advanced, has_tray, notice, chat_hidden, compact, visible, .. } = std::mem::take(app);
            *app = App { name, mic_device, output_device, camera_device, profile, advanced, has_tray, notice, chat_hidden, compact, visible, ..Default::default() };
            tray::update(tray_state(app));
            return windowed.chain(operation::focus(NAME_INPUT));
        }
        Message::ToggleAutostart(enabled) => match startup::set_autostart(enabled) {
            Ok(()) => app.autostart = enabled,
            Err(error) => app.notice = Some((error, Instant::now())),
        },
        Message::TrayPoll => {
            let mut tasks: Vec<_> = tray::actions().into_iter().map(|action| tray_action(app, action)).collect();
            if startup::show_requested() {
                tasks.push(show_window(app));
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
        Message::ToggleOutput => audio(app, AudioControl::Output { enabled: !app.view.output_enabled, device_id: app.output_device.clone() }),
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
            if app.view.output_enabled {
                app.output_restart = true;
                audio(app, AudioControl::Output { enabled: false, device_id: None });
            }
        }
        Message::OpenWizard => {
            app.share_menu = None;
            app.wizard = Some(Wizard::open());
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
                wizard.thumbs.insert(id, handle);
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
            app.share_menu = page;
            app.device_picker = None;
        }
        Message::DevicePicker(picker) => {
            app.device_picker = picker;
            app.share_menu = None;
        }
        Message::ToggleAdvanced => app.advanced = !app.advanced,
        Message::PeerOptions(target) => app.peer_options = target,
        Message::TogglePeerMute(peer) => {
            let muted = app.muted_peers.insert(peer.clone()) || !app.muted_peers.remove(&peer);
            audio(app, AudioControl::Peer { peer_id: peer, muted });
        }
        Message::ToggleHideVideo(key) => {
            if !app.hidden_videos.insert(key.clone()) {
                app.hidden_videos.remove(&key);
            }
        }
        Message::StartShare => {
            if let (Some(running), Some(options)) = (&app.running, app.wizard.as_ref().and_then(|w| w.options(app.profile))) {
                app.share_height = options.height;
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
                    let _ = running.screen.send((app.view.generation, Some(options)));
                }
                app.wizard = None;
                return operation::focus(room::CHAT_INPUT);
            }
        }
        Message::StopShare => {
            if let Some(running) = &app.running {
                let _ = running.screen.send((app.view.generation, None));
            }
        }
        Message::Key(key, modifiers) => return shortcut(app, key, modifiers),
        Message::Tick => return tick(app),
        Message::Allocated(key, seq, Some(allocation)) => {
            if is_shared(&app.view, &key) {
                let tile = app.tiles.entry(key).or_default();
                // Uploads finish out of order; keep the newest.
                if seq > tile.shown {
                    tile.shown = seq;
                    tile.frame = Some(allocation);
                }
            }
        }
        Message::Allocated(..) => {}
        Message::Focus(key) => app.focused = (app.focused.as_ref() != Some(&key)).then_some(key),
        Message::Fullscreen(key) => return set_fullscreen(app, key),
        Message::ToggleChat => app.chat_hidden = !app.chat_hidden,
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
            app.notice = Some(("Código copiado".into(), Instant::now()));
            return iced::clipboard::write(app.room.clone());
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
        (Key::Named(Named::Escape), _) if app.confirm_leave => Message::CancelLeave,
        (Key::Named(Named::Enter), _) if app.confirm_leave => Message::Leave,
        (Key::Named(Named::Escape), Some(_)) => Message::CloseWizard,
        (Key::Named(Named::Escape), None) if app.peer_options.is_some() => Message::PeerOptions(None),
        (Key::Named(Named::Escape), None) if app.fullscreen.is_some() => Message::Fullscreen(None),
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

fn audio(app: &mut App, control: AudioControl) {
    if let Some(running) = &app.running {
        let _ = running.audio.send((app.view.generation, control));
    }
}

fn tick(app: &mut App) -> Task<Message> {
    let Some(running) = &app.running else { return Task::none() };
    app.view = running.view.lock().unwrap().clone();
    app.joined |= app.view.local_socket_id.is_some();
    let errors = room::errors(app).join(" · ");
    if errors != app.shown_errors {
        if !errors.is_empty() {
            app.notice = Some((errors.clone(), Instant::now()));
        }
        app.shown_errors = errors;
    }
    if app.view.local_socket_id.is_some() && !app.devices_requested {
        app.devices_requested = true;
        let _ = running.devices.send(app.view.generation);
    }
    if !sharing(&app.view) {
        if let Some(options) = app.pending_share.take() {
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
    // Take ownership of the newest frames so their pixels move into the image handles without a copy.
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
    app.tiles.retain(|key, _| is_shared(view, key));
    if app.focused.as_ref().is_some_and(|key| !is_shared(view, key)) {
        app.focused = None;
    }
    // Like the browser leaving fullscreen when the video element goes away.
    let windowed = if app.fullscreen.as_ref().is_some_and(|key| !is_shared(view, key)) { set_fullscreen(app, None) } else { Task::none() };
    // iced uploads images asynchronously: each tile swaps once its upload is done, dropping late ones.
    windowed.chain(Task::batch(frames.into_iter().map(|(key, snapshot)| {
        let snapshot = Arc::try_unwrap(snapshot).unwrap_or_else(|shared| (*shared).clone());
        app.sent += 1;
        let seq = app.sent;
        image::allocate(image::Handle::from_rgba(snapshot.width, snapshot.height, snapshot.rgba)).map(move |r| Message::Allocated(key.clone(), seq, r.ok()))
    })))
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
        eprintln!("session ended: {result:?}");
    });
    Running { stop, thread: Some(thread), chat, screen, camera, devices, audio, share_profile, view, surface }
}

fn view(app: &App) -> Element<'_, Message> {
    if app.compact {
        return ui::panel::view(app);
    }
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
            checkbox(app.autostart).label("Iniciar com o sistema e voltar a esta sala").on_toggle(Message::ToggleAutostart).text_size(13),
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
    // Tray clicks and a second launch's "show" request.
    subscriptions.push(iced::time::every(Duration::from_millis(100)).map(|_| Message::TrayPoll));
    if app.compact && app.visible {
        subscriptions.push(window::events().filter_map(|(_, event)| matches!(event, window::Event::Unfocused).then_some(Message::PanelBlur)));
    }
    if app.running.is_some() {
        subscriptions.push(iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick));
    }
    if app.wizard.is_some() {
        // Like the web wizard: windows that open or close show up while it is open.
        subscriptions.push(iced::time::every(Duration::from_secs(2)).map(|_| Message::RefreshSources(false)));
    }
    Subscription::batch(subscriptions)
}

fn main() -> iced::Result {
    if !startup::first_instance() {
        return Ok(());
    }
    iced::application(boot, update, view)
        .title("Screenshare")
        .theme(|_: &App| theme::theme())
        .font(theme::BARLOW)
        .font(theme::BARLOW_BOLD)
        .default_font(theme::FONT)
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
