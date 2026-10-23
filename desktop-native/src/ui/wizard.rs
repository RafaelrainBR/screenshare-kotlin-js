//! Share wizard, like the web #quality-modal: pick a screen or window (with thumbnails),
//! the profile and whether to send its audio.
use std::collections::HashMap;

use desktop_native::{
    ScreenAudioMode, ShareSource,
    sources::{Source, SourceKind},
    session::ScreenOptions,
    share::{SharePreset, ShareProfile},
};
use iced::{
    Alignment, Element, Length, Task,
    widget::{Column, button, column, container, image, pick_list, row, scrollable, space, text},
};

use super::{motion, profile, theme};
use crate::Message;

pub struct Wizard {
    /// Screens first, then windows by title, as the web lists them.
    pub sources: Option<Vec<Source>>,
    /// None: the capture failed (usually a minimized window).
    pub thumbs: HashMap<String, Option<image::Handle>>,
    /// When each thumbnail arrived: it fades in instead of popping in.
    pub thumbs_at: HashMap<String, std::time::Instant>,
    pub selected: Option<Source>,
    pub audio: bool,
    /// A listing is in flight, so the 2 s refresh does not stack them.
    pub loading: bool,
}

impl Wizard {
    pub fn open() -> Self {
        Self { sources: None, thumbs: HashMap::new(), thumbs_at: HashMap::new(), selected: None, audio: false, loading: true }
    }

    pub fn set_thumbnail(&mut self, id: String, handle: Option<image::Handle>) {
        self.thumbs_at.entry(id.clone()).or_insert_with(std::time::Instant::now);
        self.thumbs.insert(id, handle);
    }

    pub fn animating(&self) -> bool {
        self.thumbs_at.values().any(|at| motion::running(Some(*at), motion::ENTER))
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
                desktop_native::sources::enumerate().into_iter().filter(|s| s.pid != own || s.kind == SourceKind::Screen).collect()
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
    let (width, height, rgba) = desktop_native::sources::thumbnail(source)?;
    Some(image::Handle::from_rgba(width, height, rgba))
}

fn same(a: &Source, b: &Source) -> bool {
    a.id == b.id && a.pid == b.pid
}

impl Wizard {
    /// Takes a fresh listing and returns the sources that still need a thumbnail.
    /// A closed window disappears and can no longer be chosen.
    pub fn set_sources(&mut self, mut sources: Vec<Source>) -> Vec<Source> {
        self.loading = false;
        sources.sort_by_cached_key(|s| (s.kind != SourceKind::Screen, if s.kind == SourceKind::Screen { String::new() } else { s.title.to_lowercase() }));
        self.selected = self.selected.take().and_then(|picked| sources.iter().find(|s| same(s, &picked)).cloned());
        let missing = sources.iter().filter(|s| !self.thumbs.contains_key(&s.id)).cloned().collect();
        self.sources = Some(sources);
        missing
    }

    pub fn options(&self, profile: ShareProfile) -> Option<ScreenOptions> {
        let source = self.selected.as_ref()?;
        let (left, top, right, bottom) = source.rect;
        let height = (bottom - top).max(1) as u32;
        let window = source.kind == SourceKind::Window;
        Some(ScreenOptions {
            source: if window {
                ShareSource::Window { hwnd: source.hwnd, pid: source.pid }
            } else {
                ShareSource::Screen { left, top, width: (right - left).max(1) as u32, height }
            },
            profile,
            // The bitrate ceiling follows the source height, capped like the web presets at 1080p.
            height: height.min(1080),
            // Like nativeAudioMode in ScreenSharing.kt: a window sends its app, a screen the PC.
            audio_mode: match (self.audio, window) {
                (false, _) => ScreenAudioMode::None,
                (true, true) => ScreenAudioMode::Application,
                (true, false) => ScreenAudioMode::Computer,
            },
        })
    }

    pub fn select(&mut self, id: &str) {
        self.selected = self.sources.iter().flatten().find(|s| s.id == id).cloned();
    }

    /// Arrow keys: move the selection through the tiles.
    pub fn step(&mut self, delta: isize) {
        let Some(sources) = &self.sources else { return };
        if sources.is_empty() {
            return;
        }
        let current = sources.iter().position(|s| self.selected.as_ref().is_some_and(|picked| same(s, picked)));
        let next = match current {
            Some(i) => (i as isize + delta).rem_euclid(sources.len() as isize) as usize,
            None => 0,
        };
        self.selected = Some(sources[next].clone());
    }

    /// Texts of setShareAudio in InterfaceHandlers.kt.
    fn audio_texts(&self) -> (&'static str, String) {
        match &self.selected {
            None => ("Áudio", "Escolha uma tela ou janela.".into()),
            Some(s) if s.kind == SourceKind::Screen => {
                ("Áudio do computador", "Exclui o Discord desktop já aberto. Se abrir ou reiniciar o Discord, compartilhe novamente.".into())
            }
            Some(s) => ("Áudio do aplicativo", format!("Enviar só o som de {}.", s.title)),
        }
    }

    /// updateSourceState in InterfaceHandlers.kt.
    fn state(&self, profile: ShareProfile) -> &'static str {
        match (&self.sources, &self.selected) {
            (None, _) => "Carregando telas e janelas…",
            (Some(sources), _) if sources.is_empty() => "Nenhuma tela ou janela disponível agora.",
            (_, None) => "Escolha o que você quer compartilhar.",
            (_, Some(s)) if s.kind == SourceKind::Window && profile.fps > 30 => "Janelas são enviadas a até 30 fps.",
            _ => "",
        }
    }

    fn tile<'a>(&'a self, source: &'a Source) -> Element<'a, Message> {
        let preview: Element<'_, Message> = match self.thumbs.get(&source.id) {
            Some(Some(handle)) => {
                let shown = self.thumbs_at.get(&source.id).map_or(1.0, |at| motion::progress(*at, motion::ENTER));
                image(handle.clone()).width(Length::Fill).height(124).opacity(shown).into()
            }
            Some(None) => container(text("Sem preview (janela minimizada?)").size(11).color(theme::DIM)).center_x(Length::Fill).center_y(124).into(),
            None => container(text("Carregando preview…").size(11).color(theme::DIM)).center_x(Length::Fill).center_y(124).into(),
        };
        let picked = self.selected.as_ref().is_some_and(|s| same(s, source));
        let tile = button(column![preview, text(&source.title).size(13)].spacing(7))
            .padding(8)
            .width(240)
            .style(move |t, status| theme::tile(picked, t, status))
            .on_press(Message::Pick(source.id.clone()));
        motion::press(tile, 0.98)
    }

    /// `advanced`: the "Ajustes avançados" section is open.
    pub fn view(&self, profile: ShareProfile, advanced: bool) -> Element<'_, Message> {
        let mut list = Column::new().spacing(8);
        if let Some(sources) = &self.sources {
            for (kind, label) in [(SourceKind::Screen, "Telas"), (SourceKind::Window, "Janelas")] {
                let group: Vec<_> = sources.iter().filter(|s| s.kind == kind).collect();
                if group.is_empty() {
                    continue;
                }
                list = list.push(text(label).size(11).font(theme::BOLD).color(theme::DIM));
                list = list.push(group.into_iter().fold(row![].spacing(8), |tiles, s| tiles.push(self.tile(s))).wrap().vertical_spacing(8));
            }
        }
        let (audio_title, audio_desc) = self.audio_texts();
        let audio = button(
            row![
                column![text(audio_title).font(theme::BOLD), text(audio_desc).size(11).color(theme::DIM)].spacing(3).width(Length::Fill),
                theme::switch(self.audio),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([10, 12])
        .width(Length::Fill)
        .style(|t, status| theme::tile(false, t, status))
        .on_press(Message::ToggleShareAudio);
        let presets: Vec<_> = SharePreset::ALL.into_iter().map(PresetChoice).collect();
        let preset = pick_list(presets, profile::matching(profile).map(PresetChoice), |c| Message::SetProfile(c.0.profile()))
            .placeholder("Personalizado")
            .width(Length::Fill);
        // #share-quality: a summary that opens the three fields; the native capture keeps the source size.
        let mut quality = column![
            button(
                row![
                    text(if advanced { "▾" } else { "▸" }).color(theme::DIM),
                    text("Ajustes avançados").size(12).font(theme::BOLD),
                    text(profile::summary(profile)).size(12).color(theme::DIM),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([4, 0])
            .style(button::text)
            .on_press(Message::ToggleAdvanced),
        ]
        .spacing(6);
        if advanced {
            let fields = profile::Field::ALL.into_iter().fold(row![].spacing(8), |line, field| {
                line.push(
                    column![
                        text(field.title()).size(12).font(theme::BOLD),
                        pick_list(field.choices(), field.current(profile), move |choice| Message::SetProfile(field.set(profile, choice.value))).width(Length::Fill),
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                )
            });
            quality = quality.push(fields);
            if profile.max_bitrate.is_some() {
                quality = quality.push(text("Acima do seu upload, a imagem trava para todos.").size(11).color(theme::DIM));
            }
            quality = quality.push(text(profile::LIVE_NOTE).size(11).color(theme::DIM));
        }
        let start = button(text("Compartilhar").font(theme::BOLD)).padding([8, 16]).style(button::primary);
        column![
            text("Compartilhamento de tela").size(12).color(theme::DIM),
            text("Preparar compartilhamento").size(20).font(theme::BOLD),
            text("Escolha a tela ou janela e o áudio que quer enviar.").color(theme::DIM),
            text("Setas escolhem · A liga o áudio · P muda o preset · Enter compartilha · Esc cancela").size(11).color(theme::FAINT),
            row![
                text("O que compartilhar").size(12).font(theme::BOLD).width(Length::Fill),
                button(text("Atualizar").size(13)).style(button::text).on_press(Message::RefreshSources(true)),
            ]
            .align_y(Alignment::Center),
            scrollable(list).height(Length::Fill),
            text(self.state(profile)).size(12).color(theme::DIM),
            text("O que você vai mostrar").size(12).font(theme::BOLD),
            preset,
            quality,
            audio,
            row![
                space::horizontal(),
                button("Cancelar").style(button::text).on_press(Message::CloseWizard),
                if self.options(profile).is_some() { start.on_press(Message::StartShare) } else { start },
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
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
        let mut w = Wizard::open();
        w.set_sources(vec![source("window", SourceKind::Window, (10, 20, 970, 560)), source("screen", SourceKind::Screen, (-1920, 0, 0, 1440))]);
        w
    }

    #[test]
    fn maps_sources_to_screen_options() {
        let mut w = wizard();
        assert!(w.options(SharePreset::Video.profile()).is_none(), "nothing picked yet");
        w.step(1);
        let screen = w.options(SharePreset::Video.profile()).unwrap();
        assert_eq!(screen.source, ShareSource::Screen { left: -1920, top: 0, width: 1920, height: 1440 }, "screens come first");
        assert_eq!(screen.height, 1080, "bitrate height capped at 1080p");
        w.step(1);
        let window = w.options(SharePreset::Text.profile()).unwrap();
        assert_eq!(window.source, ShareSource::Window { hwnd: 42, pid: 7 });
        assert_eq!((window.height, window.profile), (540, SharePreset::Text.profile()));
    }

    #[test]
    fn audio_switch_follows_the_source_kind() {
        let mut w = wizard();
        w.audio = true;
        w.step(1);
        assert_eq!(w.options(SharePreset::Video.profile()).unwrap().audio_mode, ScreenAudioMode::Computer);
        w.step(1);
        assert_eq!(w.options(SharePreset::Video.profile()).unwrap().audio_mode, ScreenAudioMode::Application);
        w.audio = false;
        assert_eq!(w.options(SharePreset::Video.profile()).unwrap().audio_mode, ScreenAudioMode::None);
    }

    #[test]
    fn refresh_keeps_the_pick_and_only_new_sources_need_thumbnails() {
        let mut w = wizard();
        w.select("window");
        w.thumbs.insert("screen".into(), None);
        w.thumbs.insert("window".into(), None);
        let fresh = w.set_sources(vec![source("screen", SourceKind::Screen, (0, 0, 1, 1)), source("window", SourceKind::Window, (0, 0, 1, 1)), source("new", SourceKind::Window, (0, 0, 1, 1))]);
        assert_eq!(fresh.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["new"]);
        assert_eq!(w.selected.as_ref().map(|s| s.id.as_str()), Some("window"));
        w.set_sources(vec![source("screen", SourceKind::Screen, (0, 0, 1, 1))]);
        assert!(w.selected.is_none(), "a closed window can no longer be chosen");
    }
}
