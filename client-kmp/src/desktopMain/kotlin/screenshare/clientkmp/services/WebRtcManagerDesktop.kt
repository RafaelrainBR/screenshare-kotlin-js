package screenshare.clientkmp.services

import com.shepeliev.webrtckmp.AudioStreamTrack
import com.shepeliev.webrtckmp.IceCandidate
import com.shepeliev.webrtckmp.IceServer
import com.shepeliev.webrtckmp.MediaDevices
import com.shepeliev.webrtckmp.MediaStream
import com.shepeliev.webrtckmp.getDisplayMediaForSource
import com.shepeliev.webrtckmp.MediaStreamTrackState
import com.shepeliev.webrtckmp.OfferAnswerOptions
import com.shepeliev.webrtckmp.PeerConnection
import com.shepeliev.webrtckmp.RtcConfiguration
import com.shepeliev.webrtckmp.SessionDescription
import com.shepeliev.webrtckmp.SessionDescriptionType
import com.shepeliev.webrtckmp.VideoStreamTrack
import com.shepeliev.webrtckmp.WebRtc
import com.shepeliev.webrtckmp.audioTracks
import com.shepeliev.webrtckmp.onIceCandidate
import com.shepeliev.webrtckmp.onIceConnectionStateChange
import com.shepeliev.webrtckmp.onIceGatheringState
import com.shepeliev.webrtckmp.onConnectionStateChange
import com.shepeliev.webrtckmp.onTrack
import com.shepeliev.webrtckmp.videoTracks
import dev.onvoid.webrtc.media.MediaDevices as NativeMediaDevices
import dev.onvoid.webrtc.media.audio.AudioDeviceModule
import dev.onvoid.webrtc.media.video.desktop.DesktopSource
import dev.onvoid.webrtc.media.video.desktop.ScreenCapturer
import dev.onvoid.webrtc.media.video.desktop.WindowCapturer
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import screenshare.common.Packet

class WebRtcManagerDesktop(
    private val holder: WebRtcAppStateHolder,
    private val websocketService: WebsocketService,
) : WebRtcManager {
    private val appState get() = holder.appState
    private val scope get() = holder.coroutineScope

    private val peers = mutableMapOf<String, PeerConnection>()
    private val peerJobs = mutableMapOf<String, List<Job>>()
    // Buffers ICE candidates that arrive before setRemoteDescription completes
    private val pendingCandidates = mutableMapOf<String, MutableList<IceCandidate>>()
    private var localMicStream: MediaStream? = null
    private var localScreenStream: MediaStream? = null

    private val _currentVideoTrack = MutableStateFlow<Any?>(null)
    override val currentVideoTrack: StateFlow<Any?> = _currentVideoTrack.asStateFlow()

    init {
        // The default AudioDeviceModule builder calls startPlayout() eagerly, before any
        // WebRTC audio data is flowing. On Windows this renders uninitialised buffers as
        // loud static noise. Override it so startPlayout() is only called by the
        // PeerConnectionFactory internally when remote audio actually arrives.
        WebRtc.configureBuilder {
            audioModuleBuilder = {
                AudioDeviceModule().apply {
                    NativeMediaDevices.getDefaultAudioRenderDevice()?.let { device ->
                        setPlayoutDevice(device)
                        initPlayout()
                        // ← intentionally no startPlayout() here
                    }
                    NativeMediaDevices.getDefaultAudioCaptureDevice()?.let { device ->
                        setRecordingDevice(device)
                        initRecording()
                        // ← intentionally no startRecording() here
                    }
                }
            }
        }
    }

    // ─── WebRtcManager ──────────────────────────────────────────────────────

    override fun createPeerConnection(
        socketId: String,
        isInitiator: Boolean,
    ) {
        val pc = PeerConnection(RtcConfiguration(
            iceServers = listOf(
                // Multiple STUN servers for redundancy
                IceServer(urls = listOf(
                    "stun:stun.l.google.com:19302",
                    "stun:stun1.l.google.com:19302",
                    "stun:stun2.l.google.com:19302",
                    "stun:stun3.l.google.com:19302",
                    "stun:stun4.l.google.com:19302",
                    "stun:stun.stunprotocol.org:3478",
                    "stun:stun.ekiga.net",
                )),
                // freestun.net — free open TURN server
                IceServer(
                    urls = listOf(
                        "turn:freestun.net:3479",
                        "turn:freestun.net:3479?transport=tcp",
                        "turns:freestun.net:5350",
                    ),
                    username = "free",
                    password = "free",
                ),
                // openrelay — fallback TURN
                IceServer(
                    urls = listOf(
                        "turn:openrelay.metered.ca:80",
                        "turn:openrelay.metered.ca:443?transport=tcp",
                    ),
                    username = "openrelayproject",
                    password = "openrelayproject",
                ),
            ),
        ))
        addTracksIfNotPresent(pc)
        peers[socketId] = pc

        println("[WebRTC] createPeerConnection peer=${socketId.take(8)} initiator=$isInitiator")

        val iceJob = scope.launch {
            val candidateCounts = mutableMapOf("host" to 0, "srflx" to 0, "prflx" to 0, "relay" to 0)
            pc.onIceCandidate.collect { ice ->
                val room = appState.currentRoom ?: return@collect
                val json = iceToJson(ice)
                val type = when {
                    ice.candidate.contains("typ relay") -> "relay(TURN)"
                    ice.candidate.contains("typ srflx") -> "srflx(STUN)"
                    ice.candidate.contains("typ prflx") -> "prflx"
                    else -> "host"
                }
                val key = type.substringBefore('(')
                candidateCounts[key] = (candidateCounts[key] ?: 0) + 1
                val proto = Regex("\\d+ (udp|tcp)").find(ice.candidate)?.groupValues?.get(1) ?: ""
                println("[ICE] peer=${socketId.take(8)} local $proto $type")
                websocketService.sendIceCandidate(room.roomId, json, socketId)
            }
        }

        val gatheringJob = scope.launch {
            pc.onIceGatheringState.collect { state ->
                println("[ICE] peer=${socketId.take(8)} gathering → $state")
            }
        }

        val iceStateJob = scope.launch {
            pc.onIceConnectionStateChange.collect { state ->
                println("[ICE] peer=${socketId.take(8)} state → $state")
                if (state == com.shepeliev.webrtckmp.IceConnectionState.Failed) {
                    println("[ICE] ⚠ peer=${socketId.take(8)} FALHOU — sem relay(TURN)? Verifique firewall/TURN server")
                }
            }
        }

        val connStateJob = scope.launch {
            pc.onConnectionStateChange.collect { state ->
                println("[WebRTC] peer=${socketId.take(8)} connection → $state")
            }
        }

        val trackJob = scope.launch {
            pc.onTrack.collect { event ->
                when (event.track) {
                    is VideoStreamTrack -> {
                        println("[WebRTC] peer=${socketId.take(8)} onTrack video — aguardando ICE Connected para exibir")
                        // Aguarda ICE conectar antes de exibir o vídeo
                        scope.launch {
                            pc.onIceConnectionStateChange
                                .filter { it == com.shepeliev.webrtckmp.IceConnectionState.Connected ||
                                          it == com.shepeliev.webrtckmp.IceConnectionState.Completed }
                                .first()
                            println("[WebRTC] peer=${socketId.take(8)} ICE conectado — exibindo vídeo")
                            appState.updateRoom { room -> room.copy(currentSharerSocketId = socketId) }
                            _currentVideoTrack.value = event.track
                        }
                    }

                    is AudioStreamTrack -> { /* remote audio plays automatically via WebRTC */ }

                    else -> {}
                }
            }
        }

        peerJobs[socketId] = listOf(iceJob, gatheringJob, iceStateJob, connStateJob, trackJob)

        if (isInitiator) {
            scope.launch {
                runCatching { sendOffer(pc, socketId) }
                    .onFailure { println("[WebRTC Desktop] offer error [$socketId]: ${it.message}") }
            }
        }
    }

    override fun closePeerConnection(socketId: String) {
        peerJobs.remove(socketId)?.forEach { it.cancel() }
        pendingCandidates.remove(socketId)
        peers.remove(socketId)?.close()
        if (appState.currentRoom?.currentSharerSocketId == socketId) {
            _currentVideoTrack.value = null
        }
    }

    override fun clearVideoTrack() {
        _currentVideoTrack.value = null
    }

    override suspend fun handleIceCandidate(packet: Packet.IceCandidateReceived) {
        val ice = jsonToIce(packet.candidate) ?: run {
            println("[ICE] peer=${packet.senderId.take(8)} candidato inválido ignorado")
            return
        }
        val pc = peers[packet.senderId]
        if (pc == null) {
            // Peer ainda não criado — buffering
            println("[ICE] peer=${packet.senderId.take(8)} remote candidate buffered (peer não existe ainda)")
            pendingCandidates.getOrPut(packet.senderId) { mutableListOf() }.add(ice)
            return
        }
        val typ = Regex("typ (\\w+)").find(packet.candidate)?.groupValues?.get(1) ?: "?"
        println("[ICE] peer=${packet.senderId.take(8)} remote $typ")
        runCatching { pc.addIceCandidate(ice) }
            .onFailure { println("[ICE] peer=${packet.senderId.take(8)} addIceCandidate ERRO: ${it.message}") }
    }

    override suspend fun handleDescription(packet: Packet.DescriptionReceived) {
        val type = packet.description["type"] ?: return
        val sdp = packet.description["sdp"] ?: return
        when (type) {
            "offer" -> {
                if (!peers.containsKey(packet.senderId)) {
                    createPeerConnection(packet.senderId, isInitiator = false)
                }
                val pc = peers[packet.senderId] ?: return
                pc.setRemoteDescription(SessionDescription(SessionDescriptionType.Offer, sdp))
                // Flush any candidates that arrived before setRemoteDescription
                val pending = pendingCandidates.remove(packet.senderId)
                if (!pending.isNullOrEmpty()) {
                    println("[ICE] peer=${packet.senderId.take(8)} flushing ${pending.size} buffered candidates")
                    pending.forEach { ice ->
                        runCatching { pc.addIceCandidate(ice) }
                            .onFailure { println("[ICE] peer=${packet.senderId.take(8)} addIceCandidate (buffered) ERRO: ${it.message}") }
                    }
                }
                val answer = pc.createAnswer(OfferAnswerOptions())
                pc.setLocalDescription(answer)
                val room = appState.currentRoom ?: return
                websocketService.sendDescription(
                    roomId = room.roomId,
                    description = mapOf("type" to "answer", "sdp" to answer.sdp),
                    targetId = packet.senderId,
                )
            }

            "answer" -> {
                val pc = peers[packet.senderId] ?: return
                pc.setRemoteDescription(SessionDescription(SessionDescriptionType.Answer, sdp))
            }
        }
    }

    override fun recreateAllConnections() {
        scope.launch {
            peers.forEach { (socketId, pc) ->
                addTracksIfNotPresent(pc)
                runCatching { sendOffer(pc, socketId) }
                    .onFailure { println("[WebRTC Desktop] recreate error [$socketId]: ${it.message}") }
            }
        }
    }

    override suspend fun startLocalMic(): Boolean {
        return runCatching {
            val stream = MediaDevices.getUserMedia {
                audio {
                    echoCancellation(true)
                    noiseSuppression(true)
                }
            }
            localMicStream = stream
            recreateAllConnections()
            true
        }.getOrElse { e ->
            println("[WebRTC Desktop] startLocalMic error: ${e.message}")
            false
        }
    }

    override fun toggleMic(isMuted: Boolean) {
        localMicStream?.audioTracks?.forEach { track ->
            track.enabled = !isMuted
        }
    }

    override suspend fun startScreenShare(config: ScreenShareConfig, onStreamEnd: () -> Unit): Boolean {
        return runCatching {
            // If a specific source was selected in the dialog, capture it directly.
            // Otherwise fall back to the OS system picker.
            val stream = if (config.sourceId != null) {
                val isWindow = config.sourceId.startsWith("window:")
                val rawId = config.sourceId.substringAfter(':').toLong()
                getDisplayMediaForSource(rawId, isWindow)
            } else {
                MediaDevices.getDisplayMedia()
            }
            localScreenStream = stream
            _currentVideoTrack.value = stream.videoTracks.firstOrNull()
            // Notify when the screen track ends (user stops sharing via OS UI)
            stream.videoTracks.firstOrNull()?.let { track ->
                scope.launch {
                    track.state.collect { state ->
                        if (state is MediaStreamTrackState.Ended) {
                            _currentVideoTrack.value = null
                            localScreenStream = null
                            recreateAllConnections()
                            onStreamEnd()
                        }
                    }
                }
            }
            recreateAllConnections()
            true
        }.getOrElse { e ->
            println("[WebRTC Desktop] startScreenShare error: ${e.message}")
            false
        }
    }

    override fun stopScreenShare() {
        localScreenStream?.videoTracks?.forEach { it.stop() }
        localScreenStream = null
        _currentVideoTrack.value = null
        recreateAllConnections()
    }

    override suspend fun enumerateScreenSources(): List<ScreenSource> =
        runCatching {
            ensureNativeInitialized()
            val sources = mutableListOf<ScreenSource>()
            runCatching {
                ScreenCapturer().getDesktopSources()?.forEach { src: DesktopSource ->
                    sources.add(ScreenSource(id = "monitor:${src.id}", title = src.title ?: "Tela ${src.id}", isMonitor = true))
                }
            }.onFailure { println("[WebRTC Desktop] error enumerateScreenSources (screens): ${it.message}"); it.printStackTrace() }
            runCatching {
                WindowCapturer().getDesktopSources()?.forEach { src: DesktopSource ->
                    sources.add(ScreenSource(id = "window:${src.id}", title = src.title ?: "Janela ${src.id}", isMonitor = false))
                }
            }.onFailure { println("[WebRTC Desktop] error enumerateScreenSources (windows): ${it.message}"); it.printStackTrace() }
            sources
        }.getOrElse { e ->
            println("[WebRTC Desktop] error enumerateScreenSources: ${e.message}")
            emptyList()
        }

    override suspend fun enumerateAudioInputs(): List<AudioDevice> =
        runCatching {
            NativeMediaDevices.getAudioCaptureDevices()?.map { d ->
                AudioDevice(id = d.descriptor, label = d.name)
            } ?: emptyList()
        }.getOrElse { e ->
            println("[WebRTC Desktop] error enumerateAudioInputs: ${e.message}")
            emptyList()
        }

    override suspend fun enumerateAudioOutputs(): List<AudioDevice> =
        runCatching {
            NativeMediaDevices.getAudioRenderDevices()?.map { d ->
                AudioDevice(id = d.descriptor, label = d.name)
            } ?: emptyList()
        }.getOrElse { e ->
            println("[WebRTC Desktop] error enumerateAudioOutputs: ${e.message}")
            e.printStackTrace()
            emptyList()
        }

    override suspend fun applyDeviceSettings(settings: DeviceSettings) {
        runCatching {
            // Switch mic device if requested
            if (settings.micDeviceId != null) {
                val newStream = MediaDevices.getUserMedia {
                    audio {
                        deviceId(settings.micDeviceId)
                        echoCancellation(true)
                        noiseSuppression(true)
                    }
                }
                localMicStream?.audioTracks?.forEach { it.stop() }
                localMicStream = newStream
                recreateAllConnections()
            }
            // Adjust mic gain via track enabled state is basic; advanced volume via WebRTC AudioDeviceModule
            localMicStream?.audioTracks?.forEach { track ->
                track.enabled = settings.micVolume > 0f
            }
        }.onFailure { println("[WebRTC Desktop] applyDeviceSettings error: ${it.message}") }
    }

    // ─── Private helpers ─────────────────────────────────────────────────────

    /**
     * Creating and immediately closing a [PeerConnection] forces [WebRtc.peerConnectionFactory]
     * to initialize, which loads the native WebRTC library. Without this, classes like
     * [ScreenCapturer] and [WindowCapturer] fail with [UnsatisfiedLinkError] on their
     * native `initialize()` call when no peer connections have been established yet.
     */
    private fun ensureNativeInitialized() {
        if (peers.isNotEmpty()) return // already bootstrapped via normal usage
        runCatching {
            val dummy = PeerConnection(RtcConfiguration())
            dummy.close()
        }.onFailure { println("[WebRTC Desktop] ensureNativeInitialized error: ${it.message}") }
    }

    private fun addTracksIfNotPresent(pc: PeerConnection) {
        val existingSenders = pc.getSenders()
        val existingTrackIds = existingSenders.mapNotNull { it.track?.id }.toSet()

        fun addStream(stream: MediaStream?) {
            stream ?: return
            stream.tracks.forEach { track ->
                if (track.id !in existingTrackIds) {
                    pc.addTrack(track, stream)
                }
            }
        }

        addStream(localScreenStream)
        addStream(localMicStream)
    }

    private suspend fun sendOffer(
        pc: PeerConnection,
        socketId: String,
    ) {
        val room = appState.currentRoom ?: return
        val offer = pc.createOffer(OfferAnswerOptions())
        pc.setLocalDescription(offer)
        websocketService.sendDescription(
            roomId = room.roomId,
            description = mapOf("type" to "offer", "sdp" to offer.sdp),
            targetId = socketId,
        )
    }

    /**
     * Serializes an [IceCandidate] to the JSON format expected by the JS client:
     * `{"candidate":"<sdp>","sdpMid":"<mid>","sdpMLineIndex":<idx>}`
     */
    private fun iceToJson(ice: IceCandidate): String =
        """{"candidate":"${ice.candidate}","sdpMid":"${ice.sdpMid}","sdpMLineIndex":${ice.sdpMLineIndex}}"""

    /**
     * Parses the JSON ICE candidate string coming from the JS client.
     */
    private fun jsonToIce(json: String): IceCandidate? =
        runCatching {
            val candidate = extractJsonString(json, "candidate") ?: return null
            val sdpMid = extractJsonString(json, "sdpMid") ?: "0"
            val sdpMLineIndex = extractJsonInt(json, "sdpMLineIndex") ?: 0
            IceCandidate(sdpMid, sdpMLineIndex, candidate)
        }.getOrNull()

    private fun extractJsonString(
        json: String,
        key: String,
    ): String? {
        val pattern = """"$key"\s*:\s*"([^"]*)"""".toRegex()
        return pattern.find(json)?.groupValues?.get(1)
    }

    private fun extractJsonInt(
        json: String,
        key: String,
    ): Int? {
        val pattern = """"$key"\s*:\s*(\d+)""".toRegex()
        return pattern.find(json)?.groupValues?.get(1)?.toIntOrNull()
    }
}
