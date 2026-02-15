package services

import decorators.RTCIceCandidate
import decorators.RTCPeerConnectionDecorator
import decorators.RTCSessionDescription
import decorators.createRTCIceCandidate
import getSessionOrAlert
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import kotlin.js.Json

class PeerConnections(
    private val voiceChat: VoiceChat,
    private val screenSharing: ScreenSharing,
) {
    val peers: MutableMap<String, RTCPeerConnectionDecorator> = mutableMapOf()

    private fun addTracksIfNotPresent(peerConnection: RTCPeerConnectionDecorator) {
        screenSharing.localScreenStream?.getTracks()?.forEach { track ->
            if (!peerConnection.hasTrack(track)) {
                console.log("Adding screen track: ${track.id}")
                peerConnection.addTrack(track, screenSharing.localScreenStream!!)
            } else {
                console.log("Screen track already present: ${track.id}")
            }
        }

        voiceChat.localMicStream?.getTracks()?.forEach { track ->
            if (!peerConnection.hasTrack(track)) {
                console.log("Adding mic track: ${track.id}")
                peerConnection.addTrack(track, voiceChat.localMicStream!!)
            } else {
                console.log("Mic track already present: ${track.id}")
            }
        }
    }

    fun recreatePeerConnections(
        websocketService: WebsocketService,
        roomId: String,
        isInitiator: Boolean,
        coroutineScope: CoroutineScope,
    ) {
        console.log("Recreating peer connections for ${peers.size} peers")

        peers.forEach { (peerId, peerConnection) ->
            addTracksIfNotPresent(peerConnection)

            if (isInitiator) {
                coroutineScope.launch {
                    runCatching {
                        val offer = peerConnection.createOffer().await()
                        peerConnection.setLocalDescription(offer).await()

                        val offerDescriptionAsMap =
                            mapOf("type" to offer["type"] as String, "sdp" to offer["sdp"] as String)
                        websocketService.sendDescription(
                            roomId = roomId,
                            targetId = peerId,
                            description = offerDescriptionAsMap,
                        )
                    }.onFailure { error ->
                        console.error("Error recreating offer for peer $peerId", error)
                    }
                }
            }
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

        addTracksIfNotPresent(peerConnection)

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

                    val offerDescriptionAsMap =
                        mapOf("type" to offer["type"] as String, "sdp" to offer["sdp"] as String)
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

        peerConnection.onTrack { streams ->
            console.log("Received track from [$socketId]: $streams")
            val remoteStream = streams[0]
            val isScreenStream = remoteStream.getVideoTracks().isNotEmpty()
            console.log("isScreenStream: $isScreenStream")
            if (isScreenStream) {
                screenSharing.handleRemoteScreen(socketId, remoteStream)
                getSessionOrAlert().currentSharerSocketId = socketId
            } else {
                voiceChat.handleRemoteAudio(socketId, remoteStream)
            }
        }

        peers[socketId] = peerConnection

        return peerConnection
    }

    fun closePeerConnection(socketId: String) {
        peers[socketId]?.let { peerConnection ->
            peerConnection.close()
            peers.remove(socketId)
        }
    }

    fun contains(socketId: String): Boolean = peers.containsKey(socketId)

    suspend fun updateIceCandidate(
        senderId: String,
        candidate: String,
    ) {
        peers[senderId]?.let { peerConnectionDecorator ->
            val candidateAsJson = JSON.parse<Json>(candidate)
            val rtcIceCandidate =
                createRTCIceCandidate(
                    candidate = candidateAsJson["candidate"] as String,
                    sdpMid = candidateAsJson["sdpMid"] as String,
                    sdpMLineIndex = candidateAsJson["sdpMLineIndex"] as Int,
                )
            val rtcCandidate = RTCIceCandidate(rtcIceCandidate)
            peerConnectionDecorator.addIceCandidate(rtcCandidate).await()
        }
    }

    suspend fun updateDescriptionFromOffer(
        websocketService: WebsocketService,
        roomId: String,
        senderId: String,
        descriptionJson: Json,
    ) {
        peers[senderId]?.let { peerConnection ->
            console.log("Setting remote description: ${JSON.stringify(descriptionJson)}")
            peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson)).await()

            val answer = peerConnection.createAnswer().await()
            peerConnection.setLocalDescription(answer).await()

            val answerDescriptionMap = mapOf("type" to "answer", "sdp" to answer["sdp"] as String)
            websocketService.sendDescription(
                roomId = roomId,
                description = answerDescriptionMap,
                targetId = senderId,
            )
        }
    }

    fun updateDescriptionFromAnswer(
        senderId: String,
        descriptionJson: Json,
    ) {
        peers[senderId]?.let { peerConnection ->
            console.log("Setting remote description: ${JSON.stringify(descriptionJson)}")
            peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson))
        }
    }
}
