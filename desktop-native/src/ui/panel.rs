//! The compact tray panel (index.html #tray-panel, body.compact-tray): the room at a glance,
//! media toggles, the next share's preset, who is in, and a way into the full window.
use desktop_native::share::SharePreset;
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length,
    widget::{button, column, container, row, scrollable, space, text},
};

use super::{dock, profile, theme};
use crate::{App, Message, tray::Action};

/// Tauri's present_window size for the compact mode.
pub const SIZE: (f32, f32) = (430.0, 660.0);

const MONO: Font = Font::MONOSPACE;
const ON_ACCENT: Color = iced::color!(0x0c1022);
const OPEN: &str = r#"<path d="M14 4h6v6M20 4l-9 9M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"/>"#;
const CHEVRON_RIGHT: &str = r#"<path d="m9 5 7 7-7 7"/>"#;

pub fn view(app: &App) -> Element<'_, Message> {
    let view = &app.view;
    let in_room = app.running.is_some();
    let connected = view.local_socket_id.is_some();
    let sharing = crate::sharing(view);
    let camera = crate::camera_on(view);

    // .tray-header
    let room = if in_room { app.room.as_str() } else { "Nenhuma sala" };
    let count = container(text(format!("{} participantes", view.participants.len())).size(11).color(theme::DIM))
        .padding([4, 8])
        .style(|_| container::Style { border: Border { color: theme::LINE, width: 1.0, radius: 6.0.into() }, ..Default::default() });
    let header = row![
        column![text("SALA").size(10).font(MONO).color(theme::DIM), text(room).size(16).font(Font { weight: iced::font::Weight::Bold, ..MONO })]
            .spacing(3)
            .width(Length::Fill),
        count,
    ]
    .align_y(Alignment::Center);
    let mut panel = column![header].spacing(12).padding(16).height(Length::Fill);

    // #tray-connection-status: hidden once connected, like .sr-only.
    if !connected {
        let (status, color) = if in_room { super::room::status(app) } else { ("Entre em uma sala para usar a mídia.".into(), theme::DIM) };
        let status = if status.is_empty() { "Conectando à sala…".into() } else { status };
        panel = panel.push(text(status).size(11).color(color));
    }

    // .tray-media: the share on its own line, then mic, camera and sound.
    let screen_label = if sharing { "Parar compartilhamento" } else { "Compartilhar tela" };
    let screen_tone = if sharing { Tone::Live(theme::RED) } else { Tone::Primary };
    let screen = media_button(
        row![dock::icon(if sharing { dock::STOP_SCREEN } else { dock::SCREEN }, 20.0, screen_tone.glyph()), text(screen_label).size(14).font(theme::BOLD)]
            .spacing(9)
            .align_y(Alignment::Center)
            .into(),
        screen_tone,
        connected.then_some(Message::Tray(Action::Screen)),
    );
    let mic = view.microphone_enabled;
    let muted_output = !view.output_enabled;
    let toggles = row![
        media_icon(format!("{}{}", dock::MIC, if mic { "" } else { dock::SLASH }), if mic { Tone::Live(theme::GREEN) } else { Tone::Off }, connected.then_some(Message::Tray(Action::Mic))),
        media_icon(if camera { dock::CAMERA_ON } else { dock::CAMERA_OFF }.into(), if camera { Tone::Live(theme::GREEN) } else { Tone::Off }, connected.then_some(Message::Tray(Action::Camera))),
        media_icon(format!("{}{}", dock::OUTPUT, if muted_output { dock::SLASH } else { "" }), if muted_output { Tone::Live(theme::RED) } else { Tone::Plain }, Some(Message::Tray(Action::Output))),
    ]
    .spacing(8);
    panel = panel.push(column![screen, toggles].spacing(8));

    // .tray-quality-section: the native capture keeps the source size, so the presets stand in for 720p/1080p/4K.
    let chosen = profile::matching(app.profile);
    let presets = SharePreset::ALL.into_iter().fold(row![].spacing(4), |line, preset| {
        let on = chosen == Some(preset);
        line.push(
            button(container(text(profile::preset_name(preset)).size(13).color(if on { theme::TEXT } else { theme::DIM })).center_x(Length::Fill))
                .padding([7, 4])
                .width(Length::Fill)
                .style(move |_, status| segment(on, status))
                .on_press(Message::SetProfile(preset.profile())),
        )
    });
    let presets = container(presets).padding(3).style(|_| container::Style {
        background: Some(theme::theme().palette().background.into()),
        border: Border { color: theme::LINE, width: 1.0, radius: 9.0.into() },
        ..Default::default()
    });
    let more = button(row![text("Mais opções").size(12).width(Length::Fill), dock::icon(CHEVRON_RIGHT, 14.0, theme::DIM)].align_y(Alignment::Center))
        .padding([8, 2])
        .width(Length::Fill)
        .style(|_, status| button::Style {
            text_color: if matches!(status, button::Status::Hovered) { theme::TEXT } else { theme::DIM },
            ..Default::default()
        })
        .on_press_maybe(connected.then_some(Message::PanelOptions));
    panel = panel.push(column![text("Qualidade da próxima tela").size(12).color(theme::DIM), presets, more].spacing(8));

    // .tray-participants
    if !view.participants.is_empty() {
        let local = view.local_socket_id.as_deref();
        let people = view.participants.iter().fold(column![].spacing(0), |list, p| {
            let me = Some(p.socket_id.as_str()) == local;
            let speaking = view.speaking.contains(&p.socket_id);
            let avatar = container(text(super::room::initials(&p.username)).size(12).font(theme::BOLD)).center(30).style(move |_| container::Style {
                background: Some(theme::SURFACE_2.into()),
                border: Border { color: if speaking { theme::GREEN } else { Color::TRANSPARENT }, width: 2.0, radius: 15.0.into() },
                ..Default::default()
            });
            let name = if me { format!("{} (Você)", p.username) } else { p.username.clone() };
            let screen = view.screen_sharers.contains(&p.socket_id);
            let camera = view.camera_sharers.contains(&p.socket_id);
            let mut icons = row![].spacing(8).align_y(Alignment::Center);
            if screen {
                icons = icons.push(dock::icon(dock::SCREEN, 16.0, theme::DIM));
            }
            if camera {
                icons = icons.push(dock::icon(dock::CAMERA_ON, 16.0, theme::DIM));
            }
            let mic = if p.is_muted { format!("{}{}", dock::MIC, dock::SLASH) } else { dock::MIC.into() };
            icons = icons.push(dock::icon(&mic, 16.0, if p.is_muted { theme::RED } else if speaking { theme::GREEN } else { theme::DIM }));
            let line = row![avatar, text(name).width(Length::Fill).wrapping(text::Wrapping::None), icons].spacing(10).align_y(Alignment::Center);
            let mut item = column![line].spacing(6).padding([7, 0]);
            // .tray-user-volume: the voice volume for someone else, under the name.
            if !me {
                let owner = p.socket_id.clone();
                let level = crate::volume(app, &p.socket_id, false);
                let slider = iced::widget::slider(0..=100u8, level, move |value| Message::Volume(owner.clone(), false, value)).width(Length::Fill);
                item = item.push(row![space().width(30), dock::icon(dock::OUTPUT, 16.0, theme::DIM), slider].spacing(8).align_y(Alignment::Center));
            }
            // .tray-view-actions: open the full window on that video.
            if !me && (screen || camera) {
                let mut views = row![space().width(30)].spacing(6);
                if screen {
                    views = views.push(ghost("Visualizar tela", Message::ViewTile((p.socket_id.clone(), false))));
                }
                if camera {
                    views = views.push(ghost("Visualizar câmera", Message::ViewTile((p.socket_id.clone(), true))));
                }
                item = item.push(views);
            }
            list.push(item)
        });
        let people = column![text("Participantes").size(12).color(theme::DIM), scrollable(people).height(Length::Fill)].spacing(6);
        panel = panel.push(rule()).push(container(people).height(Length::Fill));
    } else {
        panel = panel.push(space::vertical());
    }

    // .tray-footer
    let open = button(container(row![dock::icon(OPEN, 17.0, theme::TEXT), text("Abrir sala").size(13).font(theme::BOLD)].spacing(6).align_y(Alignment::Center)).center_x(Length::Fill))
        .padding([10, 6])
        .width(Length::Fill)
        .style(|_, status| btn(Tone::Plain, status))
        .on_press(Message::Tray(Action::Open));
    let leave = button(container(dock::icon(dock::LEAVE, 17.0, theme::RED)).center(36))
        .padding(0)
        .width(44)
        .height(38)
        .style(|_, status| btn(Tone::Danger, status))
        .on_press_maybe(in_room.then_some(Message::Tray(Action::Leave)));
    panel = panel.push(rule()).push(row![open, leave].spacing(6));

    container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style { background: Some(theme::SURFACE.into()), border: Border { color: theme::LINE, width: 1.0, radius: 0.0.into() }, ..Default::default() })
        .into()
}

#[derive(Clone, Copy)]
enum Tone {
    Plain,
    /// .btn-primary
    Primary,
    /// aria-pressed: border and glyph in this colour.
    Live(Color),
    /// A muted toggle: red glyph only.
    Off,
    /// .btn-error
    Danger,
}

impl Tone {
    fn glyph(self) -> Color {
        match self {
            Tone::Plain => theme::TEXT,
            Tone::Primary => ON_ACCENT,
            Tone::Live(color) => color,
            Tone::Off | Tone::Danger => theme::RED,
        }
    }
}

/// .btn with the tray-media states; disabled buttons fade like .btn:disabled.
fn btn(tone: Tone, status: button::Status) -> button::Style {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let (background, border) = match tone {
        Tone::Primary => (if hover { iced::color!(0xa6c4fb) } else { theme::ACCENT }, theme::ACCENT),
        Tone::Live(color) => (if hover { theme::HOVER } else { theme::SURFACE_2 }, color),
        Tone::Danger => (if hover { theme::HOVER } else { theme::SURFACE_2 }, theme::RED_LINE),
        Tone::Plain | Tone::Off => (if hover { theme::HOVER } else { theme::SURFACE_2 }, theme::LINE),
    };
    let alpha = if matches!(status, button::Status::Disabled) { 0.5 } else { 1.0 };
    let fade = |color: Color| Color { a: color.a * alpha, ..color };
    button::Style {
        background: Some(Background::Color(fade(background))),
        text_color: fade(tone.glyph()),
        border: Border { color: fade(border), width: 1.0, radius: 7.0.into() },
        ..Default::default()
    }
}

fn media_button(content: Element<'static, Message>, tone: Tone, press: Option<Message>) -> Element<'static, Message> {
    button(container(content).center_x(Length::Fill))
        .padding([11, 6])
        .width(Length::Fill)
        .style(move |_, status| btn(tone, status))
        .on_press_maybe(press)
        .into()
}

fn media_icon(paths: String, tone: Tone, press: Option<Message>) -> Element<'static, Message> {
    media_button(dock::icon(&paths, 20.0, tone.glyph()), tone, press)
}

/// .tray-quality .btn, the picked one raised.
fn segment(on: bool, status: button::Status) -> button::Style {
    let hover = matches!(status, button::Status::Hovered);
    button::Style {
        background: (on || hover).then(|| Background::Color(if on { theme::SURFACE_2 } else { theme::HOVER })),
        text_color: if on { theme::TEXT } else { theme::DIM },
        border: Border { color: if on { theme::LINE } else { Color::TRANSPARENT }, width: 1.0, radius: 7.0.into() },
        ..Default::default()
    }
}

/// .btn-ghost in .tray-view-actions
fn ghost(label: &'static str, press: Message) -> Element<'static, Message> {
    button(text(label).size(11))
        .padding([6, 9])
        .style(|_, status| button::Style {
            background: matches!(status, button::Status::Hovered).then(|| Background::Color(theme::HOVER)),
            text_color: theme::DIM,
            border: Border { color: theme::LINE, width: 1.0, radius: 7.0.into() },
            ..Default::default()
        })
        .on_press(press)
        .into()
}

fn rule() -> Element<'static, Message> {
    container(space()).width(Length::Fill).height(1).style(|_| container::Style { background: Some(theme::LINE.into()), ..Default::default() }).into()
}

/// tray_window_position in the Tauri app: right-aligned with the icon, above it when the tray is in
/// the lower half, kept inside the work area. All values physical: (x, y, width, height).
pub fn position(tray: (f64, f64, f64, f64), window: (f64, f64), area: (f64, f64, f64, f64)) -> (f64, f64) {
    let x = (tray.0 + tray.2 - window.0).clamp(area.0, area.0 + (area.2 - window.0).max(0.0));
    let y = if tray.1 >= area.1 + area.3 / 2.0 { tray.1 - window.1 } else { tray.1 + tray.3 }.clamp(area.1, area.1 + (area.3 - window.1).max(0.0));
    (x, y)
}

/// The work area (no taskbar) of the monitor holding a physical point.
pub fn work_area(x: f64, y: f64) -> Option<(f64, f64, f64, f64)> {
    use windows::Win32::{
        Foundation::POINT,
        Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint},
    };
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: MonitorFromPoint always returns a monitor with DEFAULTTONEAREST; info is a sized out-parameter.
    let ok = unsafe { GetMonitorInfoW(MonitorFromPoint(POINT { x: x as i32, y: y as i32 }, MONITOR_DEFAULTTONEAREST), &mut info).as_bool() };
    let r = info.rcWork;
    ok.then(|| (f64::from(r.left), f64::from(r.top), f64::from(r.right - r.left), f64::from(r.bottom - r.top)))
}

/// Where the cursor is: the monitor the user is looking at when the full window comes back.
pub fn cursor() -> (f64, f64) {
    use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut point = POINT::default();
    // SAFETY: a plain out-parameter.
    let _ = unsafe { GetCursorPos(&mut point) };
    (f64::from(point.x), f64::from(point.y))
}

#[cfg(test)]
mod tests {
    use super::position;

    // The Tauri app's cases (tray_window_position).
    #[test]
    fn the_panel_sits_by_the_tray_inside_the_work_area() {
        assert_eq!(position((600.0, 450.0, 20.0, 20.0), (700.0, 680.0), (0.0, 0.0, 640.0, 480.0)), (0.0, 0.0));
        assert_eq!(position((-30.0, 1040.0, 20.0, 40.0), (430.0, 660.0), (-1920.0, 0.0, 1920.0, 1040.0)), (-440.0, 380.0));
        assert_eq!(position((-1900.0, 0.0, 20.0, 30.0), (430.0, 660.0), (-1920.0, 30.0, 1920.0, 1050.0)), (-1920.0, 30.0));
        // Bottom taskbar on a 1080p screen: above the icon, its right edge on the icon's.
        assert_eq!(position((1800.0, 1050.0, 24.0, 30.0), (430.0, 660.0), (0.0, 0.0, 1920.0, 1040.0)), (1394.0, 380.0));
    }
}
