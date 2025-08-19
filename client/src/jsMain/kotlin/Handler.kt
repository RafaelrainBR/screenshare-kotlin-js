import decorators.RTCIceCandidate
import decorators.RTCSessionDescription
import decorators.createRTCIceCandidate
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import screenshare.common.ChatMessage
import screenshare.common.Packet
import kotlin.js.Date
import kotlin.js.Json
import kotlin.js.json

fun handlePacket(
    websocketService: WebsocketService,
    packet: Packet,
    coroutineScope: CoroutineScope
) {
    println("Received packet: $packet")
    when (packet) {
        is Packet.UserConnected -> handleUserConnected(websocketService, packet, coroutineScope)
        is Packet.UserDisconnected -> handleUserDisconnected(packet)
        is Packet.ChatMessageReceived -> handleChatMessageReceived(packet)
        is Packet.UserList -> handleUserList(packet)
        is Packet.IceCandidateReceived -> handleIceCandidateReceived(
            packet = packet,
            coroutineScope = coroutineScope,
        )

        is Packet.DescriptionReceived -> handleDescriptionReceived(
            websocketService = websocketService,
            packet = packet,
            coroutineScope = coroutineScope
        )

        is Packet.ScreenShareStarted -> {}
        is Packet.ScreenShareStopped -> {}
        else -> {
            println("Unknown packet type: ${packet::class.simpleName}")
        }
    }
}

private fun handleUserConnected(
    websocketService: WebsocketService,
    packet: Packet.UserConnected,
    coroutineScope: CoroutineScope
) {
    if (packet.username != localUsername) {
        addMessageToChat(
            ChatMessage(
                username = "Sistema",
                content = "${packet.username} entrou na sala",
                timestamp = Date().getTime().toLong(),
            ),
        )

        createPeerConnection(
            websocketService = websocketService,
            socketId = packet.socketId,
            roomId = localRoomId,
            isInitiator = true,
            coroutineScope = coroutineScope,
        ).also { peerConnection ->
            peers[packet.socketId] = peerConnection
        }
    }
}

private fun handleUserDisconnected(packet: Packet.UserDisconnected) {
    addMessageToChat(
        ChatMessage(
            username = "Sistema",
            content = "${packet.username} saiu da sala",
            timestamp = Date().getTime().toLong(),
        ),
    )

    peers[packet.socketId]?.let { peerConnection ->
        peerConnection.close()
        peers.remove(packet.socketId)
    }

    if (currentSharer == packet.socketId) {
        Elements.screenVideo.srcObject = null
        Elements.videoContainer.classList.add("hidden")
        Elements.noScreenMessage.classList.remove("hidden")
        currentSharer = null
    }
}

private fun handleChatMessageReceived(packet: Packet.ChatMessageReceived) {
    addMessageToChat(packet.message)
}

private fun handleUserList(packet: Packet.UserList) {
    userList = packet.users
    updateUserList(userList)
}

private fun handleIceCandidateReceived(
    packet: Packet.IceCandidateReceived,
    coroutineScope: CoroutineScope
) = coroutineScope.launch {
    runCatching {
        peers[packet.senderId]?.let { peerConnection ->
            val candidateAsJson = JSON.parse<Json>(packet.candidate)
            val rtcIceCandidate = createRTCIceCandidate(
                candidate = candidateAsJson["candidate"] as String,
                sdpMid = candidateAsJson["sdpMid"] as String,
                sdpMLineIndex = candidateAsJson["sdpMLineIndex"] as Int,
            )
            val candidate = RTCIceCandidate(rtcIceCandidate)
            peerConnection.addIceCandidate(candidate).await()
        }
    }.onFailure {
        println("Failed to add ICE candidate: ${it.message}")
    }
}

private fun handleDescriptionReceived(
    websocketService: WebsocketService,
    packet: Packet.DescriptionReceived,
    coroutineScope: CoroutineScope
) = coroutineScope.launch {
    runCatching {
        val type = packet.description["type"] as String
        val sdp = packet.description["sdp"] as String

        if (type == "offer") {
            if (!peers.contains(packet.senderId)) {
                val peerConnection = createPeerConnection(
                    websocketService = websocketService,
                    socketId = packet.senderId,
                    roomId = localRoomId,
                    isInitiator = false,
                    coroutineScope = coroutineScope,
                ).also {
                    peers[packet.senderId] = it
                }

                peerConnection.onIceCandidateAdd { iceCandidate ->
                    if (iceCandidate != null) {
                        coroutineScope.launch {
                            websocketService.sendIceCandidate(
                                roomId = packet.roomId,
                                targetId = packet.senderId,
                                candidate = iceCandidate
                            )
                        }
                    }
                }

                peerConnection.onTrack { streams ->
                    console.log("Received track: $streams")
                    val remoteStream = streams[0]
                    val isScreenStream = remoteStream.getVideoTracks().isNotEmpty()
                    if (isScreenStream) {
                        with(Elements.screenVideo) {
                            if (srcObject == null || srcObject.asDynamic().id != remoteStream.id) {
                                srcObject = remoteStream
                                currentSharer = packet.senderId

                                Elements.videoContainer.classList.remove("hidden")
                                Elements.noScreenMessage.classList.add("hidden")
                            }
                        }
                    } else {
                        handleRemoteAudio(userId = packet.senderId, remoteStream)
                    }
                }
            }

            peers[packet.senderId]?.let { peerConnection ->
                val descriptionJson = json("type" to type, "sdp" to sdp)
                println("Setting remote description: $descriptionJson")
                peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson)).await()

                val answer = peerConnection.createAnswer().await()
                peerConnection.setLocalDescription(answer).await()

                val answerDescriptionMap = mapOf("type" to "answer", "sdp" to answer["sdp"] as String)
                websocketService.sendDescription(
                    roomId = packet.roomId,
                    description = answerDescriptionMap,
                    targetId = packet.senderId
                )
            }
        }

        if (type == "answer") {
            val descriptionJson = json("type" to type, "sdp" to sdp)
            println("Setting remote description: $descriptionJson")

            peers[packet.senderId]?.let { peerConnection ->
                peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson)).await()

                peerConnection.onTrack { streams ->
                    console.log("Received track: $streams")
                    val remoteStream = streams[0]
                    val isScreenStream = remoteStream.getVideoTracks().isNotEmpty()
                    if (isScreenStream) {
                        with(Elements.screenVideo) {
                            if (srcObject == null || srcObject.asDynamic().id != remoteStream.id) {
                                srcObject = remoteStream
                                currentSharer = packet.senderId

                                Elements.videoContainer.classList.remove("hidden")
                                Elements.noScreenMessage.classList.add("hidden")
                            }
                        }
                    } else {
                        handleRemoteAudio(userId = packet.senderId, remoteStream)
                    }
                }
            }
        }
    }.onFailure {
        println("Failed to set remote description from packet [$packet]: ${it.message}")
    }
}

private fun handleRemoteAudio(userId: String, stream: MediaStream) {
    remoteAudioStreams[userId] = stream

    val audioElement = (document.createElement("audio") as HTMLAudioElement).apply {
        id = "remote-audio-$userId"
        srcObject = stream
        autoplay = true
    }
    document.body?.appendChild(audioElement)
}
