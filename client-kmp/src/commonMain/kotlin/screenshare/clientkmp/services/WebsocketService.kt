package screenshare.clientkmp.services

import io.ktor.client.HttpClient
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.webSocketSession
import io.ktor.http.URLProtocol
import io.ktor.websocket.Frame
import io.ktor.websocket.WebSocketSession
import io.ktor.websocket.readReason
import io.ktor.websocket.readText
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.consumeEach
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import screenshare.common.Packet

class WebsocketService(
    private val urlProtocol: URLProtocol,
    private val host: String,
    private val port: Int,
    private val onPacketReceived: suspend (Packet) -> Unit,
    var onClose: () -> Unit = {},
    private val client: HttpClient = createPlatformHttpClient(),
) {
    private var session: WebSocketSession? = null

    suspend fun connect(coroutineScope: CoroutineScope) {
        session =
            client.webSocketSession {
                url.protocol = urlProtocol
                url.host = host
                url.port = port
            }
        startListening(session!!, coroutineScope)
    }

    suspend fun joinRoom(
        roomId: String,
        username: String,
    ) {
        sendPacket(Packet.JoinRoom(roomId, username))
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

    suspend fun sendIceCandidate(
        roomId: String,
        candidate: String,
        targetId: String,
    ) {
        sendPacket(
            Packet.SendIceCandidate(
                roomId = roomId,
                candidate = candidate,
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

    private fun startListening(
        wsSession: WebSocketSession,
        coroutineScope: CoroutineScope,
    ) {
        coroutineScope.launch {
            println("Listening for incoming messages...")
            try {
                wsSession.incoming.consumeEach { frame ->
                    when (frame) {
                        is Frame.Text -> {
                            val text = frame.readText()
                            val packet = Json.decodeFromString<Packet>(text)
                            onPacketReceived(packet)
                        }

                        is Frame.Close -> {
                            println("Websocket closed: ${frame.readReason()}")
                            this@WebsocketService.session = null
                            onClose()
                        }

                        else -> {
                            Unit
                        }
                    }
                }
                close()
            } catch (e: Exception) {
                println("Websocket connection error: ${e.message}")
                runCatching { close() }
                onClose()
            }
        }
    }

    private fun close() {
        session?.cancel()
        session = null
    }

    private suspend fun sendPacket(packet: Packet) {
        println("[WS] >>> ${packetSummary(packet)}")
        session?.send(Frame.Text(Json.encodeToString(packet)))
    }

    private fun packetSummary(packet: Packet): String = when (packet) {
        is Packet.SendDescription -> "SendDescription(type=${packet.description["type"]}, room=${packet.roomId}, to=${packet.targetId.take(8)})"
        is Packet.SendIceCandidate -> {
            val typ = Regex("typ (\\w+)").find(packet.candidate)?.groupValues?.get(1) ?: "?"
            "SendIceCandidate(typ=$typ, room=${packet.roomId}, to=${packet.targetId.take(8)})"
        }
        is Packet.JoinRoom -> "JoinRoom(room=${packet.roomId}, user=${packet.username})"
        else -> packet::class.simpleName ?: packet.toString()
    }
}
