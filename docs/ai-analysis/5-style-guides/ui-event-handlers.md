# Style Guide: ui-event-handlers

## Unique Conventions

### Single Registration Function
All event listeners are registered in one call to `registerUIHandlers(...)` from `main()`. The function takes named lambda parameters for each user action:
```kotlin
fun registerUIHandlers(
    joinRoom: (username: String, roomId: String) -> Unit,
    sendChatMessage: (message: String) -> Unit,
    onMicButtonToggle: () -> Unit,
    onStartScreenShare: () -> Unit,
    onStopScreenShare: () -> Unit,
)
```

### Private `setupX` Functions
Each action's listener setup is a private top-level function in `InterfaceHandlers.kt`. The pattern is `private fun setupXyzHandler(callback: ...) = Elements.xyzButton.addEventListener("click", ...)`.

### `e.preventDefault()` on All Click Handlers
Every click listener calls `e.preventDefault()` explicitly:
```kotlin
Elements.joinButton.addEventListener("click", { e ->
    e.preventDefault()
    // ...
})
```

### Input Trimming Before Use
Text inputs are always `.trim()`-ed before being passed to callbacks:
```kotlin
val username = Elements.usernameInput.value.trim()
val message = Elements.messageInput.value.trim()
```

### Empty Room ID → Generate Random
If `roomIdInput` is blank, a random UUID is generated via `generateRandomRoomId().take(8)`:
```kotlin
val roomId = Elements.roomIdInput.value.trim().takeIf { it.isNotBlank() } ?: generateRandomRoomId().take(8)
```

### Clear Input After Send
The `messageInput` is cleared after a message is sent:
```kotlin
sendChatMessage(message)
Elements.messageInput.value = ""
```
