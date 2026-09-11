package services

import decorators.RTCIceCandidate
import decorators.RTCPeerConnectionDecorator
import decorators.RTCSessionDescription
import decorators.createRTCIceCandidate
import decorators.upgradeAudioQualitySdp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import kotlin.js.Json
import kotlin.js.json
import org.w3c.dom.mediacapture.MediaStreamTrack

class PeerConnections(
    private val voiceChat: VoiceChat,
    private val screenSharing: ScreenSharing,
    private val cameraSharing: CameraSharing,
) {
    val peers: MutableMap<String, RTCPeerConnectionDecorator> = mutableMapOf()

    // Perfect-negotiation state, keyed by socketId (see MDN "Perfect Negotiation").
    private val makingOffer = mutableMapOf<String, Boolean>()
    private val ignoreOffer = mutableMapOf<String, Boolean>()
    private val isPolite = mutableMapOf<String, Boolean>()

    // Sinalização local de quais compartilhamentos cada peer está transmitindo.
    // Usado para decidir, em onTrack, se um track de vídeo é da TELA ou da CÂMERA
    // (ambos têm video track). Mantidos em sincronia pelos packets Start/Stop.
    private val screenShareSockets = mutableSetOf<String>()
    private val cameraShareSockets = mutableSetOf<String>()

    // Contador de tracks de vídeo recebidos por peer. Como o remetente sempre
    // adiciona a tela ANTES da câmera, o 1º track de vídeo é a tela e o 2º a
    // câmera quando ambos estão ativos. Zerado ao parar um compartilhamento.
    private val receivedVideoCount = mutableMapOf<String, Int>()

    fun markScreenSharing(socketId: String, active: Boolean) {
        if (active) screenShareSockets.add(socketId) else screenShareSockets.remove(socketId)
        if (!active) receivedVideoCount.remove(socketId)
    }

    fun markCameraSharing(socketId: String, active: Boolean) {
        if (active) cameraShareSockets.add(socketId) else cameraShareSockets.remove(socketId)
        if (!active) receivedVideoCount.remove(socketId)
    }

    fun isScreenSharing(socketId: String): Boolean = screenShareSockets.contains(socketId)

    fun isCameraSharing(socketId: String): Boolean = cameraShareSockets.contains(socketId)

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
                peerConnection.addTrack(track, screenSharing.localScreenStream!!)

                if (track.kind == "video") {
                    // Screen share is also used for video players. `motion` avoids
                    // treating every fullscreen movie frame as a static desktop,
                    // which can cause severe macroblocking when motion increases.
                    val dynamicTrack = track.unsafeCast<dynamic>()
                    dynamicTrack.contentHint = "motion"

                    // Prefere codec (VP9) e controla bitrate/fps ANTES de createOffer.
                    peerConnection.preferVideoCodecs()
                    val (bitrate, fps) = encodingForShare()
                    peerConnection.applyVideoEncoding(
                        trackId = track.id,
                        maxBitrate = bitrate,
                        maxFramerate = fps,
                        degradationPreference = "balanced",
                    )
                }
            }
        }

        voiceChat.localMicStream?.getTracks()?.forEach { track ->
            if (!peerConnection.hasTrack(track)) {
                peerConnection.addTrack(track, voiceChat.localMicStream!!)
            }
        }

        cameraSharing.localCameraStream?.getTracks()?.forEach { track ->
            if (!peerConnection.hasTrack(track)) {
                peerConnection.addTrack(track, cameraSharing.localCameraStream!!)

                if (track.kind == "video") {
                    // contentHint 'motion' prioriza movimento (padrão de vídeo de
                    // câmera), diferente do 'detail' usado na tela.
                    val dynamicTrack = track.unsafeCast<dynamic>()
                    dynamicTrack.contentHint = "motion"

                    peerConnection.preferVideoCodecs()
                    peerConnection.applyVideoEncoding(
                        trackId = track.id,
                        maxBitrate = 2_500_000,
                        maxFramerate = 30,
                        degradationPreference = "maintain-framerate",
                    )
                }
            }
        }
    }

    fun recreatePeerConnections(
        websocketService: WebsocketService,
        roomId: String,
        isInitiator: Boolean,
        coroutineScope: CoroutineScope,
    ) {
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
                }.onFailure {
                    console.warn("Não foi possível renegociar a conexão")
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
                }.onFailure {
                    console.warn("Não foi possível iniciar a conexão de mídia")
                }
                makingOffer[socketId] = false
            }
        }

        peerConnection.onTrack { streams ->
            val remoteStream = streams[0]
            val isVideo = remoteStream.getVideoTracks().isNotEmpty()
            if (!isVideo) {
                voiceChat.handleRemoteAudio(socketId, remoteStream)
                return@onTrack
            }

            val sharingScreen = screenShareSockets.contains(socketId)
            val sharingCamera = cameraShareSockets.contains(socketId)
            val videoIndex = receivedVideoCount[socketId] ?: 0
            receivedVideoCount[socketId] = videoIndex + 1

            val isCamera =
                when {
                    // Câmera sozinha: flag é suficiente (não há track de tela).
                    sharingCamera && !sharingScreen -> true
                    // Ambos ativos: o remetente adiciona tela antes de câmera,
                    // então o 1º track é tela e os seguintes câmera.
                    sharingCamera && videoIndex > 0 -> true
                    else -> false
                }

            if (isCamera) {
                cameraSharing.handleRemoteCamera(socketId, remoteStream)
            } else {
                screenSharing.handleRemoteScreen(socketId, remoteStream)
            }
        }

        // Reconexão automática: se a conexão cai (troca de NAT/CGNAT, queda
        // transitória), faz ICE restart reenviando um novo offer. Evita forçar
        // o usuário a recarregar a página.
        peerConnection.onConnectionStateChange { state ->
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
                    }.onFailure {
                        console.warn("Não foi possível recuperar a conexão de mídia")
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
        screenShareSockets.remove(socketId)
        cameraShareSockets.remove(socketId)
        receivedVideoCount.remove(socketId)
    }

    fun closeAll() {
        peers.values.forEach { it.close() }
        peers.clear()
        makingOffer.clear()
        ignoreOffer.clear()
        isPolite.clear()
        screenShareSockets.clear()
        cameraShareSockets.clear()
        receivedVideoCount.clear()
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

            // Handle glare: if we're mid-negotiation (not stable), an incoming
            // offer may collide with our own offer. The polite peer rolls back.
            if (peerConnection.signalingState != "stable") {
                val polite = isPolite[senderId] ?: true
                if (!polite) {
                    ignoreOffer[senderId] = true
                    return@let
                }
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
            peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson))
        }
    }
}
