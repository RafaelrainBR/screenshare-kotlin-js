//! The room dock as the web .dock: status on the left, icon tools in the middle (share | camera,
//! microphone, sound), each with a chevron opening its picker above it, and leave on the right.
//! Icons are the index.html SVGs, tinted like the web's currentColor.
use iced::{
    Alignment, Background, Border, Color, Element, Length, Padding,
    border::Radius,
    widget::{button, column, container, responsive, row, space, svg, text, tooltip},
};

use super::{profile, room::Device, theme};
use crate::{App, Message};

/// The chevron menus (web .device-picker): one device list each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picker {
    Camera,
    Microphone,
    Output,
}

// Web --control-size, the chevron width, the dock gap and the separator.
const CONTROL: f32 = 42.0;
const CHEVRON: f32 = 24.0;
const GAP: f32 = 13.0;
const GROUP: f32 = CONTROL + CHEVRON;
/// Left edge of each tool group from the start of the tools: share, separator, camera, mic, sound.
const SHARE_X: f32 = 0.0;
const CAMERA_X: f32 = GROUP + GAP + 1.0 + GAP;
const MIC_X: f32 = CAMERA_X + GROUP + GAP;
const OUTPUT_X: f32 = MIC_X + GROUP + GAP;
const TOOLS: f32 = OUTPUT_X + GROUP;
/// The room's outer padding, the dock's padding and border, as laid out in room::view.
const OUTER: f32 = 12.0;
const INSET: f32 = OUTER + 10.0 + 1.0;
const DOCK_HEIGHT: f32 = CONTROL + 2.0 * 10.0 + 2.0;

#[derive(Clone, Copy)]
enum Tone {
    Plain,
    /// .muted: a red glyph on the usual background.
    Muted,
    /// .btn-error: red glyph and red border.
    Danger,
}

pub const SCREEN: &str = r#"<rect x="3" y="4" width="18" height="14" rx="2"/><path d="M8 21h8M12 18v3"/>"#;
pub const STOP_SCREEN: &str = r#"<rect x="3" y="4" width="18" height="14" rx="2"/><path d="M9 8h6v6H9z"/>"#;
pub const CAMERA_ON: &str = r#"<rect x="3" y="6" width="12" height="12" rx="2"/><path d="m15 10 6-3v10l-6-3"/>"#;
pub const CAMERA_OFF: &str = r#"<rect x="3" y="6" width="12" height="12" rx="2"/><path d="m15 10 6-3v10l-6-3M3 3l18 18"/>"#;
pub const MIC: &str = r#"<path d="M9 5a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0V5ZM5 11a7 7 0 0 0 14 0M12 18v3M8 21h8"/>"#;
pub const OUTPUT: &str = r#"<path d="M11 5 6 9H3v6h3l5 4V5ZM15 9a4 4 0 0 1 0 6M18 6a8 8 0 0 1 0 12"/>"#;
pub const SLASH: &str = r#"<line x1="3" y1="3" x2="21" y2="21"/>"#;
pub const LEAVE: &str = r#"<path d="M14 4h4a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-4M10 16l-4-4 4-4M6 12h10"/>"#;
const CHEVRON_DOWN: &str = r#"<path d="m6 9 6 6 6-6"/>"#;

/// The web's global `svg` rule: no fill, a 1.8 round stroke in currentColor.
pub fn icon(paths: &str, size: f32, color: Color) -> Element<'static, Message> {
    let source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>"##
    );
    svg(svg::Handle::from_memory(source.into_bytes())).width(size).height(size).style(move |_, _| svg::Style { color: Some(color) }).into()
}

fn tool_style(tone: Tone, radius: Radius, status: button::Status) -> button::Style {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(Background::Color(if hover { theme::HOVER } else { theme::SURFACE_2 })),
        text_color: theme::TEXT,
        border: Border { color: if matches!(tone, Tone::Danger) { theme::RED_LINE } else { theme::LINE }, width: 1.0, radius },
        ..Default::default()
    }
}

fn glyph_color(tone: Tone) -> Color {
    if matches!(tone, Tone::Plain) { theme::TEXT } else { theme::RED }
}

/// An icon button with its hint (the web title), and a chevron on its right when it has a picker.
fn tool(paths: String, tone: Tone, hint: String, press: Message, picker: Option<(String, Message)>) -> Element<'static, Message> {
    let left = if picker.is_some() { Radius { top_left: 10.0, bottom_left: 10.0, ..Radius::default() } } else { Radius::from(10.0) };
    let main = button(container(icon(&paths, 20.0, glyph_color(tone))).center(CONTROL))
        .padding(0)
        .width(CONTROL)
        .height(CONTROL)
        .style(move |_, status| tool_style(tone, left, status))
        .on_press(press);
    let mut group = row![hint_on(main.into(), hint)];
    if let Some((hint, press)) = picker {
        let right = Radius { top_right: 10.0, bottom_right: 10.0, ..Radius::default() };
        let chevron = button(container(icon(CHEVRON_DOWN, 12.0, theme::TEXT)).center(CONTROL))
            .padding(0)
            .width(CHEVRON)
            .height(CONTROL)
            .style(move |_, status| tool_style(Tone::Plain, right, status))
            .on_press(press);
        group = group.push(hint_on(chevron.into(), hint));
    }
    group.into()
}

pub fn hint_on(content: Element<'static, Message>, hint: String) -> Element<'static, Message> {
    tooltip(content, container(text(hint).size(12)).padding([4, 8]).style(theme::modal), tooltip::Position::Top).gap(6).into()
}

fn toggle_picker(app: &App, picker: Picker) -> Message {
    Message::DevicePicker((app.device_picker != Some(picker)).then_some(picker))
}

pub fn view(app: &App) -> Element<'_, Message> {
    let view = &app.view;
    let sharing = crate::sharing(view);
    let camera = crate::camera_on(view);
    // The share chevron opens the transmission menu (#share-picker), live while sharing.
    let settings = Some(("Ajustes da transmissão · F5".into(), Message::ShareMenu(app.share_menu.is_none().then_some(profile::Menu::Main))));
    let share = if sharing {
        tool(STOP_SCREEN.into(), Tone::Danger, "Parar compartilhamento · F4".into(), Message::StopShare, settings)
    } else {
        tool(SCREEN.into(), Tone::Plain, "Compartilhar tela · F4".into(), Message::OpenWizard, settings)
    };
    let tools = row![
        share,
        container(space()).width(1).height(25).style(|_| container::Style { background: Some(theme::LINE.into()), ..Default::default() }),
        tool(
            if camera { CAMERA_ON } else { CAMERA_OFF }.into(),
            if camera { Tone::Plain } else { Tone::Muted },
            if camera { "Desligar câmera · F6" } else { "Ligar câmera · F6" }.into(),
            Message::ToggleCamera,
            Some(("Escolher câmera".into(), toggle_picker(app, Picker::Camera))),
        ),
        tool(
            format!("{MIC}{}", if view.microphone_enabled { "" } else { SLASH }),
            if view.microphone_enabled { Tone::Plain } else { Tone::Muted },
            if view.microphone_enabled { "Silenciar microfone · F2" } else { "Ativar microfone · F2" }.into(),
            Message::ToggleMic,
            Some(("Escolher microfone".into(), toggle_picker(app, Picker::Microphone))),
        ),
        tool(
            format!("{OUTPUT}{}", if view.output_enabled { "" } else { SLASH }),
            if view.output_enabled { Tone::Plain } else { Tone::Muted },
            if view.output_enabled { "Silenciar saída de áudio · F3" } else { "Ativar saída de áudio · F3" }.into(),
            Message::ToggleOutput,
            Some(("Escolher saída de áudio".into(), toggle_picker(app, Picker::Output))),
        ),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center);
    let leave = tool(LEAVE.into(), Tone::Danger, "Sair ou trocar de sala".into(), Message::Leave, None);
    // .dock-status: only while something is wrong, ellipsized on the left.
    let (status, status_color) = super::room::status(app);
    row![
        container(text(status).size(11).color(status_color).wrapping(text::Wrapping::None)).width(Length::Fill).clip(true),
        tools,
        container(leave).width(Length::Fill).align_right(Length::Fill),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center)
    .into()
}

/// #share-menu-main and .picker-panel open above their tool group, like the web's popovers.
/// Positions follow the dock's fixed widths, centered in the window as the web grid centers them.
pub fn popover(app: &App) -> Option<Element<'_, Message>> {
    let (group_x, width, close) = match (app.share_menu, app.device_picker) {
        (Some(_), _) => (SHARE_X, 290.0, Message::ShareMenu(None)),
        (None, Some(Picker::Camera)) => (CAMERA_X, 260.0, Message::DevicePicker(None)),
        (None, Some(Picker::Microphone)) => (MIC_X, 260.0, Message::DevicePicker(None)),
        (None, Some(Picker::Output)) => (OUTPUT_X, 260.0, Message::DevicePicker(None)),
        (None, None) => return None,
    };
    let placed = responsive(move |size| {
        let panel = match (app.share_menu, app.device_picker) {
            (Some(page), _) => profile::menu(page, app.profile, app.view.screen_state == "active"),
            (None, Some(picker)) => device_panel(app, picker),
            (None, None) => space().into(),
        };
        let tools_x = INSET + (size.width - 2.0 * INSET - TOOLS) / 2.0;
        let x = (tools_x + group_x).clamp(OUTER, (size.width - width - OUTER).max(OUTER));
        column![space::vertical(), row![space().width(x), panel]].padding(Padding { bottom: OUTER + DOCK_HEIGHT + 8.0, ..Padding::ZERO }).into()
    });
    // A click outside closes it, like the web <details> pickers.
    let outside = iced::widget::mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(close);
    Some(iced::widget::stack![outside, placed].into())
}

/// .picker-panel: the label and the devices, the chosen one ticked.
fn device_panel(app: &App, picker: Picker) -> Element<'_, Message> {
    let view = &app.view;
    let (label, list, chosen): (&str, &[desktop_native::session::AudioDevice], &Option<String>) = match picker {
        Picker::Camera => ("Câmera", &view.cameras, &app.camera_device),
        Picker::Microphone => ("Microfone", &view.audio_inputs, &app.mic_device),
        Picker::Output => ("Saída de áudio", &view.audio_outputs, &app.output_device),
    };
    let rows = super::room::devices(list).into_iter().fold(column![text(label).font(theme::BOLD)].spacing(2), |rows, Device(id, name)| {
        let checked = &id == chosen;
        let pick = match picker {
            Picker::Camera => Message::CameraDevice(id),
            Picker::Microphone => Message::MicDevice(id),
            Picker::Output => Message::OutputDevice(id),
        };
        rows.push(
            button(row![text(name).color(if checked { theme::ACCENT } else { theme::TEXT }).width(Length::Fill), text(if checked { "✓" } else { "" }).color(theme::ACCENT)])
                .padding(8)
                .width(Length::Fill)
                .style(button::text)
                .on_press(pick),
        )
    });
    container(rows).padding(10).width(260).style(theme::modal).into()
}
