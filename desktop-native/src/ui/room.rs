//! Room screen: the stage with its rail, the chat and the dock, laid out like the UI v4.
use std::time::Duration;

use desktop_native::session::{AudioDevice, Participant};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, hover, mouse_area, opaque, row, scrollable, stack, text, text_input},
};

use super::{dock, motion, theme};
use crate::{App, Message, TileKey, awake, is_shared};

pub const CHAT_INPUT: &str = "chat-input";
const CHEVRON_RIGHT: &str = r#"<path d="m9 5 7 7-7 7"/>"#;
const CHEVRON_LEFT: &str = r#"<path d="m15 5-7 7 7 7"/>"#;
const CHEVRON_UP: &str = r#"<path d="m6 15 6-6 6 6"/>"#;
const COPY: &str = r#"<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/>"#;
const DOTS: &str = r#"<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>"#;
// The web .screen-tile-controls glyph, and the same corners pointing in to leave.
const FULLSCREEN: &str = r#"<path d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"/>"#;
const EXIT_FULLSCREEN: &str = r#"<path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5"/>"#;
// The web .glass-chip "Janela flutuante": a small window in the corner of a big one.
const FLOAT: &str = r#"<rect x="3" y="5" width="18" height="14" rx="2"/><rect x="12" y="12" width="7" height="5" rx="1"/>"#;
// The web .quality-chip triangle.
const WARNING: &str = r#"<path d="M12 3 2 20h20L12 3z"/><path d="M12 10v4M12 17h.01"/>"#;

/// pick_list entry; "Padrão" (the system default) is None.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device(pub Option<String>, pub String);

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

pub fn devices(list: &[AudioDevice]) -> Vec<Device> {
    std::iter::once(Device(None, "Padrão".into()))
        .chain(list.iter().map(|d| Device(Some(d.id.clone()), d.label.clone())))
        .collect()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let view = &app.view;
    let local = view.local_socket_id.as_deref();

    // Screens and cameras that are on, and the ones fading out in their slot (crate::tick).
    let videos: Vec<TileKey> = app
        .video_order
        .iter()
        .filter(|key| is_shared(view, key) || app.tiles.get(*key).is_some_and(|tile| tile.leaving_since.is_some()))
        .cloned()
        .collect();
    let focused = app.focused.as_ref().filter(|key| videos.contains(key));
    // A tile in or out, or one focused: the tiles that stay slide to their new place (motion::flip).
    // Videos are keyed (peer, Some(camera)), the rail's avatars (peer, None).
    let staged: Vec<(String, Option<bool>)> = videos
        .iter()
        .map(|(peer, camera)| (peer.clone(), Some(*camera)))
        .chain(view.participants.iter().filter(|p| !videos.iter().any(|(peer, _)| *peer == p.socket_id)).map(|p| (p.socket_id.clone(), None)))
        .collect();
    let mut arrangement = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(&(&staged, focused), &mut arrangement);
    app.flips.arrange(&staged, std::hash::Hasher::finish(&arrangement));
    let main: Element<'_, Message> = match (videos.len(), focused) {
        (0, _) => container(
            column![
                text("Nenhuma tela compartilhada").font(theme::BOLD),
                text("Compartilhe sua tela ou aguarde outra pessoa.").color(theme::DIM),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .style(theme::stage)
        .into(),
        (_, Some(key)) => video_tile(app, key, false),
        // .screens-main: an even grid, as square as the count allows.
        (count, None) => {
            let columns = (1..).find(|c| c * c >= count).unwrap_or(1);
            videos
                .chunks(columns)
                .fold(column![].spacing(8).height(Length::Fill), |grid, chunk| {
                    grid.push(chunk.iter().fold(row![].spacing(8).height(Length::Fill), |line, key| line.push(video_tile(app, key, false))))
                })
                .into()
        }
    };
    // .screens-rail: in focus mode the focused video and the others, then everyone without a video.
    let videos_in_rail = if focused.is_some() { videos.len() } else { 0 };
    let tiles = videos_in_rail + view.participants.iter().filter(|p| !videos.iter().any(|(peer, _)| *peer == p.socket_id)).count();
    // The layout closure outlives this function, so it gets its own copy of the list.
    let rail_videos = videos.clone();
    let build_rail = move || {
        let videos = &rail_videos;
        let mut rail = row![].spacing(8).align_y(Alignment::Center);
        if let Some(key) = focused {
            rail = rail.push(video_tile(app, key, true));
            for key in videos.iter().filter(|other| Some(*other) != focused) {
                rail = rail.push(video_tile(app, key, true));
            }
        }
        for p in view.participants.iter().filter(|p| !videos.iter().any(|(peer, _)| *peer == p.socket_id)) {
            let tile = avatar_tile(p, Some(p.socket_id.as_str()) == local, crate::speaking(app, &p.socket_id));
            rail = rail.push(motion::flip(tile, &app.flips, (p.socket_id.clone(), None)));
        }
        rail
    };
    // .strip-toggle hides the rail; the rail keeps 46 px on the right for it, as .screens-rail does.
    // Centered while it fits, scrolled sideways with a hidden bar once it does not (safe center, overflow-x).
    // Hiding folds it down to the toggle's 34 px strip and showing unfolds it, empty while it moves.
    let rail_moved = app.rail_toggled.map_or(1.0, |at| motion::progress(at, motion::SLIDE));
    let rail_open = if app.rail_hidden { 1.0 - rail_moved } else { rail_moved };
    let rail: Element<'_, Message> = if app.rail_hidden || rail_moved < 1.0 {
        iced::widget::space().height(34.0 + (110.0 - 34.0) * rail_open).into()
    } else {
        let width = tiles as f32 * (168.0 + 8.0) - 8.0;
        let fitted = iced::widget::responsive(move |size| {
            if width <= size.width {
                container(build_rail()).center_x(Length::Fill).into()
            } else {
                let hidden = scrollable::Scrollbar::new().width(0).scroller_width(0);
                scrollable(build_rail()).direction(scrollable::Direction::Horizontal(hidden)).width(Length::Fill).into()
            }
        });
        container(fitted).height(110).center_y(110).padding(iced::Padding { right: 46.0, ..iced::Padding::ZERO }).into()
    };
    let (paths, hint) = if app.rail_hidden { (CHEVRON_UP, "Mostrar prévias") } else { (dock::CHEVRON_DOWN, "Ocultar prévias") };
    let toggle = glass_button(paths, 16.0, 32.0, hint, Message::ToggleRail);
    let toggle = container(toggle)
        .padding(iced::Padding { right: 6.0, bottom: 2.0 + (40.0 - 2.0) * rail_open, ..iced::Padding::ZERO })
        .align_right(Length::Fill)
        .align_bottom(Length::Fill);
    let stage = stack![column![main, rail].spacing(8), toggle];

    // .chat: avatar, name and time over the bubble; ours on the right with the accent avatar.
    let messages = view.messages.iter().fold(column![].spacing(16), |list, m| {
        let own = m.sender_socket_id.is_some() && m.sender_socket_id.as_deref() == local;
        let avatar = container(text(initials(&m.username)).size(10).font(theme::BOLD).color(if own { theme::ON_ACCENT } else { theme::TEXT }))
            .center(26)
            .style(move |_| container::Style {
                background: Some(if own { theme::ACCENT } else { theme::SURFACE_2 }.into()),
                border: iced::Border { radius: 13.0.into(), ..Default::default() },
                ..Default::default()
            });
        let header = row![
            text(if own { "Você" } else { &m.username }).size(11).color(theme::DIM),
            text(local_time(m.timestamp)).size(10).font(iced::Font::MONOSPACE).color(theme::FAINT),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let side = if own { Alignment::End } else { Alignment::Start };
        let body = column![header, container(text(&m.content)).padding([7, 10]).style(theme::bubble(own))].spacing(3).width(Length::Fill).align_x(side);
        list.push(if own { row![body, avatar] } else { row![avatar, body] }.spacing(8))
    });
    // .conversation-header: the room code chip copies, like #copyRoomBtn.
    // A private room's 32 characters would push the header out, as #current-room-id's ellipsis.
    let shown = if app.room.chars().count() > 12 { format!("{}…", app.room.chars().take(8).collect::<String>()) } else { app.room.clone() };
    let code = row![
        text("SALA").size(9).color(theme::FAINT),
        text(shown).size(11).font(iced::Font::MONOSPACE).wrapping(text::Wrapping::None),
        dock::icon(COPY, 13.0, theme::TEXT),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let code = button(code).padding([5, 7]).style(|_, status| theme::chip_button(status)).on_press(Message::CopyRoom);
    let header = row![text("Conversa").size(13).font(theme::BOLD).width(Length::Fill), code, chat_button(CHEVRON_RIGHT, "Ocultar conversa", 27.0)]
        .spacing(6)
        .align_y(Alignment::Center);
    let composer = text_input("Mensagem", &app.draft)
        .id(CHAT_INPUT)
        .on_input(Message::Draft)
        .on_submit(Message::Send)
        .size(13)
        .padding([8, 12])
        .style(theme::input);
    // .sidebar: header, messages and composer read as one bordered box split by lines.
    let chat = container(
        column![
            container(header).padding([0, 8]).height(32).center_y(32),
            rule(),
            people(app),
            rule(),
            scrollable(container(messages).padding([15, 10])).height(Length::Fill).anchor_bottom(),
            rule(),
            container(composer).padding([0, 8]).height(54).center_y(54),
        ],
    )
    .style(theme::panel);

    let dock = dock::view(app);

    // .workspace.chat-collapsed: the stage takes the width and .chat-restore floats at its top right.
    let stage = container(stage).padding(10);
    let chat_moved = app.chat_toggled.map_or(1.0, |at| motion::progress(at, motion::SLIDE));
    let chat_padding = iced::Padding { top: 10.0, right: 10.0, bottom: 10.0, left: 0.0 };
    let body: Element<'_, Message> = if chat_moved < 1.0 {
        // The column slides open or shut as an empty panel; the conversation shows once it is open.
        let width = 300.0 * if app.chat_hidden { 1.0 - chat_moved } else { chat_moved };
        let slide = container(iced::widget::space()).width(Length::Fill).height(Length::Fill).style(theme::panel);
        row![stage.width(Length::Fill), container(slide).padding(chat_padding).width(width).height(Length::Fill).clip(true)].into()
    } else if app.chat_hidden {
        let restore = chat_button(CHEVRON_LEFT, "Mostrar conversa", 32.0);
        stack![stage, container(restore).padding(16).align_right(Length::Fill)].into()
    } else {
        row![stage.width(Length::Fill), container(chat).padding(chat_padding).width(300).height(Length::Fill)].into()
    };

    let notice = app.notice.as_ref().filter(|(_, since)| since.elapsed() < Duration::from_secs(6));
    // Like requestFullscreen on the .screen-tile: only that video, edge to edge; Esc or its button leaves.
    let screen: Element<'_, Message> = match app.fullscreen.as_ref().filter(|key| videos.contains(key)) {
        Some(key) => video_tile(app, key, false),
        // .dock: a 64 px bar under a line, on the page background.
        None => column![container(body).height(Length::Fill), rule(), container(dock).padding([8, 12]).height(63).center_y(63)].into(),
    };
    let mut layers = iced::widget::stack![screen];
    if let Some(popover) = dock::popover(app) {
        layers = layers.push(popover);
    }
    if let Some((peer, camera)) = &app.peer_options {
        // Opens over the stage, above the rail; a click outside or Esc closes it.
        let outside = mouse_area(container(text("")).width(Length::Fill).height(Length::Fill)).on_press(Message::PeerOptions(None));
        // At the click, kept 8 px inside the window (ui.js openContextMenu). The height is the rows it holds.
        let at = app.menu_at.unwrap_or_default();
        let rows = 1 + usize::from(camera.is_some());
        let remote = app.view.local_socket_id.as_deref() != Some(peer.as_str());
        // The volume label and slider add about 50 px for someone else.
        let height = 14.0 + 26.0 + rows as f32 * 33.0 + 30.0 + if remote { 50.0 } else { 0.0 };
        // Rises 4 px into place while its content fades in, like the web .context-menu.
        let shown = app.menu_opened.map_or(1.0, |at| motion::progress(at, motion::POP));
        let menu = iced::widget::responsive(move |size| {
            let x = at.x.min(size.width - 248.0 - 8.0).max(8.0);
            let y = at.y.min(size.height - height - 8.0).max(8.0) + 4.0 * (1.0 - shown);
            let menu = motion::veil(options_menu(app, peer, *camera), theme::SURFACE_2, 1.0 - shown, 11.0);
            column![iced::widget::space().height(y), row![iced::widget::space().width(x), menu]].into()
        });
        layers = layers.push(outside).push(menu);
    }
    // Modals: the backdrop dims in and the panel's content fades in over it.
    let opened = app.modal_at.map_or(1.0, |at| motion::progress(at, motion::ENTER));
    if let Some(wizard) = &app.wizard {
        // The web #quality-modal: a dimmed room with the panel on top; opaque keeps clicks off the room.
        let content = motion::veil(wizard.view(app.profile, app.advanced, app.share_border), theme::SURFACE, 1.0 - opened, 0.0);
        let panel = container(content).padding(24).max_width(540).height(Length::Fill).style(theme::modal);
        layers = layers.push(opaque(container(panel).center_x(Length::Fill).padding(18).style(theme::backdrop_at(opened))));
    }
    if let Some((_, help)) = app.permission {
        let panel = motion::veil(super::permission::view(help), theme::SURFACE, 1.0 - opened, 14.0);
        layers = layers.push(opaque(container(panel).center(Length::Fill).style(theme::backdrop_at(opened))));
    }
    if app.confirm_leave {
        // leaveRoom's window.confirm: Enter leaves, Esc stays.
        let buttons = row![
            button(text("Cancelar")).padding([8, 16]).style(button::secondary).on_press(Message::CancelLeave),
            button(text("Sair da sala").font(theme::BOLD)).padding([8, 16]).style(button::danger).on_press(Message::Leave),
        ]
        .spacing(8);
        let panel = column![
            text("Há uma mensagem não enviada. Sair da sala e descartar o rascunho?"),
            container(buttons).align_right(Length::Fill),
        ]
        .spacing(16);
        let panel = container(motion::veil(panel.into(), theme::SURFACE, 1.0 - opened, 0.0)).padding(20).max_width(420).style(theme::modal);
        layers = layers.push(opaque(container(panel).center(Length::Fill).style(theme::backdrop_at(opened))));
    }
    // A restarted game: one question, never an automatic share (crate::Follow).
    if let Some(source) = app.follow.as_ref().and_then(|follow| follow.found.as_ref()) {
        let question = text(format!("{} reabriu. Continuar compartilhando?", source.title)).size(13).width(Length::Fill);
        let buttons = row![
            button(text("Agora não").size(13)).style(button::text).on_press(Message::FollowDismiss),
            button(text("Compartilhar").size(13).font(theme::BOLD)).style(button::primary).on_press(Message::FollowAccept),
        ]
        .spacing(8);
        let banner = container(row![question, buttons].spacing(12).align_y(Alignment::Center)).padding([10, 14]).max_width(560).style(theme::modal);
        layers = layers.push(container(banner).center_x(Length::Fill).padding(iced::Padding { top: 64.0, right: 12.0, bottom: 12.0, left: 12.0 }));
    }
    // Floats over the top like the web .notice, without moving the layout; drops 8 px in, fades out at the end.
    if let Some((notice, since)) = notice {
        let arrived = motion::progress(*since, motion::ENTER);
        let left = Duration::from_secs(6).saturating_sub(since.elapsed()).as_secs_f32() / motion::LEAVE.as_secs_f32();
        let toast = container(text(notice.as_str()).size(12)).padding([12, 15]).max_width(560).style(theme::notice(arrived.min(left.min(1.0))));
        let top = 12.0 - 8.0 * (1.0 - arrived);
        layers = layers.push(container(toast).center_x(Length::Fill).padding(iced::Padding { top, right: 12.0, bottom: 12.0, left: 12.0 }));
    }
    layers.into()
}

/// Errors worth a toast. While reconnecting, the dock status says it instead of the raw socket error.
pub fn errors(app: &App) -> Vec<&str> {
    let view = &app.view;
    let reconnecting = view.local_socket_id.is_none() && app.joined;
    // Camera and microphone failures open the permission help instead.
    let audio = if app.mic_attempt { &None } else { &view.audio_error };
    [&view.error, &view.screen_error, audio, &view.devices_error]
        .into_iter()
        .skip(usize::from(reconnecting))
        .filter_map(|e| e.as_deref())
        .map(super::permission::plain)
        .collect()
}

pub fn status(app: &App) -> (String, iced::Color) {
    let view = &app.view;
    let ended = app.running.as_ref().is_some_and(|r| r.thread.as_ref().is_none_or(|t| t.is_finished()));
    if ended {
        (format!("Desconectado. {}", view.error.as_deref().unwrap_or("")).trim_end().into(), theme::RED)
    } else if view.local_socket_id.is_some() {
        (String::new(), theme::DIM)
    } else if app.joined {
        ("Sem conexão. Tentando reconectar…".into(), theme::RED)
    } else {
        ("Conectando à sala…".into(), theme::DIM)
    }
}

/// HH:MM in the macOS time zone, like toLocaleTimeString("pt-BR") in the web chat.
#[cfg(target_os = "macos")]
fn local_time(timestamp_ms: i64) -> String {
    let seconds = (timestamp_ms.max(0) / 1000) as libc::time_t;
    // SAFETY: localtime_r only writes the tm it is given.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&seconds, &mut tm) }.is_null() {
        return String::new();
    }
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

/// HH:MM in the Windows time zone, like toLocaleTimeString("pt-BR") in the web chat.
#[cfg(windows)]
fn local_time(timestamp_ms: i64) -> String {
    use windows::Win32::{
        Foundation::{FILETIME, SYSTEMTIME},
        System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime},
    };
    // FILETIME counts 100 ns since 1601.
    let ticks = (timestamp_ms.max(0) as u64) * 10_000 + 116_444_736_000_000_000;
    let file = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    let (mut utc, mut local) = (SYSTEMTIME::default(), SYSTEMTIME::default());
    // SAFETY: plain out-parameters on the stack.
    let ok = unsafe { FileTimeToSystemTime(&file, &mut utc).is_ok() && SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).is_ok() };
    if ok { format!("{:02}:{:02}", local.wHour, local.wMinute) } else { String::new() }
}

/// A remote screen or camera (.screen-tile); click shows it alone or goes back to the grid.
/// In the rail it is 168×94 with its owner's name, like .screens-rail .tile-name.
fn video_tile<'a>(app: &'a App, key: &TileKey, small: bool) -> Element<'a, Message> {
    let (peer, camera) = key;
    let name = app.view.participants.iter().find(|p| &p.socket_id == peer).map_or("…", |p| p.username.as_str());
    let content: Element<'a, Message> = match app.tiles.get(key).and_then(|tile| Some((tile.frame.clone()?, tile.version, tile))) {
        _ if app.hidden_videos.contains(key) => container(text("Vídeo oculto").color(theme::DIM)).center(Length::Fill).into(),
        Some((frame, version, tile)) => {
            let video = iced::widget::Shader::new(super::video::Video { key: key.clone(), frame, version }).width(Length::Fill).height(Length::Fill).into();
            // The first frame fades in over the tile, the last one out before the tile goes.
            let entering = tile.shown_at.map_or(0.0, |at| 1.0 - motion::progress(at, motion::ENTER));
            let leaving = tile.leaving_since.map_or(0.0, |at| motion::progress(at, motion::LEAVE));
            motion::veil(video, theme::VIDEO, entering.max(leaving), 0.0)
        }
        None => container(text(format!("Conectando à {} de {name}…", if *camera { "câmera" } else { "tela" })).size(12).color(theme::DIM))
            .center(Length::Fill)
            .into(),
    };
    let mut layers = stack![content];
    // Fullscreen is for watching: the name and the controls leave with the cursor, like the web's .awake.
    let shown = small || app.fullscreen.as_ref() != Some(key) || awake(app);
    if small {
        let chip = container(text(name).size(11)).padding([1, 6]).style(theme::name_chip);
        layers = layers.push(container(chip).padding(6).align_bottom(Length::Fill));
    } else if shown {
        // Whose screen or camera: bottom left, centred on the 34 px controls on the right; ours says Você.
        let own = app.view.local_socket_id.as_deref() == Some(peer.as_str());
        let label = text(if own { "Você" } else { name }).size(12).wrapping(text::Wrapping::None);
        let chip = container(label).padding([4, 9]).max_width(240).clip(true).style(theme::label_chip);
        layers = layers.push(container(chip).padding(iced::Padding { left: 12.0, bottom: 16.0, ..iced::Padding::ZERO }).align_bottom(Length::Fill));
    }
    // "Por que está travando?" (crate::Quality): the label, or only the triangle in the rail; a click explains.
    if let Some((problem, label, explanation)) = app.quality.shown.get(key) {
        let glyph = dock::icon(WARNING, if small { 13.0 } else { 16.0 }, theme::AMBER);
        let content: Element<'a, Message> = if small { glyph } else { row![glyph, text(label.as_str()).size(13).font(theme::BOLD)].spacing(6).align_y(Alignment::Center).into() };
        let chip = button(content).padding(if small { [3, 5] } else { [6, 10] }).style(theme::quality_chip).on_press(Message::Explain(explanation.clone()));
        let mut bar = row![chip].spacing(6).align_y(Alignment::Center);
        // The web's .quality-action: our own encoder at its limit, one frame-rate step lighter, live.
        let sending = matches!(problem, desktop_native::quality::Problem::SenderCpu | desktop_native::quality::Problem::SenderBandwidth);
        if let Some(fps) = [30, 15, 10].into_iter().find(|fps| *fps < app.profile.fps).filter(|_| sending && !small) {
            let lighter = crate::ShareProfile { fps, ..app.profile };
            bar = bar.push(button(text(format!("Usar {fps} qps")).size(13)).padding([6, 10]).style(|_, status| theme::glass(status)).on_press(Message::SetProfile(lighter)));
        }
        layers = layers.push(container(bar).padding(if small { 5 } else { 12 }));
    }
    // .screen-tile-controls: the ⋯ options in the corner.
    let mut controls = row![].spacing(6);
    if !small {
        let (paths, label) = if app.fullscreen.as_ref() == Some(key) { (EXIT_FULLSCREEN, "Sair da tela cheia") } else { (FULLSCREEN, "Tela cheia") };
        let target = (app.fullscreen.as_ref() != Some(key)).then(|| key.clone());
        controls = controls.push(glass_button(paths, 17.0, 34.0, label, Message::Fullscreen(target)));
        if app.view.local_socket_id.as_deref() != Some(peer.as_str()) {
            controls = controls.push(glass_button(FLOAT, 17.0, 34.0, "Janela flutuante", Message::Float(Some(key.clone()))));
        }
    }
    // The screen's .glass-chip volume: a speaker that mutes and a 78 px slider, on someone else's large screen tile.
    if !small && !*camera && app.view.local_socket_id.as_deref() != Some(peer.as_str()) {
        let level = crate::volume(app, peer, true);
        let (glyph, hint) = if level == 0 { (format!("{}{}", dock::OUTPUT, dock::SLASH), "Ativar áudio da tela") } else { (dock::OUTPUT.to_owned(), "Silenciar áudio da tela") };
        let toggle = button(dock::icon(&glyph, 17.0, theme::TEXT)).padding(0).style(button::text).on_press(Message::TogglePeerMute(peer.clone(), true));
        let owner = peer.clone();
        let slider = iced::widget::slider(0..=100u8, level, move |value| Message::Volume(owner.clone(), true, value)).width(78);
        let chip = container(row![dock::hint_on(toggle.into(), hint.into()), slider].spacing(6).align_y(Alignment::Center))
            .padding([0, 9])
            .height(34)
            .center_y(34)
            .style(|_| container::Style { background: Some(iced::Color { a: 0.92, ..theme::BG }.into()), border: iced::Border { color: theme::LINE_2, width: 1.0, radius: 8.0.into() }, ..Default::default() });
        controls = controls.push(chip);
    }
    let options = Message::PeerOptions(Some((peer.clone(), Some(*camera))));
    let options = if small { options_button(peer, Some(*camera)) } else { glass_button(DOTS, 17.0, 34.0, "Opções", options) };
    if shown {
        layers = layers.push(container(controls.push(options)).padding(if small { 5 } else { 12 }).align_right(Length::Fill).align_bottom(Length::Fill));
    }
    let focused = app.focused.as_ref() == Some(key);
    let speaking = crate::speaking(app, peer);
    let tile = button(layers).padding(1).style(move |_, _| theme::video_tile(focused, speaking)).on_press(Message::Focus(key.clone()));
    let tile = if small { tile.width(168).height(94) } else { tile.width(Length::Fill).height(Length::Fill) };
    // A right click opens the same menu as the corner button, like the web contextmenu on .screen-tile.
    let tile = mouse_area(tile).on_right_press(Message::PeerOptions(Some((peer.clone(), Some(*camera)))));
    motion::flip(tile, &app.flips, (peer.clone(), Some(*camera)))
}

/// .chat-hide (27 px, in the header) and .chat-restore (32 px glass, over the stage).
fn chat_button(paths: &str, label: &'static str, size: f32) -> Element<'static, Message> {
    if size > 30.0 {
        return glass_button(paths, 16.0, size, label, Message::ToggleChat);
    }
    let press = button(container(dock::icon(paths, 16.0, theme::DIM)).center(size - 2.0)).padding(0).width(size).height(size).style(|_, status| theme::chip_button(status));
    dock::hint_on(press.on_press(Message::ToggleChat).into(), label.into())
}

/// .glass-chip, .strip-toggle and .chat-restore: a dark see-through square over the video.
fn glass_button(paths: &str, glyph: f32, size: f32, label: &'static str, press: Message) -> Element<'static, Message> {
    let square = button(container(dock::icon(paths, glyph, theme::TEXT)).center(size - 2.0)).padding(0).width(size).height(size).style(|_, status| theme::glass(status));
    dock::hint_on(square.on_press(press).into(), label.into())
}

/// A 1 px line in the border colour, for the web's border-top and border-bottom.
fn rule() -> Element<'static, Message> {
    container(iced::widget::space()).width(Length::Fill).height(1).style(|_| container::Style { background: Some(theme::LINE.into()), ..Default::default() }).into()
}

/// button.participant-options
fn options_button(peer: &str, camera: Option<bool>) -> Element<'static, Message> {
    button(container(dock::icon(DOTS, 16.0, theme::TEXT)).center(26))
        .padding(0)
        .width(28)
        .height(28)
        .style(|_, status| theme::options_button(status))
        .on_press(Message::PeerOptions(Some((peer.to_owned(), camera))))
        .into()
}

/// .participant-tile: initials, the speaking ring, the mute badge and the name, for someone without a video.
fn avatar_tile<'a>(p: &'a Participant, me: bool, speaking: f32) -> Element<'a, Message> {
    let circle = container(text(initials(&p.username)).size(13).font(theme::BOLD)).center(46).style(move |_: &iced::Theme| theme::avatar(speaking));
    let mut badge = stack![circle];
    if p.is_muted {
        let mute = container(text("×").size(12)).center(18).style(theme::mute_badge);
        badge = badge.push(container(mute).width(51).height(49).align_right(51).align_bottom(49));
    }
    let name = if me { format!("{} (Você)", p.username) } else { p.username.clone() };
    let tile = container(column![badge, text(name).size(11)].spacing(4).align_x(Alignment::Center)).center_x(168).center_y(94).style(theme::participant_tile);
    let tile = stack![tile, container(options_button(&p.socket_id, None)).padding(5).align_right(168)];
    mouse_area(tile).on_right_press(Message::PeerOptions(Some((p.socket_id.clone(), None)))).into()
}

/// #participant-context: per-viewer options for one participant, opened by ⋯.
fn options_menu<'a>(app: &'a App, peer: &'a str, camera: Option<bool>) -> Element<'a, Message> {
    let name = app.view.participants.iter().find(|p| p.socket_id == peer).map_or("Participante", |p| p.username.as_str());
    let me = app.view.local_socket_id.as_deref() == Some(peer);
    // .context-menu button: 12 px rows that light up on hover.
    let row_button = |label: &'a str, message: Message| {
        button(text(label).size(12))
            .padding(8)
            .width(Length::Fill)
            .style(|_, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed).then(|| theme::HOVER.into()),
                text_color: theme::TEXT,
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            })
            .on_press(message)
    };
    let mut menu = column![container(text(name).size(12).font(theme::BOLD)).padding([5, 8])].spacing(2);
    if me {
        menu = menu.push(container(text("Use o microfone no dock para alterar seu áudio.").size(12).color(theme::DIM)).padding([4, 8]));
    } else {
        // On a screen tile the slider is that screen's audio; elsewhere the person's voice (ui.js openContext).
        let screen = camera == Some(false);
        let level = crate::volume(app, peer, screen);
        let owner = peer.to_owned();
        let volume = iced::widget::slider(0..=100u8, level, move |value| Message::Volume(owner.clone(), screen, value)).width(Length::Fill);
        menu = menu
            .push(container(text("Volume para você").size(11).color(theme::DIM)).padding(iced::Padding { top: 9.0, right: 8.0, bottom: 4.0, left: 8.0 }))
            .push(container(volume).padding([0, 8]))
            .push(row_button(if level == 0 { "Reativar áudio para você" } else { "Silenciar áudio para você" }, Message::TogglePeerMute(peer.to_owned(), screen)));
    }
    if let Some(camera) = camera {
        let key = (peer.to_owned(), camera);
        let label = if app.hidden_videos.contains(&key) { "Mostrar vídeo para você" } else { "Ocultar vídeo para você" };
        menu = menu.push(row_button(label, Message::ToggleHideVideo(key)));
    }
    // .context-note: under a line, 10 px and faint.
    let menu = menu.push(rule()).push(container(text("Estas opções afetam apenas sua visualização.").size(10).color(theme::FAINT)).padding(iced::Padding { top: 6.0, right: 8.0, bottom: 2.0, left: 8.0 }));
    container(menu)
        .padding(7)
        .width(248)
        .style(|_| container::Style {
            background: Some(theme::SURFACE_2.into()),
            border: iced::Border { color: theme::LINE_2, width: 1.0, radius: 11.0.into() },
            ..Default::default()
        })
        .into()
}

/// "Na sala" (the web's .room-people): you first, then the server's order, which never shuffles while people talk.
fn people(app: &App) -> Element<'_, Message> {
    let local = app.view.local_socket_id.as_deref();
    let mut ordered: Vec<&Participant> = app.view.participants.iter().collect();
    ordered.sort_by_key(|p| Some(p.socket_id.as_str()) != local);
    let rows = ordered.into_iter().fold(column![].spacing(2), |list, p| list.push(person_row(app, p, Some(p.socket_id.as_str()) == local)));
    let thin = scrollable::Scrollbar::new().width(4).scroller_width(4);
    let mut section = column![
        text(format!("Na sala · {}", app.view.participants.len())).size(12).color(theme::DIM),
        // Five rows, then it scrolls.
        container(scrollable(rows).direction(scrollable::Direction::Vertical(thin))).max_height(5.0 * 32.0),
    ]
    .spacing(4);
    if app.view.participants.len() == 1 {
        let copy = button(text("Copiar link").size(12)).padding([4, 8]).style(|_, status| theme::chip_button(status)).on_press(Message::CopyRoom);
        section = section.push(row![text("Só você aqui.").size(12).color(theme::DIM).width(Length::Fill), copy].align_y(Alignment::Center));
    }
    container(section).padding([8, 8]).into()
}

/// .person-row: avatar with the speaking ring, name, and the screen (amber while it struggles), camera and muted icons.
/// A click shows their screen, else their camera; ⋯ on hover and a right click open the participant menu.
fn person_row(app: &App, p: &Participant, me: bool) -> Element<'static, Message> {
    let peer = p.socket_id.clone();
    let (screen, camera) = ((peer.clone(), false), (peer.clone(), true));
    let speaking = crate::speaking(app, &peer);
    let avatar = container(text(initials(&p.username)).size(10).font(theme::BOLD)).center(24).style(move |_: &iced::Theme| theme::person_avatar(speaking));
    let name = text(if me { format!("{} (Você)", p.username) } else { p.username.clone() }).size(13).wrapping(text::Wrapping::None);
    let mut icons = row![].spacing(6).align_y(Alignment::Center);
    if is_shared(&app.view, &screen) {
        icons = icons.push(dock::icon(dock::SCREEN, 15.0, if app.quality.shown.contains_key(&screen) { theme::AMBER } else { theme::DIM }));
    }
    if is_shared(&app.view, &camera) {
        icons = icons.push(dock::icon(dock::CAMERA_ON, 15.0, theme::DIM));
    }
    if p.is_muted {
        icons = icons.push(dock::icon(&format!("{}{}", dock::MIC, dock::SLASH), 15.0, theme::RED));
    }
    let content = row![avatar, container(name).width(Length::Fill).clip(true), icons].spacing(8).align_y(Alignment::Center);
    let target = [screen, camera].into_iter().find(|key| is_shared(&app.view, key));
    let press = target.as_ref().filter(|key| app.focused.as_ref() != Some(*key)).cloned().map(Message::Focus);
    let main = button(content).padding([3, 6]).width(Length::Fill).style(theme::person_row).on_press_maybe(press);
    let main = if target.is_some() { dock::hint_on(main.into(), "Mostrar no palco".into()) } else { main.into() };
    if me {
        return main;
    }
    // ⋯ shows over the icons while the row is hovered.
    let options = container(options_button(&peer, None)).padding(iced::Padding { right: 1.0, ..iced::Padding::ZERO }).align_right(Length::Fill).center_y(Length::Fill);
    mouse_area(hover(main, options)).on_right_press(Message::PeerOptions(Some((peer, None)))).into()
}

/// getUsernameInitials in Util.kt: the first letter of up to two words.
pub fn initials(name: &str) -> String {
    name.split(' ').filter_map(|word| word.chars().next()).take(2).flat_map(char::to_uppercase).collect()
}

/// The floating window (crate::set_floating): only `key`'s video, and a way back to the room.
pub fn floating<'a>(app: &'a App, key: &TileKey) -> Element<'a, Message> {
    let video: Element<'a, Message> = match app.tiles.get(key).and_then(|tile| Some((tile.frame.clone()?, tile.version))) {
        Some((frame, version)) => iced::widget::Shader::new(super::video::Video { key: key.clone(), frame, version }).width(Length::Fill).height(Length::Fill).into(),
        None => container(text("Conectando…").size(12).color(theme::DIM)).center(Length::Fill).into(),
    };
    // Over a game the window keeps only the video until the cursor comes by (crate::awake).
    let mut stage = stack![video];
    if awake(app) {
        let name = app.view.participants.iter().find(|p| p.socket_id == key.0).map_or("…", |p| p.username.as_str());
        let chip = container(text(name).size(11).wrapping(text::Wrapping::None)).padding([1, 6]).max_width(160).clip(true).style(theme::name_chip);
        let back = glass_button(EXIT_FULLSCREEN, 15.0, 30.0, "Voltar para a sala", Message::Float(None));
        stage = stage.push(container(chip).padding(8).align_bottom(Length::Fill)).push(container(back).padding(8).align_right(Length::Fill));
    }
    container(stage).style(|_| container::Style { background: Some(theme::VIDEO.into()), ..Default::default() }).width(Length::Fill).height(Length::Fill).into()
}
