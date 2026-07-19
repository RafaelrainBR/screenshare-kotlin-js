package screenshare.clientkmp.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color

/**
 * wasmJs stub — video rendering via HTML <video> elements is handled at the JS interop layer.
 * The track received here is always null on wasmJs (currentVideoTrack stub).
 */
@Composable
actual fun VideoView(
    track: Any?,
    modifier: Modifier,
) {
    // On wasmJs the video is rendered by the browser directly via RTCPeerConnection.ontrack.
    // When this composable is reached with a non-null track it means native wasmJs rendering
    // hasn't been wired yet — show a black placeholder.
    if (track != null) {
        Box(modifier = modifier.fillMaxSize().background(Color.Black))
    }
}
