package ui

import getUsernameInitials
import kotlinx.browser.document
import kotlinx.browser.window
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.HTMLDivElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLParagraphElement
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.url.URL
import screenshare.common.ChatMessage
import kotlin.js.Date

object InterfaceMutations {
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

    fun endScreenSharing() {
        Elements.screenVideo.srcObject = null

        Elements.videoContainer.classList.add("hidden")
        Elements.stopScreenShareButton.classList.add("hidden")

        Elements.noScreenMessage.classList.remove("hidden")
        Elements.shareScreenButton.classList.remove("hidden")
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
            }
        document.body?.appendChild(audioElement)
    }

    fun updateScreenContainer(
        screenStream: MediaStream,
        isInitiator: Boolean,
    ) {
        with(Elements.screenVideo) {
            if (srcObject == null || srcObject.asDynamic().id != screenStream.id) {
                srcObject = screenStream

                Elements.videoContainer.classList.remove("hidden")
                Elements.noScreenMessage.classList.add("hidden")

                if (isInitiator) {
                    Elements.stopScreenShareButton.classList.remove("hidden")
                    Elements.shareScreenButton.classList.add("hidden")
                } else {
                    Elements.stopScreenShareButton.classList.add("hidden")
                    Elements.shareScreenButton.classList.add("hidden")
                }
            }
        }
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
