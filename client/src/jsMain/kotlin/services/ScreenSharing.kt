package services

import decorators.getDisplayMedia
import platform.DesktopCaptureBridge
import platform.nativeSystemAudioSupported
import kotlin.js.json
import kotlinx.browser.window
import kotlinx.coroutines.await
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import ui.InterfaceMutations

class ScreenSharing(
    private val resolveUsername: (String) -> String,
    private val localUsername: String,
    private val coroutineScope: CoroutineScope,
) {
    var localScreenStream: MediaStream? = null
    val remoteScreenStreams: MutableMap<String, MediaStream> = mutableMapOf()

    var lastShareWidth: Int = 1920
    var lastShareHeight: Int = 1080
    var lastShareFrameRate: Int = 30
    private var hasNativeSystemAudio = false

    enum class DesktopAudioMode { NONE, SYSTEM, PROCESS }

    suspend fun setupLocalScreenStream(
        width: Int,
        height: Int,
        frameRate: Int,
        useSourceResolution: Boolean,
        desktopAudioMode: DesktopAudioMode = DesktopAudioMode.NONE,
        audioSourceProcessId: Int? = null,
        onStreamEnd: () -> Unit,
        recreatePeerConnections: () -> Unit,
    ) {
        lastShareWidth = width
        lastShareHeight = height
        lastShareFrameRate = frameRate

        val displayStream =
            window.navigator.mediaDevices
                .getDisplayMedia(buildMediaStreamConstraints(width, height, frameRate, useSourceResolution))
                .await()
        localScreenStream = displayStream

        // On desktop, the optional Rust path replaces (never duplicates) the
        // WebView display-audio track. Browser behavior stays exactly unchanged.
        // The WebView picker remains the consent boundary. Its optional audio
        // checkbox does not define the native route: the explicit local mode
        // does, but only after getDisplayMedia has returned successfully.
        if (desktopAudioMode != DesktopAudioMode.NONE && nativeSystemAudioSupported(
                isDesktop = DesktopCaptureBridge.isAvailable,
                bridgeAvailable = DesktopCaptureBridge.isAvailable,
                generatorAvailable = jsTypeOf(window.asDynamic().MediaStreamTrackGenerator) != "undefined",
            )
        ) {
            runCatching {
                val nativeTrack = when (desktopAudioMode) {
                    DesktopAudioMode.SYSTEM -> DesktopCaptureBridge.startSystemAudio()
                    DesktopAudioMode.PROCESS -> DesktopCaptureBridge.startProcessAudio(requireNotNull(audioSourceProcessId))
                    DesktopAudioMode.NONE -> error("unreachable")
                }
                // The Windows picker is authoritative only for video.  Never
                // publish its optional audio alongside, or instead of, the
                // deliberate local audio choice.
                displayStream.getAudioTracks().forEach { it.stop() }
                localScreenStream = createMediaStream(displayStream.getVideoTracks().first(), nativeTrack)
                hasNativeSystemAudio = true
            }.onFailure {
                // Native failures are recoverable, but must not silently fall
                // back to an unrelated audio track selected in the OS picker.
                displayStream.getAudioTracks().forEach { it.stop() }
                localScreenStream = createMediaStream(displayStream.getVideoTracks().first())
                console.warn("O áudio local não pôde ser iniciado; o vídeo continua ativo")
            }
        }
        val publishedStream = localScreenStream
        val videoTrack = publishedStream?.getVideoTracks()?.firstOrNull()
        if (videoTrack != null) {
            videoTrack.onended = {
                // A stopped/replaced stream can report `ended` after the next
                // share has already started. It must not tear down that share.
                if (localScreenStream === publishedStream) {
                    stopNativeSystemAudio()
                    localScreenStream = null
                    onStreamEnd()
                }
            }

            recreatePeerConnections()
        }
        InterfaceMutations.addOrUpdateScreenTile(
            tileId = LOCAL_TILE_ID,
            stream = localScreenStream!!,
            username = localUsername,
            isLocal = true,
        )
        InterfaceMutations.updateShareControls(isLocalSharing = true)
    }

    fun handleRemoteScreen(
        socketId: String,
        stream: MediaStream,
    ) {
        val previous = remoteScreenStreams[socketId]
        if (previous != null && previous.id != stream.id) {
            previous.getTracks().forEach { it.stop() }
        }
        remoteScreenStreams[socketId] = stream
        InterfaceMutations.addOrUpdateScreenTile(
            tileId = socketId,
            stream = stream,
            username = resolveUsername(socketId),
            isLocal = false,
        )
    }

    fun stopRemoteScreen(socketId: String) {
        remoteScreenStreams.remove(socketId)?.getTracks()?.forEach { it.stop() }
        InterfaceMutations.removeScreenTile(socketId)
    }

    fun stopScreenSharing(recreatePeerConnections: () -> Unit) {
        val stream = localScreenStream
        // Detach the browser callback before stopping tracks. This makes an
        // explicit stop and an audio-source replacement single-shot.
        stream?.getVideoTracks()?.forEach { it.onended = null }
        localScreenStream = null
        stream?.getTracks()?.forEach { track -> track.stop() }
        stopNativeSystemAudio()
        recreatePeerConnections()
        InterfaceMutations.removeScreenTile(LOCAL_TILE_ID)
        InterfaceMutations.updateShareControls(isLocalSharing = false)
    }

    private fun stopNativeSystemAudio() {
        if (!hasNativeSystemAudio) return
        hasNativeSystemAudio = false
        // The bridge operation is idempotent and intentionally detached from the
        // browser track's synchronous `onended` callback.
        coroutineScope.launch { runCatching { DesktopCaptureBridge.stopAudio() } }
    }

    private fun createMediaStream(videoTrack: org.w3c.dom.mediacapture.MediaStreamTrack): MediaStream =
        js("new MediaStream([videoTrack])")

    private fun createMediaStream(videoTrack: org.w3c.dom.mediacapture.MediaStreamTrack, audioTrack: org.w3c.dom.mediacapture.MediaStreamTrack): MediaStream =
        js("new MediaStream([videoTrack, audioTrack])")

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

    companion object {
        const val LOCAL_TILE_ID = "local"
    }
}
