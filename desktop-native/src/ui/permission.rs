//! #permission-recovery: when the camera or the microphone fails, say why and offer a retry and the
//! system privacy page, like the web's MediaPermissionRecovery and the Tauri open_privacy_settings.
use iced::{
    Border, Element,
    widget::{button, column, container, row, text},
};

use super::theme;
use crate::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    Camera,
    Microphone,
}

/// mediaPermissionHelp, from the engine's error codes instead of the browser's error names.
pub fn help(media: Media, error: &str) -> &'static str {
    match error.split_once(": ").map(|(code, _)| code) {
        // NotAllowedError
        Some("PERMISSION_DENIED") => {
            "O acesso foi negado ou bloqueado pelo sistema. Libere o acesso nas configurações de privacidade do sistema (no Windows, ative também \"Permitir que aplicativos da área de trabalho acessem\") e tente novamente."
        }
        // NotFoundError
        _ if error.contains("não encontrada") => "Nenhum dispositivo compatível foi encontrado. Conecte uma câmera ou microfone e tente novamente.",
        _ => match media {
            Media::Camera => "Não foi possível iniciar a câmera. Confira as permissões de privacidade do sistema e tente novamente.",
            Media::Microphone => "Não foi possível iniciar o microfone. Confira as permissões de privacidade do sistema e tente novamente.",
        },
    }
}

/// open_privacy_settings in the Tauri app: apps cannot grant themselves access, so open the OS page.
pub fn open_settings(media: Media) {
    #[cfg(windows)]
    let (opener, page) = ("explorer", match media {
        Media::Camera => "ms-settings:privacy-webcam",
        Media::Microphone => "ms-settings:privacy-microphone",
    });
    #[cfg(target_os = "macos")]
    let (opener, page) = ("open", match media {
        Media::Camera => "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera",
        Media::Microphone => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
    });
    let _ = std::process::Command::new(opener).arg(page).spawn();
}

/// Drops the engine's "CODE: " prefix before a message reaches a toast.
pub fn plain(error: &str) -> &str {
    match error.split_once(": ") {
        Some((code, rest)) if !code.is_empty() && code.chars().all(|c| c.is_ascii_uppercase() || c == '_') => rest,
        _ => error,
    }
}

pub fn view(help: &'static str) -> Element<'static, Message> {
    let ghost = |label: &'static str, press: Message| {
        button(text(label)).padding([9, 12]).style(|_, status| button::Style {
            background: matches!(status, button::Status::Hovered).then(|| theme::HOVER.into()),
            text_color: theme::DIM,
            border: Border { radius: 8.0.into(), ..Default::default() },
            ..Default::default()
        })
        .on_press(press)
    };
    let actions = row![
        button(text("Tentar novamente").font(theme::BOLD)).padding([9, 14]).style(button::primary).on_press(Message::PermissionRetry),
        ghost("Abrir configurações", Message::PermissionSettings),
        ghost("Depois", Message::PermissionClose),
    ]
    .spacing(8);
    let panel = column![text("Permissão necessária").size(21).font(theme::BOLD), text(help), actions].spacing(12);
    container(panel)
        .padding(24)
        .max_width(420)
        .style(|_| container::Style {
            background: Some(theme::SURFACE.into()),
            border: Border { color: theme::LINE, width: 1.0, radius: 14.0.into() },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_codes_map_to_the_web_texts() {
        assert!(help(Media::Camera, "PERMISSION_DENIED: o Windows bloqueou a câmera.").starts_with("O acesso foi negado"));
        assert!(help(Media::Camera, "DEVICE_LOST: câmera não encontrada. Escolha outra no dock.").starts_with("Nenhum dispositivo"));
        assert!(help(Media::Microphone, "DEVICE_LOST: recording device unavailable").starts_with("Não foi possível iniciar o microfone"));
        assert_eq!(plain("DEVICE_LOST: a câmera parou."), "a câmera parou.");
        assert_eq!(plain("Sem conexão: tente de novo"), "Sem conexão: tente de novo");
    }
}
