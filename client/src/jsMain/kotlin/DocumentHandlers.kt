import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import org.w3c.dom.Node
import org.w3c.dom.events.Event
import org.w3c.dom.mediacapture.MediaStreamConstraints

fun registerDocumentHandlers(
    websocketService: WebsocketService,
    coroutineScope: CoroutineScope,
) {
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

        coroutineScope.launch {
            joinRoom(websocketService, roomId, username)
        }
    })

    Elements.userListButton.addEventListener("click", {
        Elements.userListDropdown.classList.toggle("hidden")
    })

    document.addEventListener("click", { event ->
        if (!Elements.userListButton.contains(event.target as Node) && !Elements.userListDropdown.contains(event.target as Node)) {
            Elements.userListDropdown.classList.add("hidden")
        }
    })

    Elements.sendMessageButton.addEventListener("click", {
        val message = Elements.messageInput.value.trim()

        if (message.isNotBlank()) {
            coroutineScope.launch {
                websocketService.sendChatMessage(
                    roomId = localRoomId,
                    message = message,
                )
            }
            Elements.messageInput.value = ""
        }
    })

    Elements.messageInput.addEventListener("keypress", { event ->
        if (event.isEnterKey()) {
            Elements.sendMessageButton.click()
        }
    })

    Elements.usernameInput.addEventListener("keypress", { event ->
        if (event.isEnterKey()) {
            Elements.joinButton.click()
        }
    })

    Elements.roomIdInput.addEventListener("keypress", { event ->
        if (event.isEnterKey()) {
            Elements.joinButton.click()
        }
    })

    Elements.micToggle.addEventListener("click", { event ->
        coroutineScope.launch {
            if (localMicStream == null) {
                runCatching {
                    setupLocalMic(websocketService, localRoomId, coroutineScope)
                }.onFailure { error ->
                    console.error("Error getting microphone", error)
                    window.alert("Permissao de mic necessária")
                    return@launch
                }
            }

            isMicMuted = !isMicMuted
            localMicStream?.getTracks()?.forEach { track -> track.enabled = !isMicMuted }
            updateAudioControls()
        }
    })
}

private fun Event.isEnterKey(): Boolean {
    return this.asDynamic().key.toString().equals("enter", ignoreCase = true)
}

private suspend fun setupLocalMic(
    websocketService: WebsocketService,
    roomId: String,
    coroutineScope: CoroutineScope
) {
    localMicStream = window.navigator.mediaDevices.getUserMedia(MediaStreamConstraints(audio = true)).await()
    val audioTrack = localMicStream?.getAudioTracks()?.firstOrNull()
    if (audioTrack != null) {
        recreatePeerConnections(websocketService, roomId, isInitiator = true, coroutineScope = coroutineScope)
    }
}

private fun updateAudioControls() {
    val micIcon = document.querySelector("#micToggle i")!!
    val micStatus = document.getElementById("micStatus")!!
    if (isMicMuted) {
        micIcon.classList.replace("fa-microphone", "fa-microphone-slash");
        micStatus.classList.add("bg-red-500");
    } else {
        micIcon.classList.replace("fa-microphone-slash", "fa-microphone");
        micStatus.classList.remove("bg-red-500");
    }
}