# Domain: ui

## Overview
The browser UI is built with raw Kotlin/JS DOM manipulation. Styling uses TailwindCSS utility classes and DaisyUI components (loaded via CDN). The UI code is split into three distinct layers: DOM element references, DOM mutations, and event handler registration.

---

## Files
- `client/src/jsMain/kotlin/ui/Elements.kt`
- `client/src/jsMain/kotlin/ui/InterfaceMutations.kt`
- `client/src/jsMain/kotlin/ui/mutations/UserListMutations.kt`
- `client/src/jsMain/kotlin/ui/InterfaceHandlers.kt`
- `client/src/jsMain/resources/index.html`

---

## Layer 1: Elements (DOM References)

All element references are lazy-initialized singletons from `object Elements`:
```kotlin
object Elements {
    val joinScreen = getElement<HTMLElement>("join-screen")
    val appScreen = getElement<HTMLElement>("app-screen")
    val usernameInput = getElement<HTMLInputElement>("username")
    val roomIdInput = getElement<HTMLInputElement>("roomId")
    val joinButton = getElement<HTMLButtonElement>("joinBtn")
    val chatMessages = getElement<HTMLElement>("chat-messages")
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
    runCatching { document.getElementById(id) as T }.getOrElse {
        throw IllegalStateException("Element with id '$id' not found or is not of type ${T::class.simpleName}")
    }
```

---

## Layer 2: Mutations

### InterfaceMutations (global UI state)

**Screen navigation:**
```kotlin
fun navigateToRoomScreen(roomId: String, username: String) {
    Elements.joinScreen.classList.add("hidden")
    Elements.appScreen.classList.remove("hidden")
    val newUrl = "${URL(window.location.href).origin}/?roomId=$roomId"
    window.history.pushState(null, "", newUrl)
}
```

**Chat message rendering** (uses DaisyUI `chat`/`chat-bubble` classes, `chat-end` for own messages, `chat-start` for others):
```kotlin
fun addMessageToChat(message: ChatMessage, localUsername: String) {
    val wrapper = document.createElement("div") as HTMLDivElement
    wrapper.className = "chat ${if (isCurrentUser) "chat-end" else "chat-start"}"
    // Builds avatar, header, bubble, then appends to chat
    Elements.chatMessages.appendChild(messageElement)
    messageElement.scrollIntoView(js("{ behavior: 'smooth', block: 'end' }"))
}
```

**Screen share container:**
```kotlin
fun updateScreenContainer(screenStream: MediaStream, isInitiator: Boolean) {
    Elements.screenVideo.srcObject = screenStream
    Elements.videoContainer.classList.remove("hidden")
    Elements.noScreenMessage.classList.add("hidden")
    if (isInitiator) {
        Elements.stopScreenShareButton.classList.remove("hidden")
        Elements.shareScreenButton.classList.add("hidden")
    }
}
fun endScreenSharing() {
    Elements.screenVideo.srcObject = null
    Elements.videoContainer.classList.add("hidden")
    Elements.stopScreenShareButton.classList.add("hidden")
    Elements.noScreenMessage.classList.remove("hidden")
    Elements.shareScreenButton.classList.remove("hidden")
}
```

### UserListMutations

Handles the participant list panel:
```kotlin
fun updateUserList(users: List<SocketUser>, localUserName: String) {
    Elements.userList.innerHTML = ""
    users.forEach { user -> Elements.userList.appendChild(createUserListItem(user, localUserName)) }
    Elements.userCount.textContent = users.size.toString()
}
fun setUserSpeaking(socketId: String, isSpeaking: Boolean) {
    val avatarCircle = document.querySelector("article#user-list-$socketId .avatar-circle") as? HTMLElement
    if (isSpeaking) {
        avatarCircle?.classList?.add("ring-2", "ring-success", "ring-offset-2", "ring-offset-base-100")
    } else {
        avatarCircle?.classList?.remove("ring-2", "ring-success", "ring-offset-2", "ring-offset-base-100")
    }
}
```

---

## Layer 3: Event Handler Registration

All event listeners are wired in `registerUIHandlers(...)` called once from `main()`:
```kotlin
fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
    onStartScreenShare: () -> Unit,
    onStopScreenShare: () -> Unit,
) {
    setupJoinButtonHandler(joinRoom)
    setupSendMessageButtonHandler(sendChatMessage)
    setupMicToggleButtonHandler(onMicButtonToggle)
    setupScreenShareButtonHandlers(onStartScreenShare, onStopScreenShare)
    setupFullScreenButtonHandler()
}
```

Each `setupX` function registers a `click` event listener on the corresponding `Elements` reference.

---

## HTML Structure
- `#join-screen` – visible on load; hides after joining
- `#app-screen` – hidden on load; shown after joining
- Navbar contains room ID, theme switcher, screen-share buttons, mic toggle
- Main area: `#video-container` / `#no-screen-message` (screen share view)
- Sidebar `#sidebar-chat`: user list, chat messages, message input

## Theming
DaisyUI theme attribute on `<html data-theme="night">`. Switchable via `theme-controller` radio inputs. Available themes: `dracula`, `garden`, `night`, `coffee`, `synthwave`.
