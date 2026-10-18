//! UI v4 tokens (client/src/jsMain/resources/ui.css) as an iced theme.
use iced::{Border, Color, Font, Theme, color, theme::Palette, widget::container};

pub const BARLOW: &[u8] = include_bytes!("../../../client/src/jsMain/resources/fonts/barlow-regular.ttf");
pub const BARLOW_BOLD: &[u8] = include_bytes!("../../../client/src/jsMain/resources/fonts/barlow-bold.ttf");
pub const FONT: Font = Font::with_name("Barlow");
pub const BOLD: Font = Font { weight: iced::font::Weight::Bold, ..FONT };

pub const SURFACE: Color = color!(0x171d26);
pub const VIDEO: Color = color!(0x13171e);
pub const LINE: Color = color!(0x354151);
pub const DIM: Color = color!(0xaab6c8);
pub const FAINT: Color = color!(0x8592a6);
pub const GREEN: Color = color!(0x60c99b);
pub const RED: Color = color!(0xef8791);
pub const RADIUS: f32 = 10.0;

pub fn theme() -> Theme {
    Theme::custom(
        "Screenshare v4",
        Palette {
            background: color!(0x101318),
            text: color!(0xeef2fa),
            primary: color!(0x8eb4fa),
            success: GREEN,
            warning: color!(0xe8c36a),
            danger: color!(0x9b3845),
        },
    )
}

pub fn panel(_: &Theme) -> container::Style {
    container::Style {
        background: Some(SURFACE.into()),
        border: Border { color: LINE, width: 1.0, radius: RADIUS.into() },
        ..Default::default()
    }
}

pub fn stage(_: &Theme) -> container::Style {
    container::Style {
        background: Some(VIDEO.into()),
        border: Border { color: LINE, width: 1.0, radius: RADIUS.into() },
        ..Default::default()
    }
}

/// Chat bubble (.chat-bubble / .chat-bubble-primary).
pub fn bubble(own: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(if own { color!(0x2649cc) } else { color!(0x222b38) }.into()),
        text_color: own.then_some(Color::WHITE),
        border: Border { radius: 8.0.into(), ..Default::default() },
        ..Default::default()
    }
}

/// Toast (.notice).
pub fn notice(_: &Theme) -> container::Style {
    container::Style {
        background: Some(color!(0x2a1a21).into()),
        border: Border { color: RED, width: 1.0, radius: 9.0.into() },
        ..Default::default()
    }
}

/// Selected tile in the share wizard.
pub fn picked(_: &Theme) -> container::Style {
    container::Style {
        background: Some(color!(0x2b3546).into()),
        border: Border { color: color!(0x8eb4fa), width: 2.0, radius: RADIUS.into() },
        ..Default::default()
    }
}
