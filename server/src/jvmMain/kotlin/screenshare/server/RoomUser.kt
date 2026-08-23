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
    var isMuted: Boolean = true,
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
        outgoing.send(packet)
    }

    companion object {
        fun create(
            session: WebSocketSession,
            username: String,
        ): RoomUser =
            RoomUser(
                id = UUID.randomUUID().toString(),
                session = session,
                username = username,
            )
    }
}
