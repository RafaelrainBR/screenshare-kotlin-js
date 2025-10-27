package ui

import generateRandomRoomId
import kotlinx.browser.window
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.events.Event

fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
) {
    setupJoinButtonHandler(joinRoom)
    setupSendMessageButtonHandler(sendChatMessage)
    setupMicToggleButtonHandler(onMicButtonToggle)
    setupEnterKeyHandlers()
}

private fun setupJoinButtonHandler(joinRoom: (username: String, roomId: String) -> Unit) =
    Elements.joinButton.addEventListener("click", {
        val username = Elements.usernameInput.value.trim()
        val roomId =
            Elements.roomIdInput.value.trim()
                .takeIf { it.isNotBlank() }
                ?: generateRandomRoomId()

        if (username.isBlank()) {
            window.alert("Please enter a username.")
            return@addEventListener
        }

        joinRoom(username, roomId)
    })

private fun setupSendMessageButtonHandler(sendChatMessage: (message: String) -> Unit) =
    Elements.sendMessageButton.addEventListener("click", {
        val message = Elements.messageInput.value.trim()

        if (message.isNotBlank()) {
            sendChatMessage(message)
            Elements.messageInput.value = ""
        }
    })

private fun setupMicToggleButtonHandler(onMicButtonToggle: () -> Unit) =
    Elements.micToggle.addEventListener("click", {
        onMicButtonToggle()
    })

private fun setupEnterKeyHandlers() {
    setupEnterKeyHandler(Elements.messageInput, Elements.sendMessageButton)
    setupEnterKeyHandler(Elements.usernameInput, Elements.joinButton)
    setupEnterKeyHandler(Elements.roomIdInput, Elements.joinButton)
}

private fun setupEnterKeyHandler(
    inputElement: HTMLInputElement,
    buttonElement: HTMLButtonElement,
) = inputElement.addEventListener("keypress", { event ->
    if (event.isEnterKey()) {
        buttonElement.click()
    }
})

private fun Event.isEnterKey(): Boolean {
    return this.asDynamic().key.toString().equals("enter", ignoreCase = true)
}
