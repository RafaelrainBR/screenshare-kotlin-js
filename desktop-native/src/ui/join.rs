//! The join screen as the web #join-screen: a card with the logo, two fields with icons and the button.
use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{button, checkbox, column, container, row, stack, svg, text, text_input},
};

use super::{dock, theme};
use crate::{App, Message, NAME_INPUT};

const LOGO: &[u8] = include_bytes!("../../../client/src/jsMain/resources/brand/screenshare-logo-on-dark.svg");
const PERSON: &str = r#"<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>"#;
const HASH: &str = r#"<path d="M5 9h15M4 15h15M10 3 8 21M16 3l-2 18"/>"#;
const ARROW: &str = r#"<path d="M5 12h14M13 6l6 6-6 6"/>"#;
// The star, outlined or filled (the web uses ☆ and ★).
const STAR: &str = r#"<path d="m12 3 2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9L12 3z"/>"#;
const STAR_FILLED: &str = r##"<path fill="#fff" d="m12 3 2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9L12 3z"/>"##;

pub fn view(app: &App) -> Element<'_, Message> {
    let ready = !app.name.trim().is_empty();
    let name = text_input("Como você aparece na sala", &app.name).id(NAME_INPUT).on_input(Message::Name).on_submit(Message::Join);
    let room = text_input("ex: 4f8a2c1e", &app.room).on_input(Message::Room).on_submit(Message::Join);
    let join = button(
        container(row![text("Entrar na sala").font(theme::BOLD).color(theme::ON_ACCENT), dock::icon(ARROW, 18.0, theme::ON_ACCENT)].spacing(6).align_y(Alignment::Center))
            .center_x(Length::Fill),
    )
    .padding([12, 12])
    .width(Length::Fill)
    .style(|_, status| primary(status))
    .on_press_maybe(ready.then_some(Message::Join));
    let fields = column![
        text("Seu nome"),
        with_icon(PERSON, name),
        text("Código da sala"),
        with_icon(HASH, room),
        text("Código opcional ao criar uma sala.").size(11).color(theme::FAINT),
        recent_rooms(app),
        // .startup-option: shown in the desktop apps.
        checkbox(app.autostart).label("Iniciar com o sistema e voltar a esta sala").on_toggle(Message::ToggleAutostart).text_size(13).spacing(9),
        container(join).padding(iced::Padding { top: 5.0, ..iced::Padding::ZERO }),
    ]
    .spacing(8);
    // The web's margins: 22 under the logo, 7 under the title, 24 under the description.
    let card = column![
        svg(svg::Handle::from_memory(LOGO)).width(112).height(87),
        column![
            text("Participar de uma sala").size(26).font(theme::BOLD),
            text("Use um código recebido ou deixe vazio para criar uma sala.").color(theme::DIM),
        ]
        .spacing(7),
        container(fields).padding(iced::Padding { top: 2.0, ..iced::Padding::ZERO }),
    ]
    .spacing(22);
    let card = container(card).padding(28).width(430).style(|_| container::Style {
        background: Some(theme::SURFACE.into()),
        border: Border { color: theme::LINE, width: 1.0, radius: 14.0.into() },
        ..Default::default()
    });
    // A newer release: the same action as the tray's update item.
    let update = crate::update_label(app).map(|label| {
        button(text(label).size(13).color(theme::ACCENT)).padding([6, 10]).style(|_, _| button::Style::default()).on_press_maybe((!app.updating).then_some(Message::Update))
    });
    container(column![card].push(update).spacing(12).align_x(Alignment::Center)).center(Length::Fill).padding(24).into()
}

/// .input-icon: the glyph at 12 px inside a 43 px input padded to 40 px.
fn with_icon<'a>(paths: &str, input: text_input::TextInput<'a, Message>) -> Element<'a, Message> {
    let input = input.padding(iced::Padding { top: 12.0, bottom: 12.0, left: 40.0, right: 12.0 }).style(theme::input);
    let glyph = container(dock::icon(paths, 18.0, theme::DIM)).padding(iced::Padding { left: 12.0, ..iced::Padding::ZERO }).center_y(Length::Fill);
    stack![input, glyph].into()
}

/// .btn-primary.btn-lg, faded while the name is empty.
fn primary(status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => iced::color!(0xa8c5fb),
        button::Status::Disabled => Color { a: 0.5, ..theme::ACCENT },
        button::Status::Active => theme::ACCENT,
    };
    button::Style { background: Some(Background::Color(color)), text_color: theme::ON_ACCENT, border: Border { radius: 8.0.into(), ..Default::default() }, ..Default::default() }
}

/// .recent-rooms: up to five recent rooms and the favorites, a click joins, the star pins one.
fn recent_rooms(app: &App) -> Element<'_, Message> {
    let chips = app.rooms.iter().map(|room| {
        let join = button(text(room.id.as_str()).size(13).font(iced::Font::MONOSPACE))
            .padding(iced::Padding { top: 6.0, bottom: 6.0, left: 10.0, right: 4.0 })
            .style(|_, _| button::Style { text_color: theme::TEXT, ..Default::default() })
            .on_press(Message::PickRoom(room.id.clone()));
        let (glyph, color, hint) = if room.favorite { (STAR_FILLED, theme::AMBER, "Tirar dos favoritos") } else { (STAR, theme::FAINT, "Favoritar") };
        let star = button(dock::icon(glyph, 14.0, color)).padding([7, 8]).style(|_, _| button::Style::default()).on_press(Message::ToggleFavoriteRoom(room.id.clone()));
        let chip = container(row![join, dock::hint_on(star.into(), hint.into())].align_y(Alignment::Center)).style(|_| container::Style {
            background: Some(theme::SURFACE_2.into()),
            border: Border { color: theme::LINE, width: 1.0, radius: 8.0.into() },
            ..Default::default()
        });
        chip.into()
    });
    iced::widget::Row::with_children(chips).spacing(6).wrap().vertical_spacing(6).into()
}
