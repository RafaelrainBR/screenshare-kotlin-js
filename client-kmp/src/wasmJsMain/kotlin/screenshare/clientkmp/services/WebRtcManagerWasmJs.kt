@file:OptIn(kotlin.js.ExperimentalWasmJsInterop::class)

package screenshare.clientkmp.services

import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import screenshare.clientkmp.util.currentTimeMillis
import screenshare.common.Packet
import kotlin.js.JsAny

private const val SPEAKING_THRESHOLD = 30
private const val POLL_INTERVAL_MS = 50L
private const val MEDIA_TIMEOUT_MS = 30_000L

/**
 * Full browser WebRTC implementation for Kotlin/wasmJs.
 *
 * Async JS operations (getUserMedia, createOffer, etc.) use a queue-based
 * polling pattern instead of JsPromise / .await(), which are not available
 * in Kotlin/WasmJs.
 */
class WebRtcManagerWasmJs(
    private val holder: WebRtcAppStateHolder,
    private val websocketService: WebsocketService,
) : WebRtcManager {
    private val appState get() = holder.appState
    private val scope get() = holder.coroutineScope

    private val peers = mutableMapOf<String, JsAny>()
    private val audioMonitors = mutableMapOf<String, JsAny>()
    private var localMicStream: JsAny? = null
    private var localScreenStream: JsAny? = null

    // Video rendering on wasmJs is handled via HTML <video> elements (JS interop).
    // This stub satisfies the interface; a full wasmJs VideoView is a future task.
    override val currentVideoTrack: StateFlow<Any?> = MutableStateFlow(null)

    init {
        scope.launch {
            while (true) {
                drainSpeakingQueue()
                delay(100)
            }
        }
    }

    override fun createPeerConnection(
        socketId: String,
        isInitiator: Boolean,
    ) {
        val pc = jsNewPeerConnection()
        jsSetupTrackQueue(pc)
        addTracksIfAbsent(pc)
        peers[socketId] = pc
        startTrackPolling(socketId, pc)
        if (isInitiator) {
            scope.launch {
                runCatching { sendOffer(pc, socketId) }
                    .onFailure { println("[WebRTC] offer error [$socketId]: ${it.message}") }
            }
        }
    }

    override fun closePeerConnection(socketId: String) {
        peers.remove(socketId)?.let { jsPcClose(it) }
        audioMonitors.remove(socketId)?.let { jsStopAudioMonitor(it) }
        appState.setSpeaking(socketId, false)
    }

    override fun clearVideoTrack() {
        // wasmJs renders video via browser <video> elements; nothing to clear here.
    }

    override fun recreateAllConnections() {
        scope.launch {
            peers.forEach { (socketId, pc) ->
                addTracksIfAbsent(pc)
                runCatching { sendOffer(pc, socketId) }
                    .onFailure { println("[WebRTC] recreate error [$socketId]: ${it.message}") }
            }
        }
    }

    override suspend fun handleIceCandidate(packet: Packet.IceCandidateReceived) {
        peers[packet.senderId]?.let { pc ->
            jsAddIceCandidate(pc, packet.candidate)
        }
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
                jsSetRemoteAndAnswer(pc, type, sdp)
                val result =
                    waitForSdpResult(pc) ?: run {
                        println("[WebRTC] answer timed out for ${packet.senderId}")
                        return
                    }
                if (jsSdpResultIsError(result)) {
                    println("[WebRTC] answer JS error for ${packet.senderId}")
                    return
                }
                val room = appState.currentRoom ?: return
                websocketService.sendDescription(
                    roomId = room.roomId,
                    description = mapOf("type" to jsSdpType(result), "sdp" to jsSdpContent(result)),
                    targetId = packet.senderId,
                )
            }

            "answer" -> {
                val pc = peers[packet.senderId] ?: return
                jsSetRemoteAnswer(pc, type, sdp)
            }
        }
    }

    override suspend fun startLocalMic(): Boolean {
        jsRequestUserMedia()
        val result = waitForMediaResult("__micQueue") ?: return false
        if (!jsMediaResultOk(result)) return false
        val stream = jsMediaResultStream(result)
        localMicStream = stream
        startAudioMonitor(stream, "self")
        recreateAllConnections()
        return true
    }

    override fun toggleMic(isMuted: Boolean) {
        val stream = localMicStream ?: return
        val tracks = jsGetAllTracks(stream)
        repeat(jsArrayLength(tracks)) { i -> jsSetTrackEnabled(jsArrayGet(tracks, i), !isMuted) }
    }

    override suspend fun startScreenShare(onStreamEnd: () -> Unit): Boolean {
        jsRequestDisplayMedia()
        val result = waitForMediaResult("__screenQueue") ?: return false
        if (!jsMediaResultOk(result)) return false
        val stream = jsMediaResultStream(result)
        localScreenStream = stream
        recreateAllConnections()
        return true
    }

    override fun stopScreenShare() {
        localScreenStream?.let { stream ->
            val tracks = jsGetAllTracks(stream)
            repeat(jsArrayLength(tracks)) { i -> jsTrackStop(jsArrayGet(tracks, i)) }
        }
        localScreenStream = null
        recreateAllConnections()
    }

    // ─── Private helpers ─────────────────────────────────────────────────────

    private fun addTracksIfAbsent(pc: JsAny) {
        fun addStream(stream: JsAny?) {
            stream ?: return
            val tracks = jsGetAllTracks(stream)
            repeat(jsArrayLength(tracks)) { i ->
                val track = jsArrayGet(tracks, i)
                if (!jsHasTrack(pc, jsTrackId(track))) jsAddTrack(pc, track, stream)
            }
        }
        addStream(localScreenStream)
        addStream(localMicStream)
    }

    private suspend fun sendOffer(
        pc: JsAny,
        socketId: String,
    ) {
        val room = appState.currentRoom ?: return
        jsCreateOffer(pc)
        val result =
            waitForSdpResult(pc) ?: run {
                println("[WebRTC] offer SDP timed out for $socketId")
                return
            }
        if (jsSdpResultIsError(result)) {
            println("[WebRTC] offer JS error for $socketId")
            return
        }
        websocketService.sendDescription(
            roomId = room.roomId,
            description = mapOf("type" to jsSdpType(result), "sdp" to jsSdpContent(result)),
            targetId = socketId,
        )
    }

    /** Polls pc.__sdpQueue until a result appears or [MEDIA_TIMEOUT_MS] elapses. */
    private suspend fun waitForSdpResult(pc: JsAny): JsAny? {
        val deadline = currentTimeMillis() + MEDIA_TIMEOUT_MS
        while (currentTimeMillis() < deadline) {
            jsPopSdpResult(pc)?.let { return it }
            delay(POLL_INTERVAL_MS)
        }
        return null
    }

    /** Polls a named globalThis queue until a result appears or timeout. */
    private suspend fun waitForMediaResult(queue: String): JsAny? {
        val deadline = currentTimeMillis() + MEDIA_TIMEOUT_MS
        while (currentTimeMillis() < deadline) {
            jsPopMediaResult(queue)?.let { return it }
            delay(POLL_INTERVAL_MS)
        }
        return null
    }

    private fun startTrackPolling(
        socketId: String,
        pc: JsAny,
    ) {
        scope.launch {
            while (peers.containsKey(socketId)) {
                jsPopTrackEvent(pc)?.let { event ->
                    if (jsEventIsVideo(event)) {
                        appState.updateRoom { room -> room.copy(currentSharerSocketId = socketId) }
                    } else {
                        startAudioMonitor(jsEventStream(event), socketId)
                    }
                }
                delay(100)
            }
        }
    }

    private fun startAudioMonitor(
        stream: JsAny,
        socketId: String,
    ) {
        audioMonitors[socketId]?.let { jsStopAudioMonitor(it) }
        audioMonitors[socketId] = jsStartAudioMonitor(stream, SPEAKING_THRESHOLD, socketId)
    }

    private fun drainSpeakingQueue() {
        while (true) {
            val event = jsPopSpeakEvent() ?: break
            appState.setSpeaking(jsSpeakId(event), jsSpeakFlag(event))
        }
    }
}
