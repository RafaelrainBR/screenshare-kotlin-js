package com.rafaelrain.screenshare

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioRecord
import android.media.ImageReader
import android.media.projection.MediaProjection
import android.os.Handler
import android.os.HandlerThread
import com.rafaelrain.screenshare.engine.Engine
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** The share's size: the shorter side at [shortSide] (720 or 1080), the display's proportions, even sides. */
fun captureSize(displayWidth: Int, displayHeight: Int, shortSide: Int): Pair<Int, Int> {
    val scale = min(1.0, shortSide.toDouble() / min(displayWidth, displayHeight))
    fun even(value: Double) = max(2, (value.roundToInt() / 2) * 2)
    return even(displayWidth * scale) to even(displayHeight * scale)
}

/**
 * MediaProjection → VirtualDisplay → ImageReader (RGBA) → the engine, and with audio on,
 * AudioRecord's playback capture → the share's own `screen-audio` track. The capture outlives a
 * reconnection: the engine's new session takes the frames once the share is started again.
 */
class ScreenCapture(
    private val context: Context,
    private val projection: MediaProjection,
    private val shortSide: Int,
    private val withAudio: Boolean,
    private val onStop: () -> Unit,
) {
    private val thread = HandlerThread("screen-capture").apply { start() }
    private val handler = Handler(thread.looper)
    private var display: VirtualDisplay? = null
    private var reader: ImageReader? = null
    private var audio: AudioRecord? = null
    private var audioThread: Thread? = null

    @Volatile private var running = true

    var size = 0 to 0
        private set

    /** False when the app the person shares does not allow capturing its audio (it reads silence). */
    @Volatile var audioSilent = false
        private set

    fun start() {
        projection.registerCallback(object : MediaProjection.Callback() {
            override fun onStop() {
                if (running) onStop()
            }

            override fun onCapturedContentResize(width: Int, height: Int) {
                handler.post { resize(width, height) }
            }
        }, handler)
        val metrics = context.resources.displayMetrics
        resize(metrics.widthPixels, metrics.heightPixels, create = true)
        if (withAudio) startAudio()
    }

    /** A rotation or the shared app's new size: a new ImageReader at the new proportions. */
    fun resize(width: Int, height: Int, create: Boolean = false) {
        if (!running || width <= 0 || height <= 0) return
        val (w, h) = captureSize(width, height, shortSide)
        if (!create && (w to h) == size) return
        size = w to h
        val old = reader
        val next = ImageReader.newInstance(w, h, PixelFormat.RGBA_8888, 3)
        next.setOnImageAvailableListener({ r -> deliver(r) }, handler)
        reader = next
        val dpi = context.resources.displayMetrics.densityDpi
        val current = display
        if (current == null) {
            display = projection.createVirtualDisplay("screenshare", w, h, dpi, DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, next.surface, null, handler)
        } else {
            current.resize(w, h, dpi)
            current.surface = next.surface
        }
        old?.close()
    }

    private fun deliver(reader: ImageReader) {
        val image = try {
            reader.acquireLatestImage()
        } catch (_: IllegalStateException) {
            null
        } ?: return
        image.use {
            val plane = it.planes[0]
            Engine.pushScreenFrame(plane.buffer, it.width, it.height, plane.rowStride, it.timestamp)
        }
    }

    @SuppressLint("MissingPermission") // RECORD_AUDIO is asked before the share sheet offers audio.
    private fun startAudio() {
        val config = AudioPlaybackCaptureConfiguration.Builder(projection)
            .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
            .addMatchingUsage(AudioAttributes.USAGE_GAME)
            .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN)
            .build()
        val format = AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
            .setSampleRate(48_000)
            .setChannelMask(AudioFormat.CHANNEL_IN_STEREO)
            .build()
        val record = try {
            AudioRecord.Builder()
                .setAudioPlaybackCaptureConfig(config)
                .setAudioFormat(format)
                .setBufferSizeInBytes(max(AudioRecord.getMinBufferSize(48_000, AudioFormat.CHANNEL_IN_STEREO, AudioFormat.ENCODING_PCM_16BIT), 1920 * 8))
                .build()
        } catch (_: Exception) {
            null
        } ?: return
        audio = record
        record.startRecording()
        audioThread = Thread({
            // 10 ms of 48 kHz stereo 16-bit, what the engine's NativeAudioSource takes.
            val chunk = ByteBuffer.allocateDirect(1920).order(ByteOrder.nativeOrder())
            var silentChunks = 0
            while (running) {
                chunk.clear()
                var read = 0
                while (read < 1920 && running) {
                    val n = record.read(chunk, 1920 - read)
                    if (n <= 0) break
                    read += n
                    chunk.position(read)
                }
                if (read < 1920) continue
                chunk.position(0)
                val loud = (0 until 960).any { kotlin.math.abs(chunk.getShort(it * 2).toInt()) > 2 }
                silentChunks = if (loud) 0 else silentChunks + 1
                // 10 s of digital silence: the app playing blocks capture (allowAudioPlaybackCapture) or plays nothing.
                audioSilent = silentChunks > 1000
                Engine.pushScreenAudio(chunk)
            }
        }, "screen-audio").apply { start() }
    }

    fun stop() {
        running = false
        audioThread?.join(500)
        audio?.run {
            try {
                stop()
            } catch (_: IllegalStateException) {
            }
            release()
        }
        display?.release()
        reader?.close()
        projection.stop()
        thread.quitSafely()
    }
}
