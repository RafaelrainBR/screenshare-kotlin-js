# Style Guide: entrypoints

## Unique Conventions

### Client Entrypoint: main() in Main.kt
`main()` is the sole entry point for the client. It:
1. Creates `WebsocketService` from `window.location` (protocol, hostname, port)
2. Creates a `CoroutineScope(Dispatchers.Main + SupervisorJob())`
3. Launches the WebSocket connect coroutine
4. Calls `registerUIHandlers(...)` with lambdas that reference the global `session` variable

```kotlin
fun main() {
    val websocketService = with(window.location) { WebsocketService(...) }
    val websocketCoroutineScope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    websocketCoroutineScope.launch { websocketService.connect(websocketCoroutineScope) }
    registerUIHandlers(
        joinRoom = { username, roomId -> session = Session(...) },
        // ...
    )
}
```

### Server Entrypoint: EngineMain + Application.module()
```kotlin
fun main(args: Array<String>) = io.ktor.server.cio.EngineMain.main(args)

fun Application.module() {
    install(ContentNegotiation) { json() }
    install(WebSockets) { pingPeriod = 5.seconds; timeout = 10.seconds; ... }
    val rooms = mutableMapOf<String, Room>()
    routing { ... }
}
```

The server uses CIO engine. The `main` function delegates directly to `EngineMain`.

### WSS/WS Auto-Detection
The client reads `window.location.protocol` to choose `WSS` vs. `WS` and the port from `window.location.port` (defaults to 443 for HTTPS, 80 for HTTP).
