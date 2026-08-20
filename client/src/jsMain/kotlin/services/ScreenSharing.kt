package services

import decorators.getDisplayMedia
import kotlin.js.json
import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import ui.InterfaceMutations

class ScreenSharing {
    var localScreenStream: MediaStream? = null
    val remoteScreenStreams: MutableMap<String, MediaStream> = mutableMapOf()

    var lastShareWidth: Int = 1920
    var lastShareHeight: Int = 1080
    var lastShareFrameRate: Int = 30

    suspend fun setupLocalScreenStream(
        width: Int,
        height: Int,
        frameRate: Int,
        useSourceResolution: Boolean,
        onStreamEnd: () -> Unit,
        recreatePeerConnections: () -> Unit,
    ) {
        lastShareWidth = width
        lastShareHeight = height
        lastShareFrameRate = frameRate

        localScreenStream =
            window.navigator.mediaDevices
                .getDisplayMedia(buildMediaStreamConstraints(width, height, frameRate, useSourceResolution))
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

    private fun buildMediaStreamConstraints(
        width: Int,
        height: Int,
        frameRate: Int,
        useSourceResolution: Boolean,
    ): MediaStreamConstraints {
        // IMPORTANTE: kotlin.js.json (objeto JS plano), NÃO Map do Kotlin. Um
        // Map não expõe as propriedades que o browser lê (audio.echoCancellation,
        // width.ideal, etc.), então o Chrome ignora as constraints e aplica
        // DEFAULTS — AGC/NS/eco ON no áudio (som "abafado/anti-ruído") e
        // resolução/fps incorretos no vídeo.
        val video: dynamic =
            if (!useSourceResolution) {
                json(
                    "cursor" to "always",
                    "frameRate" to json("ideal" to frameRate, "max" to frameRate),
                    "width" to json("ideal" to width),
                    "height" to json("ideal" to height),
                    "resizeMode" to "crop-and-scale",
                )
            } else {
                json(
                    "cursor" to "always",
                    "frameRate" to json("ideal" to frameRate, "max" to frameRate),
                )
            }

        val audio: dynamic =
            json(
                "echoCancellation" to false,
                "noiseSuppression" to false,
                "autoGainControl" to false,
                "channelCount" to 2,
                "sampleRate" to 48000,
                "sampleSize" to 16,
            )

        return MediaStreamConstraints(
            video = video,
            audio = audio,
        )
    }
}
