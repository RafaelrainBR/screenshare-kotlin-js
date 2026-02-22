# Style Guide: client-services

## Unique Conventions

### Plain Classes with Manual Construction
Services are plain Kotlin classes (`class Session`, `class VoiceChat`, etc.). There is no dependency-injection framework. Instances are created explicitly and passed as constructor parameters.

```kotlin
// Session constructs its own service objects
val voiceChat = VoiceChat()
val screenSharing = ScreenSharing()
val peerConnections = PeerConnections(voiceChat, screenSharing)
```

### CoroutineScope Delegation
`Session` delegates `CoroutineScope` to the injected scope:
```kotlin
class Session(..., coroutineScope: CoroutineScope) : CoroutineScope by coroutineScope
```

### Action Methods Return Job
User-triggered actions are `fun handleX() = launch { ... }`. They always return a `Job` and never block.

### Mutable State as Class Properties
Service state is stored as `var` / `val` properties on the service class itself:
```kotlin
class VoiceChat {
    var isMicMuted = true
    var localMicStream: MediaStream? = null
    val remoteAudioStreams: MutableMap<String, MediaStream> = mutableMapOf()
}
```

### Callback Parameters for Cross-Service Effects
Services that need to trigger effects in other services receive callbacks (lambdas) as parameters rather than holding direct references:
```kotlin
suspend fun setupLocalMic(recreatePeerConnections: suspend () -> Unit) { ... }
suspend fun toggleMute(broadcastMuted: suspend (isMuted: Boolean) -> Unit) { ... }
```

### Error Handling Pattern
All packet handling is wrapped in `runCatching { ... }.onFailure { error -> println("...${error.message}") }` to prevent unhandled exceptions from crashing the WebSocket loop.
