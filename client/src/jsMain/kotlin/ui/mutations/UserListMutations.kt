package ui.mutations

import createSafeElement
import getUsernameInitials
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLSpanElement
import platform.DesktopEnvironment
import sanitizeHTML
import screenshare.common.SocketUser
import ui.Elements
import ui.InterfaceMutations

object UserListMutations {
    val userVolumes: MutableMap<String, Int> = mutableMapOf()
    private var currentUsers: List<SocketUser> = emptyList()
    private var currentLocalSocketId: String? = null
    private val speakingUsers = mutableSetOf<String>()
    private val mutedUsers = mutableMapOf<String, Boolean>()
    private val screenSharers = mutableSetOf<String>()
    private val cameraSharers = mutableSetOf<String>()

    fun setUserMediaSharing(
        socketId: String,
        camera: Boolean,
        active: Boolean,
    ) {
        val sharers = if (camera) cameraSharers else screenSharers
        if (active) sharers.add(socketId) else sharers.remove(socketId)
        updateUserList(currentUsers, currentLocalSocketId)
    }

    fun restoreUserMediaSharing(
        screenSocketIds: List<String>,
        cameraSocketIds: List<String>,
    ) {
        screenSharers.clear()
        cameraSharers.clear()
        screenSharers.addAll(screenSocketIds)
        cameraSharers.addAll(cameraSocketIds)
        updateUserList(currentUsers, currentLocalSocketId)
        syncAvatarTiles()
    }

    fun clearUserMediaSharing() {
        screenSharers.clear()
        cameraSharers.clear()
        updateUserList(currentUsers, currentLocalSocketId)
    }

    fun hasRemoteShares(localSocketId: String?): Boolean =
        screenSharers.any { it != localSocketId } || cameraSharers.any { it != localSocketId }

    fun updateUserList(
        users: List<SocketUser>,
        localSocketId: String?,
    ) {
        currentUsers = users
        currentLocalSocketId = localSocketId
        users.forEach { user ->
            val volume =
                if (users.count { it.username == user.username } ==
                    1
                ) {
                    window.localStorage
                        .getItem(
                            "screenshare.volume.user.${user.username}",
                        )?.toIntOrNull()
                        ?.coerceIn(0, 100)
                } else {
                    null
                }
            if (volume != null) {
                userVolumes[user.socketId] = volume
                (document.getElementById("remote-audio-${user.socketId}") as? HTMLAudioElement)?.volume = volume / 100.0
            }
        }
        users.forEach { user ->
            mutedUsers[if (user.socketId == localSocketId) "self" else user.socketId] = user.isMuted
        }
        Elements.userList.innerHTML = ""

        users.forEach { user ->
            val userElement = createUserListItem(user, localSocketId)

            Elements.userList.appendChild(userElement)
        }

        Elements.userCount.textContent = users.size.toString()
        CoroutineScope(Dispatchers.Main).launch {
            runCatching { DesktopEnvironment.setTrayState("participants", false, users.size) }
        }
        document.getElementById("tray-share-notice")?.textContent =
            if (screenSharers.isNotEmpty() ||
                cameraSharers.isNotEmpty()
            ) {
                "Compartilhamento disponível na sala"
            } else {
                "Nenhum compartilhamento disponível"
            }
        syncAvatarTiles()
    }

    fun syncAvatarTiles() {
        val rail = Elements.screensRail
        val previous = rail.querySelectorAll(".participant-tile")
        for (index in previous.length - 1 downTo 0) {
            previous
                .item(
                    index,
                )?.parentElement
                ?.removeChild(previous.item(index)!!)
        }

        currentUsers.forEach { user ->
            val owner = if (user.socketId == currentLocalSocketId) "self" else user.socketId
            if (hasVideo(owner)) return@forEach
            val button = document.createSafeElement("div") as HTMLElement
            button.tabIndex = 0
            button.setAttribute("role", "group")
            button.className = "participant-tile"
            button.setAttribute("data-owner-id", owner)
            button.setAttribute("data-username", user.username)
            button.title = user.username
            val muted = mutedUsers[owner] ?: user.isMuted
            val state =
                when {
                    muted -> "mudo"
                    owner in speakingUsers -> "falando"
                    else -> "áudio ativo"
                }
            button.setAttribute("aria-label", "${user.username}, $state")

            val avatar = document.createSafeElement("span") as HTMLElement
            avatar.className = "avatar-circle"
            if (owner in speakingUsers) avatar.classList.add("speaking")
            avatar.textContent = user.username.getUsernameInitials()
            if (muted) {
                val mute = document.createSafeElement("span") as HTMLElement
                mute.className = "mute-overlay"
                mute.setAttribute("aria-hidden", "true")
                mute.innerHTML =
                    "<svg viewBox='0 0 24 24'><path d='M3 3l18 18M9 5v6a3 3 0 0 0 5 2M5 11a7 7 0 0 0 12 5M12 18v3M8 21h8'/></svg>"
                avatar.appendChild(mute)
            }
            button.appendChild(avatar)
            val name = document.createSafeElement("span") as HTMLElement
            name.className = "participant-name"
            name.textContent = if (owner == "self") "${user.username} (Você)" else user.username
            button.appendChild(name)
            button.appendChild(InterfaceMutations.participantOptionsButton(user.username))
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
        localSocketId: String?,
    ): HTMLElement {
        val article = document.createSafeElement("article") as HTMLElement
        article.setAttribute("role", "listitem")
        article.style.apply {
            padding = "0.75rem"
            marginBottom = "0.5rem"
        }

        val isCurrentUser = user.socketId == localSocketId
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
        userInfo.className = "tray-user-info"

        val nameSpan = document.createSafeElement("span") as HTMLSpanElement
        nameSpan.textContent =
            buildString {
                append(user.username)
                if (isCurrentUser) {
                    append(" (Você)")
                }
            }
        nameSpan.style.fontWeight = "400"
        userInfo.appendChild(nameSpan)

        val sharingScreen = user.socketId in screenSharers
        val sharingCamera = user.socketId in cameraSharers
        val status = document.createSafeElement("span") as HTMLElement
        status.className = "tray-user-status"
        status.textContent =
            when {
                sharingScreen -> "Compartilhando tela"
                sharingCamera -> "Compartilhando câmera"
                else -> "Conectado"
            }
        if (sharingScreen || sharingCamera) status.classList.add("sharing")
        userInfo.appendChild(status)

        container.appendChild(userInfo)

        val micIcon = createMicIcon(user.socketId, user.isMuted)
        container.appendChild(micIcon)

        article.appendChild(container)

        if (!isCurrentUser) {
            article.appendChild(createVolumeControl(user.socketId, user.username))
            if (DesktopEnvironment.isAvailable) {
                val views = document.createSafeElement("div") as HTMLElement
                views.className = "tray-view-actions"

                fun addViewButton(
                    label: String,
                    tileId: String,
                ) {
                    val button = document.createSafeElement("button") as HTMLButtonElement
                    button.type = "button"
                    button.className = "btn btn-ghost"
                    button.textContent = label
                    button.addEventListener("click", {
                        CoroutineScope(Dispatchers.Main).launch {
                            runCatching { DesktopEnvironment.showMainWindow() }
                                .onSuccess { InterfaceMutations.focusOnTile(tileId) }
                                .onFailure { InterfaceMutations.showError("Não foi possível abrir a sala.") }
                        }
                    })
                    views.appendChild(button)
                }
                if (user.socketId in screenSharers) addViewButton("Visualizar tela", user.socketId)
                if (user.socketId in
                    cameraSharers
                ) {
                    addViewButton("Visualizar câmera", services.CameraSharing.cameraTileId(user.socketId))
                }
                article.appendChild(views)
            }
        }

        article.id = "user-list-${if (isCurrentUser) "self" else user.socketId}"

        return article
    }

    private fun createVolumeControl(
        userId: String,
        username: String,
    ): HTMLElement {
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
        slider.value = (userVolumes[userId] ?: 100).coerceIn(0, 100).toString()
        userVolumes[userId] = slider.value.toInt()
        slider.className = "range range-primary range-xs flex-1"
        slider.setAttribute("aria-label", "Volume do participante")
        slider.addEventListener("input", {
            val volume = slider.value.toInt()
            userVolumes[userId] = volume
            if (currentUsers.count { it.username == username } ==
                1
            ) {
                window.localStorage.setItem("screenshare.volume.user.$username", volume.toString())
            }
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
            fontWeight = "700"
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
