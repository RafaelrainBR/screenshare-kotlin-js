package services

import kotlin.js.json
import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import org.w3c.dom.mediacapture.MediaStreamTrack
import ui.InterfaceMutations
import ui.mutations.UserListMutations

private const val SPEAKING_THRESHOLD = 30

class VoiceChat {
    var isMicMuted = true
    var isAudioMuted = true

    var localMicStream: MediaStream? = null
    var selectedInputDeviceId: String? = null
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

    suspend fun setupLocalMic(
        recreatePeerConnections: suspend () -> Unit,
        deviceId: String? = null,
        onMicTrackReplaced: ((oldTrackId: String?, newTrack: MediaStreamTrack?) -> Unit)? = null,
    ) {
        val effectiveDeviceId = deviceId ?: selectedInputDeviceId
        selectedInputDeviceId = effectiveDeviceId

        val oldTrack = localMicStream?.getAudioTracks()?.firstOrNull()
        val newStream =
            window.navigator.mediaDevices
                .getUserMedia(buildMediaStreamConstraints(effectiveDeviceId))
                .await()
        val audioTrack = newStream.getAudioTracks().firstOrNull()

        // Replace the track on the same RTP sender (before stopping the old
        // one) so switching devices never leaves the previous mic playing.
        onMicTrackReplaced?.invoke(oldTrack?.id, audioTrack)

        localMicStream?.getTracks()?.forEach { track -> track.stop() }
        localMicStream = newStream

        if (audioTrack != null) {
            recreatePeerConnections()
        }

        runCatching {
            InterfaceMutations.populateAudioDevices()
        }.onFailure { error ->
            console.error("Error refreshing audio devices", error)
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

    private fun buildMediaStreamConstraints(deviceId: String?): MediaStreamConstraints {
        // IMPORTANTE: usar kotlin.js.json (objeto JS plano), NÃO um Map do
        // Kotlin. O browser lê audio.echoCancellation / audio.noiseSuppression /
        // audio.autoGainControl como propriedades próprias do objeto; um Map do
        // Kotlin não as expõe e o Chrome cai nos DEFAULTS (AGC/NS/eco ON),
        // o que processa o áudio e corta frequências (som "abafado/anti-ruído").
        val audio: dynamic =
            json(
                "echoCancellation" to false,
                "noiseSuppression" to false,
                "autoGainControl" to false,
                "sampleRate" to 48000,
                "sampleSize" to 16,
                "channelCount" to 2,
                "latency" to 0,
            )

        if (!deviceId.isNullOrBlank()) {
            audio["deviceId"] = json("exact" to deviceId)
        }

        return MediaStreamConstraints(audio = audio)
    }
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
