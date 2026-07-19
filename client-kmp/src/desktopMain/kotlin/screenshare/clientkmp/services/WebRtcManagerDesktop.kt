package screenshare.clientkmp.services

import com.shepeliev.webrtckmp.AudioStreamTrack
import com.shepeliev.webrtckmp.IceCandidate
import com.shepeliev.webrtckmp.MediaDevices
import com.shepeliev.webrtckmp.MediaStream
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
import com.shepeliev.webrtckmp.onTrack
import com.shepeliev.webrtckmp.videoTracks
import dev.onvoid.webrtc.media.MediaDevices as NativeMediaDevices
import dev.onvoid.webrtc.media.audio.AudioDeviceModule
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
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
        val pc = PeerConnection(RtcConfiguration())
        addTracksIfNotPresent(pc)
        peers[socketId] = pc

        val iceJob = scope.launch {
            pc.onIceCandidate.collect { ice ->
                val room = appState.currentRoom ?: return@collect
                val json = iceToJson(ice)
                websocketService.sendIceCandidate(room.roomId, json, socketId)
            }
        }

        val trackJob = scope.launch {
            pc.onTrack.collect { event ->
                when (event.track) {
                    is VideoStreamTrack -> {
                        appState.updateRoom { room -> room.copy(currentSharerSocketId = socketId) }
                        _currentVideoTrack.value = event.track
                    }

                    is AudioStreamTrack -> { /* remote audio plays automatically via WebRTC */ }

                    else -> {}
                }
            }
        }

        peerJobs[socketId] = listOf(iceJob, trackJob)

        if (isInitiator) {
            scope.launch {
                runCatching { sendOffer(pc, socketId) }
                    .onFailure { println("[WebRTC Desktop] offer error [$socketId]: ${it.message}") }
            }
        }
    }

    override fun closePeerConnection(socketId: String) {
        peerJobs.remove(socketId)?.forEach { it.cancel() }
        peers.remove(socketId)?.close()
        if (appState.currentRoom?.currentSharerSocketId == socketId) {
            _currentVideoTrack.value = null
        }
    }

    override fun clearVideoTrack() {
        _currentVideoTrack.value = null
    }

    override suspend fun handleIceCandidate(packet: Packet.IceCandidateReceived) {
        val pc = peers[packet.senderId] ?: return
        val ice = jsonToIce(packet.candidate) ?: return
        runCatching { pc.addIceCandidate(ice) }
            .onFailure { println("[WebRTC Desktop] addIceCandidate error for ${packet.senderId}: ${it.message}") }
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

    override suspend fun startScreenShare(onStreamEnd: () -> Unit): Boolean {
        return runCatching {
            val stream = MediaDevices.getDisplayMedia()
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

    // ─── Private helpers ─────────────────────────────────────────────────────

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
