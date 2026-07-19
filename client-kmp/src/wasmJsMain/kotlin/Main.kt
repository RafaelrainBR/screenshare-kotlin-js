@file:OptIn(
    androidx.compose.ui.ExperimentalComposeUiApi::class,
)

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.window.ComposeViewport
import io.ktor.http.URLProtocol
import kotlinx.browser.document
import kotlinx.browser.window
import screenshare.clientkmp.services.PacketHandler
import screenshare.clientkmp.services.SessionManager
import screenshare.clientkmp.services.WebsocketService
import screenshare.clientkmp.state.AppState
import screenshare.clientkmp.ui.App

/** Extrai o valor de um query param da URL atual, ex: ?roomId=abc → "abc" */
private fun urlParam(key: String): String? {
    val search = window.location.search.removePrefix("?")
    return search
        .split("&")
        .mapNotNull { param ->
            val (k, v) = param.split("=").let { it[0] to it.getOrElse(1) { "" } }
            if (k == key) v.ifEmpty { null } else null
        }.firstOrNull()
}

fun main() {
    val initialRoomId = urlParam("roomId")

    ComposeViewport(document.body!!) {
        val appState = remember { AppState(initialRoomId = initialRoomId) }
        val coroutineScope = rememberCoroutineScope()

        val packetHandlerRef = remember { mutableStateOf<PacketHandler?>(null) }

        val websocketService =
            remember {
                val loc = window.location
                val protocol = if (loc.protocol == "https:") URLProtocol.WSS else URLProtocol.WS
                val port = loc.port.toIntOrNull() ?: if (loc.protocol == "https:") 443 else 80
                WebsocketService(
                    urlProtocol = protocol,
                    host = loc.hostname,
                    port = port,
                    onPacketReceived = { packet -> packetHandlerRef.value?.handle(packet) },
                )
            }

        val sessionManager = remember { SessionManager(appState, websocketService, coroutineScope) }

        remember { PacketHandler(appState, sessionManager.webRtcManager).also { packetHandlerRef.value = it } }

        LaunchedEffect(Unit) { websocketService.connect(coroutineScope) }

        App(sessionManager, appState)
    }
}
