package ui

import kotlinx.browser.document
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLVideoElement

object Elements {
    val joinScreen = getElement<HTMLElement>("join-screen")
    val appScreen = getElement<HTMLElement>("app-screen")
    val usernameInput = getElement<HTMLInputElement>("username")
    val roomIdInput = getElement<HTMLInputElement>("roomId")
    val joinButton = getElement<HTMLButtonElement>("joinBtn")
    val chatMessages = getElement<HTMLElement>("chat-messages")
    val currentRoomId = getElement<HTMLElement>("current-room-id")
    val screenVideo = getElement<HTMLVideoElement>("screen-video")
    val videoContainer = getElement<HTMLElement>("video-container")
    val noScreenMessage = getElement<HTMLElement>("no-screen-message")
    val userList = getElement<HTMLElement>("user-list")
    val userCount = getElement<HTMLElement>("participant-count")
    val sendMessageButton = getElement<HTMLButtonElement>("send-message-btn")
    val messageInput = getElement<HTMLInputElement>("message-input")
    val micToggle = getElement<HTMLButtonElement>("micToggle")
    val shareScreenButton = getElement<HTMLButtonElement>("shareScreenBtn")
    val stopScreenShareButton = getElement<HTMLButtonElement>("stopSharingBtn")
    val fullScreenButton = getElement<HTMLButtonElement>("fullscreenBtn")
}

private inline fun <reified T : HTMLElement> getElement(id: String): T =
    runCatching {
        document.getElementById(id) as T
    }.getOrElse {
        println("Failed to get element with id '$id' and type ${T::class.simpleName}: ${it.message}")
        throw IllegalStateException("Element with id '$id' not found or is not of type ${T::class.simpleName}")
    }
