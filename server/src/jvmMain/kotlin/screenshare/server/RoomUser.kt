package screenshare.server

import io.ktor.websocket.Frame.Text
import io.ktor.websocket.WebSocketSession
import kotlinx.serialization.json.Json
import screenshare.common.Packet
import java.util.UUID

data class RoomUser(
    val id: String,
    val session: WebSocketSession,
    val username: String,
    var isMuted: Boolean = true,
) {
    suspend fun sendPacket(packet: Packet) {
        session.send(Text(Json.encodeToString(packet)))
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
