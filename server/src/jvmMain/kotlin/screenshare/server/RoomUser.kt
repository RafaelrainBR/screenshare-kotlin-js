package screenshare.server

import io.ktor.websocket.Frame.Text
import io.ktor.websocket.WebSocketSession
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import screenshare.common.Packet
import java.util.UUID

data class RoomUser(
    val id: String,
    val session: WebSocketSession,
    val username: String,
    val clientVersion: Int = 1,
    var isMuted: Boolean = true,
    var isScreenSharing: Boolean = false,
    var isCameraSharing: Boolean = false,
    /** What limits this user's share encoder (a ShareProblem), as its client last reported it. */
    var shareProblem: String? = null,
) {
    private val outgoing = Channel<Packet>(capacity = Channel.UNLIMITED)

    fun start(
        scope: CoroutineScope,
        onDisconnected: suspend () -> Unit = {},
    ) {
        scope.launch {
            for (packet in outgoing) {
                try {
                    session.send(Text(Json.encodeToString(packet)))
                } catch (e: Exception) {
                    onDisconnected()
                    break
                }
            }
        }
    }

    suspend fun sendPacket(packet: Packet) {
        val compatible =
            if (clientVersion < 2 && packet is Packet.ChatMessageReceived) {
                packet.copy(message = packet.message.copy(senderSocketId = null, clientMessageId = null))
            } else {
                packet
            }
        outgoing.send(compatible)
    }

    companion object {
        fun create(
            session: WebSocketSession,
            username: String,
            clientVersion: Int = 1,
        ): RoomUser =
            RoomUser(
                id = UUID.randomUUID().toString(),
                session = session,
                username = username,
                clientVersion = clientVersion,
            )
    }
}
