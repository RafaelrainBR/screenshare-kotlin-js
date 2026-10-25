//! The tray icon and its menu, with the Tauri app's texts. Left click opens the compact panel;
//! closing the window keeps the app here, and "Encerrar app" quits.
use std::cell::RefCell;

use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

/// What the tray asked for, read on every UI tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Open,
    /// Left click: the compact panel, placed by the icon (physical x, y, width, height).
    Panel(f64, f64, f64, f64),
    Mic,
    Camera,
    Output,
    Screen,
    /// "Compartilhar de novo: …", the last source of this run.
    ShareAgain,
    Leave,
    /// "Atualizar para a versão …": install it, or open its page.
    Update,
    /// "Abrir pasta de logs".
    Logs,
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
    /// The update item's text, once a newer release is known.
    pub update: Option<String>,
    /// The last shared source's title, while it can be shared again.
    pub share_again: Option<String>,
}

struct Tray {
    icon: TrayIcon,
    menu: Menu,
    /// Added at the top of the menu when an update is found.
    update: Option<MenuItem>,
    mic: MenuItem,
    camera: MenuItem,
    output: MenuItem,
    screen: MenuItem,
    share_again: MenuItem,
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
        let (open, mic, camera, output, screen, share_again, leave, logs, quit) = (
            item("open", "Abrir sala", true),
            item("mic", "Microfone silenciado · Ativar", false),
            item("camera", "Câmera desligada · Ligar", false),
            item("output", "Silenciar áudio", true),
            item("screen", "Compartilhar tela ou janela…", false),
            item("share_again", "Compartilhar de novo", false),
            item("leave", "Sair da sala", false),
            item("logs", "Abrir pasta de logs", true),
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
            &share_again,
            &PredefinedMenuItem::separator(),
            &leave,
            &PredefinedMenuItem::separator(),
            &logs,
            &quit,
        ])?;
        let icon = TrayIconBuilder::new()
            .with_icon(tray_icon::Icon::from_rgba(rgba, width, height)?)
            .with_tooltip("Screenshare")
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .build()?;
        Ok(Tray { icon, menu, update: None, mic, camera, output, screen, share_again, leave, shown: State::default() })
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
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
            actions.push(Action::Panel(rect.position.x, rect.position.y, f64::from(rect.size.width), f64::from(rect.size.height)));
        }
    }
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        actions.extend(match event.id().as_ref() {
            "open" => Some(Action::Open),
            "mic" => Some(Action::Mic),
            "camera" => Some(Action::Camera),
            "output" => Some(Action::Output),
            "screen" => Some(Action::Screen),
            "share_again" => Some(Action::ShareAgain),
            "leave" => Some(Action::Leave),
            "update" => Some(Action::Update),
            "logs" => Some(Action::Logs),
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
        tray.share_again.set_text(share_again_label(state.share_again.as_deref()));
        tray.share_again.set_enabled(state.share_again.is_some());
        match (&tray.update, &state.update) {
            (Some(item), Some(label)) => item.set_text(label),
            (None, Some(label)) => {
                let item = MenuItem::with_id("update", label, true, None);
                if tray.menu.prepend_items(&[&item, &PredefinedMenuItem::separator()]).is_ok() {
                    tray.update = Some(item);
                }
            }
            _ => {}
        }
        let _ = tray.icon.set_tooltip(Some(tooltip(&state)));
        tray.shown = state;
    });
}

/// The title says what will go out; a long one is cut, as menus do not wrap.
fn share_again_label(title: Option<&str>) -> String {
    match title {
        Some(title) if title.chars().count() > 40 => format!("Compartilhar de novo: {}…", title.chars().take(39).collect::<String>()),
        Some(title) => format!("Compartilhar de novo: {title}"),
        None => "Compartilhar de novo".into(),
    }
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
        assert_eq!(share_again_label(None), "Compartilhar de novo");
        assert_eq!(share_again_label(Some("Valorant")), "Compartilhar de novo: Valorant");
        assert_eq!(share_again_label(Some(&"a".repeat(50))).chars().count(), "Compartilhar de novo: ".len() + 40);
        let (rgba, width, height) = icon_rgba().unwrap();
        assert_eq!((width, height, rgba.len()), (32, 32, 32 * 32 * 4));
    }
}
