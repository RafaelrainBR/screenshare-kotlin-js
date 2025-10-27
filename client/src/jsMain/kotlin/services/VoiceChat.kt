package services

import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import ui.InterfaceMutations

class VoiceChat {
    var isMicMuted = true
    var isAudioMuted = true

    var localMicStream: MediaStream? = null
    val remoteAudioStreams: MutableMap<String, MediaStream> = mutableMapOf()

    fun handleRemoteAudio(socketId: String, stream: MediaStream) {
        remoteAudioStreams[socketId] = stream

        InterfaceMutations.addAudioElementForUser(userId = socketId, stream = stream)
    }

    suspend fun setupLocalMic(
        recreatePeerConnections: suspend () -> Unit
    ) {
        localMicStream = window.navigator.mediaDevices.getUserMedia(MediaStreamConstraints(audio = true)).await()
        val audioTrack = localMicStream?.getAudioTracks()?.firstOrNull()
        if (audioTrack != null) {
            recreatePeerConnections()
        }
    }

    suspend fun toggleMute(
        broadcastMuted: suspend (isMuted: Boolean) -> Unit
    ) {
        isMicMuted = !isMicMuted
        localMicStream?.let { stream ->
            stream.getTracks().forEach { track -> track.enabled = !isMicMuted }
        }

        InterfaceMutations.updateAudioControls(isMicMuted = isMicMuted)
        broadcastMuted(isMicMuted)
    }
}
