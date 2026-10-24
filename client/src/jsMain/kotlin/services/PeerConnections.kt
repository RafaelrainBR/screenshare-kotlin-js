package services

import decorators.RTCIceCandidate
import decorators.RTCPeerConnectionDecorator
import decorators.RTCSessionDescription
import decorators.createRTCIceCandidate
import decorators.upgradeAudioQualitySdp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.w3c.dom.mediacapture.MediaStreamTrack
import screenshare.common.IceServer
import kotlin.js.Json
import kotlin.js.json

class PeerConnections(
    private val voiceChat: VoiceChat,
    private val screenSharing: ScreenSharing,
    private val cameraSharing: CameraSharing,
) {
    val peers: MutableMap<String, RTCPeerConnectionDecorator> = mutableMapOf()

    // Perfect-negotiation state, keyed by socketId (see MDN "Perfect Negotiation").
    private val isPolite = mutableMapOf<String, Boolean>()
    private val negotiationLocks = mutableMapOf<String, Mutex>()

    // Sinalização local de quais compartilhamentos cada peer está transmitindo.
    // Usado para decidir, em onTrack, se um track de vídeo é da TELA ou da CÂMERA
    // (ambos têm video track). Mantidos em sincronia pelos packets Start/Stop.
    private val screenShareSockets = mutableSetOf<String>()
    private val cameraShareSockets = mutableSetOf<String>()

    // Tracks de vídeo recebidos por peer (por id) e se cada um é a câmera.
    // Zerado ao parar um compartilhamento.
    private val receivedVideo = mutableMapOf<String, MutableMap<String, Pair<MediaStreamTrack, Boolean>>>()
    private var videoReceptionEnabled = true

    /** STUN/TURN from the server's ice-servers packet; it arrives before any peer is created. */
    var iceServers: List<IceServer> = emptyList()

    fun markScreenSharing(
        socketId: String,
        active: Boolean,
    ) {
        if (active) screenShareSockets.add(socketId) else screenShareSockets.remove(socketId)
        if (!active) receivedVideo.remove(socketId)
    }

    fun markCameraSharing(
        socketId: String,
        active: Boolean,
    ) {
        if (active) cameraShareSockets.add(socketId) else cameraShareSockets.remove(socketId)
        if (!active) receivedVideo.remove(socketId)
    }

    fun isScreenSharing(socketId: String): Boolean = screenShareSockets.contains(socketId)

    fun isCameraSharing(socketId: String): Boolean = cameraShareSockets.contains(socketId)

    fun setVideoReception(enabled: Boolean) {
        if (videoReceptionEnabled == enabled) return
        videoReceptionEnabled = enabled
        peers.values.forEach { it.setVideoReception(enabled) }
    }

    /** Re-applies the share profile to every viewer; setParameters needs no renegotiation. */
    suspend fun applyShareEncoding(): Boolean {
        val track = screenSharing.localScreenStream?.getVideoTracks()?.firstOrNull() ?: return false
        return peers.values.map { applyShareEncoding(it, track) }.all { it.await() }
    }

    private fun applyShareEncoding(
        peerConnection: RTCPeerConnectionDecorator,
        track: MediaStreamTrack,
    ): kotlin.js.Promise<Boolean> {
        val profile = screenSharing.lastShareProfile
        track.asDynamic().contentHint = profile.priority.contentHint
        return peerConnection.applyVideoEncoding(
            trackId = track.id,
            maxBitrate = shareBitrate(profile, screenSharing.lastShareHeight),
            maxFramerate = profile.fps,
            degradationPreference = profile.priority.degradationPreference,
        )
    }

    private fun addTracksIfNotPresent(peerConnection: RTCPeerConnectionDecorator) {
        peerConnection.removeEndedSenders()

        screenSharing.localScreenStream?.getTracks()?.forEach { track ->
            if (!peerConnection.hasTrack(track)) {
                peerConnection.addTrack(track, screenSharing.localScreenStream!!)

                if (track.kind == "video") {
                    // Prefere codec e controla bitrate/fps ANTES de createOffer. Fora do
                    // modo nitidez, o encoder da GPU poupa a CPU do jogo compartilhado.
                    peerConnection.preferVideoCodecs(
                        preferHardware = screenSharing.lastShareProfile.priority != SharePriority.SHARPNESS,
                    )
                    applyShareEncoding(peerConnection, track)
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

        peerConnection.setVideoReception(videoReceptionEnabled)
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
        val peerConnection = peerConnection ?: RTCPeerConnectionDecorator.create(iceServers)

        addTracksIfNotPresent(peerConnection)

        isPolite[socketId] = !isInitiator
        val negotiationLock = Mutex()
        negotiationLocks[socketId] = negotiationLock

        peerConnection.onIceCandidateAdd { iceCandidate ->
            if (iceCandidate != null) {
                coroutineScope.launch {
                    websocketService.sendIceCandidate(roomId = roomId, targetId = socketId, candidate = iceCandidate)
                }
            }
        }

        // Serialize SDP operations, including the awaits inside createOffer and rollback.
        suspend fun sendOffer() {
            negotiationLock.withLock {
                if (peerConnection.signalingState != "stable") return@withLock
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
                    console.warn("Não foi possível renegociar a conexão")
                }
            }
        }

        peerConnection.onNegotiationNeeded {
            coroutineScope.launch { sendOffer() }
        }

        // An empty connection may not fire negotiationneeded for its first offer.
        if (isInitiator) coroutineScope.launch { sendOffer() }

        peerConnection.onTrack { streams ->
            val remoteStream = streams[0]
            // Renegotiation re-fires `track` for a share we already stopped (older senders keep
            // the dead track); showing it would replace a live screen with a frozen/gray one.
            if (remoteStream.getTracks().all { it.asDynamic().readyState == "ended" }) return@onTrack
            val isVideo = remoteStream.getVideoTracks().isNotEmpty()
            if (!isVideo) {
                voiceChat.handleRemoteAudio(socketId, remoteStream)
                return@onTrack
            }

            // The screen's audio and a page coming back visible re-fire `track` for a video
            // already placed: it keeps its place instead of counting as another one.
            val video = remoteStream.getVideoTracks().first()
            val received = receivedVideo.getOrPut(socketId) { mutableMapOf() }
            val others =
                received.filterKeys { it != video.id }.values.filter {
                    it.first.asDynamic().readyState ==
                        "live"
                }
            val isCamera =
                isCameraTrack(
                    known = received[video.id]?.second,
                    liveScreen = others.any { !it.second },
                    liveCamera = others.any { it.second },
                    sharingScreen = screenShareSockets.contains(socketId),
                    sharingCamera = cameraShareSockets.contains(socketId),
                    earlierVideos = received.size,
                )
            received[video.id] = video to isCamera

            if (isCamera) {
                cameraSharing.handleRemoteCamera(socketId, remoteStream)
            } else {
                screenSharing.handleRemoteScreen(socketId, remoteStream)
            }
        }

        // Reconexão automática: se a conexão cai (troca de NAT/CGNAT, queda
        // transitória), faz ICE restart reenviando um novo offer. Evita forçar
        // o usuário a recarregar a página. restartIce dispara negotiationneeded,
        // então o offer segue o caminho normal, com resolução de glare.
        peerConnection.onConnectionStateChange { state ->
            if (state == "failed") peerConnection.restartIce()
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
        receivedVideo.remove(socketId)
        isPolite.remove(socketId)
        negotiationLocks.remove(socketId)
    }

    fun closeAll() {
        peers.values.forEach { it.close() }
        peers.clear()
        isPolite.clear()
        negotiationLocks.clear()
        screenShareSockets.clear()
        cameraShareSockets.clear()
        receivedVideo.clear()
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
        val lock = negotiationLocks[senderId] ?: return
        lock.withLock {
            peers[senderId]?.let { peerConnection ->

                // Glare: the impolite peer ignores the incoming offer (the polite side
                // will answer ours); the polite peer rolls back its own offer.
                val collision = peerConnection.signalingState != "stable"
                if (collision && isPolite[senderId] == false) return@let
                if (peerConnection.signalingState != "stable") peerConnection.rollback().await()

                peerConnection.setRemoteDescription(RTCSessionDescription(descriptionJson)).await()
                peerConnection.setVideoReception(videoReceptionEnabled)

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
    }

    suspend fun updateDescriptionFromAnswer(
        senderId: String,
        descriptionJson: Json,
    ) {
        val lock = negotiationLocks[senderId] ?: return
        lock.withLock {
            peers[senderId]?.setRemoteDescription(RTCSessionDescription(descriptionJson))?.await()
        }
    }
}

/**
 * Whether a peer's incoming video is its camera (true) or its screen. A peer sends at most one
 * of each, so with the other kind already live the answer does not depend on whether the offer
 * or the start-*-share packet arrived first (both senders offer before announcing).
 */
internal fun isCameraTrack(
    known: Boolean?,
    liveScreen: Boolean,
    liveCamera: Boolean,
    sharingScreen: Boolean,
    sharingCamera: Boolean,
    earlierVideos: Int,
): Boolean =
    known ?: when {
        liveScreen && !liveCamera -> true

        liveCamera && !liveScreen -> false

        // Câmera sozinha: flag é suficiente (não há track de tela).
        sharingCamera && !sharingScreen -> true

        // Ambos ativos: o remetente adiciona tela antes de câmera,
        // então o 1º track é tela e os seguintes câmera.
        sharingCamera && earlierVideos > 0 -> true

        else -> false
    }
