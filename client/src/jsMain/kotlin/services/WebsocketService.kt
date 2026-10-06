package services

import getSessionOrAlert
import io.ktor.client.HttpClient
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.webSocketSession
import io.ktor.http.URLProtocol
import io.ktor.websocket.Frame.Close
import io.ktor.websocket.Frame.Text
import io.ktor.websocket.WebSocketSession
import io.ktor.websocket.readText
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.consumeEach
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.serialization.json.Json
import screenshare.common.Packet

class WebsocketService(
    private val client: HttpClient = createHttpClient(),
    private val urlProtocol: URLProtocol = URLProtocol.WS,
    private val host: String = "0.0.0.0",
    private val port: Int = 8080,
    private val handler: suspend (Session, Packet, CoroutineScope) -> Unit,
    private val onClose: () -> Unit = {},
) {
    private var session: WebSocketSession? = null
    private var lastJoin: Packet.JoinRoom? = null
    private var reportedDisconnect = false

    suspend fun connect(coroutineScope: CoroutineScope) {
        var attempt = 0
        while (coroutineScope.isActive) {
            var connected: WebSocketSession? = null
            try {
                val opened = client.webSocketSession {
                    url.protocol = urlProtocol
                    url.host = host
                    url.port = port
                }
                connected = opened
                session = opened
                reportedDisconnect = false
                lastJoin?.let {
                    opened.send(Text(Json.encodeToString(it)))
                    opened.send(Text(Json.encodeToString(Packet.RequestMediaState(it.roomId))))
                }
                opened.incoming.consumeEach { frame ->
                    when (frame) {
                        is Text -> handler(getSessionOrAlert(), Json.decodeFromString(frame.readText()), coroutineScope)
                        is Close -> throw kotlinx.coroutines.CancellationException("websocket closed")
                        else -> Unit
                    }
                }
            } catch (_: Exception) {
                // A failed network attempt is retried below; local media state is untouched.
            } finally {
                if (session === connected) session = null
                runCatching { connected?.cancel() }
            }
            if (coroutineScope.isActive && lastJoin != null && !reportedDisconnect) {
                onClose()
                reportedDisconnect = true
            }
            if (coroutineScope.isActive) {
                delay(reconnectDelayMillis(attempt++))
            }
        }
    }

    suspend fun joinRoom(
        roomId: String,
        username: String,
    ) {
        val join = Packet.JoinRoom(roomId, username)
        lastJoin = join
        sendPacket(join)
        requestMediaState(roomId)
    }

    suspend fun requestMediaState(roomId: String) = sendPacket(Packet.RequestMediaState(roomId))

    fun leaveRoom() {
        lastJoin = null
        session?.cancel()
    }

    suspend fun sendIceCandidate(
        roomId: String,
        candidate: kotlin.js.Json,
        targetId: String,
    ) {
        sendPacket(
            Packet.SendIceCandidate(
                roomId = roomId,
                candidate = JSON.stringify(candidate),
                targetId = targetId,
            ),
        )
    }

    suspend fun sendDescription(
        roomId: String,
        description: Map<String, String>,
        targetId: String,
    ) {
        sendPacket(
            Packet.SendDescription(
                roomId = roomId,
                description = description,
                targetId = targetId,
            ),
        )
    }

    suspend fun startScreenSharing(roomId: String) {
        sendPacket(Packet.StartScreenShare(roomId))
    }

    suspend fun stopScreenSharing(roomId: String) {
        sendPacket(Packet.StopScreenShare(roomId))
    }

    suspend fun startCameraShare(roomId: String) {
        sendPacket(Packet.StartCameraShare(roomId))
    }

    suspend fun stopCameraShare(roomId: String) {
        sendPacket(Packet.StopCameraShare(roomId))
    }

    suspend fun sendChatMessage(
        roomId: String,
        message: String,
    ) {
        sendPacket(Packet.SendChatMessage(roomId, message))
    }

    suspend fun sendToggleMute(
        roomId: String,
        isMuted: Boolean,
    ) {
        if (isMuted) {
            sendPacket(Packet.SendMuted(roomId))
        } else {
            sendPacket(Packet.SendUnmuted(roomId))
        }
    }

    private suspend fun sendPacket(packet: Packet) {
        session?.send(Text(Json.encodeToString(packet)))
    }
}

internal fun reconnectDelayMillis(attempt: Int): Long =
    (1_000L shl attempt.coerceIn(0, 4)).coerceAtMost(15_000L)

private fun createHttpClient(): HttpClient =
    HttpClient {
        install(WebSockets)
    }
