package ui

import generateRandomRoomId
import kotlinx.browser.document
import kotlinx.browser.window
import org.w3c.dom.HTMLElement

private const val DEFAULT_WIDTH = 1920
private const val DEFAULT_HEIGHT = 1080
private const val DEFAULT_FPS = 30

private var selectedWidth = DEFAULT_WIDTH
private var selectedHeight = DEFAULT_HEIGHT
private var selectedFps = DEFAULT_FPS
private var useSourceResolution = false

fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
    onStartScreenShare: (width: Int, height: Int, fps: Int, useSourceResolution: Boolean) -> Unit,
    onStopScreenShare: () -> Unit,
    onInputDeviceChange: (deviceId: String) -> Unit,
    onOutputDeviceChange: (deviceId: String) -> Unit,
) {
    println("Registering UI handlers")
    setupJoinButtonHandler(joinRoom)
    setupSendMessageButtonHandler(sendChatMessage)
    setupMicToggleButtonHandler(onMicButtonToggle)
    setupShareQualityModal(onStartScreenShare)
    setupStopScreenShareButtonHandler(onStopScreenShare)
    setupFullScreenButtonHandler()
    setupScreenVolumeHandler()
    setupDeviceHandlers(onInputDeviceChange, onOutputDeviceChange)
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
            window.alert("Please enter a username.")
            return@addEventListener
        }

        Elements.currentRoomId.textContent = roomId

        println("Joining room '$roomId' as user '$username'")
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

private fun setupShareQualityModal(onStartScreenShare: (width: Int, height: Int, fps: Int, useSourceResolution: Boolean) -> Unit) {
    Elements.shareScreenButton.addEventListener("click", { e ->
        e.preventDefault()
        resetQualitySelection()
        Elements.qualityModal.classList.remove("hidden")
    })

    Elements.confirmShare.addEventListener("click", { e ->
        e.preventDefault()
        Elements.qualityModal.classList.add("hidden")
        onStartScreenShare(selectedWidth, selectedHeight, selectedFps, useSourceResolution)
    })

    Elements.cancelShare.addEventListener("click", { e ->
        e.preventDefault()
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

private fun setupFullScreenButtonHandler() {
    Elements.fullScreenButton.addEventListener("click", { e ->
        e.preventDefault()
        Elements.screenVideo.requestFullscreen()
    })
}

private fun setupScreenVolumeHandler() {
    Elements.screenVolume.addEventListener("input", {
        val volume = Elements.screenVolume.value.toInt() / 100.0
        Elements.screenVideo.volume = volume
    })
}

private fun setupDeviceHandlers(
    onInputDeviceChange: (deviceId: String) -> Unit,
    onOutputDeviceChange: (deviceId: String) -> Unit,
) {
    Elements.inputDevices.addEventListener("change", {
        onInputDeviceChange(Elements.inputDevices.value)
    })

    Elements.outputDevices.addEventListener("change", {
        onOutputDeviceChange(Elements.outputDevices.value)
    })
}
