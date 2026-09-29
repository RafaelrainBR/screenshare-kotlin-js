package ui.mutations

import createSafeElement
import getUsernameInitials
import kotlinx.browser.document
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLSpanElement
import sanitizeHTML
import screenshare.common.SocketUser
import ui.Elements

object UserListMutations {
    val userVolumes: MutableMap<String, Int> = mutableMapOf()
    private var currentUsers: List<SocketUser> = emptyList()
    private var currentLocalName = ""
    private val speakingUsers = mutableSetOf<String>()
    private val mutedUsers = mutableMapOf<String, Boolean>()

    fun updateUserList(
        users: List<SocketUser>,
        localUserName: String,
    ) {
        currentUsers = users
        currentLocalName = localUserName
        users.forEach { user ->
            mutedUsers[if (user.username == localUserName) "self" else user.socketId] = user.isMuted
        }
        Elements.userList.innerHTML = ""

        users.forEach { user ->
            val userElement = createUserListItem(user, localUserName)

            Elements.userList.appendChild(userElement)
        }

        Elements.userCount.textContent = users.size.toString()
        syncAvatarTiles()
    }

    fun syncAvatarTiles() {
        val rail = Elements.screensRail
        val previous = rail.querySelectorAll(".participant-tile")
        for (index in previous.length - 1 downTo 0) previous.item(index)?.parentElement?.removeChild(previous.item(index)!!)

        currentUsers.forEach { user ->
            val owner = if (user.username == currentLocalName) "self" else user.socketId
            if (hasVideo(owner)) return@forEach
            val button = document.createSafeElement("button") as HTMLButtonElement
            button.type = "button"
            button.className = "participant-tile"
            button.setAttribute("data-owner-id", owner)
            button.setAttribute("data-username", user.username)
            button.title = user.username
            val muted = mutedUsers[owner] ?: user.isMuted
            val state = when {
                muted -> "mudo"
                owner in speakingUsers -> "falando"
                else -> "áudio ativo"
            }
            button.setAttribute("aria-label", "${user.username}, $state, abrir opções")

            val avatar = document.createSafeElement("span") as HTMLElement
            avatar.className = "avatar-circle"
            if (owner in speakingUsers) avatar.classList.add("speaking")
            avatar.textContent = user.username.getUsernameInitials()
            if (muted) {
                val mute = document.createSafeElement("span") as HTMLElement
                mute.className = "mute-overlay"
                mute.setAttribute("aria-hidden", "true")
                mute.innerHTML = "<svg viewBox='0 0 24 24'><path d='M3 3l18 18M9 5v6a3 3 0 0 0 5 2M5 11a7 7 0 0 0 12 5M12 18v3M8 21h8'/></svg>"
                avatar.appendChild(mute)
            }
            button.appendChild(avatar)
            rail.appendChild(button)
        }
        rail.classList.toggle("hidden", rail.childElementCount == 0)
        document.getElementById("toggle-previews")?.classList?.toggle("hidden", rail.childElementCount == 0)
    }

    private fun hasVideo(owner: String): Boolean {
        // ponytail: scan is fine for room-sized groups; index by owner if rooms become large.
        val tiles = document.getElementsByClassName("screen-tile")
        for (index in 0 until tiles.length) {
            if ((tiles.item(index) as? HTMLElement)?.getAttribute("data-owner-id") == owner) return true
        }
        return false
    }

    fun updateUserMuted(
        socketId: String,
        isMicMuted: Boolean,
    ) {
        mutedUsers[socketId] = isMicMuted
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
        syncAvatarTiles()
    }

    fun setUserSpeaking(
        socketId: String,
        isSpeaking: Boolean,
    ) {
        if (isSpeaking) speakingUsers.add(socketId) else speakingUsers.remove(socketId)
        val rail = Elements.screensRail
        val avatars = rail.getElementsByClassName("participant-tile")
        for (index in 0 until avatars.length) {
            val tile = avatars.item(index) as? HTMLElement ?: continue
            if (tile.getAttribute("data-owner-id") == socketId) {
                tile.querySelector(".avatar-circle")?.classList?.toggle("speaking", isSpeaking)
            }
        }
        val videos = document.getElementsByClassName("screen-tile")
        for (index in 0 until videos.length) {
            val tile = videos.item(index) as? HTMLElement ?: continue
            if (tile.getAttribute("data-owner-id") == socketId) tile.classList.toggle("speaking", isSpeaking)
        }
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
