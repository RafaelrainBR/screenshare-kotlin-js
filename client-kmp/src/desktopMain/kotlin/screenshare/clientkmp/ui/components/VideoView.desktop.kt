package screenshare.clientkmp.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asComposeImageBitmap
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import com.shepeliev.webrtckmp.VideoStreamTrack
import dev.onvoid.webrtc.media.FourCC
import dev.onvoid.webrtc.media.video.VideoBufferConverter
import dev.onvoid.webrtc.media.video.VideoFrame
import dev.onvoid.webrtc.media.video.VideoTrackSink
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.jetbrains.skia.Bitmap
import org.jetbrains.skia.ColorAlphaType
import org.jetbrains.skia.ColorType
import org.jetbrains.skia.ImageInfo
import java.nio.ByteBuffer

@Composable
actual fun VideoView(
    track: Any?,
    modifier: Modifier,
) {
    val videoTrack = track as? VideoStreamTrack
    if (videoTrack == null) return

    val renderer = remember(videoTrack) { VideoRenderer(track = videoTrack) }

    DisposableEffect(videoTrack) {
        renderer.start()
        onDispose { renderer.stop() }
    }

    val image = renderer.image.collectAsState()

    BoxWithConstraints(modifier = modifier) {
        val constraints = this.constraints
        Canvas(modifier = Modifier.fillMaxSize()) {
            val containerW = constraints.maxWidth
            val containerH = constraints.maxHeight
            drawRect(
                color = Color.Black,
                size = Size(containerW.toFloat(), containerH.toFloat()),
            )
            image.value?.let { bmp ->
                val srcW = bmp.width.takeIf { it > 0 } ?: containerW
                val srcH = bmp.height.takeIf { it > 0 } ?: containerH
                val srcAspect = srcW.toFloat() / srcH
                val containerAspect = containerW.toFloat() / containerH
                val (dstW, dstH) = if (srcAspect > containerAspect) {
                    // wider than container — letterbox (bars top/bottom)
                    Pair(containerW, (containerW / srcAspect).toInt())
                } else {
                    // taller than container — pillarbox (bars left/right)
                    Pair((containerH * srcAspect).toInt(), containerH)
                }
                val offsetX = (containerW - dstW) / 2
                val offsetY = (containerH - dstH) / 2
                drawImage(
                    image = bmp.asComposeImageBitmap(),
                    dstOffset = IntOffset(offsetX, offsetY),
                    dstSize = IntSize(dstW, dstH),
                )
            }
        }
    }
}

private class VideoRenderer(
    private val track: VideoStreamTrack,
) : VideoTrackSink {

    private val _image = MutableStateFlow<Bitmap?>(null)
    val image = _image.asStateFlow()
    private var byteBuffer: ByteBuffer? = null
    private var frameBuffer: Bitmap? = null

    fun start() {
        track.addSink(this)
        track.enabled = true
    }

    fun stop() {
        track.removeSink(this)
        _image.value = null
        byteBuffer = null
        frameBuffer = null
    }

    override fun onVideoFrame(frame: VideoFrame) {
        try {
            frame.retain()
            val buffer = frame.buffer
            val width = buffer.width
            val height = buffer.height

            if (frameBuffer == null || frameBuffer?.width != width || frameBuffer?.height != height) {
                byteBuffer = ByteBuffer.allocate(width * height * 4)
                frameBuffer =
                    Bitmap().apply {
                        allocPixels(
                            ImageInfo(
                                width = width,
                                height = height,
                                colorType = ColorType.RGBA_8888,
                                alphaType = ColorAlphaType.OPAQUE,
                            ),
                        )
                    }
            }

            byteBuffer?.let {
                VideoBufferConverter.convertFromI420(buffer, it, FourCC.ABGR)
                frameBuffer?.installPixels(it.array())
                _image.value = frameBuffer?.makeClone()
            }
        } catch (ex: Exception) {
            println("[VideoRenderer] frame error: ${ex.message}")
        } finally {
            frame.release()
        }
    }
}
