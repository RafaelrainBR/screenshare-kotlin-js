//! The share profile (fps, "Se piorar", bitrate) edited by the wizard's advanced settings and the
//! dock's transmission menu, like the web #share-quality and #share-picker. Changes reach a
//! running share live.
use std::path::PathBuf;

use desktop_native::share::{SharePreset, SharePriority, ShareProfile};
use iced::{
    Alignment, Element, Length,
    widget::{Column, button, column, container, row, text},
};

use super::theme;
use crate::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Fps,
    Priority,
    Bitrate,
}

/// One entry of a field's list, as the web <select> options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub label: &'static str,
    pub value: u64,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

const fn c(label: &'static str, value: u64) -> Choice {
    Choice { label, value }
}

impl Field {
    pub const ALL: [Self; 3] = [Self::Fps, Self::Priority, Self::Bitrate];

    /// The wizard label; the menu row uses the short one.
    pub fn title(self) -> &'static str {
        match self {
            Self::Fps => "Quadros por segundo",
            Self::Priority => "Se a conexão piorar",
            Self::Bitrate => "Bitrate máximo",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Fps => "Quadros/s",
            Self::Priority => "Se piorar",
            Self::Bitrate => "Bitrate",
        }
    }

    pub fn choices(self) -> &'static [Choice] {
        const FPS: [Choice; 5] = [c("5 fps", 5), c("10 fps", 10), c("15 fps", 15), c("30 fps", 30), c("60 fps", 60)];
        const PRIORITY: [Choice; 3] = [c("Manter nitidez", 0), c("Equilibrar", 1), c("Manter fluidez", 2)];
        const BITRATE: [Choice; 5] = [c("Automático", 0), c("4 Mbps", 4_000_000), c("8 Mbps", 8_000_000), c("15 Mbps", 15_000_000), c("25 Mbps", 25_000_000)];
        match self {
            Self::Fps => &FPS,
            Self::Priority => &PRIORITY,
            Self::Bitrate => &BITRATE,
        }
    }

    fn value(self, profile: ShareProfile) -> u64 {
        match self {
            Self::Fps => profile.fps.into(),
            Self::Priority => priority_index(profile.priority),
            Self::Bitrate => profile.max_bitrate.unwrap_or(0),
        }
    }

    /// The chosen entry; a value outside the list (none today) shows no selection.
    pub fn current(self, profile: ShareProfile) -> Option<Choice> {
        self.choices().iter().copied().find(|choice| choice.value == self.value(profile))
    }

    pub fn set(self, profile: ShareProfile, value: u64) -> ShareProfile {
        match self {
            Self::Fps => ShareProfile { fps: value as u32, ..profile },
            Self::Priority => ShareProfile { priority: PRIORITIES[value as usize % 3], ..profile },
            Self::Bitrate => ShareProfile { max_bitrate: (value > 0).then_some(value), ..profile },
        }
    }
}

const PRIORITIES: [SharePriority; 3] = [SharePriority::Sharpness, SharePriority::Balanced, SharePriority::Smoothness];

fn priority_index(priority: SharePriority) -> u64 {
    PRIORITIES.iter().position(|p| *p == priority).unwrap_or(1) as u64
}

pub fn preset_name(preset: SharePreset) -> &'static str {
    match preset {
        SharePreset::Game => "Jogo",
        SharePreset::Video => "Vídeo",
        SharePreset::Text => "Código e texto",
    }
}

/// SharePreset.matching: None once the profile was customized away from every preset.
pub fn matching(profile: ShareProfile) -> Option<SharePreset> {
    SharePreset::ALL.into_iter().find(|preset| preset.profile() == profile)
}

/// The #share-quality-summary line, without the resolution the native capture does not choose.
pub fn summary(profile: ShareProfile) -> String {
    let priority = ["Nitidez", "Equilíbrio", "Fluidez"][priority_index(profile.priority) as usize];
    let bitrate = profile.max_bitrate.map_or("Bitrate auto".into(), |b| format!("{} Mbps", b / 1_000_000));
    let custom = matching(profile).is_none().then_some("Personalizado".to_owned());
    custom.into_iter().chain([format!("{} fps", profile.fps), priority.into(), bitrate]).collect::<Vec<_>>().join(" · ")
}

/// The web keeps the profile in localStorage; here it is one small JSON file per user.
fn path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Screenshare-native").join("share-profile.json"))
}

pub fn load() -> ShareProfile {
    path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str::<ShareProfile>(&json).ok())
        .filter(|profile| (1..=60).contains(&profile.fps))
        .unwrap_or_else(|| SharePreset::Video.profile())
}

pub fn save(profile: ShareProfile) {
    if let (Some(path), Ok(json)) = (path(), serde_json::to_string(&profile)) {
        let _ = path.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(path, json);
    }
}

/// Which page of the dock menu is open: the main list, or one field's options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Main,
    Field(Field),
}

fn row_button<'a>(content: Element<'a, Message>, message: Message) -> Element<'a, Message> {
    button(content).padding(8).width(Length::Fill).style(button::text).on_press(message).into()
}

/// #share-picker: presets, the three fields, and the source; "Ao vivo" while sharing.
pub fn menu(page: Menu, profile: ShareProfile, live: bool) -> Element<'static, Message> {
    let body: Column<'static, Message> = match page {
        Menu::Main => {
            let state = container(text(if live { "Ao vivo" } else { "Próxima tela" }).size(11)).padding([2, 8]).style(move |_: &iced::Theme| theme::chip(live));
            let preset = matching(profile);
            let presets = SharePreset::ALL.into_iter().fold(row![].spacing(4), |line, p| {
                line.push(
                    button(container(text(preset_name(p)).size(12)).center_x(Length::Fill))
                        .padding([8, 4])
                        .width(Length::Fill)
                        .style(move |t, status| theme::tile(preset == Some(p), t, status))
                        .on_press(Message::SetProfile(p.profile())),
                )
            });
            let mut list = column![row![text("Transmissão").font(theme::BOLD).width(Length::Fill), state].align_y(Alignment::Center).padding([4, 6]), presets].spacing(6);
            if preset.is_none() {
                list = list.push(text("Personalizado").size(11).color(theme::DIM).width(Length::Fill));
            }
            for field in Field::ALL {
                let value = field.current(profile).map_or("", |choice| choice.label);
                list = list.push(row_button(
                    row![text(field.short()).width(Length::Fill), text(value).color(theme::DIM), text("›").color(theme::DIM)].spacing(8).into(),
                    Message::ShareMenu(Some(Menu::Field(field))),
                ));
            }
            list.push(row_button(text(if live { "Trocar tela ou janela…" } else { "Escolher tela ou janela…" }).into(), Message::OpenWizard))
        }
        Menu::Field(field) => {
            let current = field.current(profile);
            field.choices().iter().fold(
                column![row_button(row![text("‹").color(theme::DIM), text(field.title()).font(theme::BOLD)].spacing(8).into(), Message::ShareMenu(Some(Menu::Main)))],
                |list, choice| {
                    let checked = current == Some(*choice);
                    let label = text(choice.label).color(if checked { theme::ACCENT } else { theme::TEXT });
                    list.push(row_button(
                        row![label.width(Length::Fill), text(if checked { "✓" } else { "" }).color(theme::ACCENT)].into(),
                        Message::SetProfile(field.set(profile, choice.value)),
                    ))
                },
            )
        }
    };
    container(body.spacing(2)).padding(6).width(290).style(theme::modal).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_edit_the_profile_and_presets_match_like_the_web() {
        let video = SharePreset::Video.profile();
        assert_eq!(summary(video), "30 fps · Equilíbrio · Bitrate auto");
        let custom = Field::Bitrate.set(Field::Fps.set(video, 15), 8_000_000);
        assert_eq!(matching(custom), None);
        assert_eq!(summary(custom), "Personalizado · 15 fps · Equilíbrio · 8 Mbps");
        assert_eq!(Field::Priority.set(video, 0).priority, SharePriority::Sharpness);
        assert_eq!(Field::Bitrate.set(custom, 0).max_bitrate, None, "Automático clears the ceiling");
        assert_eq!(matching(Field::Priority.set(Field::Fps.set(video, 60), 2)), Some(SharePreset::Game));
        for field in Field::ALL {
            assert!(field.current(video).is_some(), "every preset value is in the lists");
        }
    }
}
