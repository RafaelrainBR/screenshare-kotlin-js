package platform

fun mediaPermissionHelp(kind: String, errorName: String?): String =
    when (errorName) {
        "NotAllowedError" -> "O acesso foi negado ou bloqueado pelo sistema. Confira as permissões de privacidade do sistema para este app e tente novamente."
        "NotFoundError" -> "Nenhum dispositivo compatível foi encontrado. Conecte uma câmera ou microfone e tente novamente."
        "NotReadableError" -> "O dispositivo existe, mas não pôde ser aberto. Feche outros apps que o estejam usando e tente novamente."
        "OverconstrainedError" -> "O dispositivo escolhido não atende a esta configuração. Selecione outro dispositivo nas opções e tente novamente."
        "SecurityError" -> "O WebView bloqueou o acesso por política de segurança. Atualize o app e tente novamente."
        else -> when (kind) {
            "camera" -> "Não foi possível iniciar a câmera. Confira as permissões de privacidade do sistema e tente novamente."
            "microphone" -> "Não foi possível iniciar o microfone. Confira as permissões de privacidade do sistema e tente novamente."
            else -> "A escolha ou permissão de captura foi cancelada ou recusada. Permita a captura na janela do sistema e tente novamente."
        }
    }
