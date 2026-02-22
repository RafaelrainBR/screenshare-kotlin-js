# Domain: room-management

## Overview
The server maintains rooms entirely in memory. A `Room` is created on the first `JoinRoom` packet and deleted when empty. Each participant in a room is represented by a `RoomUser` that holds a reference to the active `WebSocketServerSession`.

---

## Files
- `server/src/jvmMain/kotlin/screenshare/server/Application.kt`
- `server/src/jvmMain/kotlin/screenshare/server/Room.kt`

---

## Room Lifecycle

In `Application.module()`:
```kotlin
val rooms = mutableMapOf<String, Room>()

val connectUser: suspend (JoinRoom) -> Unit = { joinRoomPacket ->
    roomUser = RoomUser.create(session = this, username = joinRoomPacket.username)
    roomId = joinRoomPacket.roomId
    val room = rooms.computeIfAbsent(roomId) { Room.create(roomId) }
    room.addUser(roomUser)
}

val disconnectUser = suspend {
    val room = roomId?.let { rooms[it] }
    val roomUser = roomUser
    if (roomUser != null && room != null) {
        room.removeUser(roomUser)
        if (room.isEmpty) { rooms.remove(room.id) }
    }
}
```

---

## Room Class

### Data Structure
```kotlin
class Room(
    val id: String,
    private val users: MutableMap<String, RoomUser> = mutableMapOf(),
) {
    private val messages = mutableListOf<ChatMessage>()

    val isEmpty: Boolean get() = users.isEmpty()
    val allUsers: List<RoomUser> get() = users.values.toList()
}
```

### User Join
When a user joins, message history is replayed and all users are notified:
```kotlin
suspend fun addUser(user: RoomUser) {
    users[user.id] = user
    sendMessageHistory(user)
    notifyUserJoin(user)
}

private suspend fun notifyUserJoin(user: RoomUser) {
    broadcast(UserConnected(roomId = id, socketId = user.id, username = user.username))
    broadcast(createUserListPacket())
}
```

### Packet Dispatch
```kotlin
suspend fun consumePacket(user: RoomUser, packet: Packet) {
    if (packet.getSide() != CLIENT) return
    when (packet) {
        is SendChatMessage -> {
            val chatMessage = ChatMessage(username = user.username, content = packet.message, timestamp = System.currentTimeMillis())
            messages.add(chatMessage)
            broadcast(ChatMessageReceived(roomId = id, message = chatMessage))
        }
        is ListUsers -> { user.sendPacket(createUserListPacket()) }
        is SendIceCandidate -> {
            getUserById(packet.targetId)?.sendPacket(
                IceCandidateReceived(roomId = id, candidate = packet.candidate, senderId = user.id)
            )
        }
        is SendDescription -> {
            getUserById(packet.targetId)?.sendPacket(
                DescriptionReceived(roomId = id, description = packet.description, senderId = user.id)
            )
        }
        is StartScreenShare -> broadcast(ScreenShareStarted(roomId = id, senderId = user.id))
        is Packet.StopScreenShare -> broadcast(ScreenShareStopped(roomId = id, senderId = user.id))
        is Packet.SendMuted -> handleToggleMute(socketId = user.id, isMuted = true)
        is Packet.SendUnmuted -> handleToggleMute(socketId = user.id, isMuted = false)
        else -> {}
    }
}
```

### Broadcast vs. Targeted Send
```kotlin
// Broadcast to all users in room
private suspend fun broadcast(packet: Packet) {
    users.values.forEach { user ->
        user.session.send(Text(Json.encodeToString(packet)))
    }
}

// Send to one specific user
private fun getUserById(id: String): RoomUser? = users[id]
// Then: getUserById(targetId)?.sendPacket(packet)
```

---

## RoomUser

Created via factory method that generates a unique socket ID:
```kotlin
// RoomUser.create(session, username): RoomUser
```

Holds:
- `id: String` – unique socket ID
- `username: String`
- `isMuted: Boolean` – mutable, toggled by `SendMuted`/`SendUnmuted`
- `session: DefaultWebSocketServerSession` – used for sending frames
