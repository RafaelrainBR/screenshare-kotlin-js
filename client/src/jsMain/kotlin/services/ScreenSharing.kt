package services

import decorators.getDisplayMedia
import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import ui.InterfaceMutations

class ScreenSharing {
    var localScreenStream: MediaStream? = null
    val remoteScreenStreams: MutableMap<String, MediaStream> = mutableMapOf()

    suspend fun setupLocalScreenStream(
        onStreamEnd: () -> Unit,
        recreatePeerConnections: () -> Unit,
    ) {
        localScreenStream =
            window.navigator.mediaDevices
                .getDisplayMedia(buildMediaStreamConstraints())
                .await()
        val videoTrack = localScreenStream?.getVideoTracks()?.firstOrNull()
        if (videoTrack != null) {
            videoTrack.onended = {
                localScreenStream = null
                onStreamEnd()
            }

            recreatePeerConnections()
        }
        InterfaceMutations.updateScreenContainer(localScreenStream!!, isInitiator = true)
    }

    fun handleRemoteScreen(
        socketId: String,
        stream: MediaStream,
    ) {
        remoteScreenStreams[socketId] = stream
        InterfaceMutations.updateScreenContainer(stream, isInitiator = false)
    }

    fun stopScreenSharing(recreatePeerConnections: () -> Unit) {
        localScreenStream?.getTracks()?.forEach { track -> track.stop() }
        localScreenStream = null
        recreatePeerConnections()
        InterfaceMutations.endScreenSharing()
    }

    private fun buildMediaStreamConstraints(): MediaStreamConstraints =
        MediaStreamConstraints(
            video =
                mapOf(
                    "cursor" to "always",
                    "frameRate" to
                        mapOf(
                            "ideal" to 30,
                            "max" to 60,
                        ),
                    "width" to mapOf("ideal" to 1920),
                    "height" to mapOf("ideal" to 1080),
                    "resizeMode" to "crop-and-scale",
                ),
            audio =
                mapOf(
                    "sampleSize" to 32,
                    "sampleRate" to 48000,
                    "echoCancellation" to false,
                    "noiseSuppression" to false,
                    "autoGainControl" to false,
                    "sampleRate" to 48000,
                    "channelCount" to 2,
                    "latency" to 0,
                ),
        )
}
