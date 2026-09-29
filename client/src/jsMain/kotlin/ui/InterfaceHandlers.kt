package ui

import generateRandomRoomId
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLSelectElement
import platform.DesktopCaptureBridge
import platform.DesktopCaptureSource
import platform.DesktopCaptureSourceKind
import services.ScreenSharing

private const val DEFAULT_WIDTH = 1920
private const val DEFAULT_HEIGHT = 1080
private const val DEFAULT_FPS = 30

private var selectedWidth = DEFAULT_WIDTH
private var selectedHeight = DEFAULT_HEIGHT
private var selectedFps = DEFAULT_FPS
private var useSourceResolution = false
private var selectedAudioMode = ScreenSharing.DesktopAudioMode.NONE
private var selectedAudioSource: DesktopCaptureSource? = null
private val desktopAudioPickerScope = CoroutineScope(Dispatchers.Main)
private var desktopAudioRefreshTimer: Int? = null
private var desktopAudioRefreshGeneration = 0
private var desktopAudioRefreshInFlight = false
private var renderedAudioSourcesSignature = ""

fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
    onStartScreenShare: (
        width: Int,
        height: Int,
        fps: Int,
        useSourceResolution: Boolean,
        audioMode: ScreenSharing.DesktopAudioMode,
        audioSourceProcessId: Int?,
    ) -> Unit,
    onChangeDesktopAudio: () -> Unit,
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
    setupChangeDesktopAudioButton(onChangeDesktopAudio)
    setupStopScreenShareButtonHandler(onStopScreenShare)
    setupCameraButtons(onStartCameraShare, onStopCameraShare)
    setupDeviceHandlers(onInputDeviceChange, onOutputDeviceChange, onCameraDeviceChange)
    Elements.outputToggle.addEventListener("click", { InterfaceMutations.toggleOutputMute() })
}

private fun setupJoinButtonHandler(joinRoom: (username: String, roomId: String) -> Unit) =
    Elements.joinButton.addEventListener("click", { e ->
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

        joinRoom(username, roomId)
    })

private fun setupSendMessageButtonHandler(sendChatMessage: (message: String) -> Unit) =
    Elements.sendMessageButton.addEventListener("click", { e ->
        e.preventDefault()
        val message = Elements.messageInput.value.trim()

        if (message.isNotBlank()) {
            sendChatMessage(message)
            Elements.messageInput.value = ""
        }
    })

private fun setupMicToggleButtonHandler(onMicButtonToggle: () -> Unit) =
    Elements.micToggle.addEventListener("click", { e ->
        e.preventDefault()
        onMicButtonToggle()
    })

private fun setupShareQualityModal(
    onStartScreenShare: (Int, Int, Int, Boolean, ScreenSharing.DesktopAudioMode, Int?) -> Unit,
) {
    Elements.shareScreenButton.addEventListener("click", { e ->
        e.preventDefault()
        resetQualitySelection()
        // Browsers never load the bridge or enumerate local windows. Their
        // direct, standard getDisplayMedia flow remains intentionally simple.
        if (!DesktopCaptureBridge.isAvailable) {
            onStartScreenShare(selectedWidth, selectedHeight, selectedFps, useSourceResolution, ScreenSharing.DesktopAudioMode.NONE, null)
            return@addEventListener
        }
        Elements.qualityModal.classList.remove("hidden")
        resetDesktopAudioWizard()
    })

    Elements.confirmShare.addEventListener("click", { e ->
        e.preventDefault()
        if (selectedAudioMode == ScreenSharing.DesktopAudioMode.PROCESS && selectedAudioSource == null) {
            InterfaceMutations.showError("Escolha um aplicativo para o áudio antes de continuar.")
            return@addEventListener
        }
        stopAudioSourceRefresh()
        Elements.qualityModal.classList.add("hidden")
        val source = selectedAudioSource
        onStartScreenShare(
            selectedWidth,
            selectedHeight,
            selectedFps,
            useSourceResolution,
            selectedAudioMode,
            source?.processId,
        )
    })

    Elements.cancelShare.addEventListener("click", { e ->
        e.preventDefault()
        stopAudioSourceRefresh()
        Elements.qualityModal.classList.add("hidden")
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
            } else {
                useSourceResolution = false
                option.getAttribute("data-width")?.toIntOrNull()?.let { selectedWidth = it }
                option.getAttribute("data-height")?.toIntOrNull()?.let { selectedHeight = it }
            }
            option.getAttribute("data-fps")?.toIntOrNull()?.let { selectedFps = it }
        })
    }
}

private fun resetDesktopAudioWizard() {
    stopAudioSourceRefresh()
    selectedAudioMode = ScreenSharing.DesktopAudioMode.NONE
    selectedAudioSource = null
    Elements.confirmShare.disabled = false
    Elements.desktopWizardStepOne.classList.remove("hidden")
    Elements.desktopWizardStepTwo.classList.add("hidden")
    Elements.desktopAudioThumbnail.classList.add("hidden")
    Elements.desktopAudioPickerState.textContent = ""
    bindAudioMode("none", ScreenSharing.DesktopAudioMode.NONE)
    bindAudioMode("system", ScreenSharing.DesktopAudioMode.SYSTEM)
    bindAudioMode("process", ScreenSharing.DesktopAudioMode.PROCESS)
    (document.getElementById("desktop-audio-mode-none") as HTMLElement).classList.add("selected")
    Elements.desktopBackToAudioModes.onclick = {
        stopAudioSourceRefresh()
        Elements.desktopWizardStepTwo.classList.add("hidden")
        Elements.desktopWizardStepOne.classList.remove("hidden")
        null
    }
    Elements.desktopAudioRefresh.onclick = {
        refreshAudioSources(forceRender = true)
        null
    }
}

private fun bindAudioMode(id: String, mode: ScreenSharing.DesktopAudioMode) {
    val button = document.getElementById("desktop-audio-mode-$id") as org.w3c.dom.HTMLButtonElement
    button.onclick = {
        selectedAudioMode = mode
        selectedAudioSource = null
        val options = document.getElementsByClassName("audio-mode-card")
        for (index in 0 until options.length) (options.item(index) as HTMLElement).classList.remove("selected")
        button.classList.add("selected")
        if (mode == ScreenSharing.DesktopAudioMode.PROCESS) openAudioSourceStep() else {
            Elements.confirmShare.disabled = false
            stopAudioSourceRefresh()
            Elements.desktopWizardStepOne.classList.remove("hidden")
            Elements.desktopWizardStepTwo.classList.add("hidden")
        }
        null
    }
}

private fun openAudioSourceStep() {
    Elements.desktopWizardStepOne.classList.add("hidden")
    Elements.desktopWizardStepTwo.classList.remove("hidden")
    Elements.desktopAudioSourceGrid.innerHTML = ""
    Elements.desktopAudioThumbnail.classList.add("hidden")
    Elements.desktopAudioPickerState.textContent = "Carregando aplicativos…"
    Elements.confirmShare.disabled = true
    renderedAudioSourcesSignature = ""
    val generation = ++desktopAudioRefreshGeneration
    refreshAudioSources(generation = generation)
    desktopAudioRefreshTimer = window.setInterval({ refreshAudioSources(generation = generation) }, 2_000)
}

private fun stopAudioSourceRefresh() {
    desktopAudioRefreshTimer?.let { window.clearInterval(it) }
    desktopAudioRefreshTimer = null
    desktopAudioRefreshInFlight = false
    desktopAudioRefreshGeneration++
}

private fun refreshAudioSources(
    forceRender: Boolean = false,
    generation: Int = desktopAudioRefreshGeneration,
) {
    if (desktopAudioRefreshInFlight || generation != desktopAudioRefreshGeneration) return
    desktopAudioRefreshInFlight = true
    desktopAudioPickerScope.launch {
        runCatching { DesktopCaptureBridge.enumerateSources() }.onSuccess { sources ->
            if (generation != desktopAudioRefreshGeneration) return@onSuccess
            val windows = sources
                .filter { it.kind == DesktopCaptureSourceKind.WINDOW }
                .sortedBy { it.title.lowercase() }
            val signature = windows.joinToString("|") { "${it.id}:${it.processId}:${it.title}" }
            if (forceRender || signature != renderedAudioSourcesSignature) {
                renderedAudioSourcesSignature = signature
                renderAudioSources(windows)
            }
            Elements.desktopAudioPickerState.textContent = when {
                windows.isEmpty() -> "Nenhum aplicativo disponível agora. A lista será atualizada automaticamente."
                selectedAudioSource != null -> "Aplicativo escolhido. A lista continua sendo atualizada automaticamente."
                else -> "Escolha o aplicativo do qual você quer enviar o áudio."
            }
        }.onFailure {
            if (generation == desktopAudioRefreshGeneration) {
                Elements.desktopAudioPickerState.textContent = "Não foi possível atualizar os aplicativos. Tente novamente."
            }
        }
        if (generation == desktopAudioRefreshGeneration) desktopAudioRefreshInFlight = false
    }
}

private fun renderAudioSources(sources: List<DesktopCaptureSource>) {
    val selectedId = selectedAudioSource?.id
    val selectedProcessId = selectedAudioSource?.processId
    Elements.desktopAudioSourceGrid.innerHTML = ""
    sources.forEach { source -> Elements.desktopAudioSourceGrid.appendChild(createAudioSourceTile(source)) }
    selectedAudioSource = sources.firstOrNull { it.id == selectedId && it.processId == selectedProcessId }
    Elements.confirmShare.disabled = selectedAudioSource == null
    if (selectedId != null && selectedAudioSource == null) {
        Elements.desktopAudioThumbnail.classList.add("hidden")
    }
}

private fun createAudioSourceTile(source: DesktopCaptureSource): HTMLElement {
    val button = document.createElement("button") as org.w3c.dom.HTMLButtonElement
    button.type = "button"
    button.className = "audio-source-tile"
    if (selectedAudioSource?.let { it.id == source.id && it.processId == source.processId } == true) {
        button.classList.add("selected")
    }
    button.setAttribute("aria-label", "Usar ${source.title} como fonte de áudio")
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
        selectedAudioSource = source
        Elements.confirmShare.disabled = false
        val tiles = Elements.desktopAudioSourceGrid.getElementsByClassName("audio-source-tile")
        for (index in 0 until tiles.length) (tiles.item(index) as HTMLElement).classList.remove("selected")
        button.classList.add("selected")
        Elements.desktopAudioPickerState.textContent = "Aplicativo escolhido. O vídeo continuará sendo escolhido pelo Windows."
        loadAudioSourcePreview(source)
        null
    }
    return button
}

private fun loadAudioSourcePreview(source: DesktopCaptureSource) = CoroutineScope(Dispatchers.Main).launch {
    val thumbnail = DesktopCaptureBridge.captureThumbnail(source.hwnd) ?: return@launch
    Elements.desktopAudioThumbnail.innerHTML = ""
    Elements.desktopAudioThumbnail.appendChild(thumbnailCanvas(thumbnail))
    Elements.desktopAudioThumbnail.classList.remove("hidden")
}

private fun loadThumbnailInto(source: DesktopCaptureSource, container: HTMLElement) = desktopAudioPickerScope.launch {
    val thumbnail = DesktopCaptureBridge.captureThumbnail(source.hwnd)
    if (thumbnail == null) {
        if (container.isConnected) container.textContent = "Preview indisponível"
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
    val pixels: dynamic = js("new Uint8ClampedArray(rgba)")
    val imageData: dynamic = js("new ImageData(pixels, thumbnail.width, thumbnail.height)")
    canvas.getContext("2d").putImageData(imageData, 0, 0)
    return canvas as HTMLElement
}

private fun setupChangeDesktopAudioButton(onChangeDesktopAudio: () -> Unit) =
    Elements.changeDesktopAudioButton.addEventListener("click", { event ->
        event.preventDefault()
        onChangeDesktopAudio()
    })

private fun resetQualitySelection() {
    selectedWidth = DEFAULT_WIDTH
    selectedHeight = DEFAULT_HEIGHT
    selectedFps = DEFAULT_FPS
    useSourceResolution = false

    val options = document.getElementsByClassName("quality-option")
    for (i in 0 until options.length) {
        val option = options.item(i) as HTMLElement
        val isDefault = option.getAttribute("data-width") == DEFAULT_WIDTH.toString() ||
            option.getAttribute("data-fps") == DEFAULT_FPS.toString()
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
        onOutputDeviceChange(Elements.outputDevices.value)
    })

    Elements.cameraDevices.addEventListener("change", {
        onCameraDeviceChange(Elements.cameraDevices.value)
    })
}
