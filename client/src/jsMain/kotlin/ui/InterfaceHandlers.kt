package ui

import generateRandomRoomId
import kotlinx.browser.window

fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
    onStartScreenShare: () -> Unit,
    onStopScreenShare: () -> Unit,
) {
    println("Registering UI handlers")
    setupJoinButtonHandler(joinRoom)
    setupSendMessageButtonHandler(sendChatMessage)
    setupMicToggleButtonHandler(onMicButtonToggle)
    setupScreenShareButtonHandlers(onStartScreenShare, onStopScreenShare)
    setupFullScreenButtonHandler()
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

private fun setupScreenShareButtonHandlers(
    onStartScreenShare: () -> Unit,
    onStopScreenShare: () -> Unit,
) {
    Elements.shareScreenButton.addEventListener("click", { e ->
        e.preventDefault()
        onStartScreenShare()
    })

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
