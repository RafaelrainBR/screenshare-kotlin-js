package services

import decorators.RTCIceCandidate
import decorators.RTCPeerConnectionDecorator
import decorators.RTCSessionDescription
import decorators.createRTCIceCandidate
import decorators.upgradeAudioQualitySdp
import getSessionOrAlert
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import kotlin.js.Json
import kotlin.js.json
import org.w3c.dom.mediacapture.MediaStreamTrack

class PeerConnections(
    private val voiceChat: VoiceChat,
    private val screenSharing: ScreenSharing,
) {
    val peers: MutableMap<String, RTCPeerConnectionDecorator> = mutableMapOf()

    // Perfect-negotiation state, keyed by socketId (see MDN "Perfect Negotiation").
    private val makingOffer = mutableMapOf<String, Boolean>()
    private val ignoreOffer = mutableMapOf<String, Boolean>()
    private val isPolite = mutableMapOf<String, Boolean>()

    // Mapeia a resolução/fps escolhidos na UI para um bitrate adequado.
    // 'maintain-resolution' mantém o quadro nítido; o bitrate alto evita a
    // macroblocagem típica de filme em movimento.
    private fun encodingForShare(): Pair<Int, Int> {
        val height = screenSharing.lastShareHeight
        val fps = screenSharing.lastShareFrameRate
        val baseBitrate =
            when {
                height <= 720 -> 2_500_000
                height <= 1080 -> 6_000_000
                height <= 1440 -> 10_000_000
                else -> 15_000_000
            }
        val bitrate = if (fps >= 60) (baseBitrate * 1.5).toInt() else baseBitrate
        return Pair(bitrate, fps)
    }

    private fun addTracksIfNotPresent(peerConnection: RTCPeerConnectionDecorator) {
        screenSharing.localScreenStream?.getTracks()?.forEach { track ->
            if (!peerConnection.hasTrack(track)) {
                console.log("Adding screen track: ${track.id}")
                peerConnection.addTrack(track, screenSharing.localScreenStream!!)

                if (track.kind == "video") {
                    // contentHint 'detail' melhora a nitidez de conteúdo estático
                    // de alto detalhe (traço de anime/texto): o encoder prioriza
                    // linhas/esquinas, reduzindo ringing e banding.
                    val dynamicTrack = track.unsafeCast<dynamic>()
                    dynamicTrack.contentHint = "detail"

                    // Prefere codec (VP9) e controla bitrate/fps ANTES de createOffer.
                    peerConnection.preferVideoCodecs()
                    val (bitrate, fps) = encodingForShare()
                    peerConnection.applyVideoEncoding(
                        trackId = track.id,
                        maxBitrate = bitrate,
                        maxFramerate = fps,
                        degradationPreference = "maintain-resolution",
                    )
                }
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

        // Only add the new tracks; the browser's onnegotiationneeded handler
        // will fire automatically and send the renegotiation offer.
        peers.forEach { (_, peerConnection) ->
            addTracksIfNotPresent(peerConnection)
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

        makingOffer[socketId] = false
        ignoreOffer[socketId] = false
        isPolite[socketId] = !isInitiator

        peerConnection.onIceCandidateAdd { iceCandidate ->
            if (iceCandidate != null) {
                coroutineScope.launch {
                    websocketService.sendIceCandidate(roomId = roomId, targetId = socketId, candidate = iceCandidate)
                }
            }
        }

        // renegotiationneeded: fires on initial connection (if tracks exist) AND
        // whenever a track is added later (screen share start/stop). Guard against
        // firing while we are already mid-negotiation (signaling state != stable)
        // to avoid a race where a renegotiation offer collides with the answer we
        // are about to send.
        peerConnection.onNegotiationNeeded {
            if (makingOffer[socketId] == true) return@onNegotiationNeeded
            if (peerConnection.signalingState != "stable") return@onNegotiationNeeded
            coroutineScope.launch {
                runCatching {
                    makingOffer[socketId] = true
                    val offer = peerConnection.createOffer().await()
                    val sdp = upgradeAudioQualitySdp(offer["sdp"] as String)
                    peerConnection.setLocalDescription(json("type" to offer["type"] as String, "sdp" to sdp)).await()
                    websocketService.sendDescription(
                        roomId = roomId,
                        targetId = socketId,
                        description = mapOf("type" to offer["type"] as String, "sdp" to sdp),
                    )
                }.onFailure { error ->
                    console.error("Error creating offer for $socketId", error)
                }
                makingOffer[socketId] = false
            }
        }

        // For the peer who created the connection (isInitiator), send the
        // initial offer explicitly.  negotiationneeded may not fire for a
        // completely empty PC, and we must not depend on it for the first
        // offer.
        if (isInitiator) {
            makingOffer[socketId] = true
            coroutineScope.launch {
                runCatching {
                    val offer = peerConnection.createOffer().await()
                    val sdp = upgradeAudioQualitySdp(offer["sdp"] as String)
                    peerConnection.setLocalDescription(json("type" to offer["type"] as String, "sdp" to sdp)).await()
                    websocketService.sendDescription(
                        roomId = roomId,
                        targetId = socketId,
                        description = mapOf("type" to offer["type"] as String, "sdp" to sdp),
                    )
                }.onFailure { error ->
                    console.error("Error creating initial offer for $socketId", error)
                }
                makingOffer[socketId] = false
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

        // Reconexão automática: se a conexão cai (troca de NAT/CGNAT, queda
        // transitória), faz ICE restart reenviando um novo offer. Evita forçar
        // o usuário a recarregar a página.
        peerConnection.onConnectionStateChange { state ->
            console.log("Connection state [$socketId]: $state")
            if (state == "failed") {
                coroutineScope.launch {
                    runCatching {
                        makingOffer[socketId] = true
                        val offer = peerConnection.iceRestartOffer().await()
                        val sdp = upgradeAudioQualitySdp(offer["sdp"] as String)
                        peerConnection.setLocalDescription(json("type" to offer["type"] as String, "sdp" to sdp)).await()
                        websocketService.sendDescription(
                            roomId = roomId,
                            targetId = socketId,
                            description = mapOf("type" to offer["type"] as String, "sdp" to sdp),
                        )
                    }.onFailure { error ->
                        console.error("ICE restart failed for $socketId", error)
                    }
                    makingOffer[socketId] = false
                }
            }
        }

        peers[socketId] = peerConnection

        return peerConnection
    }

    fun replaceMicTrack(
        oldTrackId: String?,
        newTrack: MediaStreamTrack,
    ) {
        peers.values.forEach { peerConnection ->
            peerConnection.replaceTrack(oldTrackId, newTrack)
        }
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

            // Handle glare: if we're mid-negotiation (not stable), an incoming
            // offer may collide with our own offer. The polite peer rolls back.
            if (peerConnection.signalingState != "stable") {
                val polite = isPolite[senderId] ?: true
                if (!polite) {
                    console.log("Ignoring colliding offer (impolite peer)")
                    ignoreOffer[senderId] = true
                    return@let
                }
                console.log("Rolling back to handle colliding offer (polite peer)")
                peerConnection.rollback().await()
            }

            peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson)).await()

            if (ignoreOffer[senderId] == true) {
                ignoreOffer[senderId] = false
                peerConnection.rollback().await()
                return@let
            }

            val answer = peerConnection.createAnswer().await()
            val answerSdp = upgradeAudioQualitySdp(answer["sdp"] as String)
            peerConnection.setLocalDescription(json("type" to "answer", "sdp" to answerSdp)).await()

            val answerDescriptionMap = mapOf("type" to "answer", "sdp" to answerSdp)
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
