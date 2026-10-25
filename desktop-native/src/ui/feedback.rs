//! "Algo deu errado?" (tray menu), like the web's #feedback-dialog: a few words to Sentry, with the
//! end of screenshare.log (crate::telemetry::send_feedback).
use iced::{
    Element, Length,
    widget::{button, column, container, opaque, row, text, text_input},
};

use super::theme;
use crate::Message;

pub const INPUT: &str = "feedback";

pub fn view(draft: &str) -> Element<'_, Message> {
    let actions = row![
        button(text("Enviar").font(theme::BOLD)).padding([9, 14]).style(button::primary).on_press(Message::SendFeedback),
        button(text("Cancelar")).padding([9, 12]).style(button::secondary).on_press(Message::Feedback(None)),
    ]
    .spacing(8);
    let panel = column![
        text("Algo deu errado?").size(21).font(theme::BOLD),
        text("Conte o que aconteceu. Vai junto o fim do registro do app (a pasta de logs), sem seu nome nem a sala.").color(theme::DIM),
        text_input("O que aconteceu", draft).id(INPUT).padding(10).on_input(|draft| Message::Feedback(Some(draft))).on_submit(Message::SendFeedback),
        container(actions).align_right(Length::Fill),
    ]
    .spacing(12);
    let panel = container(panel).padding(24).max_width(440).style(theme::modal);
    opaque(container(panel).center(Length::Fill).style(theme::backdrop_at(1.0)))
}
