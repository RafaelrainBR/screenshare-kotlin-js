// Screenshare native desktop: iced UI over the session and libwebrtc engine.
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Sender, channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use iced::{
    Element, Length, Subscription, Task,
    widget::{button, column, container, image, row, scrollable, text, text_input},
    window,
};
use desktop_native::{
    VideoSurface,
    session::{SessionView, UiConnection, run_controlled_with_ui},
};

#[derive(Debug, Clone)]
enum Message {
    Room(String),
    Name(String),
    Join,
    Draft(String),
    Send,
    Watch(String, bool),
    Tick,
    Allocated(u64, Option<image::Allocation>),
    CloseRequested(window::Id),
}

struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    chat: Sender<(u64, String)>,
    screen: Sender<(u64, Option<desktop_native::session::ScreenOptions>)>,
    // shortcut: SPIKE_SHARE test driver (start/stop cycles) until the F4 share wizard exists.
    share_plan: Option<(desktop_native::session::ScreenOptions, u32, Instant, bool, u32)>,
    video: Sender<(u64, Option<String>, bool)>,
    view: Arc<Mutex<SessionView>>,
    surface: VideoSurface,
}

impl Running {
    fn shutdown(&mut self) {
        let started = Instant::now();
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        eprintln!("shutdown took {} ms", started.elapsed().as_millis());
    }
}

struct App {
    room: String,
    name: String,
    draft: String,
    running: Option<Running>,
    view: SessionView,
    frame: Option<image::Allocation>,
    sent: u64,
    shown: u64,
    frame_size: (u32, u32),
    frames: u64,
    fps: f64,
    fps_window: (Instant, u64),
    auto_watch: bool,
}

fn boot() -> (App, Task<Message>) {
    // SPIKE_AUTO=1 entra e assiste ao primeiro participante sem cliques (medição automatizada).
    let auto = std::env::var_os("SPIKE_AUTO").is_some();
    let app = App {
        room: "spike".into(),
        name: "Nativo".into(),
        draft: String::new(),
        running: None,
        view: SessionView::default(),
        frame: None,
        sent: 0,
        shown: 0,
        frame_size: (0, 0),
        frames: 0,
        fps: 0.0,
        fps_window: (Instant::now(), 0),
        auto_watch: auto,
    };
    (app, if auto { Task::done(Message::Join) } else { Task::none() })
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Room(value) => app.room = value,
        Message::Name(value) => app.name = value,
        Message::Join if app.running.is_none() && !app.room.trim().is_empty() && !app.name.trim().is_empty() => {
            app.running = Some(start(app.room.trim().into(), app.name.trim().into()));
        }
        Message::Join => {}
        Message::Draft(value) => app.draft = value,
        Message::Send => {
            if let Some(running) = &app.running {
                if !app.draft.trim().is_empty() {
                    let _ = running.chat.send((app.view.generation, std::mem::take(&mut app.draft)));
                }
            }
        }
        Message::Watch(peer, camera) => {
            if let Some(running) = &app.running {
                let _ = running.video.send((app.view.generation, Some(peer), camera));
            }
        }
        Message::Tick => {
            let Some(running) = &mut app.running else { return Task::none() };
            app.view = running.view.lock().unwrap().clone();
            drive_spike_share(running, &app.view);
            if app.auto_watch {
                let local = app.view.local_socket_id.clone();
                if let Some(peer) = app.view.participants.iter().find(|p| local.is_some() && Some(&p.socket_id) != local.as_ref()) {
                    let _ = running.video.send((app.view.generation, Some(peer.socket_id.clone()), std::env::var_os("SPIKE_CAMERA").is_some()));
                    app.auto_watch = false;
                }
            }
            // Take ownership of the newest frame so its pixels move into the image handle without a copy.
            // iced 0.14 uploads images asynchronously: swap the shown frame once its GPU allocation is done,
            // letting uploads overlap and dropping any that finish after a newer one.
            let snapshot = running.surface.lock().unwrap().take();
            if let Some(snapshot) = snapshot {
                let snapshot = Arc::try_unwrap(snapshot).unwrap_or_else(|shared| crate_clone(&shared));
                app.frame_size = (snapshot.width, snapshot.height);
                app.sent += 1;
                let seq = app.sent;
                fps_update(app);
                return image::allocate(image::Handle::from_rgba(snapshot.width, snapshot.height, snapshot.rgba)).map(move |r| Message::Allocated(seq, r.ok()));
            }
            fps_update(app);
        }
        Message::Allocated(seq, allocation) => {
            if seq > app.shown && allocation.is_some() {
                app.shown = seq;
                app.frame = allocation;
                app.frames += 1;
            }
        }
        Message::CloseRequested(id) => {
            if let Some(running) = &mut app.running {
                running.shutdown();
            }
            return window::close(id).chain(iced::exit());
        }
    }
    Task::none()
}

fn fps_update(app: &mut App) {
    let elapsed = app.fps_window.0.elapsed();
    if elapsed >= Duration::from_secs(1) {
        app.fps = (app.frames - app.fps_window.1) as f64 / elapsed.as_secs_f64();
        app.fps_window = (Instant::now(), app.frames);
        eprintln!("fps {:.1} frames {} size {}x{} video {}", app.fps, app.frames, app.frame_size.0, app.frame_size.1, app.view.video_state);
    }
}

fn crate_clone(shared: &desktop_native::VideoFrameSnapshot) -> desktop_native::VideoFrameSnapshot {
    desktop_native::VideoFrameSnapshot {
        width: shared.width,
        height: shared.height,
        rgba: shared.rgba.clone(),
        peer_id: shared.peer_id.clone(),
    }
}

fn start(room: String, name: String) -> Running {
    let endpoint = std::env::var("SCREENSHARE_WS").unwrap_or_else(|_| "ws://localhost:8080/".into());
    // TURN credentials stay outside Git: point SCREENSHARE_ENGINE_CONFIG at a local JSON file.
    let config = std::env::var("SCREENSHARE_ENGINE_CONFIG")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| r#"{"schemaVersion":1,"iceServers":[],"iceTransportPolicy":"all"}"#.into());
    let stop = Arc::new(AtomicBool::new(false));
    let view = Arc::new(Mutex::new(SessionView::default()));
    let surface = VideoSurface::default();
    let (chat, chat_rx) = channel();
    let (video, video_rx) = channel();
    let (screen_tx, screen) = channel();
    let (_devices, devices) = channel();
    let (_audio, audio) = channel();
    let ui = UiConnection {
        chat: chat_rx,
        screen,
        devices,
        audio,
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
    Running { stop, thread: Some(thread), chat, screen: screen_tx, share_plan: spike_share(), video, view, surface }
}

fn view(app: &App) -> Element<'_, Message> {
    let Some(_) = &app.running else {
        return container(
            column![
                text("Entrar na sala").size(24),
                text_input("Sala", &app.room).on_input(Message::Room).on_submit(Message::Join),
                text_input("Nome", &app.name).on_input(Message::Name).on_submit(Message::Join),
                button("Entrar").on_press(Message::Join),
            ]
            .spacing(12)
            .max_width(360),
        )
        .center(Length::Fill)
        .into();
    };
    let local = app.view.local_socket_id.as_deref();
    let participants = app.view.participants.iter().fold(column![text("Participantes").size(18)].spacing(6), |list, p| {
        let label = text(if Some(p.socket_id.as_str()) == local { format!("{} (você)", p.username) } else { p.username.clone() });
        list.push(if Some(p.socket_id.as_str()) == local {
            row![label]
        } else {
            let mut entry = row![label].spacing(8);
            if app.view.screen_sharers.contains(&p.socket_id) {
                entry = entry.push(button("Tela").on_press(Message::Watch(p.socket_id.clone(), false)));
            }
            if app.view.camera_sharers.contains(&p.socket_id) {
                entry = entry.push(button("Câmera").on_press(Message::Watch(p.socket_id.clone(), true)));
            }
            entry
        })
    });
    let messages = app.view.messages.iter().fold(column![].spacing(4), |list, m| {
        list.push(text(format!("{}: {}", m.username, m.content)))
    });
    let chat = column![
        text("Chat").size(18),
        scrollable(messages).height(Length::Fill).anchor_bottom(),
        text_input("Mensagem", &app.draft).on_input(Message::Draft).on_submit(Message::Send),
    ]
    .spacing(8)
    .width(320);
    let status = format!(
        "{} · vídeo {} · {}x{} · {:.1} fps · {} frames{}",
        if local.is_some() { "conectado" } else { "conectando" },
        if app.view.video_state.is_empty() { "-" } else { &app.view.video_state },
        app.frame_size.0,
        app.frame_size.1,
        app.fps,
        app.frames,
        app.view.error.as_deref().map(|e| format!(" · erro: {e}")).unwrap_or_default(),
    );
    let video: Element<'_, Message> = match &app.frame {
        Some(allocation) => image(allocation.handle().clone()).width(Length::Fill).height(Length::Fill).into(),
        None => container(text("Sem vídeo")).center(Length::Fill).into(),
    };
    column![
        text(status),
        row![participants.width(200), container(video).width(Length::Fill).height(Length::Fill), chat].spacing(12),
    ]
    .spacing(8)
    .padding(12)
    .into()
}

fn subscription(app: &App) -> Subscription<Message> {
    let close = window::close_requests().map(Message::CloseRequested);
    if app.running.is_some() {
        Subscription::batch([close, iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick)])
    } else {
        close
    }
}

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title("Screenshare nativo (spike iced)")
        .subscription(subscription)
        .exit_on_close_request(false)
        .window_size((1400.0, 820.0))
        .run()
}

/// SPIKE_SHARE="window,<hwnd>,<pid>,<none|application|computer>" or "screen,<left>,<top>,<w>,<h>,<audio>";
/// SPIKE_PRESET=game|video|text; SPIKE_CYCLES=N starts and stops the share N times (8 s on, 3 s off).
fn spike_share() -> Option<(desktop_native::session::ScreenOptions, u32, Instant, bool, u32)> {
    use desktop_native::{ScreenAudioMode, ShareSource, share::SharePreset};
    let spec = std::env::var("SPIKE_SHARE").ok()?;
    let parts: Vec<&str> = spec.split(',').collect();
    let num = |i: usize| parts.get(i).and_then(|v| v.parse::<i64>().ok());
    let (source, audio) = match parts.first().copied()? {
        "window" => (ShareSource::Window { hwnd: num(1)? as u64, pid: num(2)? as u32 }, parts.get(3)),
        _ => (ShareSource::Screen { left: num(1)? as i32, top: num(2)? as i32, width: num(3)? as u32, height: num(4)? as u32 }, parts.get(5)),
    };
    let audio_mode = match audio.copied() {
        Some("application") => ScreenAudioMode::Application,
        Some("computer") => ScreenAudioMode::Computer,
        _ => ScreenAudioMode::None,
    };
    let preset = match std::env::var("SPIKE_PRESET").as_deref() {
        Ok("game") => SharePreset::Game,
        Ok("text") => SharePreset::Text,
        _ => SharePreset::Video,
    };
    let cycles = std::env::var("SPIKE_CYCLES").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let options = desktop_native::session::ScreenOptions { source, profile: preset.profile(), height: 1080, audio_mode };
    Some((options, cycles, Instant::now(), false, 0))
}

fn drive_spike_share(running: &mut Running, view: &SessionView) {
    let Some((options, cycles, since, sharing, done)) = &mut running.share_plan else { return };
    if view.local_socket_id.is_none() || *done >= *cycles {
        return;
    }
    let on = std::env::var("SPIKE_ON").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let wait = Duration::from_secs(if *sharing { on } else { 3 });
    if since.elapsed() < wait {
        return;
    }
    *sharing = !*sharing;
    if !*sharing {
        *done += 1;
    }
    eprintln!("spike share {} (cycle {})", if *sharing { "start" } else { "stop" }, *done + 1);
    let _ = running.screen.send((view.generation, sharing.then(|| options.clone())));
    *since = Instant::now();
}
