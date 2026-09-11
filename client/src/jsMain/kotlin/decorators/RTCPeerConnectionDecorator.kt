package decorators

import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamTrack
import kotlin.js.Json
import kotlin.js.Promise

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
     * Crucial para os objetivos 1 e 2: com 'maintain-resolution' o encoder
     * mantém a imagem nítida (essencial para filme/texto) e só cai o fps sob
     * congestionamento, em vez de embaçar o quadro.
     */
    fun applyVideoEncoding(
        trackId: String,
        maxBitrate: Int,
        maxFramerate: Int,
        degradationPreference: String,
    ) {
        val sender = senderForTrack(trackId) ?: return
        val params = sender.getParameters()
        val encodings = params.encodings as? Array<dynamic>
        if (encodings != null) {
            for (enc in encodings) {
                enc.maxBitrate = maxBitrate
                enc.maxFramerate = maxFramerate
            }
        } else {
            params.encodings = js("[{ maxBitrate: maxBitrate, maxFramerate: maxFramerate }]")
        }
        params.degradationPreference = degradationPreference
        runCatching { sender.setParameters(params) }
            .onFailure { console.warn("Não foi possível aplicar a configuração de vídeo") }
    }

    /**
     * Prefere codecs na ordem: VP9 > H264 > VP8 > AV1.
     * Deve ser chamado DEPOIS de adicionar o track e ANTES de createOffer().
     * VP9 oferece a melhor qualidade a menor bitrate para conteúdo de tela/filme
     * no Chrome moderno; H264 fica como fallback de compatibilidade/hardware.
     */
    fun preferVideoCodecs() {
        // Mira o sender de VÍDEO (não o primeiro sender): se o mic fosse
        // adicionado antes da tela, firstOrNull() cairia num sender de áudio
        // e a preferência de codec não teria efeito no vídeo.
        val sender =
            getSenders().firstOrNull { s -> (s.track?.kind as? String) == "video" } ?: return
        val transceiver = sender.transceiver ?: return
        // getCapabilities belongs to RTCRtpSender, not RTCPeerConnection.
        val capabilities: dynamic = js("RTCRtpSender.getCapabilities('video')") ?: return
        val allCodecs: Array<dynamic> = capabilities.codecs as Array<dynamic>
        val order = listOf("vp9", "h264", "vp8", "av1")
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
            runCatching { transceiver.setCodecPreferences(preferred) }
                .onFailure { console.warn("Não foi possível aplicar a preferência de codec") }
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

    fun rollback(): Promise<Json> = windowRTCPeerConnection.rollback() as Promise<Json>

    fun onIceCandidateAdd(block: (Json?) -> Unit) {
        windowRTCPeerConnection.onicecandidate = { event: dynamic ->
            block(event.candidate.unsafeCast<Json?>())
        }
    }

    fun setLocalDescription(description: Json): Promise<Json> =
        windowRTCPeerConnection.setLocalDescription(description) as Promise<Json>

    fun setRemoteDescription(description: RTCSessionDescription): Promise<Json> {
        remoteSet = true
        return windowRTCPeerConnection.setRemoteDescription(description)
            .then { value ->
                drainPendingCandidates()
                value
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

    private fun drainPendingCandidates() {
        val toAdd = pendingCandidates.toList()
        pendingCandidates.clear()
        toAdd.forEach { candidate ->
            runCatching { windowRTCPeerConnection.addIceCandidate(candidate) }
                .onFailure { console.warn("Não foi possível aplicar uma atualização de conexão") }
        }
    }

    fun onConnectionStateChange(block: (state: String) -> Unit) {
        windowRTCPeerConnection.onconnectionstatechange = {
            block(windowRTCPeerConnection.connectionState as String)
        }
    }

    fun iceRestartOffer(): Promise<Json> =
        windowRTCPeerConnection.createOffer(js("{ iceRestart: true }")) as Promise<Json>

    fun close() {
        windowRTCPeerConnection.close()
    }

    companion object {
        fun create(): RTCPeerConnectionDecorator =
            create(
                turnUrl = "turn:163.176.39.139:3478",
                turnUsername = "rafael",
                turnCredential = "494e15922c47d42a",
            )

        fun create(
            turnUrl: String?,
            turnUsername: String?,
            turnCredential: String?,
        ): RTCPeerConnectionDecorator {
            val peerConnection = instantiate(turnUrl, turnUsername, turnCredential)
            peerConnection.oniceconnectionstatechange = {
            }
            return RTCPeerConnectionDecorator(peerConnection)
        }

        private fun instantiate(
            turnUrl: String?,
            turnUsername: String?,
            turnCredential: String?,
        ): dynamic =
            js(
                """
                new RTCPeerConnection({
                  iceServers: [
                    {
                      urls: ['stun:163.176.39.139:3478'],
                    },
                    {
                      urls: turnUrl,
                      username: turnUsername,
                      credential: turnCredential,
                    },
                    {
                      urls: turnUrl + '?transport=tcp',
                      username: turnUsername,
                      credential: turnCredential,
                    },
                  ],
                });
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
