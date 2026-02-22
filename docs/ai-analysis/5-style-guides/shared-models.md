# Style Guide: shared-models

## Unique Conventions

### Minimal Data Classes
Shared models are flat `data class` declarations with only `@Serializable` (and field-level `@SerialName` where brevity matters). No business logic in these classes.

```kotlin
@Serializable
data class ChatMessage(
    val username: String,
    val content: String,
    val timestamp: Long,  // epoch millis
)

@Serializable
data class SocketUser(
    val socketId: String,
    val username: String,
    val roomId: String,
    val isMuted: Boolean = true,  // default = true (muted on join)
)
```

### Timestamp as Long (Epoch Millis)
Timestamps are always `Long` epoch milliseconds. On the server: `System.currentTimeMillis()`. On the client (JS): `Date().getTime().toLong()`.

### Default Values for Optional State
`SocketUser.isMuted` defaults to `true` because users join muted.

### No Dependencies Beyond kotlinx.serialization
Shared models must only use the Kotlin stdlib and `kotlinx.serialization`. No Ktor, no DOM, no coroutines.
