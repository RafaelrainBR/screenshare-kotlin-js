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

    // Owners whose screen has a quality problem (InterfaceMutations.setTileQuality), shown amber in "Na sala".
    private val screenWarnings = mutableSetOf<String>()

    fun setScreenWarning(
        owner: String,
        warning: Boolean,
    ) {
        if (if (warning) screenWarnings.add(owner) else screenWarnings.remove(owner)) syncPeople()
    }

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

    /**
     * "Na sala": you first, then everyone in the server's order, which never shuffles while people talk.
     * Every layout of the stage runs it (through syncAvatarTiles), so a row turns clickable once its tile exists.
     */
    private fun syncPeople() {
        val list = document.getElementById("room-people-list") as? HTMLElement ?: return
        document.getElementById("room-people-count")?.textContent = currentUsers.size.toString()
        list.innerHTML = ""
        currentUsers.sortedBy { it.socketId != currentLocalSocketId }.forEach { user ->
            list.appendChild(createPersonRow(user))
        }
        if (currentUsers.size == 1) list.appendChild(aloneHint())
    }

    private fun createPersonRow(user: SocketUser): HTMLElement {
        val owner = if (user.socketId == currentLocalSocketId) "self" else user.socketId
        val row = document.createSafeElement("div") as HTMLElement
        row.className = "person-row"
        row.id = "people-$owner"
        row.setAttribute("role", "listitem")
        row.setAttribute("data-owner-id", owner)
        row.setAttribute("data-username", user.username)

        // A click shows their screen (or camera) on the stage; without a video the row is just a name.
        val tileId = videoTileId(owner)
        val main = document.createSafeElement(if (tileId != null) "button" else "span") as HTMLElement
        main.className = "person-main"
        if (tileId != null) {
            (main as HTMLButtonElement).type = "button"
            main.addEventListener("click", { InterfaceMutations.focusOnTile(tileId) })
        }
        val avatar = createAvatar(user.username.getUsernameInitials())
        if (owner in speakingUsers) avatar.classList.add("speaking")
        main.appendChild(avatar)
        val name = document.createSafeElement("span") as HTMLSpanElement
        name.className = "person-name"
        name.textContent = if (owner == "self") "${user.username} (Você)" else user.username
        main.appendChild(name)

        val icons = document.createSafeElement("span") as HTMLElement
        icons.className = "person-icons"
        if (user.socketId in screenSharers) {
            val warning = owner in screenWarnings
            val screen =
                stateIcon(
                    if (warning) "Compartilhando tela, com problema de qualidade" else "Compartilhando tela",
                    SCREEN_ICON,
                )
            screen.classList.add("person-screen")
            screen.classList.toggle("warn", warning)
            icons.appendChild(screen)
        }
        if (user.socketId in cameraSharers) icons.appendChild(stateIcon("Câmera ligada", CAMERA_ICON))
        if (mutedUsers[owner] ?: user.isMuted) icons.appendChild(createMicIcon("people-$owner", true))
        main.appendChild(icons)
        row.appendChild(main)
        if (owner != "self") row.appendChild(InterfaceMutations.participantOptionsButton(user.username))
        val states =
            (0 until icons.children.length).mapNotNull {
                (icons.children.item(it) as? HTMLElement)?.getAttribute("aria-label")
            }
        val label = (listOf(name.textContent.orEmpty()) + states).joinToString(", ")
        // The button carries the summary and what a click does; a plain row is labelled as a whole.
        if (tileId != null) {
            main.setAttribute("aria-label", "$label. Mostrar no palco")
            main.title = "Mostrar no palco"
        } else {
            row.setAttribute("aria-label", label)
        }
        return row
    }

    // The screen first, as the tray's "Visualizar tela"; a leaving tile no longer counts.
    private fun videoTileId(owner: String): String? {
        val tile =
            document.querySelector(".screen-tile:not(.camera-tile):not(.leaving)[data-owner-id='$owner']")
                ?: document.querySelector(".camera-tile:not(.leaving)[data-owner-id='$owner']")
        return tile?.getAttribute("data-tile-id")
    }

    private fun aloneHint(): HTMLElement {
        val hint = document.createSafeElement("div") as HTMLElement
        hint.className = "room-people-alone"
        hint.appendChild((document.createSafeElement("span") as HTMLElement).apply { textContent = "Só você aqui." })
        val copy = document.createSafeElement("button") as HTMLButtonElement
        copy.type = "button"
        copy.className = "btn btn-ghost"
        copy.textContent = "Copiar link"
        copy.addEventListener("click", { (document.getElementById("copyRoomBtn") as? HTMLElement)?.click() })
        hint.appendChild(copy)
        return hint
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
        syncPeople()
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
        // Re-renders rebuild from currentUsers; keep that snapshot in step with live mute events.
        val target = if (socketId == "self") currentLocalSocketId else socketId
        currentUsers = currentUsers.map { if (it.socketId == target) it.copy(isMuted = isMicMuted) else it }
        document.getElementById("micIcon-user-$socketId")?.let { applyMicState(it as HTMLElement, isMicMuted) }
        (document.getElementById("user-list-$socketId") as? HTMLElement)?.let(::labelUserListItem)
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
        document.getElementById("user-list-$socketId")?.classList?.toggle("speaking", isSpeaking)
        document.querySelector("#people-$socketId .avatar-circle")?.classList?.toggle("speaking", isSpeaking)
    }

    private fun createUserListItem(
        user: SocketUser,
        localSocketId: String?,
    ): HTMLElement {
        val isCurrentUser = user.socketId == localSocketId
        val owner = if (isCurrentUser) "self" else user.socketId
        val article = document.createSafeElement("article") as HTMLElement
        article.setAttribute("role", "listitem")
        article.className = "tray-user"
        article.id = "user-list-$owner"
        if (owner in speakingUsers) article.classList.add("speaking")

        val row = document.createSafeElement("div") as HTMLElement
        row.className = "tray-user-row"
        row.appendChild(createAvatar(user.username.getUsernameInitials()))

        val nameSpan = document.createSafeElement("span") as HTMLSpanElement
        nameSpan.className = "tray-user-name"
        nameSpan.textContent = if (isCurrentUser) "${user.username} (Você)" else user.username
        row.appendChild(nameSpan)

        val icons = document.createSafeElement("span") as HTMLElement
        icons.className = "tray-user-icons"
        if (user.socketId in screenSharers) icons.appendChild(stateIcon("Compartilhando tela", SCREEN_ICON))
        if (user.socketId in cameraSharers) icons.appendChild(stateIcon("Câmera ligada", CAMERA_ICON))
        icons.appendChild(createMicIcon(owner, mutedUsers[owner] ?: user.isMuted))
        row.appendChild(icons)
        article.appendChild(row)

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

        labelUserListItem(article)
        return article
    }

    // The row's accessible name summarises the name plus the state icons.
    private fun labelUserListItem(article: HTMLElement) {
        val name = article.querySelector(".tray-user-name")?.textContent.orEmpty()
        val states = article.querySelectorAll(".tray-user-icons [aria-label]")
        val summary =
            (0 until states.length).mapNotNull { index ->
                (states.item(index) as? HTMLElement)?.getAttribute("aria-label")
            }
        article.setAttribute("aria-label", (listOf(name) + summary).joinToString(", "))
    }

    private fun stateIcon(
        label: String,
        svg: String,
    ): HTMLElement {
        val span = document.createSafeElement("span") as HTMLSpanElement
        span.className = "state-icon"
        span.setAttribute("role", "img")
        span.setAttribute("aria-label", label)
        span.title = label
        span.innerHTML = svg
        return span
    }

    private fun createVolumeControl(
        userId: String,
        username: String,
    ): HTMLElement {
        val wrapper = document.createSafeElement("div") as HTMLElement
        wrapper.className = "tray-user-volume"

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
        avatar.className = "avatar-circle"
        avatar.setAttribute("aria-hidden", "true")
        avatar.textContent = initials.sanitizeHTML()
        return avatar
    }

    private fun createMicIcon(
        owner: String,
        isMuted: Boolean,
    ): HTMLElement {
        val span = stateIcon("", MIC_ICON)
        span.id = "micIcon-user-$owner"
        applyMicState(span, isMuted)
        return span
    }

    private fun applyMicState(
        icon: HTMLElement,
        isMuted: Boolean,
    ) {
        val label = if (isMuted) "Mudo" else "Microfone ativo"
        icon.classList.add("state-mic")
        icon.classList.toggle("muted", isMuted)
        icon.setAttribute("aria-label", label)
        icon.title = label
    }

    private const val MIC_ICON =
        "<svg viewBox='0 0 24 24'><path d='M9 5a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0V5Z'/>" +
            "<path d='M5 11a7 7 0 0 0 14 0M12 18v3M8 21h8'/>" +
            "<line class='slash' x1='3' y1='3' x2='21' y2='21'/></svg>"
    private const val SCREEN_ICON =
        "<svg viewBox='0 0 24 24'><rect x='3' y='4' width='18' height='14' rx='2'/><path d='M8 21h8M12 18v3'/></svg>"
    private const val CAMERA_ICON =
        "<svg viewBox='0 0 24 24'><rect x='3' y='6' width='12' height='12' rx='2'/><path d='m15 10 6-3v10l-6-3'/></svg>"
}
