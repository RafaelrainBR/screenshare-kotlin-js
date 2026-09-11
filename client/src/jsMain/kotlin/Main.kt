import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import platform.DesktopCaptureBridge
import platform.DesktopEnvironment
import platform.resolveWebSocketEndpoint
import screenshare.common.ChatMessage
import services.Session
import services.WebsocketService
import services.handlePacket
import ui.InterfaceMutations
import ui.registerUIHandlers
import kotlin.js.Date

var session: Session? = null

fun main() {
    val websocketService =
        with(window.location) {
            val endpoint =
                resolveWebSocketEndpoint(
                    locationProtocol = protocol,
                    locationHostname = hostname,
                    locationPort = port,
                    isDesktop = DesktopEnvironment.isAvailable,
                    desktopServerUrl = DesktopEnvironment.serverUrl,
                )

            WebsocketService(
                urlProtocol = endpoint.protocol,
                host = endpoint.host,
                port = endpoint.port,
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

    // `pagehide` also fires for a WebView shutdown. Do not send room packets
    // here: the transport may already be gone, but always release local media
    // and the optional native-audio bridge.
    window.addEventListener("pagehide", {
        session?.dispose()
        DesktopCaptureBridge.stopAudioOnPageExit()
    })

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
        onStartScreenShare = { width, height, fps, useSourceResolution, audioMode, processId ->
            getSessionOrAlert().handleStartScreenShare(width, height, fps, useSourceResolution, audioMode, processId)
        },
        onChangeDesktopAudio = {
            val button = kotlinx.browser.document.getElementById("shareScreenBtn") as org.w3c.dom.HTMLButtonElement
            button.click()
        },
        onStopScreenShare = {
            getSessionOrAlert().handleStopScreenShare()
        },
        onStartCameraShare = {
            getSessionOrAlert().handleStartCameraShare()
        },
        onStopCameraShare = {
            getSessionOrAlert().handleStopCameraShare()
        },
        onInputDeviceChange = { deviceId ->
            getSessionOrAlert().handleMicInputDeviceChange(deviceId)
        },
        onOutputDeviceChange = { deviceId ->
            getSessionOrAlert().handleSpeakerOutputDeviceChange(deviceId)
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
