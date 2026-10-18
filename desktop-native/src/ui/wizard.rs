//! Share wizard: pick a screen or window (with thumbnails), the audio mode and the preset.
use std::collections::HashMap;

use desktop_capture::{
    sources::{Source, SourceKind, enumerate_sources},
    thumb::capture_thumbnail,
};
use desktop_native::{ScreenAudioMode, ShareSource, session::ScreenOptions, share::SharePreset};
use iced::{
    Element, Length, Task,
    widget::{button, column, container, image, pick_list, row, scrollable, text},
};
use windows::Win32::Foundation::{HWND, RECT};

use super::theme;
use crate::Message;

#[derive(Default)]
pub struct Wizard {
    pub sources: Option<Vec<Source>>,
    pub thumbs: HashMap<String, image::Handle>,
    pub selected: Option<String>,
    pub audio: ScreenAudioMode,
    pub preset: Option<SharePreset>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioChoice(pub ScreenAudioMode);

impl std::fmt::Display for AudioChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            ScreenAudioMode::None => "Sem áudio",
            ScreenAudioMode::Application => "Áudio do aplicativo",
            ScreenAudioMode::Computer => "Áudio do PC (sem Discord)",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetChoice(pub SharePreset);

impl std::fmt::Display for PresetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            SharePreset::Game => "Jogo · ação fluida, 60 fps",
            SharePreset::Video => "Vídeo · filmes e séries, 30 fps",
            SharePreset::Text => "Código e texto · letras nítidas, 10 fps",
        })
    }
}

/// Lists sources off the UI thread, skipping this app's own windows.
pub fn load_sources() -> Task<Message> {
    Task::perform(
        async {
            let own = std::process::id();
            tokio::task::spawn_blocking(move || {
                enumerate_sources().into_iter().filter(|s| s.pid != own || s.kind == SourceKind::Screen).collect()
            })
            .await
            .unwrap_or_default()
        },
        Message::Sources,
    )
}

/// One task per tile; desktop-capture serializes the captures itself.
pub fn load_thumbnails(sources: &[Source]) -> Task<Message> {
    Task::batch(sources.iter().cloned().map(|source| {
        Task::perform(
            async move {
                let id = source.id.clone();
                let handle = tokio::task::spawn_blocking(move || thumbnail(&source)).await.ok().flatten();
                (id, handle)
            },
            |(id, handle)| Message::Thumbnail(id, handle),
        )
    }))
}

fn thumbnail(source: &Source) -> Option<image::Handle> {
    let (left, top, right, bottom) = source.rect;
    let screen = (source.kind == SourceKind::Screen).then_some(RECT { left, top, right, bottom });
    let mut thumb = capture_thumbnail(HWND(source.hwnd as usize as *mut _), screen, 320).ok()??;
    // BGRA → RGBA for iced.
    thumb.data.chunks_exact_mut(4).for_each(|px| px.swap(0, 2));
    Some(image::Handle::from_rgba(thumb.width, thumb.height, thumb.data))
}

impl Wizard {
    pub fn options(&self) -> Option<ScreenOptions> {
        let source = self.sources.as_ref()?.iter().find(|s| Some(&s.id) == self.selected.as_ref())?;
        let (left, top, right, bottom) = source.rect;
        let height = (bottom - top).max(1) as u32;
        Some(ScreenOptions {
            source: match source.kind {
                SourceKind::Screen => ShareSource::Screen { left, top, width: (right - left).max(1) as u32, height },
                SourceKind::Window => ShareSource::Window { hwnd: source.hwnd, pid: source.pid },
            },
            profile: self.preset.unwrap_or(SharePreset::Video).profile(),
            // The bitrate ceiling follows the source height, capped like the web presets at 1080p.
            height: height.min(1080),
            audio_mode: self.audio,
        })
    }

    fn selected_is_window(&self) -> bool {
        self.sources.iter().flatten().any(|s| Some(&s.id) == self.selected.as_ref() && s.kind == SourceKind::Window)
    }

    /// Keeps the audio mode valid when the selection changes: app audio needs a window.
    pub fn select(&mut self, id: String) {
        self.selected = Some(id);
        if self.audio == ScreenAudioMode::Application && !self.selected_is_window() {
            self.audio = ScreenAudioMode::None;
        }
    }

    /// Arrow keys: move the selection through the tiles.
    pub fn step(&mut self, delta: isize) {
        let Some(sources) = &self.sources else { return };
        if sources.is_empty() {
            return;
        }
        let current = sources.iter().position(|s| Some(&s.id) == self.selected.as_ref());
        let next = match current {
            Some(i) => (i as isize + delta).rem_euclid(sources.len() as isize) as usize,
            None => 0,
        };
        self.select(sources[next].id.clone());
    }

    pub fn cycle_audio(&mut self) {
        self.audio = match self.audio {
            ScreenAudioMode::None if self.selected_is_window() => ScreenAudioMode::Application,
            ScreenAudioMode::None | ScreenAudioMode::Application => ScreenAudioMode::Computer,
            ScreenAudioMode::Computer => ScreenAudioMode::None,
        };
    }

    pub fn cycle_preset(&mut self) {
        let current = self.preset.unwrap_or(SharePreset::Video);
        let i = SharePreset::ALL.iter().position(|p| *p == current).unwrap_or(0);
        self.preset = Some(SharePreset::ALL[(i + 1) % SharePreset::ALL.len()]);
    }

    pub fn view(&self) -> Element<'_, Message> {
        let Some(sources) = &self.sources else {
            return container(text("Procurando telas e janelas…").color(theme::DIM)).center(Length::Fill).into();
        };
        let tiles = sources.iter().fold(row![].spacing(10), |tiles, source| {
            let preview: Element<'_, Message> = match self.thumbs.get(&source.id) {
                Some(handle) => image(handle.clone()).width(200).height(112).into(),
                None => container(text("…").color(theme::FAINT)).center_x(200).center_y(112).into(),
            };
            let label = if source.kind == SourceKind::Screen { format!("Tela: {}", source.title) } else { source.title.clone() };
            let tile = column![preview, text(label).size(13).width(200)].spacing(6);
            let picked = self.selected.as_ref() == Some(&source.id);
            tiles.push(
                button(container(tile).padding(8).style(if picked { theme::picked } else { theme::panel }))
                    .padding(0)
                    .style(button::text)
                    .on_press(Message::Pick(source.id.clone())),
            )
        });
        let audio_modes: Vec<_> = [ScreenAudioMode::None, ScreenAudioMode::Application, ScreenAudioMode::Computer]
            .into_iter()
            .filter(|mode| *mode != ScreenAudioMode::Application || self.selected_is_window())
            .map(AudioChoice)
            .collect();
        let presets: Vec<_> = SharePreset::ALL.into_iter().map(PresetChoice).collect();
        let start = button(text("Compartilhar").font(theme::BOLD)).padding([8, 16]).style(button::primary);
        column![
            row![
                column![
                    text("Compartilhar tela").size(22).font(theme::BOLD),
                    text("Setas escolhem · A muda o áudio · P muda o preset · Enter compartilha · Esc cancela").size(12).color(theme::FAINT),
                ]
                .width(Length::Fill),
                button("Cancelar").style(button::secondary).on_press(Message::CloseWizard),
            ],
            scrollable(tiles.wrap()).height(Length::Fill),
            row![
                text("Áudio").color(theme::DIM),
                pick_list(audio_modes, Some(AudioChoice(self.audio)), |c| Message::AudioMode(c.0)),
                text("Preset").color(theme::DIM),
                pick_list(presets, Some(PresetChoice(self.preset.unwrap_or(SharePreset::Video))), |c| Message::Preset(c.0)),
                iced::widget::space::horizontal(),
                if self.options().is_some() { start.on_press(Message::StartShare) } else { start },
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(14)
        .padding(16)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: &str, kind: SourceKind, rect: (i32, i32, i32, i32)) -> Source {
        Source { id: id.into(), title: id.into(), hwnd: 42, pid: 7, rect, kind }
    }

    fn wizard() -> Wizard {
        Wizard {
            sources: Some(vec![source("screen", SourceKind::Screen, (-1920, 0, 0, 1440)), source("window", SourceKind::Window, (10, 20, 970, 560))]),
            ..Default::default()
        }
    }

    #[test]
    fn maps_sources_to_screen_options() {
        let mut w = wizard();
        assert!(w.options().is_none(), "nothing picked yet");
        w.step(1);
        let screen = w.options().unwrap();
        assert_eq!(screen.source, ShareSource::Screen { left: -1920, top: 0, width: 1920, height: 1440 });
        assert_eq!(screen.height, 1080, "bitrate height capped at 1080p");
        w.step(1);
        w.cycle_preset();
        let window = w.options().unwrap();
        assert_eq!(window.source, ShareSource::Window { hwnd: 42, pid: 7 });
        assert_eq!((window.height, window.profile), (540, SharePreset::Text.profile()));
    }

    #[test]
    fn app_audio_only_for_windows() {
        let mut w = wizard();
        w.step(1); // screen
        w.cycle_audio();
        assert_eq!(w.audio, ScreenAudioMode::Computer, "screens skip app audio");
        w.cycle_audio();
        w.step(1); // window
        w.cycle_audio();
        assert_eq!(w.audio, ScreenAudioMode::Application);
        w.step(1); // wraps to the screen
        assert_eq!(w.audio, ScreenAudioMode::None, "app audio dropped when a screen is picked");
    }
}
