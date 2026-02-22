# Style Guide: server-routing

## Unique Conventions

### Single WebSocket Endpoint at "/"
All client-server communication goes through one WebSocket endpoint registered at `"/"`:
```kotlin
routing {
    staticResources("/", "static")  // serves client app
    webSocket("/") {                // signaling channel
        // ...
    }
}
```

### In-WebSocket Connection Lifecycle with Local Variables
Connection-scoped state (`roomUser`, `roomId`) is held as nullable local `var` inside the `webSocket` lambda:
```kotlin
webSocket("/") {
    var roomUser: RoomUser? = null
    var roomId: String? = null
    // ...
}
```

### Suspend Lambda Pattern for Connect/Disconnect
Connect and disconnect logic are captured as named `suspend` lambdas to keep the `consumeEach` block readable:
```kotlin
val connectUser: suspend (JoinRoom) -> Unit = { joinRoomPacket -> ... }
val disconnectUser: suspend () -> Unit = { ... }
```

### IP Extraction with X-Forwarded-For Support
The client IP is resolved with `X-Forwarded-For` fallback for reverse-proxy deployments:
```kotlin
val clientIp = call.request.headers["X-Forwarded-For"]
    ?.split(",")?.first()?.trim()
    ?: call.request.origin.remoteAddress
```

### Ktor Plugins Configuration
WebSockets are configured with `pingPeriod = 5.seconds` and `timeout = 10.seconds`. Content negotiation uses `json()` for Ktor/kotlinx-serialization.

### JoinRoom Special-Cased
`JoinRoom` is the only packet handled directly in the WebSocket endpoint. All other client packets are forwarded to `Room.consumePacket()`.
