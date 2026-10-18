//! Room screen: the stage with its rail, the chat and the dock, laid out like the UI v4.
use std::time::Duration;

use desktop_native::session::{AudioDevice, Participant};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, image, mouse_area, opaque, row, scrollable, stack, text, text_input},
};

use super::{dock, theme};
use crate::{App, Message, TileKey, is_shared};

pub const CHAT_INPUT: &str = "chat-input";

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

    // Remote screens and cameras that are on, like the web .screen-tile list.
    let videos: Vec<TileKey> = view
        .screen_sharers
        .iter()
        .map(|peer| (peer.clone(), false))
        .chain(view.camera_sharers.iter().map(|peer| (peer.clone(), true)))
        .filter(|key| is_shared(view, key))
        .collect();
    let focused = app.focused.as_ref().filter(|key| videos.contains(key));
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
    let mut rail = row![].spacing(8).align_y(Alignment::Center);
    if let Some(key) = focused {
        rail = rail.push(video_tile(app, key, true));
        for key in videos.iter().filter(|other| Some(*other) != focused) {
            rail = rail.push(video_tile(app, key, true));
        }
    }
    for p in view.participants.iter().filter(|p| !videos.iter().any(|(peer, _)| *peer == p.socket_id)) {
        rail = rail.push(avatar_tile(p, Some(p.socket_id.as_str()) == local, view.speaking.contains(&p.socket_id)));
    }
    // shortcut: the rail does not scroll; add a horizontal scrollable when rooms outgrow one row.
    let stage = column![main, container(rail).center_x(Length::Fill).height(110).center_y(110)].spacing(8);

    let messages = view.messages.iter().fold(column![].spacing(8), |list, m| {
        let own = m.sender_socket_id.is_some() && m.sender_socket_id.as_deref() == local;
        let header = row![
            text(if own { "Você" } else { &m.username }).size(13).font(theme::BOLD).color(theme::DIM),
            text(local_time(m.timestamp)).size(12).color(theme::FAINT),
        ]
        .spacing(8);
        let side = if own { Alignment::End } else { Alignment::Start };
        list.push(column![header, container(text(&m.content)).padding([7, 10]).style(theme::bubble(own))].spacing(2).width(Length::Fill).align_x(side))
    });
    let chat = column![
        row![
            text("Conversa").font(theme::BOLD).width(Length::Fill),
            button(text(format!("SALA {}", app.room)).size(12)).padding([3, 8]).style(button::secondary).on_press(Message::CopyRoom),
        ]
        .align_y(Alignment::Center),
        scrollable(messages).height(Length::Fill).anchor_bottom(),
        text_input("Mensagem", &app.draft).id(CHAT_INPUT).on_input(Message::Draft).on_submit(Message::Send).padding(8),
    ]
    .spacing(10);

    let dock = dock::view(app);

    let body = row![stage.width(Length::Fill), container(chat).padding(12).width(300).height(Length::Fill).style(theme::panel)].spacing(10);

    let notice = app.notice.as_ref().filter(|(_, since)| since.elapsed() < Duration::from_secs(6));
    let screen = column![body, container(dock).padding(10).style(theme::panel)].spacing(10).padding(12);
    let mut layers = iced::widget::stack![screen];
    if let Some(popover) = dock::popover(app) {
        layers = layers.push(popover);
    }
    if let Some((peer, camera)) = &app.peer_options {
        // Opens over the stage, above the rail; a click outside or Esc closes it.
        let outside = mouse_area(container(text("")).width(Length::Fill).height(Length::Fill)).on_press(Message::PeerOptions(None));
        let menu = container(options_menu(app, peer, *camera))
            .padding(iced::Padding { left: 24.0, bottom: 200.0, ..iced::Padding::ZERO })
            .align_bottom(Length::Fill);
        layers = layers.push(outside).push(menu);
    }
    if let Some(wizard) = &app.wizard {
        // The web #quality-modal: a dimmed room with the panel on top; opaque keeps clicks off the room.
        let panel = container(wizard.view(app.profile, app.advanced)).padding(24).max_width(540).height(Length::Fill).style(theme::modal);
        layers = layers.push(opaque(container(panel).center_x(Length::Fill).padding(18).style(theme::backdrop)));
    }
    // Floats over the top like the web .notice, without moving the layout.
    if let Some((notice, _)) = notice {
        let toast = container(text(notice.as_str()).size(12)).padding([12, 15]).max_width(560).style(theme::notice);
        layers = layers.push(container(toast).center_x(Length::Fill).padding(12));
    }
    layers.into()
}

/// Errors worth a toast. While reconnecting, the dock status says it instead of the raw socket error.
pub fn errors(app: &App) -> Vec<&str> {
    let view = &app.view;
    let reconnecting = view.local_socket_id.is_none() && app.joined;
    [&view.error, &view.screen_error, &view.audio_error, &view.camera_error, &view.devices_error]
        .into_iter()
        .skip(usize::from(reconnecting))
        .filter_map(|e| e.as_deref())
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

/// HH:MM in the Windows time zone, like toLocaleTimeString("pt-BR") in the web chat.
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
    let content: Element<'a, Message> = match app.tiles.get(key).and_then(|tile| tile.frame.as_ref()) {
        _ if app.hidden_videos.contains(key) => container(text("Vídeo oculto").color(theme::DIM)).center(Length::Fill).into(),
        Some(frame) => image(frame.handle().clone()).width(Length::Fill).height(Length::Fill).into(),
        None => container(text(format!("Conectando à {} de {name}…", if *camera { "câmera" } else { "tela" })).size(12).color(theme::DIM))
            .center(Length::Fill)
            .into(),
    };
    let mut layers = stack![content];
    if small {
        let chip = container(text(name).size(11)).padding([1, 6]).style(theme::name_chip);
        layers = layers.push(container(chip).padding(6).align_bottom(Length::Fill));
    }
    // .screen-tile-controls: the ⋯ options in the corner.
    layers = layers.push(container(options_button(peer, Some(*camera))).padding(if small { 5 } else { 12 }).align_right(Length::Fill).align_bottom(Length::Fill));
    let focused = app.focused.as_ref() == Some(key);
    let speaking = app.view.speaking.contains(peer);
    let tile = button(layers).padding(1).style(move |_, _| theme::video_tile(focused, speaking)).on_press(Message::Focus(key.clone()));
    if small { tile.width(168).height(94).into() } else { tile.width(Length::Fill).height(Length::Fill).into() }
}

/// button.participant-options
fn options_button(peer: &str, camera: Option<bool>) -> Element<'static, Message> {
    button(text("⋯").size(14)).padding([2, 8]).style(button::secondary).on_press(Message::PeerOptions(Some((peer.to_owned(), camera)))).into()
}

/// .participant-tile: initials, the speaking ring, the mute badge and the name, for someone without a video.
fn avatar_tile<'a>(p: &'a Participant, me: bool, speaking: bool) -> Element<'a, Message> {
    let circle = container(text(initials(&p.username)).size(13).font(theme::BOLD)).center(46).style(move |_: &iced::Theme| theme::avatar(speaking));
    let mut badge = stack![circle];
    if p.is_muted {
        let mute = container(text("×").size(12)).center(18).style(theme::mute_badge);
        badge = badge.push(container(mute).width(51).height(49).align_right(51).align_bottom(49));
    }
    let name = if me { format!("{} (Você)", p.username) } else { p.username.clone() };
    let tile = container(column![badge, text(name).size(11)].spacing(4).align_x(Alignment::Center)).center_x(168).center_y(94).style(theme::participant_tile);
    stack![tile, container(options_button(&p.socket_id, None)).padding(5).align_right(168)].into()
}

/// #participant-context: per-viewer options for one participant, opened by ⋯.
/// shortcut: no volume slider; libwebrtc's wrapper has no per-track gain, only on/off.
fn options_menu<'a>(app: &'a App, peer: &'a str, camera: Option<bool>) -> Element<'a, Message> {
    let name = app.view.participants.iter().find(|p| p.socket_id == peer).map_or("Participante", |p| p.username.as_str());
    let me = app.view.local_socket_id.as_deref() == Some(peer);
    let row_button = |label: &'a str, message: Message| button(text(label)).padding(8).width(Length::Fill).style(button::text).on_press(message);
    let mut menu = column![text(name).font(theme::BOLD)].spacing(4);
    if me {
        menu = menu.push(text("Use o microfone no dock para alterar seu áudio.").size(12).color(theme::DIM));
    } else {
        let muted = app.muted_peers.contains(peer);
        menu = menu.push(row_button(if muted { "Reativar áudio para você" } else { "Silenciar áudio para você" }, Message::TogglePeerMute(peer.to_owned())));
    }
    if let Some(camera) = camera {
        let key = (peer.to_owned(), camera);
        let label = if app.hidden_videos.contains(&key) { "Mostrar vídeo para você" } else { "Ocultar vídeo para você" };
        menu = menu.push(row_button(label, Message::ToggleHideVideo(key)));
    }
    let menu = menu.push(text("Estas opções afetam apenas sua visualização.").size(11).color(theme::DIM));
    container(menu).padding(12).width(260).style(theme::modal).into()
}

/// getUsernameInitials in Util.kt: the first letter of up to two words.
fn initials(name: &str) -> String {
    name.split(' ').filter_map(|word| word.chars().next()).take(2).flat_map(char::to_uppercase).collect()
}
