package services

import decorators.getDisplayMedia
import hideAfterLocalShare
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import org.w3c.dom.mediacapture.MediaStreamTrack
import platform.DesktopCaptureBridge
import platform.DesktopCaptureException
import platform.DesktopCaptureSource
import platform.DesktopCaptureSourceKind
import platform.DesktopEnvironment
import platform.nativeSystemAudioSupported
import ui.InterfaceMutations
import kotlin.js.json

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
    private var nativeAudioTrack: MediaStreamTrack? = null
    private var nativeVideoTrack: MediaStreamTrack? = null

    enum class DesktopAudioMode { NONE, SYSTEM, PROCESS }

    /**
     * [nativeSource] (Windows and macOS desktop) captures that screen or window natively,
     * without the WebView picker, sharing bar or border. Otherwise the standard
     * getDisplayMedia picker chooses the source.
     */
    suspend fun setupLocalScreenStream(
        isRequestCurrent: () -> Boolean,
        width: Int,
        height: Int,
        frameRate: Int,
        useSourceResolution: Boolean,
        desktopAudioMode: DesktopAudioMode = DesktopAudioMode.NONE,
        nativeSource: DesktopCaptureSource? = null,
        onStreamEnd: () -> Unit,
        recreatePeerConnections: () -> Unit,
    ) {
        lastShareWidth = width
        lastShareHeight = height
        lastShareFrameRate = frameRate

        val displayStream =
            if (nativeSource != null) {
                // The wizard already confirmed this source, so the old share ends first.
                if (localScreenStream != null) stopScreenSharing(recreatePeerConnections = {})
                var track: MediaStreamTrack? = null
                var endedEarly: String? = null
                val started =
                    DesktopCaptureBridge.startVideoCapture(nativeSource, frameRate) { error ->
                        if (track == null) {
                            endedEarly = error
                            return@startVideoCapture
                        }
                        // A replaced or stopped capture must not end the current share.
                        if (nativeVideoTrack !== track) return@startVideoCapture
                        if (error == "window-closed") {
                            InterfaceMutations.showError(
                                "A janela compartilhada foi fechada. O compartilhamento terminou.",
                            )
                        }
                        onStreamEnd()
                    }
                endedEarly?.let { throw DesktopCaptureException(it, "A captura terminou antes de começar") }
                track = started
                nativeVideoTrack = started
                createMediaStream(started)
            } else {
                window.navigator.mediaDevices
                    .getDisplayMedia(buildMediaStreamConstraints(width, height, frameRate, useSourceResolution))
                    .await()
            }
        if (!isRequestCurrent()) {
            displayStream.getTracks().forEach { it.stop() }
            stopNativeCapture()
            throw kotlinx.coroutines.CancellationException("Capture request invalidated")
        }
        if (localScreenStream != null) stopScreenSharing(recreatePeerConnections = {})
        localScreenStream = displayStream

        // On desktop, the optional Rust path replaces (never duplicates) the
        // WebView display-audio track. Browser behavior stays exactly unchanged.
        // The WebView picker (or the native wizard) remains the consent boundary.
        // The picker's optional audio checkbox does not define the native route:
        // the explicit local mode does, once the video source is running.
        if (desktopAudioMode != DesktopAudioMode.NONE &&
            nativeSystemAudioSupported(
                isDesktop = DesktopCaptureBridge.isAvailable,
                bridgeAvailable = DesktopCaptureBridge.isAvailable,
                generatorAvailable = window.asDynamic().__screenshareDesktopBridge?.nativeAudio == true,
            )
        ) {
            runCatching {
                val nativeTrack =
                    when (desktopAudioMode) {
                        DesktopAudioMode.SYSTEM -> {
                            DesktopCaptureBridge.startSystemAudio()
                        }

                        DesktopAudioMode.PROCESS -> {
                            DesktopCaptureBridge.startProcessAudio(
                                requireNotNull(nativeSource).processId,
                            )
                        }

                        DesktopAudioMode.NONE -> {
                            error("unreachable")
                        }
                    }
                // The Windows picker is authoritative only for video.  Never
                // publish its optional audio alongside, or instead of, the
                // deliberate local audio choice.
                displayStream.getAudioTracks().forEach { it.stop() }
                localScreenStream = createMediaStream(displayStream.getVideoTracks().first(), nativeTrack)
                nativeAudioTrack = nativeTrack
            }.onFailure {
                // Native failures are recoverable, but must not silently fall
                // back to an unrelated audio track selected in the OS picker.
                displayStream.getAudioTracks().forEach { it.stop() }
                localScreenStream = createMediaStream(displayStream.getVideoTracks().first())
                console.warn("O áudio local não pôde ser iniciado; o vídeo continua ativo")
                hideAfterLocalShare = false
                runCatching { DesktopEnvironment.showMainWindow() }
                InterfaceMutations.showError(
                    "O compartilhamento continua sem áudio: não foi possível iniciar o áudio escolhido. " +
                        "Para tentar de novo, pare e compartilhe outra vez.",
                )
            }
        }
        if (!isRequestCurrent()) {
            stopScreenSharing(recreatePeerConnections = {})
            throw kotlinx.coroutines.CancellationException("Capture request invalidated")
        }
        val publishedStream = localScreenStream
        val videoTrack = publishedStream?.getVideoTracks()?.firstOrNull()
        if (videoTrack != null) {
            videoTrack.onended = {
                // A stopped/replaced stream can report `ended` after the next
                // share has already started. It must not tear down that share.
                if (localScreenStream === publishedStream) {
                    stopNativeCapture()
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
        stopNativeCapture()
        recreatePeerConnections()
        InterfaceMutations.removeScreenTile(LOCAL_TILE_ID)
        InterfaceMutations.updateShareControls(isLocalSharing = false)
    }

    private fun stopNativeCapture() {
        val audio = nativeAudioTrack
        val video = nativeVideoTrack
        nativeAudioTrack = null
        nativeVideoTrack = null
        if (audio == null && video == null) return
        // The bridge stops only the captures that produced these tracks, so this
        // detached call cannot end a share that has already replaced them.
        coroutineScope.launch {
            audio?.let { runCatching { DesktopCaptureBridge.stopAudio(it) } }
            video?.let { runCatching { DesktopCaptureBridge.stopVideoCapture(it) } }
        }
    }

    private fun createMediaStream(videoTrack: org.w3c.dom.mediacapture.MediaStreamTrack): MediaStream =
        js("new MediaStream([videoTrack])")

    private fun createMediaStream(
        videoTrack: org.w3c.dom.mediacapture.MediaStreamTrack,
        audioTrack: org.w3c.dom.mediacapture.MediaStreamTrack,
    ): MediaStream = js("new MediaStream([videoTrack, audioTrack])")

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

/** A window sends only its own app's sound; a screen sends the whole default output. */
fun nativeAudioMode(
    withAudio: Boolean,
    kind: DesktopCaptureSourceKind,
): ScreenSharing.DesktopAudioMode =
    when {
        !withAudio -> ScreenSharing.DesktopAudioMode.NONE
        kind == DesktopCaptureSourceKind.WINDOW -> ScreenSharing.DesktopAudioMode.PROCESS
        else -> ScreenSharing.DesktopAudioMode.SYSTEM
    }
