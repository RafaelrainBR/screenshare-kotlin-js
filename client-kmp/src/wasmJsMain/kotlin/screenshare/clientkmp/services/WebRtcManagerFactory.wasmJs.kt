package screenshare.clientkmp.services

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import screenshare.clientkmp.state.AppState
import screenshare.common.Packet

/** wasmJs actual — delegates to full browser WebRTC implementation. */
actual fun createWebRtcManager(
    appStateHolder: WebRtcAppStateHolder,
    websocketService: WebsocketService,
): WebRtcManager = WebRtcManagerWasmJs(appStateHolder, websocketService)
