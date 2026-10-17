//! Room screen: participants, stage, chat and the dock, laid out like the UI v4.
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
        list.push(column![text(&m.username).size(13).font(theme::BOLD).color(theme::DIM), text(&m.content)].spacing(2))
    });
    let chat = column![
        text("Conversa").font(theme::BOLD),
        scrollable(messages).height(Length::Fill).anchor_bottom(),
        text_input("Mensagem", &app.draft).id(CHAT_INPUT).on_input(Message::Draft).on_submit(Message::Send).padding(8),
    ]
    .spacing(10);

    let mics = devices(&view.audio_inputs);
    let outputs = devices(&view.audio_outputs);
    let sharing = matches!(view.screen_state.as_str(), "active" | "pending" | "picking");
    let dock = row![
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

    let errors: Vec<&str> = [&view.error, &view.screen_error, &view.audio_error, &view.video_error, &view.devices_error, &app.notice]
        .into_iter()
        .filter_map(|e| e.as_deref())
        .collect();

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

    column![
        row![
            text(format!("Sala {}", app.room)).font(theme::BOLD),
            text(if local.is_some() { "Conectado" } else { "Conectando…" }).color(if local.is_some() { theme::GREEN } else { theme::DIM }),
        ]
        .spacing(12),
        body,
        text(errors.join(" · ")).size(13).color(theme::RED),
        container(dock).padding(10).style(theme::panel),
    ]
    .spacing(10)
    .padding(12)
    .into()
}

fn toggle(label: &str, on: bool, message: Message) -> Element<'_, Message> {
    button(text(label)).style(if on { button::success } else { button::secondary }).on_press(message).into()
}

fn watch_button(label: &str, active: bool, message: Message) -> Element<'_, Message> {
    let b = button(text(label).size(12)).padding([3, 8]).style(if active { button::primary } else { button::secondary });
    if active { b.into() } else { b.on_press(message).into() }
}
