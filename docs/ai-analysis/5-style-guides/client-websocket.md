# Style Guide: client-websocket

## Unique Conventions

### HttpClient Created Internally with Factory Function
`WebsocketService` accepts an optional `HttpClient` parameter (defaults to `createHttpClient()`). The factory function creates a client with only the `WebSockets` plugin installed.

### Nullable Session Reference
The underlying `WebSocketSession` is stored as `private var session: WebSocketSession? = null`. Operations on a null session are silently ignored (`session?.send(...)`).

### Listening Loop in a Separate Coroutine
`startListening()` launches a coroutine on the provided `coroutineScope` that `consumeEach` frames in a `try/catch`. On any exception, `close()` is called and `onClose()` is invoked.

### Close Callback for UI Notification
`onClose: () -> Unit` is injected at construction. In `Main.kt` it shows an alert and adds a "connection closed" system message to the chat.

### All Sends Through `sendPacket(Packet)`
Every public method on `WebsocketService` delegates to the private `sendPacket(packet: Packet)`:
```kotlin
private suspend fun sendPacket(packet: Packet) {
    println("Sending packet: $packet")
    session?.send(Text(Json.encodeToString(packet)))
}
```

New methods must follow the same pattern: accept domain-specific parameters, construct the `Packet`, call `sendPacket`.

### Handler Injection
The incoming packet handler is injected as a lambda:
```kotlin
private val handler: suspend (Session, Packet, CoroutineScope) -> Unit
```

This decouples `WebsocketService` from `Session` and `SessionHandler`.
