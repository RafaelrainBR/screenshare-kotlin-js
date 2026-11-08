package com.rafaelrain.screenshare.engine

import android.content.Context
import java.nio.ByteBuffer

/** The Rust engine (android/engine): the desktop's session and WebRTC, driven by JSON. */
object Engine {
    init {
        System.loadLibrary("screenshare_android")
    }

    /** libwebrtc's Android context (the audio device module needs it) and the Rust side's Sentry. */
    @JvmStatic external fun init(context: Context, dsn: String, release: String): Boolean

    /** The error text, or null once the session started. */
    @JvmStatic external fun start(endpoint: String, room: String, name: String): String?

    @JvmStatic external fun stop()

    /** A request such as {"type":"microphone","enabled":true}; the error text, or null when queued. */
    @JvmStatic external fun send(json: String): String?

    /** The room state as JSON when it changed since the last call, else null. */
    @JvmStatic external fun view(): String?

    /** An RGBA_8888 frame from the ImageReader; false when no share is running. */
    @JvmStatic external fun pushScreenFrame(buffer: ByteBuffer, width: Int, height: Int, rowStride: Int, timestampNs: Long): Boolean

    /** 10 ms of 48 kHz stereo 16-bit PCM (1920 bytes) in a direct buffer. */
    @JvmStatic external fun pushScreenAudio(buffer: ByteBuffer): Boolean

    @JvmStatic external fun pushCameraFrame(
        y: ByteBuffer,
        u: ByteBuffer,
        v: ByteBuffer,
        yStride: Int,
        uvStride: Int,
        uvPixelStride: Int,
        width: Int,
        height: Int,
        rotation: Int,
        timestampNs: Long,
    ): Boolean

    /** width shl 32 or height of a newer frame copied as packed I420; 0 when none; -(bytes needed) when [buffer] is too small. */
    @JvmStatic external fun takeFrame(peer: String, camera: Boolean, buffer: ByteBuffer): Long
}
