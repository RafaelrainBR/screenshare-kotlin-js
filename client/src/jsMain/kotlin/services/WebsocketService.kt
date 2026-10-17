package services

import generateRandomRoomId
import getSessionOrAlert
import io.ktor.client.HttpClient
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.webSocketSession
import io.ktor.http.URLProtocol
import io.ktor.websocket.Frame.Close
import io.ktor.websocket.Frame.Text
import io.ktor.websocket.WebSocketSession
import io.ktor.websocket.readText
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.consumeEach
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withTimeout
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
    private var joinedRoomId: String? = null
    private var roomSocketId: String? = null
    private var legacyServer = false
    private val pendingMessages = mutableMapOf<String, CompletableDeferred<Unit>>()
    val isRoomJoined: Boolean
        get() = joinedRoomId != null && joinedRoomId == lastJoin?.roomId

    suspend fun connect(coroutineScope: CoroutineScope) {
        var attempt = 0
        while (coroutineScope.isActive) {
            var connected: WebSocketSession? = null
            try {
                val opened =
                    client.webSocketSession {
                        url.protocol = urlProtocol
                        url.host = host
                        url.port = port
                    }
                connected = opened
                session = opened
                reportedDisconnect = false
                lastJoin?.let {
                    opened.send(Text(Json.encodeToString<Packet>(it)))
                    opened.send(Text(Json.encodeToString<Packet>(Packet.RequestMediaState(it.roomId))))
                }
                opened.incoming.consumeEach { frame ->
                    when (frame) {
                        is Text -> {
                            if (lastJoin == null) return@consumeEach
                            val decoded = Json.decodeFromString<Packet>(frame.readText())
                            val legacyJoin = legacySelfJoin(decoded, lastJoin, joinedRoomId)
                            if (legacyJoin != null) legacyServer = true
                            listOfNotNull(legacyJoin, decoded).forEach { packet ->
                                if (packet is Packet.RoomJoined && packet.roomId == lastJoin?.roomId) {
                                    joinedRoomId = packet.roomId
                                    roomSocketId = packet.socketId
                                    attempt = 0
                                }
                                if (packet is Packet.ChatMessageReceived && packet.roomId == joinedRoomId &&
                                    packet.message.senderSocketId == roomSocketId
                                ) {
                                    packet.message.clientMessageId?.let { pendingMessages.remove(it)?.complete(Unit) }
                                }
                                handler(getSessionOrAlert(), packet, coroutineScope)
                            }
                        }

                        is Close -> {
                            throw kotlinx.coroutines.CancellationException("websocket closed")
                        }

                        else -> {}
                    }
                }
            } catch (_: Exception) {
                // A failed network attempt is retried below; local media state is untouched.
            } finally {
                if (session === connected) session = null
                joinedRoomId = null
                roomSocketId = null
                legacyServer = false
                pendingMessages.values.forEach {
                    it.completeExceptionally(
                        IllegalStateException("Connection interrupted"),
                    )
                }
                pendingMessages.clear()
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
        val join = Packet.JoinRoom(roomId, username, clientVersion = 3)
        lastJoin = join
        joinedRoomId = null
        session?.send(Text(Json.encodeToString<Packet>(join)))
    }

    suspend fun requestMediaState(roomId: String) = sendPacket(Packet.RequestMediaState(roomId))

    fun leaveRoom() {
        lastJoin = null
        joinedRoomId = null
        roomSocketId = null
        session?.cancel()
        session = null
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
        check(isRoomJoined && joinedRoomId == roomId) { "Room is not connected" }
        if (legacyServer) return sendPacket(Packet.SendChatMessage(roomId, message))
        val id = generateRandomRoomId()
        val receipt = CompletableDeferred<Unit>()
        pendingMessages[id] = receipt
        try {
            sendPacket(Packet.SendChatMessage(roomId, message, id))
            withTimeout(10_000) { receipt.await() }
        } finally {
            pendingMessages.remove(id)
        }
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
        checkNotNull(session) { "Connection unavailable" }.send(Text(Json.encodeToString(packet)))
    }
}

internal fun reconnectDelayMillis(attempt: Int): Long = (1_000L shl attempt.coerceIn(0, 4)).coerceAtMost(15_000L)

private fun createHttpClient(): HttpClient =
    HttpClient {
        install(WebSockets)
    }

// shortcut: servers older than clientVersion 2 never send RoomJoined nor message receipts, so the
// local UserConnected (matched by name) confirms the join; remove once every server is redeployed.
internal fun legacySelfJoin(
    packet: Packet,
    join: Packet.JoinRoom?,
    joinedRoomId: String?,
): Packet.RoomJoined? =
    if (packet is Packet.UserConnected && joinedRoomId == null && join != null &&
        packet.roomId == join.roomId && packet.username == join.username
    ) {
        Packet.RoomJoined(packet.roomId, packet.socketId)
    } else {
        null
    }
