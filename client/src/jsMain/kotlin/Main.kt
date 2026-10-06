import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import platform.DesktopCaptureBridge
import platform.DesktopEnvironment
import platform.resolveWebSocketEndpoint
import services.Session
import services.WebsocketService
import services.handlePacket
import ui.InterfaceMutations
import ui.registerUIHandlers

var session: Session? = null
var hideAfterLocalShare = false

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
                    session?.prepareForReconnect()
                    InterfaceMutations.showError("Conexão interrompida. Tentando reconectar…")
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
        onCameraDeviceChange = { deviceId ->
            getSessionOrAlert().handleCameraDeviceChange(deviceId)
        },
    )
    InterfaceMutations.restoreOutputState()

    if (DesktopEnvironment.isAvailable) {
        val usernameInput = kotlinx.browser.document.getElementById("username") as org.w3c.dom.HTMLInputElement
        val roomInput = kotlinx.browser.document.getElementById("roomId") as org.w3c.dom.HTMLInputElement
        val startupToggle = kotlinx.browser.document.getElementById("start-at-login") as org.w3c.dom.HTMLInputElement
        usernameInput.value = window.localStorage.getItem("screenshare.lastUsername").orEmpty()
        roomInput.value = window.localStorage.getItem("screenshare.lastRoomId").orEmpty()
        websocketCoroutineScope.launch {
            runCatching { DesktopEnvironment.autoStartEnabled() }.onSuccess { startupToggle.checked = it }
            if (startupToggle.checked && usernameInput.value.isNotBlank() && roomInput.value.isNotBlank()) {
                kotlinx.browser.document.getElementById("joinBtn")?.asDynamic()?.click()
                runCatching { DesktopEnvironment.hideMainWindow() }
            }
        }
        startupToggle.addEventListener("change", {
            websocketCoroutineScope.launch {
                val requested = startupToggle.checked
                runCatching { DesktopEnvironment.setAutoStart(requested) }.onFailure {
                    startupToggle.checked = !requested
                    InterfaceMutations.showError("Não foi possível alterar o início automático do sistema.")
                }
            }
        })
    }

    window.addEventListener("screenshare:tray-action", { event ->
        when ((event.asDynamic().detail as? String).orEmpty()) {
            "mic" -> kotlinx.browser.document.getElementById("micToggle")?.asDynamic()?.click()
            "camera" -> {
                hideAfterLocalShare = true
                val id = if (session?.cameraSharing?.localCameraStream == null) "cameraBtn" else "stopCameraBtn"
                kotlinx.browser.document.getElementById(id)?.asDynamic()?.click()
            }
            "screen" -> {
                hideAfterLocalShare = true
                val buttonId = if (session?.screenSharing?.localScreenStream == null) "shareScreenBtn" else "stopSharingBtn"
                kotlinx.browser.document.getElementById(buttonId)?.asDynamic()?.click()
            }
            "quality720" -> kotlinx.browser.document.querySelector("[data-width='1280']")?.asDynamic()?.click()
            "quality1080" -> kotlinx.browser.document.querySelector("[data-width='1920']")?.asDynamic()?.click()
            "quality4k" -> kotlinx.browser.document.querySelector("[data-width='3840']")?.asDynamic()?.click()
            "switch" -> {
                session?.dispose()
                session = null
                websocketService.leaveRoom()
                val roomInput = kotlinx.browser.document.getElementById("roomId") as? org.w3c.dom.HTMLInputElement
                if (roomInput != null) roomInput.value = ""
                kotlinx.browser.document.getElementById("app-screen")?.classList?.add("hidden")
                kotlinx.browser.document.getElementById("join-screen")?.classList?.remove("hidden")
            }
            "leave" -> {
                session?.dispose()
                session = null
                websocketService.leaveRoom()
                websocketCoroutineScope.launch {
                    val startupToggle = kotlinx.browser.document.getElementById("start-at-login") as? org.w3c.dom.HTMLInputElement
                    runCatching { DesktopEnvironment.setAutoStart(false) }
                        .onSuccess { if (startupToggle != null) startupToggle.checked = false }
                        .onFailure {
                            if (startupToggle != null) startupToggle.checked = true
                            InterfaceMutations.showError("Não foi possível desativar o retorno automático.")
                        }
                }
                kotlinx.browser.document.getElementById("app-screen")?.classList?.add("hidden")
                kotlinx.browser.document.getElementById("join-screen")?.classList?.remove("hidden")
            }
        }
    })
    kotlinx.browser.document.addEventListener("visibilitychange", {
        session?.setVideoReception(!(kotlinx.browser.document.asDynamic().hidden as Boolean))
    })
    window.addEventListener("screenshare:session-media-available", { event ->
        session?.setMediaAvailable(event.asDynamic().detail == true)
    })
}

fun getSessionOrAlert(): Session {
    if (session == null) {
        InterfaceMutations.showError("Entre em uma sala primeiro.")
        throw IllegalStateException("Session is null")
    }
    return session!!
}
