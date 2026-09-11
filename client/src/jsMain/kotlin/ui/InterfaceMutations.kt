package ui

import getUsernameInitials
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLDivElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLOptionElement
import org.w3c.dom.HTMLParagraphElement
import org.w3c.dom.HTMLSpanElement
import org.w3c.dom.HTMLVideoElement
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.url.URL
import screenshare.common.ChatMessage
import ui.mutations.UserListMutations
import kotlin.js.Date
import platform.DesktopCaptureBridge

object InterfaceMutations {
    var selectedOutputDeviceId: String? = null
    private var focusedTileId: String? = null
    val screenVolumes: MutableMap<String, Int> = mutableMapOf()
    private val screenLastAudibleVolumes: MutableMap<String, Int> = mutableMapOf()
    fun navigateToRoomScreen(
        roomId: String,
        username: String,
    ) {
        Elements.joinScreen.classList.add("hidden")
        Elements.appScreen.classList.remove("hidden")

        val actualUrl = URL(window.location.href)
        val newUrl = "${actualUrl.origin}/?roomId=$roomId"
        window.history.pushState(null, "", newUrl)
    }

    fun addMessageToChat(
        message: ChatMessage,
        localUsername: String,
    ) {
        val messageElement = document.createElement("div") as HTMLDivElement

        val formattedDate =
            Date(message.timestamp)
                .toLocaleTimeString("pt-BR", js("({ hour: '2-digit', minute: '2-digit' })"))

        val isCurrentUser = message.username == localUsername

        val initials = message.username.getUsernameInitials()

        messageElement.className = "animate-fade-in"

        val wrapper = document.createElement("div") as HTMLDivElement
        wrapper.className = "chat ${if (isCurrentUser) "chat-end" else "chat-start"}"

        val avatarDiv = document.createElement("div") as HTMLDivElement
        avatarDiv.className = "chat-image avatar placeholder"
        avatarDiv.innerHTML =
            """
            <div class="w-10 rounded-full ${if (isCurrentUser) "bg-primary text-primary-content" else "bg-base-300 text-base-content"}">
                <span class="text-xs font-semibold">$initials</span>
            </div>
            """.trimIndent()
        wrapper.appendChild(avatarDiv)

        val headerDiv = document.createElement("div") as HTMLDivElement
        headerDiv.className = "chat-header"
        headerDiv.innerHTML =
            """
            ${if (isCurrentUser) "Você" else message.username}
            <time class="text-xs opacity-50 ml-2">$formattedDate</time>
            """.trimIndent()
        wrapper.appendChild(headerDiv)

        val bubbleDiv = document.createElement("div") as HTMLDivElement
        bubbleDiv.className = "chat-bubble ${if (isCurrentUser) "chat-bubble-primary" else "bg-base-300"}"

        val paragraph = document.createElement("p") as HTMLParagraphElement
        paragraph.textContent = message.content
        bubbleDiv.appendChild(paragraph)

        wrapper.appendChild(bubbleDiv)

        messageElement.appendChild(wrapper)
        Elements.chatMessages.appendChild(messageElement)

        messageElement.scrollIntoView(js("{ behavior: 'smooth', block: 'end' }"))
    }

    fun addOrUpdateScreenTile(
        tileId: String,
        stream: MediaStream,
        username: String,
        isLocal: Boolean,
    ) {
        val existingTile = document.getElementById("screen-tile-$tileId")
        if (existingTile != null) {
            val video = document.getElementById("screen-tile-video-$tileId") as? HTMLVideoElement
            if (video != null && video.srcObject.asDynamic().id != stream.id) {
                video.srcObject = stream
            }
            val name = document.getElementById("screen-name-$tileId") as? HTMLSpanElement
            name?.textContent = username
        } else {
            val tile = createScreenTile(tileId, stream, username, isLocal)
            Elements.screensMain.appendChild(tile)
        }
        reflowScreens()
    }

    private fun createScreenTile(
        tileId: String,
        stream: MediaStream,
        username: String,
        isLocal: Boolean,
    ): HTMLElement {
        val tile = document.createElement("div") as HTMLElement
        tile.id = "screen-tile-$tileId"
        tile.className = "screen-tile"
        tile.setAttribute("data-tile-id", tileId)

        val video = document.createElement("video") as HTMLVideoElement
        video.id = "screen-tile-video-$tileId"
        video.autoplay = true
        video.muted = isLocal
        video.setAttribute("playsinline", "")
        video.srcObject = stream
        video.addEventListener("pause", { video.play() })
        video.addEventListener("click", { e -> e.preventDefault() })
        tile.appendChild(video)

        val top = document.createElement("div") as HTMLElement
        top.className = "screen-tile-top"

        val badge = document.createElement("span") as HTMLSpanElement
        badge.className = "screen-tile-badge"
        badge.textContent = "LIVE"

        val name = document.createElement("span") as HTMLSpanElement
        name.id = "screen-name-$tileId"
        name.className = "screen-tile-name"
        name.textContent = username

        top.appendChild(badge)
        top.appendChild(name)
        tile.appendChild(top)

        val controls = document.createElement("div") as HTMLElement
        controls.className = "screen-tile-controls"

        val fullscreenButton = document.createElement("button") as HTMLButtonElement
        fullscreenButton.type = "button"
        fullscreenButton.className = "glass-chip"
        fullscreenButton.title = "Tela cheia"
        fullscreenButton.setAttribute("aria-label", "Tela cheia")
        fullscreenButton.innerHTML =
            """
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"/>
            </svg>
            """.trimIndent()
        fullscreenButton.addEventListener("click", { e ->
            e.preventDefault()
            e.stopPropagation()
            if (document.fullscreenElement == tile) {
                document.exitFullscreen()
            } else {
                tile.requestFullscreen()
            }
        })
        controls.appendChild(fullscreenButton)

        val volumeChip = document.createElement("div") as HTMLElement
        volumeChip.className = "glass-chip"
        volumeChip.addEventListener("click", { e -> e.stopPropagation() })

        val volumeIcon = document.createElement("button") as HTMLButtonElement
        volumeIcon.type = "button"
        volumeIcon.className = "screen-volume-toggle"

        val volumeSlider = document.createElement("input") as HTMLInputElement
        volumeSlider.type = "range"
        volumeSlider.min = "0"
        volumeSlider.max = "100"
        val initialVolume = screenVolumes[tileId] ?: 100
        volumeSlider.value = initialVolume.toString()
        volumeSlider.className = "range range-primary range-xs"
        volumeSlider.setAttribute("aria-label", "Volume da tela")

        fun updateVolumeIcon(volume: Int) {
            val muted = volume == 0
            volumeIcon.title = if (muted) "Ativar áudio da tela" else "Silenciar áudio da tela"
            volumeIcon.setAttribute("aria-label", volumeIcon.title)
            volumeIcon.setAttribute("aria-pressed", muted.toString())
            volumeIcon.innerHTML =
                if (muted) {
                    """
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5L6 9H2v6h4l5 4V5z"/>
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 9l5 5m0-5l-5 5"/>
                    </svg>
                    """.trimIndent()
                } else {
                    """
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5L6 9H2v6h4l5 4V5z"/>
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.5 8.5a5 5 0 010 7"/>
                    </svg>
                    """.trimIndent()
                }
        }

        fun applyScreenVolume(volume: Int) {
            val boundedVolume = volume.coerceIn(0, 100)
            if (boundedVolume > 0) screenLastAudibleVolumes[tileId] = boundedVolume
            screenVolumes[tileId] = boundedVolume
            volumeSlider.value = boundedVolume.toString()
            video.volume = boundedVolume / 100.0
            updateVolumeIcon(boundedVolume)
        }

        volumeSlider.addEventListener("input", {
            applyScreenVolume(volumeSlider.value.toInt())
        })
        volumeIcon.addEventListener("click", { event ->
            event.preventDefault()
            event.stopPropagation()
            val currentVolume = volumeSlider.value.toInt()
            val nextVolume = if (currentVolume == 0) {
                screenLastAudibleVolumes[tileId]?.takeIf { it > 0 } ?: 100
            } else {
                screenLastAudibleVolumes[tileId] = currentVolume
                0
            }
            applyScreenVolume(nextVolume)
        })
        applyScreenVolume(initialVolume)

        volumeChip.appendChild(volumeIcon)
        volumeChip.appendChild(volumeSlider)
        controls.appendChild(volumeChip)

        tile.appendChild(controls)

        tile.addEventListener("click", {
            setFocusedTile(tileId)
        })

        return tile
    }

    fun removeScreenTile(tileId: String) {
        val tile = document.getElementById("screen-tile-$tileId")
        if (tile != null) {
            tile.parentElement?.removeChild(tile)
        }
        screenVolumes.remove(tileId)
        screenLastAudibleVolumes.remove(tileId)
        if (focusedTileId == tileId) focusedTileId = null
        reflowScreens()
    }

    fun setFocusedTile(tileId: String) {
        focusedTileId = if (focusedTileId == tileId) null else tileId
        reflowScreens()
    }

    fun reflowScreens() {
        val tiles = Elements.screensStage.querySelectorAll(".screen-tile")
        val tileCount = tiles.length

        if (tileCount == 0) {
            focusedTileId = null
            Elements.screensStage.classList.add("hidden")
            Elements.noScreenMessage.classList.remove("hidden")
            return
        }

        Elements.screensStage.classList.remove("hidden")
        Elements.noScreenMessage.classList.add("hidden")

        val focusId = focusedTileId?.takeIf { id -> document.getElementById("screen-tile-$id") != null }

        if (focusId == null) {
            Elements.screensStage.classList.remove("focus-mode")
            Elements.screensRail.classList.add("hidden")
            for (i in 0 until tiles.length) {
                val tile = tiles.item(i) as HTMLElement
                tile.classList.remove("focused")
                if (tile.parentElement != Elements.screensMain) {
                    Elements.screensMain.appendChild(tile)
                }
            }
            return
        }

        Elements.screensStage.classList.add("focus-mode")

        val focusedTile = document.getElementById("screen-tile-$focusId") as HTMLElement
        for (i in 0 until tiles.length) {
            val tile = tiles.item(i) as HTMLElement
            if (tile != focusedTile) {
                tile.classList.remove("focused")
                if (tile.parentElement != Elements.screensRail) {
                    Elements.screensRail.appendChild(tile)
                }
            } else {
                tile.classList.add("focused")
                if (tile.parentElement != Elements.screensMain) {
                    Elements.screensMain.appendChild(tile)
                }
            }
        }

        if (Elements.screensRail.childElementCount > 0) {
            Elements.screensRail.classList.remove("hidden")
        } else {
            Elements.screensRail.classList.add("hidden")
        }
    }

    fun updateShareControls(isLocalSharing: Boolean) {
        if (isLocalSharing) {
            Elements.stopScreenShareButton.classList.remove("hidden")
            Elements.shareScreenButton.classList.add("hidden")
            if (DesktopCaptureBridge.isAvailable) {
                Elements.changeDesktopAudioButton.classList.remove("hidden")
            }
        } else {
            Elements.stopScreenShareButton.classList.add("hidden")
            Elements.shareScreenButton.classList.remove("hidden")
            Elements.changeDesktopAudioButton.classList.add("hidden")
        }
    }

    fun reopenDesktopShareWizard() {
        if (DesktopCaptureBridge.isAvailable) Elements.qualityModal.classList.remove("hidden")
    }

    fun updateCameraControls(isLocalCameraOn: Boolean) {
        if (isLocalCameraOn) {
            Elements.stopCameraButton.classList.remove("hidden")
            Elements.cameraButton.classList.add("hidden")
        } else {
            Elements.stopCameraButton.classList.add("hidden")
            Elements.cameraButton.classList.remove("hidden")
        }
    }

    fun addOrUpdateCameraTile(
        tileId: String,
        stream: MediaStream,
        username: String,
        isLocal: Boolean,
    ) {
        val existingTile = document.getElementById("camera-tile-$tileId")
        if (existingTile != null) {
            val video = document.getElementById("camera-tile-video-$tileId") as? HTMLVideoElement
            if (video != null && video.srcObject.asDynamic().id != stream.id) {
                video.srcObject = stream
            }
            val name = document.getElementById("camera-name-$tileId") as? HTMLSpanElement
            name?.textContent = username
        } else {
            val tile = createCameraTile(tileId, stream, username, isLocal)
            Elements.screensMain.appendChild(tile)
        }
        reflowScreens()
    }

    private fun createCameraTile(
        tileId: String,
        stream: MediaStream,
        username: String,
        isLocal: Boolean,
    ): HTMLElement {
        val tile = document.createElement("div") as HTMLElement
        tile.id = "camera-tile-$tileId"
        tile.className = "screen-tile camera-tile"
        tile.setAttribute("data-tile-id", tileId)

        val video = document.createElement("video") as HTMLVideoElement
        video.id = "camera-tile-video-$tileId"
        video.autoplay = true
        video.muted = isLocal
        video.setAttribute("playsinline", "")
        video.srcObject = stream
        video.addEventListener("pause", { video.play() })
        video.addEventListener("click", { e -> e.preventDefault() })
        tile.appendChild(video)

        val top = document.createElement("div") as HTMLElement
        top.className = "screen-tile-top"

        val badge = document.createElement("span") as HTMLSpanElement
        badge.className = "screen-tile-badge camera-badge"
        badge.textContent = "CAM"

        val name = document.createElement("span") as HTMLSpanElement
        name.id = "camera-name-$tileId"
        name.className = "screen-tile-name"
        name.textContent = username

        top.appendChild(badge)
        top.appendChild(name)
        tile.appendChild(top)

        val controls = document.createElement("div") as HTMLElement
        controls.className = "screen-tile-controls"

        val fullscreenButton = document.createElement("button") as HTMLButtonElement
        fullscreenButton.type = "button"
        fullscreenButton.className = "glass-chip"
        fullscreenButton.title = "Tela cheia"
        fullscreenButton.setAttribute("aria-label", "Tela cheia")
        fullscreenButton.innerHTML =
            """
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"/>
            </svg>
            """.trimIndent()
        fullscreenButton.addEventListener("click", { e ->
            e.preventDefault()
            e.stopPropagation()
            if (document.fullscreenElement == tile) {
                document.exitFullscreen()
            } else {
                tile.requestFullscreen()
            }
        })
        controls.appendChild(fullscreenButton)
        tile.appendChild(controls)

        tile.addEventListener("click", {
            setFocusedTile(tileId)
        })

        return tile
    }

    fun removeCameraTile(tileId: String) {
        val tile = document.getElementById("camera-tile-$tileId")
        if (tile != null) {
            tile.parentElement?.removeChild(tile)
        }
        if (focusedTileId == tileId) focusedTileId = null
        reflowScreens()
    }

    fun addAudioElementForUser(
        userId: String,
        stream: MediaStream,
    ) {
        val audioElement =
            (document.createElement("audio") as HTMLAudioElement).apply {
                id = "remote-audio-$userId"
                srcObject = stream
                autoplay = true
                volume = UserListMutations.userVolumes[userId]?.div(100.0) ?: 1.0
            }
        document.body?.appendChild(audioElement)

        if (!selectedOutputDeviceId.isNullOrBlank()) {
            runCatching { audioElement.asDynamic().setSinkId(selectedOutputDeviceId) }
        }
    }

    suspend fun populateAudioDevices() {
        val previousInput = Elements.inputDevices.value
        val previousOutput = Elements.outputDevices.value

        val devices = window.navigator.mediaDevices.enumerateDevices().await()
        val inputSelect = Elements.inputDevices
        val outputSelect = Elements.outputDevices

        inputSelect.innerHTML = ""
        outputSelect.innerHTML = ""

        devices.forEach { device ->
            when (device.asDynamic().kind as String) {
                "audioinput" -> {
                    inputSelect.appendChild(
                        createDeviceOption(device.deviceId, device.label.ifBlank { "Microfone" }),
                    )
                }

                "audiooutput" -> {
                    outputSelect.appendChild(
                        createDeviceOption(device.deviceId, device.label.ifBlank { "Alto-falante" }),
                    )
                }

                else -> {}
            }
        }

        if (devices.any { it.deviceId == previousInput }) Elements.inputDevices.value = previousInput
        if (devices.any { it.deviceId == previousOutput }) Elements.outputDevices.value = previousOutput
    }

    fun setOutputDevice(deviceId: String) {
        selectedOutputDeviceId = deviceId
        applyOutputDeviceToElements(deviceId)
    }

    private fun applyOutputDeviceToElements(deviceId: String) {
        val audioElements = document.getElementsByTagName("audio")
        for (i in 0 until audioElements.length) {
            val element = audioElements.item(i) ?: continue
            if (element.id.startsWith("remote-audio-")) {
                runCatching { element.asDynamic().setSinkId(deviceId) }
            }
        }
        val screenTiles = document.getElementsByClassName("screen-tile")
        for (i in 0 until screenTiles.length) {
            val tile = screenTiles.item(i) as? HTMLElement ?: continue
            val video = tile.querySelector("video") ?: continue
            runCatching { video.asDynamic().setSinkId(deviceId) }
        }
    }

    private fun createDeviceOption(
        deviceId: String,
        label: String,
    ): HTMLOptionElement =
        (document.createElement("option") as HTMLOptionElement).apply {
            value = deviceId
            textContent = label
        }

    fun updateAudioControls(isMicMuted: Boolean) {
        val micSlash = document.getElementById("micSlash")!!
        val micStatus = document.getElementById("micStatus")!!

        if (isMicMuted) {
            micSlash.classList.remove("hidden")
            micStatus.classList.remove("badge-success")
            micStatus.classList.add("badge-error")
        } else {
            micSlash.classList.add("hidden")
            micStatus.classList.remove("badge-error")
            micStatus.classList.add("badge-success")
        }
    }
}
