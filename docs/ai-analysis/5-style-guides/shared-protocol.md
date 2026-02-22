# Style Guide: shared-protocol

## Unique Conventions

### Sealed Class for All Messages
Every client↔server message is a `data class` inside `sealed class Packet`. No plain `String` type tags or maps are used.

### Compact JSON Keys via @SerialName
All `@SerialName` annotations on **fields** use short abbreviations to minimize wire payload:

| Meaning      | Key    |
|--------------|--------|
| roomId       | `rid`  |
| socketId/senderId | `sid` |
| targetId     | `tid`  |
| message      | `msg`  |
| ice candidate | `ice` |
| sdp description | `sdp` (server→client) |

The discriminator is the default `"type"` key.

### Packet Type Names
Type names on the wire use `kebab-case` strings (e.g., `"join-room"`, `"send-ice-candidate"`, `"user-connected"`).

### Explicit Side Classification
Every packet subclass is classified as `CLIENT` or `SERVER` in `Packet.getSide()`. This is the authoritative gate used by the server (`if (packet.getSide() != CLIENT) return`).

### Pattern for Adding a New Packet
1. Declare `data class NewPacket(...)` inside `Packet` with `@Serializable @SerialName("new-type")`
2. Add all fields with short `@SerialName` annotations
3. Add to the correct arm of `getSide()`
4. Add forward/handler logic in `Room.consumePacket()` (server) or `handlePacket()` in `SessionHandler.kt` (client)
5. If it traverses the wire in both directions, consider a separate `when` branch for each direction
