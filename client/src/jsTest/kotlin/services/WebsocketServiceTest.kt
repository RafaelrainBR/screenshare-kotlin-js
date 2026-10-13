package services

import kotlinx.coroutines.MainScope
import kotlinx.coroutines.promise
import platform.mediaPermissionHelp
import screenshare.common.Packet
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class WebsocketServiceTest {
    @Test
    fun disconnectedChatIsRejected() =
        MainScope().promise {
            val service = WebsocketService(handler = { _, _, _ -> })
            assertFailsWith<IllegalStateException> { service.sendChatMessage("room", "draft") }
        }

    @Test
    fun legacyServerJoinIsConfirmedOnlyByOwnFirstUserConnected() {
        val join = Packet.JoinRoom("room", "Ana", clientVersion = 2)
        val own = Packet.UserConnected("room", "sid-1", "Ana")
        assertEquals(Packet.RoomJoined("room", "sid-1"), legacySelfJoin(own, join, null))
        assertNull(legacySelfJoin(own, join, "room"))
        assertNull(legacySelfJoin(Packet.UserConnected("room", "sid-2", "Bia"), join, null))
        assertNull(legacySelfJoin(own, null, null))
    }

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
            "O acesso foi negado ou bloqueado pelo sistema. Libere o acesso nas configurações de privacidade do sistema (no Windows, ative também \"Permitir que aplicativos da área de trabalho acessem\") e tente novamente.",
            mediaPermissionHelp("camera", "NotAllowedError"),
        )
        assertEquals(
            "Não foi possível iniciar a câmera. Confira as permissões de privacidade do sistema e tente novamente.",
            mediaPermissionHelp("camera", null),
        )
    }
}
