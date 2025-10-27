import decorators.RTCPeerConnectionDecorator
import io.ktor.http.URLProtocol
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import org.w3c.dom.mediacapture.MediaStream
import screenshare.common.ChatMessage
import screenshare.common.SocketUser
import kotlin.js.Date

lateinit var localUsername: String
lateinit var localRoomId: String

var currentSharer: String? = null
var userList: List<SocketUser> = emptyList()

var localScreenStream: MediaStream? = null
var localMicStream: MediaStream? = null

val remoteScreenStreams: MutableMap<String, MediaStream> = mutableMapOf()
val remoteAudioStreams: MutableMap<String, MediaStream> = mutableMapOf()

var isMicMuted = true
var isAudioMuted = true

val peers: MutableMap<String, RTCPeerConnectionDecorator> = mutableMapOf()

fun main() {
    println("Hello, World!")

    val websocketService =
        with(window.location) {
            println("Connecting to WebSocket at $href")
            val port =
                if (port.isNullOrBlank()) {
                    if (protocol == "https:") "443" else "80"
                } else {
                    port
                }
            WebsocketService(
                urlProtocol = if (protocol == "https:") URLProtocol.WSS else URLProtocol.WS,
                host = hostname,
                port = port.toInt(),
                handler = ::handlePacket,
                onClose = {
                    addMessageToChat(
                        ChatMessage(
                            username = "Sistema",
                            content = "Conexão encerrada! Recarregue a página.",
                            timestamp = Date().getTime().toLong(),
                        ),
                    )
                },
            )
        }

    val websocketCoroutineScope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    websocketCoroutineScope.launch {
        websocketService.connect(websocketCoroutineScope)
    }

    registerDocumentHandlers(websocketService, websocketCoroutineScope)
}

fun recreatePeerConnections(
    websocketService: WebsocketService,
    roomId: String,
    isInitiator: Boolean,
    coroutineScope: CoroutineScope,
) {
    peers.forEach { (peerId, peerConnection) ->
        peers[peerId] =
            createPeerConnection(
                websocketService = websocketService,
                socketId = peerId,
                roomId = roomId,
                isInitiator = isInitiator,
                coroutineScope = coroutineScope,
                peerConnection = peerConnection,
            )
    }
}

fun createPeerConnection(
    websocketService: WebsocketService,
    socketId: String,
    roomId: String,
    isInitiator: Boolean,
    coroutineScope: CoroutineScope,
    peerConnection: RTCPeerConnectionDecorator? = null,
): RTCPeerConnectionDecorator {
    val peerConnection = peerConnection ?: RTCPeerConnectionDecorator.create()

    localScreenStream?.getTracks()?.forEach { track ->
        peerConnection.addTrack(track, localScreenStream!!)
    }

    localMicStream?.getTracks()?.forEach { track ->
        peerConnection.addTrack(track, localMicStream!!)
    }

    peerConnection.onIceCandidateAdd { iceCandidate ->
        if (iceCandidate != null) {
            coroutineScope.launch {
                websocketService.sendIceCandidate(roomId = roomId, targetId = socketId, candidate = iceCandidate)
            }
        }
    }

    coroutineScope.launch {
        if (isInitiator) {
            runCatching {
                val offer = peerConnection.createOffer().await()
                peerConnection.setLocalDescription(offer).await()

                val offerDescriptionAsMap = mapOf("type" to offer["type"] as String, "sdp" to offer["sdp"] as String)
                websocketService.sendDescription(
                    roomId = roomId,
                    targetId = socketId,
                    description = offerDescriptionAsMap,
                )
            }.onFailure { error ->
                console.error("Error creating offer", error)
            }
        }
    }

    return peerConnection
}
