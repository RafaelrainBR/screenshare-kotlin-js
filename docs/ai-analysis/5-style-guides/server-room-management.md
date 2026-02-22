# Style Guide: server-room-management

## Unique Conventions

### In-Memory Rooms Map
Rooms are stored in a `mutableMapOf<String, Room>()` local variable in `Application.module()`. No global state or singleton pattern.

### computeIfAbsent for Room Creation
```kotlin
val room = rooms.computeIfAbsent(roomId) { Room.create(roomId) }
```

### when-Exhaustive Packet Dispatch
`Room.consumePacket()` uses a non-exhaustive `when` expression with `else -> {}` as the default:
```kotlin
when (packet) {
    is SendChatMessage -> { ... }
    is ListUsers -> { ... }
    // ...
    else -> {}
}
```

### Message History Replay on Join
When a user joins, all prior `ChatMessage` objects are sent individually before the join notification:
```kotlin
private suspend fun sendMessageHistory(toUser: RoomUser) {
    messages.forEach { message ->
        toUser.sendPacket(ChatMessageReceived(roomId = id, message = message))
    }
}
```

### Broadcast After Every Structural Change
Both `notifyUserJoin` and `notifyUserLeave` broadcast the updated user list immediately after the join/leave event:
```kotlin
broadcast(UserConnected(...))
broadcast(createUserListPacket())
```

### Logging Pattern
Uses SLF4J `LoggerFactory.getLogger(Room::class.java)` for room-level logging. Log messages include the room ID and username:
```kotlin
logger.info("Room [$id] received from [${user.username}] the packet [$packet]")
```
