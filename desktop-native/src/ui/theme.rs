//! UI v4 tokens (client/src/jsMain/resources/ui.css) as an iced theme.
use iced::{
    Border, Color, Element, Font, Theme, alignment::Horizontal, color,
    theme::Palette,
    widget::{button, container, text},
};

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
pub const ACCENT: Color = color!(0x8eb4fa);
pub const TEXT: Color = color!(0xeef2fa);
pub const SURFACE_2: Color = color!(0x222b38);
pub const HOVER: Color = color!(0x2b3546);
pub const RED_LINE: Color = color!(0xa8525e);
pub const RADIUS: f32 = 10.0;
pub const ON_ACCENT: Color = color!(0x0c1022);
pub const LINE_2: Color = color!(0x4a586c);
pub const BG: Color = color!(0x101318);

/// .glass-chip: rgba(16,19,24,.92) with a line-2 border.
pub fn glass(status: button::Status) -> button::Style {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(if hover { HOVER } else { Color { a: 0.92, ..BG } }.into()),
        text_color: TEXT,
        border: Border { color: LINE_2, width: 1.0, radius: 8.0.into() },
        ..Default::default()
    }
}

/// .room-code and .chat-hide: surface-2 with a line border.
pub fn chip_button(status: button::Status) -> button::Style {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(if hover { HOVER } else { SURFACE_2 }.into()),
        text_color: TEXT,
        border: Border { color: LINE, width: 1.0, radius: 7.0.into() },
        ..Default::default()
    }
}

/// button.participant-options on a participant tile.
pub fn options_button(status: button::Status) -> button::Style {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(if hover { HOVER } else { SURFACE }.into()),
        text_color: TEXT,
        border: Border { color: LINE, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    }
}

/// .input: page background, line border, accent while focused.
pub fn input(_: &Theme, status: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    let focused = matches!(status, iced::widget::text_input::Status::Focused { .. });
    iced::widget::text_input::Style {
        background: BG.into(),
        border: Border { color: if focused { ACCENT } else { LINE }, width: 1.0, radius: 8.0.into() },
        icon: DIM,
        placeholder: FAINT,
        value: TEXT,
        selection: Color { a: 0.35, ..ACCENT },
    }
}

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

/// Toast (.notice), at `alpha` while it fades in or out.
pub fn notice(alpha: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Color { a: alpha, ..color!(0x2a1a21) }.into()),
        text_color: Some(Color { a: alpha, ..TEXT }),
        border: Border { color: Color { a: alpha, ..RED }, width: 1.0, radius: 9.0.into() },
        ..Default::default()
    }
}

/// .audio-source-tile / .switch-row: surface-2 with a line border, accent when picked.
pub fn tile(picked: bool, _: &Theme, status: button::Status) -> button::Style {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(if hover { color!(0x2b3546) } else { color!(0x222b38) }.into()),
        text_color: color!(0xeef2fa),
        border: Border { color: if picked { color!(0x8eb4fa) } else { LINE }, width: 1.0, radius: 7.0.into() },
        ..Default::default()
    }
}

/// .switch: a 34×20 track with a 14 px knob, accent when on.
pub fn switch<'a, M: 'a>(on: bool) -> Element<'a, M> {
    let knob = container(text("")).width(14).height(14).style(|_| container::Style {
        background: Some(color!(0xeef2fa).into()),
        border: Border { radius: 7.0.into(), ..Default::default() },
        ..Default::default()
    });
    container(knob)
        .width(34)
        .height(20)
        .padding(3)
        .align_x(if on { Horizontal::Right } else { Horizontal::Left })
        .style(move |_| container::Style {
            background: Some(if on { color!(0x8eb4fa) } else { LINE }.into()),
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

/// .screen-tile: video background, accent border while focused.
/// `speaking`, 0 to 1, eases the border to green while its owner talks (.screen-tile.speaking).
pub fn video_tile(focused: bool, speaking: f32) -> button::Style {
    let base = if focused { color!(0x8eb4fa) } else { LINE };
    button::Style {
        background: Some(VIDEO.into()),
        text_color: color!(0xeef2fa),
        border: Border { color: super::motion::mix(base, GREEN, speaking), width: 1.0, radius: RADIUS.into() },
        ..Default::default()
    }
}

/// .screens-rail .tile-name
pub fn name_chip(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color { a: 0.85, ..color!(0x101318) }.into()),
        border: Border { radius: 6.0.into(), ..Default::default() },
        ..Default::default()
    }
}

/// .participant-tile
pub fn participant_tile(_: &Theme) -> container::Style {
    container::Style {
        background: Some(color!(0x222b38).into()),
        border: Border { color: LINE, width: 1.0, radius: RADIUS.into() },
        ..Default::default()
    }
}

/// .avatar-circle; `speaking`, 0 to 1, eases in its green 2 px ring (.avatar-circle.speaking).
pub fn avatar(speaking: f32) -> container::Style {
    container::Style {
        background: Some(LINE.into()),
        border: Border { color: Color { a: speaking.clamp(0.0, 1.0), ..GREEN }, width: 2.0, radius: 23.0.into() },
        ..Default::default()
    }
}

/// .mute-overlay: red-strong dot ringed by the tile colour.
pub fn mute_badge(_: &Theme) -> container::Style {
    container::Style {
        background: Some(color!(0x9b3845).into()),
        text_color: Some(Color::WHITE),
        border: Border { color: color!(0x222b38), width: 2.0, radius: 9.0.into() },
        ..Default::default()
    }
}

/// .share-menu-state: accent while the share is live.
pub fn chip(live: bool) -> container::Style {
    container::Style {
        background: Some(if live { ACCENT } else { color!(0x222b38) }.into()),
        text_color: Some(if live { color!(0x0c1022) } else { DIM }),
        border: Border { radius: 999.0.into(), ..Default::default() },
        ..Default::default()
    }
}

/// .modal-backdrop fading in: `shown`, 0 to 1, of its 68% black.
pub fn backdrop_at(shown: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style { background: Some(Color { a: 0.68 * shown, ..Color::BLACK }.into()), ..Default::default() }
}

/// .modal-panel
pub fn modal(_: &Theme) -> container::Style {
    container::Style {
        background: Some(SURFACE.into()),
        border: Border { color: color!(0x4a586c), width: 1.0, radius: 13.0.into() },
        ..Default::default()
    }
}

