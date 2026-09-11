package platform

import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.mediacapture.MediaStreamTrack
import kotlin.js.Promise

class DesktopCaptureException(
    val code: String,
    message: String,
) : Exception(message)

data class DesktopCaptureSource(
    val id: String,
    val title: String,
    val kind: DesktopCaptureSourceKind,
    /** Native-only routing metadata; never leaves this WebView. */
    val processId: Int,
    val hwnd: Long,
)

enum class DesktopCaptureSourceKind { SCREEN, WINDOW }

data class DesktopCaptureThumbnail(
    val width: Int,
    val height: Int,
    val bgra: IntArray,
)

/**
 * The only Kotlin/JS boundary for optional native capture. The global is installed
 * solely by the Tauri asset; browser bundles contain no Tauri module import.
 */
object DesktopCaptureBridge {
    val isAvailable: Boolean
        get() =
            DesktopEnvironment.isAvailable &&
                jsTypeOf(window.asDynamic().__screenshareDesktopBridge) != "undefined"

    suspend fun startSystemAudio(): MediaStreamTrack {
        if (!isAvailable) throw DesktopCaptureException("native-unavailable", "Captura nativa não disponível")
        return try {
            (window.asDynamic().__screenshareDesktopBridge.startSystemAudio() as Promise<MediaStreamTrack>).await()
        } catch (error: dynamic) {
            throw DesktopCaptureException(
                code = error.code as? String ?: "capture-failed",
                message = error.message as? String ?: "Não foi possível iniciar a captura de áudio do sistema",
            )
        }
    }

    suspend fun stopSystemAudio() {
        stopAudio()
    }

    suspend fun startProcessAudio(
        processId: Int,
        includeProcessTree: Boolean = true,
    ): MediaStreamTrack {
        if (!isAvailable) throw DesktopCaptureException("native-unavailable", "Captura nativa não disponível")
        return try {
            (window.asDynamic().__screenshareDesktopBridge.startProcessAudio(processId, includeProcessTree) as Promise<MediaStreamTrack>)
                .await()
        } catch (error: dynamic) {
            throw DesktopCaptureException(
                error.code as? String ?: "capture-failed",
                error.message as? String ?: "Não foi possível iniciar a captura do aplicativo",
            )
        }
    }

    suspend fun enumerateSources(): List<DesktopCaptureSource> {
        if (!isAvailable) return emptyList()
        return try {
            val sources =
                (window.asDynamic().__screenshareDesktopBridge.enumerateSources() as Promise<Array<dynamic>>).await()
            sources.map { source ->
                DesktopCaptureSource(
                    source.id as String,
                    source.title as String,
                    if ((source.kind as String) == "screen") DesktopCaptureSourceKind.SCREEN else DesktopCaptureSourceKind.WINDOW,
                    source.pid as Int,
                    (source.hwnd as Number).toLong(),
                )
            }
        } catch (error: dynamic) {
            throw DesktopCaptureException(
                error.code as? String ?: "sources-failed",
                error.message as? String ?: "Não foi possível listar aplicativos",
            )
        }
    }

    suspend fun captureThumbnail(hwnd: Long): DesktopCaptureThumbnail? {
        if (!isAvailable) return null
        return try {
            val thumbnail =
                (window.asDynamic().__screenshareDesktopBridge.captureThumbnail(hwnd) as Promise<dynamic>).await()
                    ?: return null
            val data = thumbnail.data
            DesktopCaptureThumbnail(
                width = thumbnail.width as Int,
                height = thumbnail.height as Int,
                // Keep native bytes as unsigned 0..255 values. Kotlin/JS ByteArray
                // is backed by Int8Array and corrupts values above 127 when it is
                // later converted into Uint8ClampedArray for Canvas ImageData.
                bgra = IntArray(data.length as Int) { index -> (data[index] as Number).toInt() },
            )
        } catch (_: dynamic) {
            null
        }
    }

    suspend fun stopAudio() {
        if (!isAvailable) return
        (window.asDynamic().__screenshareDesktopBridge.stopAudio() as Promise<Unit>).await()
    }

    /** Best-effort release for WebView/page shutdown; the page cannot await it. */
    fun stopAudioOnPageExit() {
        if (!isAvailable) return
        window.asDynamic().__screenshareDesktopBridge.stopAudio()
    }
}

fun nativeSystemAudioSupported(
    isDesktop: Boolean,
    bridgeAvailable: Boolean,
    generatorAvailable: Boolean,
): Boolean = isDesktop && bridgeAvailable && generatorAvailable
