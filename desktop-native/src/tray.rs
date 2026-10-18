//! The tray icon and its menu, with the Tauri app's texts. Left click opens the window;
//! closing the window keeps the app here, and "Encerrar app" quits.
use std::cell::RefCell;

use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

/// What the tray asked for, read on every UI tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Mic,
    Camera,
    Output,
    Screen,
    Leave,
    Quit,
}

/// What the menu shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub in_room: bool,
    pub connected: bool,
    pub mic: bool,
    pub camera: bool,
    pub output_muted: bool,
    pub sharing: bool,
    pub participants: usize,
    pub remote_sharing: bool,
}

struct Tray {
    icon: TrayIcon,
    mic: MenuItem,
    camera: MenuItem,
    output: MenuItem,
    screen: MenuItem,
    leave: MenuItem,
    shown: State,
}

thread_local! {
    // TrayIcon is tied to the thread that pumps its messages: iced's main thread.
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

const ICON: &[u8] = include_bytes!("../../desktop-tauri/src-tauri/icons/32x32.png");

/// The app icon, also used for the window.
pub fn icon_rgba() -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory_with_format(ICON, image::ImageFormat::Png).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some((image.into_raw(), width, height))
}

/// Creates the tray; without one (Explorer down) the app still works, closing quits then.
pub fn install() -> bool {
    let build = || -> Result<Tray, Box<dyn std::error::Error>> {
        let (rgba, width, height) = icon_rgba().ok_or("icon")?;
        let item = |id: &str, text: &str, enabled: bool| MenuItem::with_id(id, text, enabled, None);
        let (open, mic, camera, output, screen, leave, quit) = (
            item("open", "Abrir sala", true),
            item("mic", "Microfone silenciado · Ativar", false),
            item("camera", "Câmera desligada · Ligar", false),
            item("output", "Silenciar áudio", true),
            item("screen", "Compartilhar tela ou janela…", false),
            item("leave", "Sair da sala", false),
            item("quit", "Encerrar app", true),
        );
        let menu = Menu::new();
        menu.append_items(&[
            &open,
            &PredefinedMenuItem::separator(),
            &mic,
            &camera,
            &output,
            &screen,
            &PredefinedMenuItem::separator(),
            &leave,
            &PredefinedMenuItem::separator(),
            &quit,
        ])?;
        let icon = TrayIconBuilder::new()
            .with_icon(tray_icon::Icon::from_rgba(rgba, width, height)?)
            .with_tooltip("Screenshare")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?;
        Ok(Tray { icon, mic, camera, output, screen, leave, shown: State::default() })
    };
    match build() {
        Ok(tray) => {
            TRAY.with(|slot| *slot.borrow_mut() = Some(tray));
            true
        }
        Err(error) => {
            eprintln!("tray unavailable: {error}");
            false
        }
    }
}

/// Clicks and menu picks since the last call.
pub fn actions() -> Vec<Action> {
    let mut actions = Vec::new();
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            actions.push(Action::Open);
        }
    }
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        actions.extend(match event.id().as_ref() {
            "open" => Some(Action::Open),
            "mic" => Some(Action::Mic),
            "camera" => Some(Action::Camera),
            "output" => Some(Action::Output),
            "screen" => Some(Action::Screen),
            "leave" => Some(Action::Leave),
            "quit" => Some(Action::Quit),
            _ => None,
        });
    }
    actions
}

/// set_tray_state in the Tauri app; only touches the menu when something changed.
pub fn update(state: State) {
    TRAY.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(tray) = slot.as_mut() else { return };
        if tray.shown == state {
            return;
        }
        tray.mic.set_text(if state.mic { "Microfone ativo · Silenciar" } else { "Microfone silenciado · Ativar" });
        tray.camera.set_text(if state.camera { "Câmera ativa · Parar" } else { "Câmera desligada · Ligar" });
        tray.output.set_text(if state.output_muted { "Áudio silenciado · Ativar" } else { "Silenciar áudio" });
        tray.screen.set_text(if state.sharing { "Compartilhando a tela · Parar" } else { "Compartilhar tela ou janela…" });
        for item in [&tray.mic, &tray.camera, &tray.screen] {
            item.set_enabled(state.connected);
        }
        tray.output.set_enabled(state.connected);
        tray.leave.set_enabled(state.in_room);
        let _ = tray.icon.set_tooltip(Some(tooltip(&state)));
        tray.shown = state;
    });
}

fn tooltip(state: &State) -> String {
    let mut text = format!("Screenshare · {} participantes", state.participants);
    if state.sharing {
        text += " · Compartilhando a tela";
    } else if state.remote_sharing {
        text += " · compartilhamento disponível";
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_and_icon_like_the_tauri_tray() {
        assert_eq!(tooltip(&State { participants: 3, ..State::default() }), "Screenshare · 3 participantes");
        assert_eq!(tooltip(&State { participants: 2, remote_sharing: true, ..State::default() }), "Screenshare · 2 participantes · compartilhamento disponível");
        assert_eq!(tooltip(&State { participants: 2, sharing: true, remote_sharing: true, ..State::default() }), "Screenshare · 2 participantes · Compartilhando a tela");
        let (rgba, width, height) = icon_rgba().unwrap();
        assert_eq!((width, height, rgba.len()), (32, 32, 32 * 32 * 4));
    }
}
