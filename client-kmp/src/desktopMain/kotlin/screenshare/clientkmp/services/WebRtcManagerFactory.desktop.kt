package screenshare.clientkmp.services

actual fun createWebRtcManager(
    appStateHolder: WebRtcAppStateHolder,
    websocketService: WebsocketService,
): WebRtcManager = WebRtcManagerDesktop(appStateHolder, websocketService)
