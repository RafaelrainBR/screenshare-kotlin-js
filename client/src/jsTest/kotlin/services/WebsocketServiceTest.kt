package services

import kotlin.test.Test
import kotlin.test.assertEquals
import platform.mediaPermissionHelp

class WebsocketServiceTest {
    @Test
    fun reconnectDelayGrowsAndStopsAtFifteenSeconds() {
        assertEquals(1_000L, reconnectDelayMillis(0))
        assertEquals(8_000L, reconnectDelayMillis(3))
        assertEquals(15_000L, reconnectDelayMillis(4))
        assertEquals(15_000L, reconnectDelayMillis(20))
    }

    @Test
    fun permissionRecoveryDoesNotGuessWhyAccessWasBlocked() {
        assertEquals(
            "O acesso foi negado ou bloqueado pelo sistema. Confira as permissões de privacidade do sistema para este app e tente novamente.",
            mediaPermissionHelp("camera", "NotAllowedError"),
        )
        assertEquals(
            "Não foi possível iniciar a câmera. Confira as permissões de privacidade do sistema e tente novamente.",
            mediaPermissionHelp("camera", null),
        )
    }
}
