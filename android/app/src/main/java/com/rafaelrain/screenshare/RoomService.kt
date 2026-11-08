package com.rafaelrain.screenshare

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.media.AudioManager
import android.media.ToneGenerator
import android.media.projection.MediaProjectionManager
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleService
import com.rafaelrain.screenshare.engine.Engine
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.json.JSONObject
import java.util.concurrent.Executors

/** What the share sheet picked: the web's presets (share.rs), the short side and the phone's audio. */
data class ShareChoice(val preset: String, val shortSide: Int, val audio: Boolean) {
    val fps get() = when (preset) { "game" -> 60; "text" -> 10; else -> 30 }
    val priority get() = if (preset == "game") "SMOOTHNESS" else "SHARPNESS"

    fun profile() = JSONObject().put("fps", fps).put("priority", priority)
}

/**
 * Owns the engine while the person is in the room: the Activity only draws. Runs in the foreground
 * with the types the room uses (media playback, microphone, camera, media projection).
 */
class RoomService : LifecycleService() {
    companion object {
        private val _view = MutableStateFlow(RoomView())
        val view: StateFlow<RoomView> = _view
        val room = MutableStateFlow<String?>(null)

        /** The Activity is on screen: poll the engine every 100 ms instead of every second. */
        @Volatile var visible = false

        @Volatile var instance: RoomService? = null
            private set

        const val CHANNEL = "room"
        private const val ID = 1
        private const val ACTION_MUTE = "mute"
        private const val ACTION_STOP_SHARE = "stopShare"
        private const val ACTION_LEAVE = "leave"

        fun join(context: Context, room: String, name: String) {
            val intent = Intent(context, RoomService::class.java).putExtra("room", room).putExtra("name", name)
            ContextCompat.startForegroundService(context, intent)
        }
    }

    private val main = Handler(Looper.getMainLooper())
    private lateinit var routes: AudioRoutes
    private var wake: PowerManager.WakeLock? = null
    private var wifi: WifiManager.WifiLock? = null
    private var capture: ScreenCapture? = null
    private var share: ShareChoice? = null
    private var cameraProvider: ProcessCameraProvider? = null
    private var cameraLens: String? = null
    private var microphoneWanted = false
    var speakerWanted = true
        private set
    private var restoredGeneration = -1L
    private var lastSound = 0L
    private val tones by lazy { ToneGenerator(AudioManager.STREAM_MUSIC, 40) }
    private var notified = ""

    override fun onCreate() {
        super.onCreate()
        instance = this
        routes = AudioRoutes(this)
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Na sala", NotificationManager.IMPORTANCE_LOW),
        )
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        when (intent?.action) {
            ACTION_MUTE -> setMicrophone(!_view.value.microphoneEnabled)
            ACTION_STOP_SHARE -> stopShare()
            ACTION_LEAVE -> leave()
            else -> {
                val code = intent?.getStringExtra("room") ?: return START_NOT_STICKY
                val name = intent.getStringExtra("name").orEmpty()
                promote()
                if (room.value != code) start(code, name)
            }
        }
        return START_NOT_STICKY
    }

    private fun start(code: String, name: String) {
        Engine.stop()
        Engine.start(BuildConfig.SERVER, code, name)?.let { error ->
            _view.value = RoomView(error = error)
            return
        }
        room.value = code
        restoredGeneration = -1
        wake = getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "screenshare:room").apply { acquire() }
        @Suppress("DEPRECATION")
        wifi = getSystemService(WifiManager::class.java).createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "screenshare:room").apply { acquire() }
        routes.apply(microphone = false, speakerWanted)
        main.post(poll)
    }

    private val poll: Runnable = object : Runnable {
        override fun run() {
            if (room.value == null) return
            Engine.view()?.let { json -> update(RoomView.parse(json)) }
            main.postDelayed(this, if (visible) 100 else 1000)
        }
    }

    private fun update(next: RoomView) {
        val before = _view.value
        _view.value = next
        if (next.joined && next.generation != restoredGeneration) {
            // A new connection (first join, server deploy, network change): output on, and what was on comes back on.
            restoredGeneration = next.generation
            send(JSONObject().put("type", "output").put("enabled", true))
            if (microphoneWanted) send(JSONObject().put("type", "microphone").put("enabled", true))
            if (capture != null) sendStartScreen()
            cameraLens?.let { send(JSONObject().put("type", "startCamera").put("device", it)) }
        }
        if (before.microphoneEnabled != next.microphoneEnabled) routes.apply(next.microphoneEnabled, speakerWanted)
        if (next.screenState == "ended" && before.screenState == "active" && capture != null && next.generation == before.generation) {
            stopCapture()
        }
        val (count, sound) = next.roomSound
        if (count != lastSound && sound != null && Settings.sounds(this)) {
            tones.startTone(if (sound == "Leave") ToneGenerator.TONE_PROP_NACK else ToneGenerator.TONE_PROP_ACK, 150)
        }
        lastSound = count
        refreshNotification()
    }

    fun send(json: JSONObject): String? = Engine.send(json.toString())

    fun setMicrophone(on: Boolean) {
        if (on && checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) return
        microphoneWanted = on
        if (on) routes.apply(true, speakerWanted)
        send(JSONObject().put("type", "microphone").put("enabled", on))
    }

    fun toggleSpeaker() {
        speakerWanted = !speakerWanted
        routes.apply(_view.value.microphoneEnabled, speakerWanted)
    }

    /** After the system consent; Android 14+ wants the mediaProjection service type before getMediaProjection. */
    fun startShare(resultCode: Int, data: Intent, choice: ShareChoice) {
        stopCapture()
        share = choice
        promote(projection = true)
        val projection = getSystemService(MediaProjectionManager::class.java).getMediaProjection(resultCode, data) ?: return
        capture = ScreenCapture(this, projection, choice.shortSide, choice.audio) {
            main.post {
                stopShare()
                _view.value = _view.value.copy(screenError = "O compartilhamento parou porque a tela foi bloqueada ou o Android encerrou a captura.")
            }
        }.also { it.start() }
        sendStartScreen()
    }

    private fun sendStartScreen() {
        val choice = share ?: return
        val (width, height) = capture?.size ?: return
        send(
            JSONObject().put("type", "startScreen").put("width", width).put("height", height)
                .put("profile", choice.profile()).put("shareHeight", minOf(width, height)).put("audio", choice.audio),
        )
    }

    fun setShareChoice(choice: ShareChoice) {
        share = choice
        if (capture != null) send(JSONObject().put("type", "shareProfile").put("profile", choice.profile()).put("shareHeight", choice.shortSide))
    }

    val shareAudioSilent get() = capture?.audioSilent == true

    fun stopShare() {
        send(JSONObject().put("type", "stopScreen"))
        stopCapture()
    }

    private fun stopCapture() {
        capture?.stop()
        capture = null
        promote()
    }

    fun startCamera(lens: String) {
        if (checkSelfPermission(android.Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) return
        cameraLens = lens
        promote()
        send(JSONObject().put("type", "startCamera").put("device", lens))
        val future = ProcessCameraProvider.getInstance(this)
        future.addListener({
            val provider = future.get()
            cameraProvider = provider
            provider.unbindAll()
            val analysis = ImageAnalysis.Builder()
                .setResolutionSelector(ResolutionSelector.Builder().setResolutionStrategy(ResolutionStrategy(android.util.Size(1280, 720), ResolutionStrategy.FALLBACK_RULE_CLOSEST_LOWER_THEN_HIGHER)).build())
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()
            analysis.setAnalyzer(Executors.newSingleThreadExecutor()) { image ->
                image.use {
                    val (y, u, v) = it.planes
                    Engine.pushCameraFrame(
                        y.buffer, u.buffer, v.buffer, y.rowStride, u.rowStride, u.pixelStride,
                        it.width, it.height, it.imageInfo.rotationDegrees, it.imageInfo.timestamp,
                    )
                }
            }
            val selector = if (lens == "back") CameraSelector.DEFAULT_BACK_CAMERA else CameraSelector.DEFAULT_FRONT_CAMERA
            provider.bindToLifecycle(this, selector, analysis)
        }, ContextCompat.getMainExecutor(this))
    }

    fun stopCamera() {
        cameraLens = null
        cameraProvider?.unbindAll()
        send(JSONObject().put("type", "stopCamera"))
        promote()
    }

    fun leave() {
        room.value = null
        main.removeCallbacks(poll)
        stopCapture()
        cameraProvider?.unbindAll()
        cameraLens = null
        microphoneWanted = false
        Engine.stop()
        routes.release()
        wake?.takeIf { it.isHeld }?.release()
        wifi?.takeIf { it.isHeld }?.release()
        _view.value = RoomView()
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        if (room.value != null) leave()
        instance = null
        super.onDestroy()
    }

    /** The service types follow what is in use; microphone from the start, so "Desmutar" works from the notification. */
    private fun promote(projection: Boolean = capture != null) {
        var types = ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
        if (checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
        if (cameraLens != null) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_CAMERA
        if (projection) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
        startForeground(ID, notification(), types)
    }

    private fun refreshNotification() {
        val text = notificationText() + _view.value.microphoneEnabled + (capture != null)
        if (text == notified || room.value == null) return
        notified = text
        getSystemService(NotificationManager::class.java).notify(ID, notification())
    }

    private fun notificationText(): String {
        val v = _view.value
        val people = v.people.size
        return when {
            !v.joined -> "Conectando…"
            else -> "Na sala · $people ${if (people == 1) "pessoa" else "pessoas"}" + (if (v.sharing) " · compartilhando" else "")
        }
    }

    private fun action(name: String, title: String): Notification.Action {
        val intent = PendingIntent.getService(this, name.hashCode(), Intent(this, RoomService::class.java).setAction(name), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Action.Builder(null, title, intent).build()
    }

    private fun notification(): Notification {
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val builder = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Screenshare")
            .setContentText(notificationText())
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(action(ACTION_MUTE, if (_view.value.microphoneEnabled) "Mutar" else "Desmutar"))
        if (capture != null) builder.addAction(action(ACTION_STOP_SHARE, "Parar compartilhamento"))
        return builder.addAction(action(ACTION_LEAVE, "Sair")).build()
    }
}
