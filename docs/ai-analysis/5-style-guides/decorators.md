# Style Guide: decorators

## Unique Conventions

### Wrapping Dynamic JS APIs
When a browser API is not available in Kotlin Wrappers (`org.w3c.dom.*`) or has `dynamic` typing, it is wrapped in a dedicated Kotlin class or extension function inside the `decorators/` package.

### Decorator Class Pattern
The `RTCPeerConnectionDecorator` holds the underlying `dynamic` object and exposes it through typed methods:
```kotlin
class RTCPeerConnectionDecorator(private val windowRTCPeerConnection: dynamic) {
    fun createOffer(): Promise<Json> = windowRTCPeerConnection.createOffer() as Promise<Json>
    fun hasTrack(track: MediaStreamTrack): Boolean {
        val senders = getSenders()
        return senders.any { sender -> sender.track?.id == track.id }
    }
    // ...
}
```

All `dynamic` access is **isolated inside this class**. No `dynamic` property access appears anywhere else in the codebase.

### Factory Method
The decorator uses a `companion object` with a `create()` factory that initializes the underlying JS object with configuration:
```kotlin
companion object {
    fun create(): RTCPeerConnectionDecorator {
        val peerConnection = instantiate()
        peerConnection.oniceconnectionstatechange = { ... }
        return RTCPeerConnectionDecorator(peerConnection)
    }
    private fun instantiate(): dynamic = js("new RTCPeerConnection({ iceServers: [...] })")
}
```

### Extension Functions for Missing Methods
When a Kotlin type is missing a method from its JS counterpart, use an extension function:
```kotlin
// DisplayMediaDecorator.kt
fun MediaDevices.getDisplayMedia(constraints: MediaStreamConstraints): Promise<MediaStream> =
    this.asDynamic().getDisplayMedia(constraints) as Promise<MediaStream>
```

### External Class Declarations
For JS constructors that need typed wrappers, use `external class`:
```kotlin
external class RTCSessionDescription(data: Json)
external class RTCIceCandidate(candidateInfo: Json)
```

### No Logic Beyond Delegation
Decorator files contain **no business logic** — only the minimal wrapping needed to expose the JS API in a type-safe way.
