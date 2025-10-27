package services

import decorators.RTCIceCandidate
import decorators.RTCPeerConnectionDecorator
import decorators.RTCSessionDescription
import decorators.createRTCIceCandidate
import getSessionOrAlert
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import org.w3c.dom.mediacapture.MediaStream
import ui.InterfaceMutations
import kotlin.js.Json

class PeerConnections(
    private val voiceChat: VoiceChat
) {
    val peers: MutableMap<String, RTCPeerConnectionDecorator> = mutableMapOf()

    var localScreenStream: MediaStream? = null
    val remoteScreenStreams: MutableMap<String, MediaStream> = mutableMapOf()

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

        voiceChat.localMicStream?.getTracks()?.forEach { track ->
            peerConnection.addTrack(track, voiceChat.localMicStream!!)
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
                InterfaceMutations.updateScreenContainer(remoteStream)
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

    fun contains(socketId: String): Boolean {
        return peers.containsKey(socketId)
    }

    suspend fun updateIceCandidate(senderId: String, candidate: String) {
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
        descriptionJson: Json
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
        descriptionJson: Json
    ) {
        peers[senderId]?.let { peerConnection ->
            console.log("Setting remote description: ${JSON.stringify(descriptionJson)}")
            peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson))
        }
    }
}
