import decorators.RTCPeerConnectionDecorator
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import platform.DesktopCaptureBridge
import platform.DesktopEnvironment
import platform.resolveWebSocketEndpoint
import platform.shouldReceiveVideo
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
                },
            )
        }

    RTCPeerConnectionDecorator.probeHardwareEncoders()

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
        onStartScreenShare = { width, height, profile, useSourceResolution, audioMode, nativeSource ->
            getSessionOrAlert().handleStartScreenShare(
                width,
                height,
                profile,
                useSourceResolution,
                audioMode,
                nativeSource,
            )
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
        // Outside a room this only sets the next share; no session is needed.
        onShareProfileChange = { profile -> session?.updateShareProfile(profile) },
    )
    InterfaceMutations.restoreOutputState()
    InterfaceMutations.setTrayRoom(null)

    val trayButtons = kotlinx.browser.document.querySelectorAll("[data-tray-action]")
    for (index in 0 until trayButtons.length) {
        val button = trayButtons.item(index) as org.w3c.dom.HTMLElement
        button.addEventListener("click", {
            val action = button.getAttribute("data-tray-action")
            window.asDynamic().dispatchEvent(js("new CustomEvent('screenshare:tray-action', {detail: action})"))
        })
    }

    if (DesktopEnvironment.isAvailable) {
        kotlinx.browser.document
            .getElementById("startup-option")
            ?.classList
            ?.remove("hidden")
        val usernameInput = kotlinx.browser.document.getElementById("username") as org.w3c.dom.HTMLInputElement
        val roomInput = kotlinx.browser.document.getElementById("roomId") as org.w3c.dom.HTMLInputElement
        val startupToggle = kotlinx.browser.document.getElementById("start-at-login") as org.w3c.dom.HTMLInputElement
        usernameInput.value = window.localStorage.getItem("screenshare.lastUsername").orEmpty()
        roomInput.value = window.localStorage.getItem("screenshare.lastRoomId").orEmpty()
        websocketCoroutineScope.launch {
            runCatching { DesktopEnvironment.autoStartEnabled() }.onSuccess { startupToggle.checked = it }
            // The native side already opened the window, or kept it hidden for a login launch.
            val atLogin = runCatching { DesktopEnvironment.launchedAtLogin() }.getOrDefault(false)
            if (startupToggle.checked && usernameInput.value.isNotBlank() && roomInput.value.isNotBlank()) {
                if (atLogin) DesktopEnvironment.viewerMode = "hidden"
                kotlinx.browser.document
                    .getElementById("joinBtn")
                    ?.asDynamic()
                    ?.click()
            } else if (atLogin) {
                // Nothing to rejoin: a tray icon alone would look like the app failed to start.
                runCatching { DesktopEnvironment.showMainWindow() }
            }
        }
        websocketCoroutineScope.launch {
            // Offline or no release yet: no banner, nothing to report.
            runCatching { DesktopEnvironment.checkForUpdate() }.getOrNull()?.let {
                showUpdateBanner(
                    it,
                    websocketCoroutineScope,
                )
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
    } else {
        // ui.js already filled the room from ?roomId=; a saved name then rejoins directly (reload or room link).
        val savedName = window.localStorage.getItem("screenshare.lastUsername").orEmpty()
        val urlRoom = ui.Elements.roomIdInput.value
        ui.Elements.usernameInput.value = savedName
        if (savedName.isNotBlank() && urlRoom.isNotBlank()) {
            ui.Elements.joinButton.click()
        }
    }

    fun leaveRoom() {
        val draft = ui.Elements.messageInput.value
        if (draft.isNotBlank() &&
            !window.confirm("Há uma mensagem não enviada. Sair da sala e descartar o rascunho?")
        ) {
            return
        }
        ui.Elements.cancelShare.click()
        kotlinx.browser.document
            .getElementById("permission-recovery")
            ?.classList
            ?.add("hidden")
        session?.dispose()
        session = null
        websocketService.leaveRoom()
        InterfaceMutations.setTrayRoom(null)
        ui.mutations.UserListMutations.updateUserList(emptyList(), null)
        ui.Elements.messageInput.value = ""
        ui.Elements.roomIdInput.value = ""
        ui.Elements.chatMessages.innerHTML = ""
        ui.Elements.appScreen.classList
            .add("hidden")
        ui.Elements.joinScreen.classList
            .remove("hidden")
        val url =
            org.w3c.dom.url
                .URL(window.location.href)
        url.searchParams.delete("roomId")
        window.history.replaceState(null, "", url.href)
        ui.Elements.usernameInput.focus()
        websocketCoroutineScope.launch {
            runCatching { DesktopEnvironment.setTrayState("remote", false) }
            if (DesktopEnvironment.isAvailable) {
                runCatching { DesktopEnvironment.showMainWindow() }
                val startupToggle =
                    kotlinx.browser.document.getElementById(
                        "start-at-login",
                    ) as org.w3c.dom.HTMLInputElement
                runCatching { DesktopEnvironment.setAutoStart(false) }
                    .onSuccess { startupToggle.checked = false }
                    .onFailure { InterfaceMutations.showError("Não foi possível desativar o retorno automático.") }
            }
        }
    }

    // Forms never open inside the tray panel: they use the full window, which
    // hides again when a flow started from the panel or tray menu finishes.
    fun clickInFullWindow(buttonId: String) {
        val fromTray = DesktopEnvironment.isAvailable && DesktopEnvironment.viewerMode != "room"
        hideAfterLocalShare = fromTray
        websocketCoroutineScope.launch {
            if (fromTray) runCatching { DesktopEnvironment.showMainWindow() }
            kotlinx.browser.document
                .getElementById(buttonId)
                ?.asDynamic()
                ?.click()
        }
    }

    window.addEventListener("screenshare:tray-action", { event ->
        when ((event.asDynamic().detail as? String).orEmpty()) {
            "open" -> {
                InterfaceMutations.showGrid()
                websocketCoroutineScope.launch { runCatching { DesktopEnvironment.showMainWindow() } }
            }

            "output" -> {
                InterfaceMutations.toggleOutputMute()
            }

            "quit" -> {
                window.asDynamic().__screenshareDesktopBridge.quitApp()
            }

            "quality-options" -> {
                clickInFullWindow("shareScreenBtn")
            }

            "mic" -> {
                kotlinx.browser.document
                    .getElementById("micToggle")
                    ?.asDynamic()
                    ?.click()
            }

            "camera" -> {
                val id = if (session?.cameraSharing?.localCameraStream == null) "cameraBtn" else "stopCameraBtn"
                kotlinx.browser.document
                    .getElementById(id)
                    ?.asDynamic()
                    ?.click()
            }

            "screen" -> {
                if (session?.screenSharing?.localScreenStream == null) {
                    clickInFullWindow("shareScreenBtn")
                } else {
                    kotlinx.browser.document
                        .getElementById("stopSharingBtn")
                        ?.asDynamic()
                        ?.click()
                }
            }

            "quality720" -> {
                ui.chooseShareResolution("1280x720")
            }

            "quality1080" -> {
                ui.chooseShareResolution("1920x1080")
            }

            "quality4k" -> {
                ui.chooseShareResolution("3840x2160")
            }

            "leave" -> {
                // The draft confirmation is a dialog; the panel would hide when it takes focus.
                val draft = ui.Elements.messageInput.value
                if (DesktopEnvironment.viewerMode == "compact" && draft.isNotBlank()) {
                    websocketCoroutineScope.launch {
                        runCatching { DesktopEnvironment.showMainWindow() }
                        leaveRoom()
                    }
                } else {
                    leaveRoom()
                }
            }
        }
    })
    kotlinx.browser.document.addEventListener("visibilitychange", {
        session?.setVideoReception(
            shouldReceiveVideo(
                DesktopEnvironment.isAvailable,
                DesktopEnvironment.viewerMode,
                kotlinx.browser.document
                    .asDynamic()
                    .hidden as Boolean,
            ),
        )
    })
    window.addEventListener("screenshare:viewer-mode", { event ->
        val mode = event.asDynamic().detail as? String ?: "hidden"
        DesktopEnvironment.viewerMode = mode
        if (mode != "hidden") {
            kotlinx.browser.document.body
                ?.classList
                ?.toggle("compact-tray", mode == "compact")
        }
        session?.setVideoReception(
            shouldReceiveVideo(
                true,
                mode,
                kotlinx.browser.document
                    .asDynamic()
                    .hidden as Boolean,
            ),
        )
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

private fun showUpdateBanner(
    update: DesktopEnvironment.AvailableUpdate,
    scope: kotlinx.coroutines.CoroutineScope,
) {
    val document = kotlinx.browser.document
    val banner = document.getElementById("update-banner") as org.w3c.dom.HTMLElement
    val text = document.getElementById("update-text") as org.w3c.dom.HTMLElement
    val progress = document.getElementById("update-progress") as org.w3c.dom.HTMLProgressElement
    val button = document.getElementById("update-btn") as org.w3c.dom.HTMLButtonElement
    text.textContent = "Versão ${update.version} disponível"
    banner.classList.remove("hidden")
    button.addEventListener("click", {
        // Never drop an active room to install.
        if (session != null) {
            InterfaceMutations.showError("Saia da sala para atualizar.")
            return@addEventListener
        }
        button.disabled = true
        button.textContent = "Atualizando…"
        progress.classList.remove("hidden")
        scope.launch {
            runCatching { DesktopEnvironment.installUpdate { progress.value = it } }.onFailure {
                button.disabled = false
                button.textContent = "Atualizar e reiniciar"
                progress.classList.add("hidden")
                text.innerHTML =
                    "Falha ao atualizar. <a href=\"https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/latest\" target=\"_blank\" rel=\"noopener\">Baixar manualmente</a>"
            }
        }
    })
}
