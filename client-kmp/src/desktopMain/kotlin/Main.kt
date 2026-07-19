import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import io.ktor.http.URLProtocol
import screenshare.clientkmp.services.PacketHandler
import screenshare.clientkmp.services.SessionManager
import screenshare.clientkmp.services.WebsocketService
import screenshare.clientkmp.state.AppState
import screenshare.clientkmp.ui.App

fun main(args: Array<String>) =
    application {
        val initialRoomId = args.firstOrNull { it.startsWith("--room=") }?.removePrefix("--room=")
        val host = args.firstOrNull { it.startsWith("--host=") }?.removePrefix("--host=") ?: "localhost"
        val isSecure = args.contains("--secure")
        val protocol = if (isSecure) URLProtocol.WSS else URLProtocol.WS
        val port = args.firstOrNull { it.startsWith("--port=") }?.removePrefix("--port=")?.toIntOrNull()
            ?: if (isSecure) 443 else 8080

        val appState = remember { AppState(initialRoomId = initialRoomId) }
        val coroutineScope = rememberCoroutineScope()

        // Late-binding: PacketHandler needs webRtcManager from SessionManager,
        // but WebsocketService must exist before SessionManager.
        val packetHandlerRef = remember { mutableStateOf<PacketHandler?>(null) }

        val websocketService =
            remember {
                WebsocketService(
                    urlProtocol = protocol,
                    host = host,
                    port = port,
                    onPacketReceived = { packet -> packetHandlerRef.value?.handle(packet) },
                )
            }

        val sessionManager = remember { SessionManager(appState, websocketService, coroutineScope) }

        // Wire PacketHandler once sessionManager (and its webRtcManager) is ready
        remember { PacketHandler(appState, sessionManager.webRtcManager).also { packetHandlerRef.value = it } }

        LaunchedEffect(Unit) {
            websocketService.connect(coroutineScope)
        }

        Window(
            onCloseRequest = ::exitApplication,
            title = "ScreenShare",
        ) {
            App(sessionManager, appState)
        }
    }
