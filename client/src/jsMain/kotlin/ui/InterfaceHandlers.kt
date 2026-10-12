package ui

import generateRandomRoomId
import hideAfterLocalShare
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.khronos.webgl.Uint8ClampedArray
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLSelectElement
import org.w3c.dom.ImageData
import platform.DesktopCaptureBridge
import platform.DesktopCaptureSource
import platform.DesktopCaptureSourceKind
import services.ScreenSharing
import services.nativeAudioMode

private const val DEFAULT_WIDTH = 1920
private const val DEFAULT_HEIGHT = 1080
private const val DEFAULT_FPS = 30

private var selectedWidth = window.localStorage.getItem("screenshare.shareWidth")?.toIntOrNull() ?: DEFAULT_WIDTH
private var selectedHeight = window.localStorage.getItem("screenshare.shareHeight")?.toIntOrNull() ?: DEFAULT_HEIGHT
private var selectedFps = window.localStorage.getItem("screenshare.shareFps")?.toIntOrNull() ?: DEFAULT_FPS
private var useSourceResolution = window.localStorage.getItem("screenshare.sourceResolution") == "true"
private var selectedSource: DesktopCaptureSource? = null
private var shareAudio = false
private val uiScope = CoroutineScope(Dispatchers.Main)
private var sourceRefreshTimer: Int? = null
private var sourceRefreshGeneration = 0
private var sourceRefreshInFlight = false
private var renderedSourcesSignature = ""

fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: suspend (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
    onStartScreenShare: (
        width: Int,
        height: Int,
        fps: Int,
        useSourceResolution: Boolean,
        audioMode: ScreenSharing.DesktopAudioMode,
        nativeSource: DesktopCaptureSource?,
    ) -> Unit,
    onStopScreenShare: () -> Unit,
    onStartCameraShare: () -> Unit,
    onStopCameraShare: () -> Unit,
    onInputDeviceChange: (deviceId: String) -> Unit,
    onOutputDeviceChange: (deviceId: String) -> Unit,
    onCameraDeviceChange: (deviceId: String) -> Unit,
) {
    setupJoinButtonHandler(joinRoom)
    setupSendMessageButtonHandler(sendChatMessage)
    setupMicToggleButtonHandler(onMicButtonToggle)
    setupShareQualityModal(onStartScreenShare)
    setupStopScreenShareButtonHandler(onStopScreenShare)
    setupCameraButtons(onStartCameraShare, onStopCameraShare)
    setupDeviceHandlers(onInputDeviceChange, onOutputDeviceChange, onCameraDeviceChange)
    Elements.outputToggle.addEventListener("click", { InterfaceMutations.toggleOutputMute() })
}

private fun setupJoinButtonHandler(joinRoom: (username: String, roomId: String) -> Unit) =
    document.getElementById("join-form")?.addEventListener("submit", { e ->
        e.preventDefault()
        val username = Elements.usernameInput.value.trim()
        val roomId =
            Elements.roomIdInput.value
                .trim()
                .takeIf { it.isNotBlank() }
                ?: generateRandomRoomId().take(8)

        if (username.isBlank()) {
            Elements.usernameInput.setAttribute("aria-invalid", "true")
            document.getElementById("join-error")?.classList?.remove("hidden")
            Elements.usernameInput.focus()
            return@addEventListener
        }

        Elements.usernameInput.removeAttribute("aria-invalid")
        document.getElementById("join-error")?.classList?.add("hidden")

        Elements.currentRoomId.textContent = roomId
        window.localStorage.setItem("screenshare.lastUsername", username)
        window.localStorage.setItem("screenshare.lastRoomId", roomId)

        joinRoom(username, roomId)
    })

private fun setupSendMessageButtonHandler(sendChatMessage: suspend (message: String) -> Unit) =
    document.getElementById("chat-form")?.addEventListener("submit", { e ->
        e.preventDefault()
        val draft = Elements.messageInput.value
        val message = draft.trim()
        if (message.isNotBlank() && !Elements.sendMessageButton.disabled) {
            Elements.sendMessageButton.disabled = true
            uiScope.launch {
                runCatching { sendChatMessage(message) }
                    .onSuccess {
                        if (Elements.messageInput.value == draft) Elements.messageInput.value = ""
                    }.onFailure {
                        InterfaceMutations.showError(
                            "Envio não confirmado. Sua mensagem foi mantida; confira a conexão antes de tentar novamente.",
                        )
                    }
                Elements.sendMessageButton.disabled = !InterfaceMutations.roomConnected
            }
        }
    })

private fun setupMicToggleButtonHandler(onMicButtonToggle: () -> Unit) =
    Elements.micToggle.addEventListener("click", { e ->
        e.preventDefault()
        onMicButtonToggle()
    })

private fun setupShareQualityModal(
    onStartScreenShare: (Int, Int, Int, Boolean, ScreenSharing.DesktopAudioMode, DesktopCaptureSource?) -> Unit,
) {
    Elements.shareScreenButton.addEventListener("click", { e ->
        e.preventDefault()
        resetQualitySelection()
        // Browsers never load the bridge or enumerate local windows. Their
        // direct, standard getDisplayMedia flow remains intentionally simple.
        if (!platform.DesktopEnvironment.isAvailable) {
            onStartScreenShare(
                selectedWidth,
                selectedHeight,
                selectedFps,
                useSourceResolution,
                ScreenSharing.DesktopAudioMode.NONE,
                null,
            )
            return@addEventListener
        }
        if (DesktopCaptureBridge.isAvailable) openNativeShareWizard()
        Elements.qualityModal.classList.remove("hidden")
    })

    Elements.confirmShare.addEventListener("click", { e ->
        e.preventDefault()
        val source = selectedSource
        if (!DesktopCaptureBridge.isAvailable) {
            Elements.qualityModal.classList.add("hidden")
            onStartScreenShare(
                selectedWidth,
                selectedHeight,
                selectedFps,
                useSourceResolution,
                ScreenSharing.DesktopAudioMode.NONE,
                null,
            )
            return@addEventListener
        }
        if (source == null) return@addEventListener
        stopSourceRefresh()
        Elements.qualityModal.classList.add("hidden")
        val (left, top, right, bottom) = source.rect
        // Native frames keep the source size; it only sets the encoder bitrate.
        onStartScreenShare(
            right - left,
            bottom - top,
            selectedFps,
            true,
            nativeAudioMode(shareAudio, source.kind),
            source,
        )
    })

    Elements.cancelShare.addEventListener("click", { e ->
        e.preventDefault()
        stopSourceRefresh()
        Elements.qualityModal.classList.add("hidden")
        // A wizard opened from the tray returns to the tray when cancelled.
        if (hideAfterLocalShare) {
            hideAfterLocalShare = false
            CoroutineScope(Dispatchers.Main).launch { runCatching { platform.DesktopEnvironment.hideMainWindow() } }
        }
    })

    val options = document.getElementsByClassName("quality-option")
    for (i in 0 until options.length) {
        val option = options.item(i) as HTMLElement
        option.addEventListener("click", {
            val group = option.parentElement
            val groupOptions = group?.getElementsByClassName("quality-option") ?: return@addEventListener
            for (j in 0 until groupOptions.length) {
                (groupOptions.item(j) as HTMLElement).classList.remove("btn-primary")
            }
            option.classList.add("btn-primary")

            if (option.getAttribute("data-source") != null) {
                useSourceResolution = true
                window.localStorage.setItem("screenshare.sourceResolution", "true")
            } else if (option.getAttribute("data-width") != null) {
                useSourceResolution = false
                window.localStorage.setItem("screenshare.sourceResolution", "false")
                option.getAttribute("data-width")?.toIntOrNull()?.let { selectedWidth = it }
                option.getAttribute("data-height")?.toIntOrNull()?.let { selectedHeight = it }
                window.localStorage.setItem("screenshare.shareWidth", selectedWidth.toString())
                window.localStorage.setItem("screenshare.shareHeight", selectedHeight.toString())
            }
            option.getAttribute("data-fps")?.toIntOrNull()?.let {
                selectedFps = it
                window.localStorage.setItem("screenshare.shareFps", selectedFps.toString())
            }
            InterfaceMutations.syncTrayQuality()
            updateQualitySummary()
        })
    }
    Elements.shareAudioOn.onclick = {
        setShareAudio(true)
        null
    }
    Elements.shareAudioOff.onclick = {
        setShareAudio(false)
        null
    }
    Elements.shareSourceRefresh.onclick = {
        refreshSources(forceRender = true)
        null
    }
}

/** Windows: one dialog picks the source, its audio and the frame rate; no OS picker follows. */
internal fun openNativeShareWizard() {
    stopSourceRefresh()
    selectedSource = null
    renderedSourcesSignature = ""
    Elements.shareWizardDesc.textContent = "Escolha a tela ou janela e o áudio que quer enviar."
    Elements.shareSourceStep.classList.remove("hidden")
    Elements.shareAudioStep.classList.remove("hidden")
    // The native capture sends the source size, so only the frame rate is a choice.
    Elements.resolutionSection.classList.add("hidden")
    Elements.shareQuality.removeAttribute("open")
    Elements.shareSourceGrid.innerHTML = ""
    Elements.confirmShare.textContent = "Compartilhar"
    Elements.confirmShare.disabled = true
    setShareAudio(false)
    updateQualitySummary()
    Elements.shareSourceState.textContent = "Carregando telas e janelas…"
    val generation = ++sourceRefreshGeneration
    refreshSources(generation = generation)
    sourceRefreshTimer = window.setInterval({ refreshSources(generation = generation) }, 2_000)
}

private fun setShareAudio(enabled: Boolean) {
    shareAudio = enabled
    Elements.shareAudioOn.classList.toggle("selected", enabled)
    Elements.shareAudioOff.classList.toggle("selected", !enabled)
    Elements.shareAudioOn.setAttribute("aria-pressed", enabled.toString())
    Elements.shareAudioOff.setAttribute("aria-pressed", (!enabled).toString())
    val source = selectedSource
    val (title, description) =
        when (source?.kind) {
            null -> "Com áudio" to "Escolha uma tela ou janela."
            DesktopCaptureSourceKind.SCREEN -> "Áudio do computador" to "Enviar todo o som da saída padrão."
            DesktopCaptureSourceKind.WINDOW -> "Áudio do aplicativo" to "Enviar só o som de ${source.title}."
        }
    document.getElementById("share-audio-on-title")?.textContent = title
    document.getElementById("share-audio-on-desc")?.textContent = description
}

private fun updateQualitySummary() {
    val resolution =
        when {
            useSourceResolution -> "Fonte"
            selectedHeight >= 2160 -> "4K"
            else -> "${selectedHeight}p"
        }
    val fps = "$selectedFps fps"
    Elements.shareQualitySummary.textContent =
        if (DesktopCaptureBridge.isAvailable) fps else "$resolution · $fps"
    updateSourceState()
}

private fun stopSourceRefresh() {
    sourceRefreshTimer?.let { window.clearInterval(it) }
    sourceRefreshTimer = null
    sourceRefreshInFlight = false
    sourceRefreshGeneration++
}

private fun refreshSources(
    forceRender: Boolean = false,
    generation: Int = sourceRefreshGeneration,
) {
    if (sourceRefreshInFlight || generation != sourceRefreshGeneration) return
    sourceRefreshInFlight = true
    uiScope.launch {
        runCatching { DesktopCaptureBridge.enumerateSources() }
            .onSuccess { sources ->
                if (generation != sourceRefreshGeneration) return@onSuccess
                val ordered =
                    sources.filter { it.kind == DesktopCaptureSourceKind.SCREEN } +
                        sources
                            .filter { it.kind == DesktopCaptureSourceKind.WINDOW }
                            .sortedBy { it.title.lowercase() }
                val signature = ordered.joinToString("|") { "${it.id}:${it.processId}:${it.title}" }
                if (forceRender || signature != renderedSourcesSignature) {
                    renderedSourcesSignature = signature
                    renderSources(ordered)
                }
                updateSourceState()
            }.onFailure {
                if (generation == sourceRefreshGeneration) {
                    Elements.shareSourceState.textContent =
                        "Não foi possível atualizar as telas e janelas. Tente novamente."
                }
            }
        if (generation == sourceRefreshGeneration) sourceRefreshInFlight = false
    }
}

private fun updateSourceState() {
    if (!DesktopCaptureBridge.isAvailable) return
    val source = selectedSource
    Elements.shareSourceState.textContent =
        when {
            renderedSourcesSignature.isEmpty() -> {
                "Nenhuma tela ou janela disponível agora."
            }

            source == null -> {
                "Escolha o que você quer compartilhar."
            }

            source.kind == DesktopCaptureSourceKind.WINDOW && selectedFps > 30 -> {
                "Janelas são enviadas a até 30 fps."
            }

            else -> {
                ""
            }
        }
}

private fun renderSources(sources: List<DesktopCaptureSource>) {
    val selectedId = selectedSource?.id
    Elements.shareSourceGrid.innerHTML = ""
    var group: DesktopCaptureSourceKind? = null
    sources.forEach { source ->
        if (source.kind != group) {
            group = source.kind
            val label = document.createElement("p") as HTMLElement
            label.className = "source-group"
            label.textContent = if (source.kind == DesktopCaptureSourceKind.SCREEN) "Telas" else "Janelas"
            Elements.shareSourceGrid.appendChild(label)
        }
        Elements.shareSourceGrid.appendChild(createSourceTile(source))
    }
    // A closed window disappears from the list and can no longer be chosen.
    val previous = selectedSource
    selectedSource = sources.firstOrNull { it.id == selectedId && it.processId == previous?.processId }
    Elements.confirmShare.disabled = selectedSource == null
    if (previous != null && selectedSource == null) setShareAudio(shareAudio)
}

private fun createSourceTile(source: DesktopCaptureSource): HTMLElement {
    val button = document.createElement("button") as org.w3c.dom.HTMLButtonElement
    button.type = "button"
    button.className = "audio-source-tile"
    val selected = selectedSource?.let { it.id == source.id && it.processId == source.processId } == true
    button.classList.toggle("selected", selected)
    button.setAttribute("aria-pressed", selected.toString())
    button.setAttribute("aria-label", "Compartilhar ${source.title}")
    val preview = document.createElement("span") as HTMLElement
    preview.className = "audio-source-preview"
    preview.setAttribute("aria-hidden", "true")
    preview.textContent = "Carregando preview…"
    val title = document.createElement("span") as HTMLElement
    title.className = "audio-source-title"
    title.textContent = source.title
    button.appendChild(preview)
    button.appendChild(title)
    loadThumbnailInto(source, preview)
    button.onclick = {
        selectedSource = source
        Elements.confirmShare.disabled = false
        val tiles = Elements.shareSourceGrid.getElementsByClassName("audio-source-tile")
        for (index in 0 until tiles.length) {
            (tiles.item(index) as HTMLElement).classList.remove("selected")
            (tiles.item(index) as HTMLElement).setAttribute("aria-pressed", "false")
        }
        button.classList.add("selected")
        button.setAttribute("aria-pressed", "true")
        setShareAudio(shareAudio)
        updateSourceState()
        null
    }
    return button
}

private fun loadThumbnailInto(
    source: DesktopCaptureSource,
    container: HTMLElement,
) = uiScope.launch {
    val thumbnail = DesktopCaptureBridge.captureThumbnail(source)
    // Minimized windows come back as 0×0, which ImageData rejects.
    if (thumbnail == null || thumbnail.width <= 0 || thumbnail.height <= 0) {
        if (container.isConnected) container.textContent = "Sem preview (janela minimizada?)"
        return@launch
    }
    if (!container.isConnected) return@launch
    container.innerHTML = ""
    container.appendChild(thumbnailCanvas(thumbnail))
}

private fun thumbnailCanvas(thumbnail: platform.DesktopCaptureThumbnail): HTMLElement {
    val canvas: dynamic = document.createElement("canvas")
    canvas.width = thumbnail.width
    canvas.height = thumbnail.height
    val rgba = thumbnail.bgra.copyOf()
    for (index in rgba.indices step 4) {
        val blue = rgba[index]
        rgba[index] = rgba[index + 2]
        rgba[index + 2] = blue
        // GDI DIB sections commonly leave alpha at zero even when their RGB
        // channels contain a valid capture. Canvas treats that as transparent.
        rgba[index + 3] = 255
    }
    // No js("...") here: minified Kotlin renames thumbnail.width, leaving ImageData an undefined size.
    val pixels = Uint8ClampedArray(rgba.size)
    pixels.asDynamic().set(rgba) // IntArray is an Int32Array; set() copies 0..255 values as-is.
    val imageData = ImageData(pixels, thumbnail.width, thumbnail.height)
    canvas.getContext("2d").putImageData(imageData, 0, 0)
    return canvas as HTMLElement
}

private fun resetQualitySelection() {
    val options = document.getElementsByClassName("quality-option")
    for (i in 0 until options.length) {
        val option = options.item(i) as HTMLElement
        val isDefault =
            if (option.getAttribute("data-fps") != null) {
                option.getAttribute("data-fps") == selectedFps.toString()
            } else if (option.getAttribute("data-source") != null) {
                useSourceResolution
            } else {
                option.getAttribute("data-width") == selectedWidth.toString()
            }
        if (isDefault) {
            option.classList.add("btn-primary")
        } else {
            option.classList.remove("btn-primary")
        }
    }
}

private fun setupStopScreenShareButtonHandler(onStopScreenShare: () -> Unit) {
    Elements.stopScreenShareButton.addEventListener("click", { e ->
        e.preventDefault()
        onStopScreenShare()
    })
}

private fun setupCameraButtons(
    onStartCameraShare: () -> Unit,
    onStopCameraShare: () -> Unit,
) {
    Elements.cameraButton.addEventListener("click", { e ->
        e.preventDefault()
        onStartCameraShare()
    })

    Elements.stopCameraButton.addEventListener("click", { e ->
        e.preventDefault()
        onStopCameraShare()
    })
}

private fun setupDeviceHandlers(
    onInputDeviceChange: (deviceId: String) -> Unit,
    onOutputDeviceChange: (deviceId: String) -> Unit,
    onCameraDeviceChange: (deviceId: String) -> Unit,
) {
    Elements.inputDevices.addEventListener("change", {
        onInputDeviceChange(Elements.inputDevices.value)
    })

    Elements.outputDevices.addEventListener("change", {
        window.localStorage.setItem("screenshare.outputDeviceId", Elements.outputDevices.value)
        onOutputDeviceChange(Elements.outputDevices.value)
    })

    Elements.cameraDevices.addEventListener("change", {
        onCameraDeviceChange(Elements.cameraDevices.value)
    })
}
