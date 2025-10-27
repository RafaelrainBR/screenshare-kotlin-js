package ui

import getUsernameInitials
import kotlinx.browser.document
import kotlinx.browser.window
import org.w3c.dom.DocumentFragment
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.HTMLDivElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLParagraphElement
import org.w3c.dom.asList
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.url.URL
import screenshare.common.ChatMessage
import screenshare.common.SocketUser
import kotlin.js.Date

object InterfaceMutations {
    fun updateUserList(
        users: List<SocketUser>,
        localUserName: String,
    ) {
        Elements.userList.innerHTML = ""
        users.forEach { user ->
            val li = document.createElement("li") as org.w3c.dom.HTMLLIElement

            val initials = user.username.getUsernameInitials()
            val isCurrentUser = user.username == localUserName
            val isMuted = user.isMuted

            val micIcon =
                """
                <svg id="micIcon-user-${user.socketId}" class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path class="micIconPath" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z"></path>
                    <line class="micSlash ${if (isMuted) "" else "hidden"}" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" x1="4" y1="4" x2="20" y2="20" class=""></line>
                </svg>
                """.trimIndent()

            // TODO: Sanitize username to prevent XSS
            li.innerHTML =
                """
                <div class="flex items-center gap-3 p-2 rounded-lg hover:bg-secondary/50 transition-all cursor-pointer group">
                    <div class="w-8 h-8 rounded-full ${if (isCurrentUser) "bg-primary/20" else "bg-secondary"} flex items-center justify-center ring-2 ring-transparent group-hover:ring-primary/30 transition-all">
                        <span class="text-xs font-semibold ${if (isCurrentUser) "text-primary" else "text-muted-foreground"}">$initials</span>
                    </div>
                    <div class="flex-1 min-w-0 flex flex-row gap-3">
                        <div class="text-sm font-medium truncate">${user.username}${if (isCurrentUser) " (Você)" else ""}</div
                        <div>$micIcon</div>
                    </div>
                    <div class="speaking-indicator"></div>
                </div>
                """.trimIndent()

            Elements.userList.appendChild(li)
        }

        Elements.userCount.textContent = users.size.toString()
    }

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

        messageElement.className = "annimate-fade-in"

        val wrapper = document.createElement("div") as HTMLDivElement
        wrapper.className = "flex gap-3 ${if (isCurrentUser) "justify-end" else ""}"

        if (isCurrentUser) {
            wrapper.innerHTML =
                """
                <div class="flex flex-col items-end max-w-[85%]">
                    <div class="flex items-center gap-2 mb-1">
                        <span class="text-xs text-muted-foreground">$formattedDate</span>
                        <span class="text-xs font-semibold text-primary">Você</span>
                    </div>
                    <div class="px-4 py-2 bg-primary text-primary-foreground rounded-2xl rounded-tr-sm shadow-sm">
                        <p class="text-sm"></p>
                    </div>
                </div>
                <div class="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0 ring-2 ring-primary/30">
                    <span class="text-xs font-semibold text-primary">$initials</span>
                </div>
                """.trimIndent()
        } else {
            // TODO: Sanitize message username to prevent XSS
            wrapper.innerHTML =
                """
                <div class="w-8 h-8 rounded-full bg-secondary flex items-center justify-center flex-shrink-0">
                    <span class="text-xs font-semibold text-muted-foreground">$initials</span>
                </div>
                <div class="flex flex-col max-w-[85%]">
                    <div class="flex items-center gap-2 mb-1">
                        <span class="text-xs font-semibold">${message.username}</span>
                        <span class="text-xs text-muted-foreground">$formattedDate</span>
                    </div>
                    <div class="px-4 py-2 bg-secondary/50 text-foreground rounded-2xl rounded-tl-sm shadow-sm">
                        <p class="text-sm"></p>
                    </div>
                </div>
                """.trimIndent()
        }

        val paragraph = wrapper.querySelector("p") as HTMLParagraphElement
        paragraph.textContent = message.content

        messageElement.appendChild(wrapper)
        Elements.chatMessages.appendChild(messageElement)

        messageElement.scrollIntoView(js("{ behavior: 'smooth', block: 'end' }"))
    }

    fun endScreenSharing() {
        Elements.screenVideo.srcObject = null
        Elements.videoContainer.classList.add("hidden")
        Elements.noScreenMessage.classList.remove("hidden")
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

    fun updateScreenContainer(screenStream: MediaStream) {
        with(Elements.screenVideo) {
            if (srcObject == null || srcObject.asDynamic().id != screenStream.id) {
                srcObject = screenStream

                Elements.videoContainer.classList.remove("hidden")
                Elements.noScreenMessage.classList.add("hidden")
            }
        }
    }

    fun updateAudioControls(isMicMuted: Boolean) {
        val micSlash = document.getElementById("micSlash")!!
        val micStatus = document.getElementById("micStatus")!!

        if (isMicMuted) {
            micSlash.classList.remove("hidden")
            micStatus.classList.remove("bg-green-500")
            micStatus.classList.add("bg-destructive")
        } else {
            micSlash.classList.add("hidden")
            micStatus.classList.remove("bg-destructive")
            micStatus.classList.add("bg-green-500")
        }
    }

    fun updateUserMuted(
        socketId: String,
        isMicMuted: Boolean,
    ) {
        val element = document.getElementById("micIcon-user-$socketId")
        if (element != null) {
            val micSlash = element.querySelector(".micSlash")
            if (isMicMuted) {
                micSlash!!.classList.remove("hidden")
            } else {
                micSlash!!.classList.add("hidden")
            }
        }
    }

    fun setUserSpeaking(
        username: String,
        isSpeaking: Boolean,
    ) {
        val userElements = Elements.userList.querySelectorAll("div.flex.items-center.gap-3")
        userElements.asList().forEach { element ->
            val nameElement = (element as DocumentFragment).querySelector("div.text-sm")
            if (nameElement?.textContent?.contains(username) == true) {
                val indicator = element.querySelector(".speaking-indicator") as? HTMLElement
                if (isSpeaking) {
                    indicator?.classList?.add("speaking-pulse")
                } else {
                    indicator?.classList?.remove("speaking-pulse")
                }
            }
        }
    }
}
