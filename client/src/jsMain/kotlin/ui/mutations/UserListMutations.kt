package ui.mutations

import createSafeElement
import getUsernameInitials
import kotlinx.browser.document
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLSpanElement
import sanitizeHTML
import screenshare.common.SocketUser
import ui.Elements

object UserListMutations {
    val userVolumes: MutableMap<String, Int> = mutableMapOf()
    fun updateUserList(
        users: List<SocketUser>,
        localUserName: String,
    ) {
        Elements.userList.innerHTML = ""

        users.forEach { user ->
            val userElement = createUserListItem(user, localUserName)

            Elements.userList.appendChild(userElement)
        }

        Elements.userCount.textContent = users.size.toString()
    }

    fun updateUserMuted(
        socketId: String,
        isMicMuted: Boolean,
    ) {
        val element = document.getElementById("micIcon-user-$socketId")
        if (element != null) {
            if (isMicMuted) {
                element.classList.add("text-error")
                element.classList.remove("text-success")
                element.classList.remove("hidden")
            } else {
                element.classList.add("text-success")
                element.classList.remove("text-error")
                element.classList.add("hidden")
            }
        }
    }

    fun setUserSpeaking(
        socketId: String,
        isSpeaking: Boolean,
    ) {
        val avatarCircle = document.querySelector("article#user-list-$socketId .avatar-circle") as? HTMLElement
        if (avatarCircle != null) {
            if (isSpeaking) {
                avatarCircle.classList.add("ring-2", "ring-success", "ring-offset-2", "ring-offset-base-100")
            } else {
                avatarCircle.classList.remove("ring-2", "ring-success", "ring-offset-2", "ring-offset-base-100")
            }
        }
    }

    private fun createUserListItem(
        user: SocketUser,
        localUserName: String,
    ): HTMLElement {
        val article = document.createSafeElement("article") as HTMLElement
        article.style.apply {
            padding = "0.75rem"
            marginBottom = "0.5rem"
        }

        val isCurrentUser = user.username == localUserName
        val initials = user.username.getUsernameInitials()

        val container = document.createSafeElement("div") as HTMLElement
        container.style.apply {
            display = "flex"
            alignItems = "center"
            columnGap = "0.75rem"
        }

        val avatar = createAvatar(initials)
        container.appendChild(avatar)

        val userInfo = document.createSafeElement("div") as HTMLElement
        userInfo.style.flex = "1"

        val nameSpan = document.createSafeElement("span") as HTMLSpanElement
        nameSpan.textContent =
            buildString {
                append(user.username.sanitizeHTML())
                if (isCurrentUser) {
                    append(" (Você)")
                }
            }
        nameSpan.style.fontWeight = "500"
        userInfo.appendChild(nameSpan)

        container.appendChild(userInfo)

        val micIcon = createMicIcon(user.socketId, user.isMuted)
        container.appendChild(micIcon)

        article.appendChild(container)

        if (!isCurrentUser) {
            article.appendChild(createVolumeControl(user.socketId))
        }

        article.id = "user-list-${if (isCurrentUser) "self" else user.socketId}"

        return article
    }

    private fun createVolumeControl(userId: String): HTMLElement {
        val wrapper = document.createSafeElement("div") as HTMLElement
        wrapper.style.apply {
            display = "flex"
            alignItems = "center"
            columnGap = "0.5rem"
            marginTop = "0.5rem"
            paddingLeft = "2.75rem"
        }

        val volumeIcon = document.createSafeElement("span") as HTMLElement
        volumeIcon.innerHTML =
            """
            <svg class="w-4 h-4 opacity-70" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5L6 9H2v6h4l5 4V5z"/>
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.5 8.5a5 5 0 010 7"/>
            </svg>
            """.trimIndent()

        val slider = document.createSafeElement("input") as HTMLInputElement
        slider.type = "range"
        slider.min = "0"
        slider.max = "100"
        slider.value = (userVolumes[userId] ?: 100).toString()
        slider.className = "range range-primary range-xs flex-1"
        slider.setAttribute("aria-label", "Volume do participante")
        slider.addEventListener("input", {
            val volume = slider.value.toInt()
            userVolumes[userId] = volume
            (document.getElementById("remote-audio-$userId") as? HTMLAudioElement)?.volume = volume / 100.0
        })

        wrapper.appendChild(volumeIcon)
        wrapper.appendChild(slider)

        return wrapper
    }

    private fun createAvatar(initials: String): HTMLElement {
        val avatar = document.createSafeElement("div") as HTMLElement
        avatar.style.apply {
            width = "2rem"
            height = "2rem"
            borderRadius = "50%"
            display = "flex"
            alignItems = "center"
            justifyContent = "center"
            fontSize = "0.75rem"
            fontWeight = "600"
        }
        avatar.className = "avatar-circle"

        avatar.textContent = initials.sanitizeHTML()
        return avatar
    }

    private fun createMicIcon(
        userId: String,
        isMuted: Boolean,
    ): HTMLElement {
        val span = document.createSafeElement("span") as HTMLSpanElement
        span.id = "micIcon-user-$userId"
        span.innerHTML =
            """
            <svg id="micIcon" class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path id="micIconPath" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z"/>
                <line id="micSlash" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" x1="4" y1="4" x2="20" y2="20"/>
            </svg>
            """.trimIndent()

        span.style.fontSize = "1.25rem"
        span.setAttribute("aria-label", if (isMuted) "Mudo" else "Microfone ativo")
        span.className =
            if (isMuted) {
                "text-error"
            } else {
                "hidden"
            }
        return span
    }
}
