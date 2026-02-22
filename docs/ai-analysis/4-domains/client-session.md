# Domain: client-session

## Overview
`Session` is the central coordinator for all client-side state and actions. It is created once when the user clicks "Join Room" and holds references to all service objects. It delegates `CoroutineScope` to the application-level scope created in `main()`.

---

## Files
- `client/src/jsMain/kotlin/services/Session.kt`
- `client/src/jsMain/kotlin/services/SessionHandler.kt`
- `client/src/jsMain/kotlin/Main.kt`

---

## Session Construction

```kotlin
class Session(
    var localUsername: String,
    var localRoomId: String,
    var currentSharerSocketId: String? = null,
    var userList: List<SocketUser> = emptyList(),
    val websocketService: WebsocketService,
    coroutineScope: CoroutineScope,
) : CoroutineScope by coroutineScope {
    val voiceChat = VoiceChat()
    val screenSharing = ScreenSharing()
    val peerConnections = PeerConnections(voiceChat, screenSharing)

    init {
        launch {
            InterfaceMutations.navigateToRoomScreen(roomId = localRoomId, username = localUsername)
            websocketService.joinRoom(roomId = localRoomId, username = localUsername)
            InterfaceMutations.addMessageToChat(
                ChatMessage(username = "Sistema", content = "Você entrou na sala $localRoomId", Date().getTime().toLong()),
                localUsername = localUsername,
            )
        }
    }
}
```

---

## Global Session Variable

In `Main.kt`:
```kotlin
var session: Session? = null

fun getSessionOrAlert(): Session {
    return session ?: run {
        window.alert("Você não está em uma sala!")
        throw IllegalStateException("Session is null")
    }
}
```

The `session` variable is assigned in the `joinRoom` callback passed to `registerUIHandlers()`.

---

## Action Methods

All user-triggered actions return `Job` via `launch {}`:
```kotlin
fun handleMessageSend(message: String) = launch {
    websocketService.sendChatMessage(roomId = localRoomId, message = message)
}

fun handleMicButtonToggle() = launch {
    if (voiceChat.localMicStream == null) {
        runCatching {
            voiceChat.setupLocalMic(recreatePeerConnections = { recreatePeerConnections() })
        }.onFailure { error ->
            console.error("Error getting microphone", error)
            window.alert("Permissao de mic necessária")
            return@launch
        }
    }
    voiceChat.toggleMute(broadcastMuted = { isMuted ->
        websocketService.sendToggleMute(roomId = localRoomId, isMuted = isMuted)
    })
}

fun handleStartScreenShare() = launch {
    screenSharing.setupLocalScreenStream(
        onStreamEnd = { handleStopScreenShare() },
        recreatePeerConnections = { recreatePeerConnections() },
    )
    websocketService.startScreenSharing(localRoomId)
}

fun handleStopScreenShare() = launch {
    screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
    websocketService.stopScreenSharing(localRoomId)
}
```

---

## Packet Handler (SessionHandler.kt)

The `handlePacket` top-level function is passed as the `handler` callback to `WebsocketService` and dispatches incoming server packets:
```kotlin
fun handlePacket(session: Session, packet: Packet, coroutineScope: CoroutineScope) {
    runCatching {
        when (packet) {
            is Packet.UserConnected -> handleUserConnected(session, packet, coroutineScope)
            is Packet.UserDisconnected -> handleUserDisconnected(session, packet)
            is Packet.ChatMessageReceived -> handleChatMessageReceived(session, packet)
            is Packet.UserList -> handleUserList(session, packet)
            is Packet.IceCandidateReceived -> handleIceCandidateReceived(session, packet, coroutineScope)
            is Packet.DescriptionReceived -> handleDescriptionReceived(session, packet, coroutineScope)
            is Packet.UserMuted, is Packet.UserUnmuted -> handleUserMuted(packet)
            else -> println("Unknown packet type: ${packet::class.simpleName}")
        }
    }.onFailure { error ->
        println("Error handling packet [$packet]: ${error.message}")
    }
}
```

## On User Connect: Peer Connection Creation
```kotlin
private fun handleUserConnected(session: Session, packet: Packet.UserConnected, coroutineScope: CoroutineScope) {
    val isLocalUser = packet.username == session.localUsername
    if (!isLocalUser) {
        InterfaceMutations.addMessageToChat(
            message = ChatMessage(username = "Sistema", content = "${packet.username} entrou na sala", ...),
            localUsername = session.localUsername,
        )
        session.peerConnections.createPeerConnection(
            websocketService = session.websocketService,
            socketId = packet.socketId,
            roomId = session.localRoomId,
            isInitiator = true,
            coroutineScope = coroutineScope,
        )
    }
}
```
