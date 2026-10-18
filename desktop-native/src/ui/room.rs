//! Room screen: participants, stage, chat and the dock, laid out like the UI v4.
use std::time::Duration;

use desktop_native::session::AudioDevice;
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, image, pick_list, row, scrollable, text, text_input},
};

use super::theme;
use crate::{App, Message};

pub const CHAT_INPUT: &str = "chat-input";

/// pick_list entry; "Padrão do sistema" is None.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device(pub Option<String>, pub String);

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

fn devices(list: &[AudioDevice]) -> Vec<Device> {
    std::iter::once(Device(None, "Padrão do sistema".into()))
        .chain(list.iter().map(|d| Device(Some(d.id.clone()), d.label.clone())))
        .collect()
}

fn chosen(options: &[Device], id: &Option<String>) -> Option<Device> {
    options.iter().find(|d| &d.0 == id).cloned()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let view = &app.view;
    let local = view.local_socket_id.as_deref();

    let participants = view.participants.iter().fold(column![text("Participantes").font(theme::BOLD)].spacing(8), |list, p| {
        let me = Some(p.socket_id.as_str()) == local;
        let mut tags = Vec::new();
        if p.is_muted {
            tags.push("mudo");
        }
        if view.screen_sharers.contains(&p.socket_id) {
            tags.push("tela");
        }
        if view.camera_sharers.contains(&p.socket_id) {
            tags.push("câmera");
        }
        let mut entry = column![
            text(if me { format!("{} (você)", p.username) } else { p.username.clone() }),
            text(tags.join(" · ")).size(12).color(theme::FAINT),
        ]
        .spacing(2);
        if !me {
            let watching = |camera: bool| view.video_peer.as_deref() == Some(p.socket_id.as_str()) && app.watching_camera == camera;
            let mut actions = row![].spacing(6);
            if view.screen_sharers.contains(&p.socket_id) {
                actions = actions.push(watch_button("Ver tela", watching(false), Message::Watch(p.socket_id.clone(), false)));
            }
            if view.camera_sharers.contains(&p.socket_id) {
                actions = actions.push(watch_button("Ver câmera", watching(true), Message::Watch(p.socket_id.clone(), true)));
            }
            entry = entry.push(actions);
        }
        list.push(entry)
    });

    let sharer = view.video_peer.as_ref().and_then(|id| view.participants.iter().find(|p| &p.socket_id == id));
    let stage: Element<'_, Message> = match (&app.frame, sharer) {
        (Some(allocation), _) => image(allocation.handle().clone()).width(Length::Fill).height(Length::Fill).into(),
        (None, Some(p)) => container(text(format!("Conectando à tela de {}…", p.username)).color(theme::DIM)).center(Length::Fill).into(),
        (None, None) => container(
            column![
                text("Nenhuma tela compartilhada").size(20).font(theme::BOLD),
                text("Compartilhe sua tela ou aguarde outra pessoa.").color(theme::DIM),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .into(),
    };
    let stage_caption = match sharer {
        Some(p) if app.frame.is_some() => format!(
            "{} de {} · {}×{} · {:.0} fps",
            if app.watching_camera { "Câmera" } else { "Tela" },
            p.username,
            app.frame_size.0,
            app.frame_size.1,
            app.fps
        ),
        _ => String::new(),
    };

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

    let mics = devices(&view.audio_inputs);
    let outputs = devices(&view.audio_outputs);
    let sharing = matches!(view.screen_state.as_str(), "active" | "pending" | "picking");
    // Like the web dock: the status only takes space while something is wrong.
    let (status, status_color) = status(app);
    let dock = row![
        text(status).color(status_color),
        toggle(if view.microphone_enabled { "Microfone ligado · F2" } else { "Microfone mudo · F2" }, view.microphone_enabled, Message::ToggleMic),
        pick_list(mics.clone(), chosen(&mics, &app.mic_device), |d| Message::MicDevice(d.0)).width(180),
        toggle(if view.output_enabled { "Som ligado · F3" } else { "Som desligado · F3" }, view.output_enabled, Message::ToggleOutput),
        pick_list(outputs.clone(), chosen(&outputs, &app.output_device), |d| Message::OutputDevice(d.0)).width(180),
        iced::widget::space::horizontal(),
        if sharing {
            button(text(if view.screen_state == "active" { "Parar compartilhamento · F4" } else { "Iniciando…" })).style(button::danger).on_press(Message::StopShare)
        } else {
            button(text("Compartilhar tela · F4")).style(button::primary).on_press(Message::OpenWizard)
        },
        // Leave keeps the old arrow icon, as in the v4 dock.
        button(text("⇤ Sair")).style(button::danger).on_press(Message::Leave),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let body: Element<'_, Message> = match &app.wizard {
        Some(wizard) => container(wizard.view()).style(theme::panel).width(Length::Fill).height(Length::Fill).into(),
        None => row![
            container(scrollable(participants)).padding(12).width(210).height(Length::Fill).style(theme::panel),
            column![
                container(stage).width(Length::Fill).height(Length::Fill).style(theme::stage).padding(1),
                text(stage_caption).size(12).color(theme::FAINT),
            ]
            .spacing(4)
            .width(Length::Fill),
            container(chat).padding(12).width(280).height(Length::Fill).style(theme::panel),
        ]
        .spacing(10)
        .into(),
    };

    let notice = app.notice.as_ref().filter(|(_, since)| since.elapsed() < Duration::from_secs(6));
    let screen = column![body, container(dock).padding(10).style(theme::panel)].spacing(10).padding(12);
    match notice {
        // Floats over the top like the web .notice, without moving the layout.
        Some((notice, _)) => {
            let toast = container(text(notice.as_str()).size(12)).padding([12, 15]).max_width(560).style(theme::notice);
            iced::widget::stack![screen, container(toast).center_x(Length::Fill).padding(12)].into()
        }
        None => screen.into(),
    }
}

/// Errors worth a toast. While reconnecting, the dock status says it instead of the raw socket error.
pub fn errors(app: &App) -> Vec<&str> {
    let view = &app.view;
    let reconnecting = view.local_socket_id.is_none() && view.generation > 1;
    [&view.error, &view.screen_error, &view.audio_error, &view.video_error, &view.devices_error]
        .into_iter()
        .skip(usize::from(reconnecting))
        .filter_map(|e| e.as_deref())
        .collect()
}

fn status(app: &App) -> (String, iced::Color) {
    let view = &app.view;
    let ended = app.running.as_ref().is_some_and(|r| r.thread.as_ref().is_none_or(|t| t.is_finished()));
    if ended {
        (format!("Desconectado. {}", view.error.as_deref().unwrap_or("")).trim_end().into(), theme::RED)
    } else if view.local_socket_id.is_some() {
        (String::new(), theme::DIM)
    } else if view.generation > 1 {
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

fn toggle(label: &str, on: bool, message: Message) -> Element<'_, Message> {
    button(text(label)).style(if on { button::success } else { button::secondary }).on_press(message).into()
}

fn watch_button(label: &str, active: bool, message: Message) -> Element<'_, Message> {
    let b = button(text(label).size(12)).padding([3, 8]).style(if active { button::primary } else { button::secondary });
    if active { b.into() } else { b.on_press(message).into() }
}
