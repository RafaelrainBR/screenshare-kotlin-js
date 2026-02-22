# Style Guide: client-webrtc

## Unique Conventions

### One Peer Connection Per Remote User
`PeerConnections.peers` is a `MutableMap<String, RTCPeerConnectionDecorator>` keyed by `socketId`. One entry per remote participant.

### All WebRTC Access Through Decorator
No code outside `decorators/` accesses `RTCPeerConnection` natively. All calls go through `RTCPeerConnectionDecorator`.

### Track Deduplication Before Every Offer
`addTracksIfNotPresent()` is called before every offer creation to avoid duplicate senders:
```kotlin
if (!peerConnection.hasTrack(track)) {
    peerConnection.addTrack(track, stream)
}
```

### Initiator vs. Responder via Parameter
`isInitiator: Boolean` is passed to `createPeerConnection(...)`. Initiators create offers; responders create answers when handling `DescriptionReceived`.

### Promise.await() for All Async WebRTC Calls
All `Promise<T>` values are resolved with `.await()` inside `coroutineScope.launch { runCatching { ... }.onFailure { ... } }` blocks:
```kotlin
coroutineScope.launch {
    runCatching {
        val offer = peerConnection.createOffer().await()
        peerConnection.setLocalDescription(offer).await()
        // ...
    }.onFailure { error -> console.error("Error creating offer", error) }
}
```

### Stream Type Inference from Track Type
No type tag is carried in the stream; the stream type is inferred:
```kotlin
peerConnection.onTrack { streams ->
    val remoteStream = streams[0]
    val isScreenStream = remoteStream.getVideoTracks().isNotEmpty()
    if (isScreenStream) { screenSharing.handleRemoteScreen(socketId, remoteStream) }
    else { voiceChat.handleRemoteAudio(socketId, remoteStream) }
}
```
