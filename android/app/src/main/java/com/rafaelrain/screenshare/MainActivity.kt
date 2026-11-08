package com.rafaelrain.screenshare

import android.Manifest
import android.app.PictureInPictureParams
import android.content.Intent
import android.content.pm.PackageManager
import android.media.projection.MediaProjectionConfig
import android.media.projection.MediaProjectionManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Rational
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.background
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle

/** `?roomId=` of the shared link (https://screen-share.fly.dev/?roomId=…), validated like a typed code. */
fun roomFromLink(uri: Uri?): String? =
    uri?.takeIf { it.scheme == "https" && it.host == "screen-share.fly.dev" && (it.path ?: "/") in setOf("", "/") }
        ?.getQueryParameter("roomId")?.trim()?.takeIf { validRoom(it) }

fun validRoom(code: String) = code.length in 1..64 && code.all { it.isLetterOrDigit() || it == '-' || it == '_' }

class MainActivity : ComponentActivity() {
    private val linkRoom = mutableStateOf<String?>(null)
    private val pip = mutableStateOf(false)
    private var pendingShare: ShareChoice? = null

    private val permissions = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {}

    private val projection = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val choice = pendingShare ?: return@registerForActivityResult
        val data = result.data
        if (result.resultCode == RESULT_OK && data != null) RoomService.instance?.startShare(result.resultCode, data, choice)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        linkRoom.value = roomFromLink(intent?.data)
        setContent {
            ScreenshareTheme {
                Box(Modifier.fillMaxSize().background(Bg).safeDrawingPadding()) {
                    val room by RoomService.room.collectAsStateWithLifecycle()
                    val view by RoomService.view.collectAsStateWithLifecycle()
                    // The floating window shows only the video being watched: someone else's screen first.
                    val watched = view.videos.firstOrNull { it.first != view.localId && !it.second } ?: view.videos.firstOrNull { it.first != view.localId }
                    when {
                        room == null -> JoinScreen(linkRoom.value, ::join)
                        pip.value && watched != null -> VideoTile(watched.first, watched.second, fit = true, modifier = Modifier.fillMaxSize())
                        else -> RoomScreen(room!!, ::askShare)
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // A link never joins by itself: it fills the code, and the person confirms.
        roomFromLink(intent.data)?.let { linkRoom.value = it }
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: android.content.res.Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        pip.value = isInPictureInPictureMode
    }

    override fun onStart() {
        super.onStart()
        RoomService.visible = true
    }

    override fun onStop() {
        RoomService.visible = false
        super.onStop()
    }

    private fun join(room: String, name: String) {
        Settings.setName(this, name)
        Settings.remember(this, room)
        val wanted = buildList {
            add(Manifest.permission.RECORD_AUDIO)
            if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.POST_NOTIFICATIONS)
        }.filter { checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED }
        if (wanted.isNotEmpty()) permissions.launch(wanted.toTypedArray())
        linkRoom.value = null
        RoomService.join(this, room, name)
    }

    private fun askShare(choice: ShareChoice) {
        Settings.setLastShare(this, choice)
        pendingShare = choice
        val manager = getSystemService(MediaProjectionManager::class.java)
        // Android 14+: the person can pick one app instead of the whole screen.
        val intent = if (Build.VERSION.SDK_INT >= 34) {
            manager.createScreenCaptureIntent(MediaProjectionConfig.createConfigForUserChoice())
        } else {
            manager.createScreenCaptureIntent()
        }
        projection.launch(intent)
    }

    /** Watching a screen and leaving the app floats it (API 29/30; 31+ enters by itself through setAutoEnterEnabled). */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        if (Build.VERSION.SDK_INT < 31 && watching()) enterPictureInPictureMode(pipParams())
    }

    private fun watching() = RoomService.view.value.videos.any { it.first != RoomService.view.value.localId }

    fun pipParams(): PictureInPictureParams = PictureInPictureParams.Builder().setAspectRatio(Rational(16, 9)).apply {
        if (Build.VERSION.SDK_INT >= 31) setAutoEnterEnabled(watching())
    }.build()
}

