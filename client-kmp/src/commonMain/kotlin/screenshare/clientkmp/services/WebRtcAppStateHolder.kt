package screenshare.clientkmp.services

import kotlinx.coroutines.CoroutineScope
import screenshare.clientkmp.state.AppState

/**
 * Wraps AppState + CoroutineScope for use by WebRtcManager implementations.
 * Avoids exposing platform-specific details in the common interface.
 */
class WebRtcAppStateHolder(
    val appState: AppState,
    val coroutineScope: CoroutineScope,
)
