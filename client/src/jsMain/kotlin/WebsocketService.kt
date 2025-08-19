import io.ktor.client.HttpClient
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.webSocketSession
import io.ktor.http.URLProtocol
import io.ktor.websocket.Frame.Close
import io.ktor.websocket.Frame.Text
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
    private val client: HttpClient = createHttpClient(),
    private val urlProtocol: URLProtocol = URLProtocol.WS,
    private val host: String = "0.0.0.0",
    private val port: Int = 8080,
    private val handler: suspend (WebsocketService, Packet, CoroutineScope) -> Unit,
    private val onClose: () -> Unit = {},
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

    suspend fun sendChatMessage(
        roomId: String,
        message: String,
    ) {
        sendPacket(Packet.SendChatMessage(roomId, message))
    }

    private fun close() {
        session?.cancel()
        session = null
    }

    private fun startListening(
        session: WebSocketSession,
        coroutineScope: CoroutineScope,
    ) {
        coroutineScope.launch {
            println("Listening for incoming messages...")
            session.incoming.consumeEach { frame ->
                when (frame) {
                    is Text -> {
                        // Handle incoming text frames
                        val text = frame.readText()
                        val packet = Json.decodeFromString<Packet>(text)
                        handler(this@WebsocketService, packet, coroutineScope)
                    }

                    is Close -> {
                        println("Websocket closed: ${frame.readReason()}")
                        this@WebsocketService.session = null
                        onClose()
                    }

                    else -> {
                        // Handle other types of frames if needed
                    }
                }
            }
            close()
        }
    }

    private suspend fun sendPacket(packet: Packet) {
        println("Sending packet: $packet")
        session?.send(Text(Json.encodeToString(packet)))
    }
}

private fun createHttpClient(): HttpClient {
    return HttpClient {
        install(WebSockets)
    }
}
