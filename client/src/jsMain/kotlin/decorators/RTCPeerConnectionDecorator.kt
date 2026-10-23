package decorators

import kotlinx.browser.window
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamTrack
import screenshare.common.IceServer
import kotlin.js.Json
import kotlin.js.Promise
import kotlin.js.json

external class RTCSessionDescription(
    data: Json,
)

external class RTCIceCandidate(
    candidateInfo: Json,
)

class RTCPeerConnectionDecorator(
    private val windowRTCPeerConnection: dynamic,
) {
    val currentRemoteDescription: Any?
        get() = windowRTCPeerConnection.currentRemoteDescription

    // Fila de candidatos recebidos ANTES do remote description. Sem isso, um
    // candidate que chega antes do SDP lança InvalidStateError e é descartado,
    // o que derruba a conexão em redes ruins (CGNAT brasileiro) quando as
    // mensagens de signaling chegam fora de ordem.
    private val pendingCandidates = mutableListOf<dynamic>()
    private var remoteSet = false
    private val videoEncodings = mutableMapOf<String, Json>()

    fun createOffer(): Promise<Json> = windowRTCPeerConnection.createOffer() as Promise<Json>

    fun createAnswer(): Promise<Json> = windowRTCPeerConnection.createAnswer() as Promise<Json>

    fun addTrack(
        track: MediaStreamTrack,
        localStream: MediaStream,
    ) {
        if (!hasTrack(track)) {
            windowRTCPeerConnection.addTrack(track, localStream)
        }
    }

    fun getSenders(): Array<dynamic> = windowRTCPeerConnection.getSenders() as Array<dynamic>

    fun setVideoReception(enabled: Boolean) {
        val transceivers = windowRTCPeerConnection.getTransceivers() as Array<dynamic>
        transceivers.forEach { transceiver ->
            // A remote stop() leaves the transceiver stopped until the answer; setting its direction throws.
            val stopped = transceiver.currentDirection == "stopped" || transceiver.direction == "stopped"
            if (transceiver.receiver.track.kind == "video" && !stopped) {
                transceiver.direction =
                    when {
                        transceiver.sender.track != null -> if (enabled) "sendrecv" else "sendonly"
                        enabled -> "recvonly"
                        else -> "inactive"
                    }
            }
        }
    }

    private fun senderForTrack(trackId: String): dynamic? {
        val senders = getSenders()
        for (i in 0 until senders.size) {
            val sender = senders[i]
            val senderTrack = sender.track
            if (senderTrack != null && senderTrack.id == trackId) {
                return sender
            }
        }
        return null
    }

    fun hasTrack(track: MediaStreamTrack): Boolean = senderForTrack(track.id) != null

    // A stopped track left on its sender keeps the m-line "sendonly"; every renegotiation then
    // re-fires `track` on the viewer with that dead track.
    fun removeEndedSenders() {
        getSenders()
            .filter { sender -> sender.track?.readyState == "ended" }
            .forEach { sender ->
                videoEncodings.remove(sender.track.id as String)
                windowRTCPeerConnection.removeTrack(sender)
            }
    }

    fun replaceTrack(
        oldTrackId: String?,
        newTrack: MediaStreamTrack,
    ) {
        if (oldTrackId == null) return
        senderForTrack(oldTrackId)?.replaceTrack(newTrack)
    }

    /**
     * Controla bitrate/fps e preferência de degradação por track.
     *
     * A preferência decide o que cai sob congestionamento: 'maintain-resolution'
     * mantém o quadro nítido e só cai o fps (texto); 'maintain-framerate' faz o
     * contrário (jogo); 'balanced' divide a perda (vídeo).
     */
    fun applyVideoEncoding(
        trackId: String,
        maxBitrate: Int,
        maxFramerate: Int,
        degradationPreference: String,
        scaleResolutionDownBy: Double = 1.0,
    ): Promise<Boolean> {
        val limits =
            json(
                "maxBitrate" to maxBitrate,
                "maxFramerate" to maxFramerate,
                "degradationPreference" to degradationPreference,
                "scaleResolutionDownBy" to scaleResolutionDownBy,
            )
        videoEncodings[trackId] = limits
        return applyVideoEncoding(trackId, limits)
    }

    private fun applyVideoEncoding(
        trackId: String,
        limits: Json,
    ): Promise<Boolean> {
        val sender = senderForTrack(trackId) ?: return Promise.resolve(false)
        val params = sender.getParameters()
        val encodings = params.encodings as? Array<dynamic>
        // addTrack can expose no encodings until SDP is applied; retry after negotiation.
        if (encodings.isNullOrEmpty()) return Promise.resolve(false)
        for (enc in encodings) {
            enc.maxBitrate = limits["maxBitrate"]
            enc.maxFramerate = limits["maxFramerate"]
            enc.scaleResolutionDownBy = limits["scaleResolutionDownBy"]
        }
        params.degradationPreference = limits["degradationPreference"]
        val applied = runCatching { sender.setParameters(params) as Promise<dynamic> }.getOrElse { Promise.reject(it) }
        return applied.then<Boolean> { true }.catch {
            console.warn("Não foi possível aplicar a configuração de vídeo")
            false
        }
    }

    private fun reapplyVideoEncodings(): Promise<Array<out Boolean>> =
        Promise.all(videoEncodings.map { (trackId, limits) -> applyVideoEncoding(trackId, limits) }.toTypedArray())

    /**
     * Prefere codecs na ordem: VP9 > H264 > VP8 > AV1.
     * Deve ser chamado DEPOIS de adicionar o track e ANTES de createOffer().
     * VP9 oferece a melhor qualidade a menor bitrate para conteúdo de tela/filme
     * no Chrome moderno; H264 fica como fallback de compatibilidade/hardware.
     * [preferHardware] põe na frente os codecs que a GPU codifica (ver [probeHardwareEncoders]).
     */
    fun preferVideoCodecs(preferHardware: Boolean = false) {
        // Mira o sender de VÍDEO (não o primeiro sender): se o mic fosse
        // adicionado antes da tela, firstOrNull() cairia num sender de áudio
        // e a preferência de codec não teria efeito no vídeo.
        val transceivers =
            (windowRTCPeerConnection.getTransceivers() as Array<dynamic>)
                .filter { it.sender.track?.kind == "video" }
        if (transceivers.isEmpty()) return
        // getCapabilities belongs to RTCRtpSender, not RTCPeerConnection.
        val capabilities: dynamic = js("RTCRtpSender.getCapabilities('video')") ?: return
        val allCodecs: Array<dynamic> = capabilities.codecs as Array<dynamic>
        val order = if (preferHardware) codecOrder(hardwareEncoders) else CODEC_ORDER
        val primary =
            order.flatMap { id ->
                allCodecs.filter { codec ->
                    (codec.mimeType as String).lowercase().endsWith(id)
                }
            }
        // Keep retransmission and redundancy codecs. Removing RTX/RED/ULPFEC
        // makes fullscreen video visibly corrupt as soon as packets are lost.
        val auxiliary =
            allCodecs.filter { codec ->
                val mime = (codec.mimeType as String).lowercase()
                mime.endsWith("rtx") || mime.endsWith("red") || mime.endsWith("ulpfec")
            }
        val preferred = (primary + auxiliary).distinct()
        if (preferred.isNotEmpty()) {
            transceivers.forEach { transceiver ->
                runCatching { transceiver.setCodecPreferences(preferred.toTypedArray()) }
                    .onFailure { console.warn("Não foi possível aplicar a preferência de codec") }
            }
        }
    }

    fun onTrack(block: (streams: Array<MediaStream>) -> Unit) {
        windowRTCPeerConnection.addEventListener("track") { event ->
            try {
                block(event.streams as Array<MediaStream>)
            } catch (_: Throwable) {
                console.warn("Não foi possível processar uma faixa de mídia remota")
            }
        }
    }

    fun onNegotiationNeeded(block: () -> Unit) {
        windowRTCPeerConnection.onnegotiationneeded = block
    }

    val signalingState: String
        get() = windowRTCPeerConnection.signalingState as String

    // RTCPeerConnection has no rollback(); the spec rolls back through setLocalDescription.
    fun rollback(): Promise<Json> =
        windowRTCPeerConnection.setLocalDescription(js("({ type: 'rollback' })")) as Promise<Json>

    fun onIceCandidateAdd(block: (Json?) -> Unit) {
        windowRTCPeerConnection.onicecandidate = { event: dynamic ->
            block(event.candidate.unsafeCast<Json?>())
        }
    }

    fun setLocalDescription(description: Json): Promise<Json> =
        windowRTCPeerConnection.setLocalDescription(description).then { value: dynamic ->
            reapplyVideoEncodings().then { value }
        } as Promise<Json>

    fun setRemoteDescription(description: RTCSessionDescription): Promise<Json> {
        remoteSet = false
        val data = description.asDynamic()
        val configured =
            RTCSessionDescription(
                json(
                    "type" to data.type,
                    "sdp" to improveVideoStartupSdp(data.sdp as String),
                ),
            )
        return windowRTCPeerConnection
            .setRemoteDescription(configured)
            .then { value ->
                remoteSet = true
                drainPendingCandidates().then { reapplyVideoEncodings().then { value } }
            } as Promise<Json>
    }

    fun addIceCandidate(candidate: RTCIceCandidate): Promise<Json> {
        if (!remoteSet) {
            // SDP ainda não aplicado: enfileira e entrega depois (evita descarte).
            pendingCandidates.add(candidate)
            return js("Promise.resolve()") as Promise<Json>
        }
        return windowRTCPeerConnection.addIceCandidate(candidate) as Promise<Json>
    }

    private fun drainPendingCandidates(): Promise<Array<out Boolean>> {
        val toAdd = pendingCandidates.toList()
        pendingCandidates.clear()
        return Promise.all(
            toAdd
                .map { candidate ->
                    val applied =
                        runCatching { windowRTCPeerConnection.addIceCandidate(candidate) as Promise<dynamic> }
                            .getOrElse { Promise.reject(it) }
                    applied.then<Boolean> { true }.catch {
                        console.warn("Não foi possível aplicar uma atualização de conexão")
                        false
                    }
                }.toTypedArray(),
        )
    }

    fun onConnectionStateChange(block: (state: String) -> Unit) {
        windowRTCPeerConnection.onconnectionstatechange = {
            block(windowRTCPeerConnection.connectionState as String)
        }
    }

    fun restartIce() {
        windowRTCPeerConnection.restartIce()
    }

    fun close() {
        windowRTCPeerConnection.close()
    }

    companion object {
        // shortcut: servers without ICE_SERVERS (old builds, the beta app, self-hosts) send no ice-servers
        // packet, so the pre-v3 list stays as a fallback; drop it once every server sends one.
        private val legacyIceServers =
            listOf(
                IceServer(listOf("stun:163.176.39.139:3478")),
                IceServer(
                    listOf("turn:163.176.39.139:3478", "turn:163.176.39.139:3478?transport=tcp"),
                    "rafael",
                    "494e15922c47d42a",
                ),
            )

        /** [iceServers] comes from the server (ice-servers packet); empty means an older server. */
        fun create(iceServers: List<IceServer>): RTCPeerConnectionDecorator {
            val servers =
                iceServers
                    .ifEmpty { legacyIceServers }
                    .map { server ->
                        json("urls" to server.urls.toTypedArray()).also {
                            if (server.username.isNotEmpty()) it["username"] = server.username
                            if (server.credential.isNotEmpty()) it["credential"] = server.credential
                        }
                    }.toTypedArray()
            return RTCPeerConnectionDecorator(instantiate(servers))
        }

        // Software VP9 at 1080p60 takes several cores from the game being shared; the GPU encoder doesn't.
        private val hardwareEncoders = mutableSetOf<String>()

        /** Asks the browser once which codecs it encodes in hardware; until it answers, none is preferred. */
        fun probeHardwareEncoders() {
            val capabilities = window.navigator.asDynamic().mediaCapabilities ?: return
            CODEC_ORDER.forEach { id ->
                val video =
                    json(
                        "contentType" to "video/${id.uppercase()}",
                        "width" to 1920,
                        "height" to 1080,
                        "bitrate" to 9_000_000,
                        "framerate" to 60,
                    )
                runCatching {
                    capabilities
                        .encodingInfo(json("type" to "webrtc", "video" to video))
                        .then { info: dynamic ->
                            if (info.supported == true && info.powerEfficient == true) hardwareEncoders.add(id)
                        }.catch { _: dynamic -> }
                }
            }
        }

        private fun instantiate(iceServers: Array<Json>): dynamic =
            js(
                """
                new RTCPeerConnection({ iceServers: iceServers });
            """,
            )
    }
}

fun createRTCIceCandidate(
    candidate: String,
    sdpMid: String,
    sdpMLineIndex: Int,
): Json =
    js(
        "new RTCIceCandidate({candidate: candidate, sdpMid: sdpMid, sdpMLineIndex: sdpMLineIndex})",
    ).unsafeCast<Json>()

private val CODEC_ORDER = listOf("vp9", "h264", "vp8", "av1")

/** [CODEC_ORDER] with the hardware-encoded codecs first, each group keeping its order. */
internal fun codecOrder(hardware: Set<String>): List<String> = CODEC_ORDER.sortedBy { it !in hardware }
