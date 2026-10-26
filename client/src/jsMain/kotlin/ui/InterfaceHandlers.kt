package ui

import generatePrivateRoomId
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
import org.w3c.dom.events.Event
import org.w3c.dom.events.KeyboardEvent
import platform.DesktopCaptureBridge
import platform.DesktopCaptureException
import platform.DesktopCaptureSource
import platform.DesktopCaptureSourceKind
import services.ScreenSharing
import services.SharePreset
import services.SharePriority
import services.ShareProfile
import services.nativeAudioMode
import services.shareProfileFrom

private const val DEFAULT_WIDTH = 1920
private const val DEFAULT_HEIGHT = 1080
private var selectedWidth = window.localStorage.getItem("screenshare.shareWidth")?.toIntOrNull() ?: DEFAULT_WIDTH
private var selectedHeight = window.localStorage.getItem("screenshare.shareHeight")?.toIntOrNull() ?: DEFAULT_HEIGHT
private var shareProfile =
    shareProfileFrom(
        window.localStorage.getItem("screenshare.shareFps"),
        window.localStorage.getItem("screenshare.sharePriority"),
        window.localStorage.getItem("screenshare.shareBitrate"),
    )
private var useSourceResolution = window.localStorage.getItem("screenshare.sourceResolution") == "true"
private var selectedSource: DesktopCaptureSource? = null
private var shareAudio = false
private val uiScope = CoroutineScope(Dispatchers.Main)
private var sourceRefreshTimer: Int? = null
private var sourceRefreshGeneration = 0
private var sourceRefreshInFlight = false
private var renderedSourcesSignature = ""
private var shareProfileListener: (ShareProfile) -> Unit = {}

fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: suspend (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
    onStartScreenShare: (
        width: Int,
        height: Int,
        profile: ShareProfile,
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
    onShareProfileChange: (ShareProfile) -> Unit,
) {
    setupJoinButtonHandler(joinRoom)
    RecentRooms.render()
    Feedback.setup()
    setupSendMessageButtonHandler(sendChatMessage)
    setupMicToggleButtonHandler(onMicButtonToggle)
    shareProfileListener = onShareProfileChange
    setupShareQualityModal(onStartScreenShare)
    setupShareMenu()
    setupStopScreenShareButtonHandler(onStopScreenShare)
    setupCameraButtons(onStartCameraShare, onStopCameraShare)
    setupDeviceHandlers(onInputDeviceChange, onOutputDeviceChange, onCameraDeviceChange)
    Elements.outputToggle.addEventListener("click", { InterfaceMutations.toggleOutputMute() })
}

private fun setupJoinButtonHandler(joinRoom: (username: String, roomId: String) -> Unit) {
    document.getElementById("private-room-btn")?.addEventListener("click", {
        Elements.roomIdInput.value = generatePrivateRoomId()
        (document.getElementById("join-form")?.asDynamic())?.requestSubmit()
    })
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
        RecentRooms.remember(roomId)

        joinRoom(username, roomId)
    })
}

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
    onStartScreenShare: (
        Int,
        Int,
        ShareProfile,
        Boolean,
        ScreenSharing.DesktopAudioMode,
        DesktopCaptureSource?,
    ) -> Unit,
) {
    Elements.shareScreenButton.addEventListener("click", { e ->
        e.preventDefault()
        renderShareSettings()
        // Browsers never load the bridge or enumerate local windows. Their
        // direct, standard getDisplayMedia flow remains intentionally simple.
        if (!platform.DesktopEnvironment.isAvailable) {
            onStartScreenShare(
                selectedWidth,
                selectedHeight,
                shareProfile,
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
                shareProfile,
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
            shareProfile,
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

    Elements.shareResolution.onchange = {
        chooseShareResolution(Elements.shareResolution.value)
        null
    }
    Elements.shareFps.onchange = {
        setShareProfile(shareProfile.copy(fps = Elements.shareFps.value.toInt()))
        renderShareSettings()
        null
    }
    Elements.sharePriority.onchange = {
        setShareProfile(shareProfile.copy(priority = SharePriority.valueOf(Elements.sharePriority.value)))
        renderShareSettings()
        null
    }
    Elements.shareBitrate.onchange = {
        val bitrate = Elements.shareBitrate.value.toInt()
        setShareProfile(shareProfile.copy(maxBitrate = bitrate.takeIf { it > 0 }))
        renderShareSettings()
        null
    }
    val presetCards = Elements.sharePresetGrid.querySelectorAll("[data-preset]")
    for (i in 0 until presetCards.length) {
        val card = presetCards.item(i) as HTMLElement
        card.addEventListener("click", { choosePreset(SharePreset.valueOf(card.getAttribute("data-preset")!!)) })
    }
    Elements.shareQuality.asDynamic().open = window.localStorage.getItem("screenshare.advancedOpen") == "true"
    Elements.shareQuality.addEventListener("toggle", {
        val open = Elements.shareQuality.asDynamic().open as Boolean
        window.localStorage.setItem("screenshare.advancedOpen", open.toString())
    })
    Elements.shareAudioSwitch.onclick = {
        setShareAudio(!shareAudio)
        null
    }
    Elements.shareSourceRefresh.onclick = {
        refreshSources(forceRender = true)
        null
    }
}

/** Desktop (Windows, macOS): one dialog picks the source, its audio and the frame rate; no OS picker follows. */
internal fun openNativeShareWizard() {
    stopSourceRefresh()
    selectedSource = null
    renderedSourcesSignature = ""
    Elements.shareWizardDesc.textContent = "Escolha a tela ou janela e o áudio que quer enviar."
    Elements.shareSourceStep.classList.remove("hidden")
    Elements.shareAudioStep.classList.remove("hidden")
    // The native capture sends the source size, so resolution is not a choice.
    Elements.resolutionSection.classList.add("hidden")
    Elements.shareSourceGrid.innerHTML = ""
    Elements.confirmShare.textContent = "Compartilhar"
    Elements.confirmShare.disabled = true
    setShareAudio(false)
    renderShareSettings()
    Elements.shareSourceState.textContent = "Carregando telas e janelas…"
    val generation = ++sourceRefreshGeneration
    refreshSources(generation = generation)
    sourceRefreshTimer = window.setInterval({ refreshSources(generation = generation) }, 2_000)
}

private fun setShareAudio(enabled: Boolean) {
    shareAudio = enabled
    Elements.shareAudioSwitch.setAttribute("aria-checked", enabled.toString())
    val source = selectedSource
    val systemAudioDescription =
        if (window.navigator.userAgent.contains("Windows")) {
            "Sem o som do Discord, de outras chamadas (Teams, Zoom, WhatsApp…) e do próprio Screenshare."
        } else {
            "Enviar todo o som da saída padrão."
        }
    val (title, description) =
        when (source?.kind) {
            null -> "Áudio" to "Escolha uma tela ou janela."
            DesktopCaptureSourceKind.SCREEN -> "Áudio do computador" to systemAudioDescription
            DesktopCaptureSourceKind.WINDOW -> "Áudio do aplicativo" to "Enviar só o som de ${source.title}."
        }
    document.getElementById("share-audio-on-title")?.textContent = title
    document.getElementById("share-audio-on-desc")?.textContent = description
}

private fun setSourceResolution(enabled: Boolean) {
    useSourceResolution = enabled
    window.localStorage.setItem("screenshare.sourceResolution", enabled.toString())
}

/** [value] is "source" or "WIDTHxHEIGHT", as in the resolution list; the tray uses it too. */
internal fun chooseShareResolution(value: String) {
    val size = value.split("x").mapNotNull { it.toIntOrNull() }
    if (size.size == 2) {
        setSourceResolution(false)
        selectedWidth = size[0]
        selectedHeight = size[1]
        window.localStorage.setItem("screenshare.shareWidth", selectedWidth.toString())
        window.localStorage.setItem("screenshare.shareHeight", selectedHeight.toString())
    } else {
        setSourceResolution(true)
    }
    InterfaceMutations.syncTrayQuality()
    renderShareSettings()
}

private fun choosePreset(preset: SharePreset) {
    setShareProfile(preset.profile)
    // Any downscale blurs small fonts, so text starts at the source size.
    if (preset == SharePreset.TEXT && !useSourceResolution) {
        setSourceResolution(true)
        InterfaceMutations.syncTrayQuality()
    }
    renderShareSettings()
}

/** One step down from the share's frame rate (60, 30, 15, 10), or null at the floor. */
fun lighterShareFps(): Int? = listOf(30, 15, 10).firstOrNull { it < shareProfile.fps }

/** The quality chip's "Usar N qps": the next step down, applied live like the share menu. */
fun lightenShare() {
    val fps = lighterShareFps() ?: return
    setShareProfile(shareProfile.copy(fps = fps))
    renderShareSettings()
}

/** Also reaches a running share, which applies it live. */
private fun setShareProfile(profile: ShareProfile) {
    shareProfile = profile
    window.localStorage.setItem("screenshare.shareFps", profile.fps.toString())
    window.localStorage.setItem("screenshare.sharePriority", profile.priority.name)
    window.localStorage.setItem("screenshare.shareBitrate", (profile.maxBitrate ?: 0).toString())
    shareProfileListener(profile)
}

/**
 * The footer menu edits the same profile as the wizard: presets on top, and each
 * setting opens the options of its wizard list in place ("‹" goes back).
 */
private fun setupShareMenu() {
    val presets = Elements.shareMenuMain.querySelectorAll("[data-menu-preset]")
    for (i in 0 until presets.length) {
        val button = presets.item(i) as HTMLElement
        button.onclick = {
            choosePreset(SharePreset.valueOf(button.getAttribute("data-menu-preset")!!))
            null
        }
    }
    val rows = Elements.shareMenuMain.querySelectorAll("[data-menu-field]")
    for (i in 0 until rows.length) {
        val row = rows.item(i) as HTMLElement
        row.onclick = {
            openShareMenuOptions(row)
            null
        }
    }
    Elements.shareMenuBack.onclick = {
        closeShareMenuOptions()
        null
    }
    Elements.shareMenuSource.onclick = {
        Elements.sharePicker.asDynamic().open = false
        Elements.shareScreenButton.click()
        null
    }
    // Esc leaves the options first; ui.js closes the menu on the next Esc.
    Elements.shareMenuOptions.addEventListener("keydown", { event ->
        if ((event as KeyboardEvent).key == "Escape") {
            event.stopPropagation()
            closeShareMenuOptions()
        }
    })
    Elements.sharePicker.addEventListener("toggle", {
        if (Elements.sharePicker.asDynamic().open != true) closeShareMenuOptions(restoreFocus = false)
    })
    renderShareSettings()
}

private var shareMenuRow: HTMLElement? = null

private fun openShareMenuOptions(row: HTMLElement) {
    val select = document.getElementById(row.getAttribute("data-menu-field")!!) as HTMLSelectElement
    shareMenuRow = row
    Elements.shareMenuOptionsTitle.textContent = (select.labels.item(0) as? HTMLElement)?.textContent
    Elements.shareMenuList.innerHTML = ""
    for (i in 0 until select.options.length) {
        val option = select.options.item(i).asDynamic()
        val button = document.createElement("button") as HTMLElement
        button.setAttribute("type", "button")
        button.className = "share-menu-row"
        button.setAttribute("role", "radio")
        button.setAttribute("aria-checked", (option.value == select.value).toString())
        button.textContent = option.text as String
        button.onclick = {
            select.value = option.value as String
            // The wizard list's own handler updates the profile and every view.
            select.dispatchEvent(Event("change"))
            closeShareMenuOptions()
            null
        }
        Elements.shareMenuList.appendChild(button)
    }
    Elements.shareMenuMain.classList.add("hidden")
    Elements.shareMenuOptions.classList.remove("hidden")
    (Elements.shareMenuList.querySelector("[aria-checked=true]") as? HTMLElement)?.focus()
}

private fun closeShareMenuOptions(restoreFocus: Boolean = true) {
    Elements.shareMenuOptions.classList.add("hidden")
    Elements.shareMenuMain.classList.remove("hidden")
    if (restoreFocus) shareMenuRow?.focus()
}

/** Changes in the menu apply to the running share, or to the next one. */
internal fun setShareMenuLive(live: Boolean) {
    Elements.shareMenuState.textContent = if (live) "Ao vivo" else "Próxima tela"
    Elements.shareMenuState.classList.toggle("live", live)
    Elements.shareMenuSource.textContent = if (live) "Trocar tela ou janela…" else "Escolher tela ou janela…"
}

private val SharePriority.label
    get() =
        when (this) {
            SharePriority.SHARPNESS -> "Nitidez"
            SharePriority.BALANCED -> "Equilíbrio"
            SharePriority.SMOOTHNESS -> "Fluidez"
        }

/** One pass from state to every control, so cards, lists and summary never disagree. */
private fun renderShareSettings() {
    val preset = SharePreset.matching(shareProfile)
    val cards = Elements.sharePresetGrid.querySelectorAll("[data-preset]")
    for (i in 0 until cards.length) {
        val card = cards.item(i) as HTMLElement
        val selected = card.getAttribute("data-preset") == preset?.name
        card.classList.toggle("selected", selected)
        card.setAttribute("aria-pressed", selected.toString())
    }

    Elements.shareResolution.value = if (useSourceResolution) "source" else "${selectedWidth}x$selectedHeight"
    Elements.shareFps.value = "${shareProfile.fps}"
    Elements.sharePriority.value = shareProfile.priority.name
    Elements.shareBitrate.value = "${shareProfile.maxBitrate ?: 0}"
    Elements.shareBitrateHint.classList.toggle("hidden", shareProfile.maxBitrate == null)

    val resolution =
        when {
            DesktopCaptureBridge.isAvailable -> null
            useSourceResolution -> "Fonte"
            selectedHeight >= 2160 -> "4K"
            else -> "${selectedHeight}p"
        }
    Elements.shareQualitySummary.textContent =
        listOfNotNull(
            "Personalizado".takeIf { preset == null },
            resolution,
            "${shareProfile.fps} fps",
            shareProfile.priority.label,
            shareProfile.maxBitrate?.let { "${it / 1_000_000} Mbps" } ?: "Bitrate auto",
        ).joinToString(" · ")
    updateSourceState()

    val menuPresets = Elements.shareMenuMain.querySelectorAll("[data-menu-preset]")
    for (i in 0 until menuPresets.length) {
        val button = menuPresets.item(i) as HTMLElement
        button.setAttribute("aria-pressed", (button.getAttribute("data-menu-preset") == preset?.name).toString())
    }
    Elements.shareMenuCustom.classList.toggle("hidden", preset != null)
    val rows = Elements.shareMenuMain.querySelectorAll("[data-menu-field]")
    for (i in 0 until rows.length) {
        val row = rows.item(i) as HTMLElement
        val select = document.getElementById(row.getAttribute("data-menu-field")!!) as HTMLSelectElement
        val selected = select.options.item(select.selectedIndex).asDynamic()
        row.querySelector(".share-menu-value")?.textContent = selected?.text as? String
    }
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
            }.onFailure { error ->
                if (generation == sourceRefreshGeneration) {
                    val permissionError =
                        (error as? DesktopCaptureException)?.takeIf { it.code == "screen-recording-denied" }
                    Elements.shareSourceState.textContent =
                        permissionError?.message
                            ?: "Não foi possível atualizar as telas e janelas. Tente novamente."
                    // Polling ScreenCaptureKit without permission opens another system prompt on every tick.
                    if (permissionError != null) stopSourceRefresh()
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

    // The same device again reopens a live microphone with the new processing.
    (document.getElementById("noise-suppression") as? org.w3c.dom.HTMLInputElement)?.let { toggle ->
        toggle.checked = services.noiseSuppressionChosen()
        toggle.addEventListener("change", {
            runCatching { window.localStorage.setItem(services.NOISE_SUPPRESSION_KEY, toggle.checked.toString()) }
            onInputDeviceChange(Elements.inputDevices.value)
        })
    }

    Elements.outputDevices.addEventListener("change", {
        window.localStorage.setItem("screenshare.outputDeviceId", Elements.outputDevices.value)
        onOutputDeviceChange(Elements.outputDevices.value)
    })

    (kotlinx.browser.document.getElementById("room-sounds") as? org.w3c.dom.HTMLInputElement)?.let { toggle ->
        toggle.checked = RoomSounds.enabled
        toggle.addEventListener("change", { RoomSounds.setEnabled(toggle.checked) })
    }

    // Only where usage is sent at all (the hosted site); errors go either way.
    (document.getElementById("usage-metrics") as? org.w3c.dom.HTMLInputElement)?.let { toggle ->
        document.getElementById("usage-metrics-option")?.classList?.toggle("hidden", !platform.Telemetry.enabled)
        toggle.checked = platform.usageMetricsOn()
        toggle.addEventListener("change", { platform.setUsageMetricsOn(toggle.checked) })
    }

    Elements.cameraDevices.addEventListener("change", {
        onCameraDeviceChange(Elements.cameraDevices.value)
    })
}
