import io.ktor.http.URLProtocol
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import screenshare.common.ChatMessage
import services.Session
import services.WebsocketService
import services.handlePacket
import ui.InterfaceMutations
import ui.registerUIHandlers
import kotlin.js.Date

var session: Session? = null

fun main() {
    println("Hello, World!")

    val websocketService =
        with(window.location) {
            println("Connecting to WebSocket at $href")
            val port =
                @Suppress("UselessCallOnNotNull")
                if (port.isNullOrBlank()) {
                    if (protocol == "https:") "443" else "80"
                } else {
                    port
                }

            WebsocketService(
                urlProtocol = if (protocol == "https:") URLProtocol.WSS else URLProtocol.WS,
                host = hostname,
                port = port.toInt(),
                handler = ::handlePacket,
                onClose = {
                    InterfaceMutations.addMessageToChat(
                        ChatMessage(
                            username = "Sistema",
                            content = "Conexão encerrada! Recarregue a página.",
                            timestamp = Date().getTime().toLong(),
                        ),
                        localUsername = session?.localUsername.orEmpty(),
                    )
                    window.alert("Conexão encerrada! Recarregue a página.")
                },
            )
        }

    val websocketCoroutineScope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    websocketCoroutineScope.launch {
        websocketService.connect(websocketCoroutineScope)
    }

    registerUIHandlers(
        joinRoom = { username, roomId ->
            session =
                Session(
                    localUsername = username,
                    localRoomId = roomId,
                    websocketService = websocketService,
                    coroutineScope = websocketCoroutineScope,
                )
        },
        sendChatMessage = { message ->
            getSessionOrAlert().handleMessageSend(message)
        },
        onMicButtonToggle = {
            getSessionOrAlert().handleMicButtonToggle()
        },
        onStartScreenShare = {
            getSessionOrAlert().handleStartScreenShare()
        },
        onStopScreenShare = {
            getSessionOrAlert().handleStopScreenShare()
        },
    )
}

fun getSessionOrAlert(): Session {
    if (session == null) {
        window.alert("Você precisa entrar em uma sala primeiro!")
        throw IllegalStateException("Session is null")
    }
    return session!!
}
