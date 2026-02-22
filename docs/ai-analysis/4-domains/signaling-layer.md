# Domain: signaling-layer

## Overview
All client↔server communication is channelled through a single WebSocket connection. Messages are strongly-typed Kotlin `Packet` instances serialized to JSON.

---

## Files
- `common/src/commonMain/kotlin/screenshare/common/Packet.kt`
- `client/src/jsMain/kotlin/services/WebsocketService.kt`
- `server/src/jvmMain/kotlin/screenshare/server/Application.kt`
- `server/src/jvmMain/kotlin/screenshare/server/Room.kt`

---

## Packet Protocol

All messages are sealed subclasses of `Packet`, residing in the `common` module.

### Sealed Class Definition
```kotlin
@Serializable
sealed class Packet {
    @Serializable
    @SerialName("join-room")
    data class JoinRoom(
        @SerialName("rid") val roomId: String,
        val username: String,
    ) : Packet()

    @Serializable
    @SerialName("send-message")
    data class SendChatMessage(
        @SerialName("rid") val roomId: String,
        @SerialName("msg") val message: String,
    ) : Packet()
    // ... other packets
}
```

### Packet Sides
Every packet belongs to exactly one side, enforced by `Packet.getSide()`:

```kotlin
fun getSide(): PacketSide =
    when (this) {
        is JoinRoom, is SendChatMessage, is ListUsers, is SendIceCandidate,
        is SendDescription, is StartScreenShare, is StopScreenShare,
        is SendMuted, is SendUnmuted -> CLIENT

        is UserConnected, is UserDisconnected, is ChatMessageReceived,
        is UserList, is IceCandidateReceived, is DescriptionReceived,
        is ScreenShareStarted, is ScreenShareStopped, is UserMuted, is UserUnmuted -> SERVER
    }
```

### JSON Key Conventions
Short `@SerialName` values are used to minimize payload size:

| Field    | SerialName |
|----------|------------|
| roomId   | `rid`      |
| socketId | `sid`      |
| targetId | `tid`      |
| message  | `msg`      |
| ice      | `ice`      |
| sdp/description | `sdp` |

The discriminator key is `"type"`.

---

## Client-Side: WebsocketService

`WebsocketService` is constructed in `main()` and injected into `Session`.

### Connection Setup
```kotlin
suspend fun connect(coroutineScope: CoroutineScope) {
    session = client.webSocketSession {
        url.protocol = urlProtocol
        url.host = host
        url.port = port
    }
    startListening(session!!, coroutineScope)
}
```

### Sending Packets
All outgoing packets go through a single private method:
```kotlin
private suspend fun sendPacket(packet: Packet) {
    println("Sending packet: $packet")
    session?.send(Text(Json.encodeToString(packet)))
}
```

Public methods simply build and send specific packet types:
```kotlin
suspend fun joinRoom(roomId: String, username: String) {
    sendPacket(Packet.JoinRoom(roomId, username))
}
suspend fun sendIceCandidate(roomId: String, candidate: kotlin.js.Json, targetId: String) {
    sendPacket(Packet.SendIceCandidate(roomId = roomId, candidate = JSON.stringify(candidate), targetId = targetId))
}
```

### Receiving Packets
Incoming `Text` frames are deserialized and dispatched to the `handler` callback:
```kotlin
is Text -> {
    val text = frame.readText()
    val packet = Json.decodeFromString<Packet>(text)
    handler(getSessionOrAlert(), packet, coroutineScope)
}
```

---

## Server-Side: Application.module()

The server registers one WebSocket endpoint at `/`:
```kotlin
routing {
    webSocket("/") {
        var roomUser: RoomUser? = null
        var roomId: String? = null

        this.incoming.consumeEach { frame ->
            when (frame) {
                is Text -> {
                    val packet = Json.decodeFromString<Packet>(frame.readText())
                    when (packet) {
                        is JoinRoom -> connectUser(packet)
                        else -> handlePacket(packet)
                    }
                }
                is Close -> disconnectUser()
                else -> {}
            }
        }
    }
}
```

`JoinRoom` is handled directly in the endpoint; all other packets are routed to `Room.consumePacket()`.

---

## Adding a New Packet Type

1. Add the data class inside `Packet` in `common/src/commonMain/kotlin/screenshare/common/Packet.kt`
2. Annotate with `@Serializable`, `@SerialName("your-type")`, and `@SerialName(...)` on fields
3. Add it to the correct `getSide()` branch (`CLIENT` or `SERVER`)
4. If it is a **CLIENT** packet: handle it in `Room.consumePacket()` on the server
5. If it is a **SERVER** packet: handle it in `handlePacket()` in `SessionHandler.kt` on the client
