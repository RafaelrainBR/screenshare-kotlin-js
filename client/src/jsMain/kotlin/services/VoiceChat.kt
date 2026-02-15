package services

import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import ui.InterfaceMutations
import ui.mutations.UserListMutations

private const val SPEAKING_THRESHOLD = 30

class VoiceChat {
    var isMicMuted = true
    var isAudioMuted = true

    var localMicStream: MediaStream? = null
    val remoteAudioStreams: MutableMap<String, MediaStream> = mutableMapOf()

    fun handleRemoteAudio(
        socketId: String,
        stream: MediaStream,
    ) {
        remoteAudioStreams[socketId] = stream

        InterfaceMutations.addAudioElementForUser(userId = socketId, stream = stream)

        monitorAudioLevel(
            stream = stream,
            socketId = socketId,
            isEnable = { remoteAudioStreams.containsKey(socketId) },
        ) { isSpeaking ->
            UserListMutations.setUserSpeaking(socketId, isSpeaking)
        }
    }

    suspend fun setupLocalMic(recreatePeerConnections: suspend () -> Unit) {
        localMicStream =
            window.navigator.mediaDevices
                .getUserMedia(buildMediaStreamConstraints())
                .await()
        val audioTrack = localMicStream?.getAudioTracks()?.firstOrNull()
        if (audioTrack != null) {
            recreatePeerConnections()
        }

        monitorAudioLevel(
            stream = localMicStream!!,
            socketId = "self",
            isEnable = { true },
        ) { isSpeaking ->
            UserListMutations.setUserSpeaking("self", isSpeaking)
        }
    }

    suspend fun toggleMute(broadcastMuted: suspend (isMuted: Boolean) -> Unit) {
        isMicMuted = !isMicMuted
        localMicStream?.let { stream ->
            stream.getTracks().forEach { track -> track.enabled = !isMicMuted }
        }

        InterfaceMutations.updateAudioControls(isMicMuted = isMicMuted)
        broadcastMuted(isMicMuted)
    }

    private fun buildMediaStreamConstraints(): MediaStreamConstraints =
        MediaStreamConstraints(
            audio =
                mapOf(
                    "echoCancellation" to true,
                    "noiseSuppression" to true,
                    "autoGainControl" to false,
                    "sampleRate" to 48000,
                    "sampleSize" to 16,
                    "channelCount" to 2,
                    "latency" to 0,
                ),
        )
}

private fun monitorAudioLevel(
    stream: MediaStream,
    socketId: String,
    isEnable: () -> Boolean,
    onSpeakingChange: (Boolean) -> Unit,
) {
    val audioContext = js("new AudioContext()").unsafeCast<dynamic>()
    val source = audioContext.createMediaStreamSource(stream)
    val analyser = audioContext.createAnalyser()

    analyser.fftSize = 512
    analyser.smoothingTimeConstant = 0.8
    source.connect(analyser)

    val bufferLength = analyser.frequencyBinCount as Int
    val dataArray = js("new Uint8Array(bufferLength)").unsafeCast<dynamic>()

    var isSpeaking = false

    fun checkAudioLevel() {
        if (!isEnable()) return

        analyser.getByteFrequencyData(dataArray)

        var sum = 0
        for (i in 0 until bufferLength) {
            sum += dataArray[i].unsafeCast<Int>()
        }
        val average = sum / bufferLength

        val nowSpeaking = average > SPEAKING_THRESHOLD
        if (nowSpeaking != isSpeaking) {
            isSpeaking = nowSpeaking
            onSpeakingChange(isSpeaking)
        }

        window.requestAnimationFrame { checkAudioLevel() }
    }

    checkAudioLevel()
}
