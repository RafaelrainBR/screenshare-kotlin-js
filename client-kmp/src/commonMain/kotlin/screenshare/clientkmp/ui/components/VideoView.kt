package screenshare.clientkmp.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/**
 * Renders a video track inside a Compose layout.
 *
 * [track] is typed as [Any?] to keep this expect declaration free of platform-specific types.
 * - Desktop (JVM): expects a `com.shepeliev.webrtckmp.VideoStreamTrack`
 * - wasmJs: video rendering is handled via HTML <video> elements; this is a placeholder.
 */
@Composable
expect fun VideoView(
    track: Any?,
    modifier: Modifier = Modifier,
)
