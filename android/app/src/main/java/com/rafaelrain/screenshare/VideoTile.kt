package com.rafaelrain.screenshare

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import com.rafaelrain.screenshare.engine.Engine
import livekit.org.webrtc.EglBase
import livekit.org.webrtc.JavaI420Buffer
import livekit.org.webrtc.RendererCommon
import livekit.org.webrtc.SurfaceViewRenderer
import livekit.org.webrtc.VideoFrame
import java.nio.ByteBuffer

/** One GL context for every tile, as WebRTC's own apps share it. */
val eglBase: EglBase by lazy { EglBase.create() }

/**
 * A remote (or our own) video: each display frame takes the engine's newest I420 frame for this
 * tile (render.rs keeps it) and hands it to libwebrtc's GL renderer. A tile off screen asks nothing.
 */
@Composable
fun VideoTile(peer: String, camera: Boolean, fit: Boolean, modifier: Modifier = Modifier) {
    val renderer = remember { arrayOfNulls<SurfaceViewRenderer>(1) }
    AndroidView(
        modifier = modifier,
        factory = { context ->
            SurfaceViewRenderer(context).apply {
                init(eglBase.eglBaseContext, null)
                setEnableHardwareScaler(true)
                renderer[0] = this
            }
        },
        update = { it.setScalingType(if (fit) RendererCommon.ScalingType.SCALE_ASPECT_FIT else RendererCommon.ScalingType.SCALE_ASPECT_FILL) },
    )
    DisposableEffect(peer, camera) {
        onDispose { renderer[0]?.release() }
    }
    LaunchedEffect(peer, camera) {
        var probe = ByteBuffer.allocateDirect(1)
        while (true) {
            withFrameNanos { now ->
                val view = renderer[0] ?: return@withFrameNanos
                val size = Engine.takeFrame(peer, camera, probe)
                when {
                    size < 0 -> probe = ByteBuffer.allocateDirect((-size).toInt())
                    size > 0 -> {
                        val width = (size ushr 32).toInt()
                        val height = (size and 0xffffffffL).toInt()
                        val cw = (width + 1) / 2
                        val ch = (height + 1) / 2
                        // shortcut: the renderer keeps the frame until it draws, so each frame gets its own buffer;
                        // pool two if the garbage collector shows up in profiles.
                        val frame = probe
                        probe = ByteBuffer.allocateDirect(frame.capacity())
                        fun plane(offset: Int, length: Int): ByteBuffer =
                            frame.duplicate().apply { position(offset); limit(offset + length) }.slice()
                        val buffer = JavaI420Buffer.wrap(
                            width, height,
                            plane(0, width * height), width,
                            plane(width * height, cw * ch), cw,
                            plane(width * height + cw * ch, cw * ch), cw,
                            null,
                        )
                        val video = VideoFrame(buffer, 0, now)
                        view.onFrame(video)
                        video.release()
                    }
                }
            }
        }
    }
}
