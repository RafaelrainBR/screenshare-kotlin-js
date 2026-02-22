# Domain: webrtc-peer-connections

## Overview
The client manages a full-mesh WebRTC topology. For every remote peer in the room, one `RTCPeerConnectionDecorator` instance is maintained in `PeerConnections.peers`. All interactions with the browser's native `RTCPeerConnection` are encapsulated in `RTCPeerConnectionDecorator`.

---

## Files
- `client/src/jsMain/kotlin/services/PeerConnections.kt`
- `client/src/jsMain/kotlin/decorators/RTCPeerConnectionDecorator.kt`
- `client/src/jsMain/kotlin/decorators/DisplayMediaDecorator.kt`

---

## RTCPeerConnectionDecorator

Wraps the dynamic JS `RTCPeerConnection` object:
```kotlin
class RTCPeerConnectionDecorator(private val windowRTCPeerConnection: dynamic) {
    fun createOffer(): Promise<Json> = windowRTCPeerConnection.createOffer() as Promise<Json>
    fun createAnswer(): Promise<Json> = windowRTCPeerConnection.createAnswer() as Promise<Json>
    fun setLocalDescription(description: Json): Promise<Json> = ...
    fun setRemoteDescription(description: RTCSessionDescription): Promise<Json> = ...
    fun addIceCandidate(candidate: RTCIceCandidate): Promise<Json> = ...
    fun addTrack(track: MediaStreamTrack, localStream: MediaStream) { ... }
    fun hasTrack(track: MediaStreamTrack): Boolean { ... }
    fun getSenders(): Array<dynamic> = ...
    fun onTrack(block: (streams: Array<MediaStream>) -> Unit) { ... }
    fun onIceCandidateAdd(block: (Json?) -> Unit) { ... }
    fun close() { ... }

    companion object {
        fun create(): RTCPeerConnectionDecorator {
            // Creates RTCPeerConnection with Google STUN servers:
            // stun:stun1.l.google.com:19302, stun:stun2.l.google.com:19302
            // iceCandidatePoolSize: 10
        }
    }
}
```

External declarations for types not in Kotlin Wrappers:
```kotlin
external class RTCSessionDescription(data: Json)
external class RTCIceCandidate(candidateInfo: Json)
```

---

## PeerConnections

Maintains `peers: MutableMap<String, RTCPeerConnectionDecorator>`.

### Creating a Peer Connection (on user join)
Called from `SessionHandler.handleUserConnected()`:
```kotlin
session.peerConnections.createPeerConnection(
    websocketService = session.websocketService,
    socketId = packet.socketId,
    roomId = session.localRoomId,
    isInitiator = true,
    coroutineScope = coroutineScope,
)
```

Inside `createPeerConnection`:
1. `addTracksIfNotPresent(peerConnection)` – attaches existing screen/mic tracks
2. Register ICE candidate handler → `WebsocketService.sendIceCandidate()`
3. If initiator: create offer → `setLocalDescription` → `WebsocketService.sendDescription()`
4. Register `onTrack` handler → route to `ScreenSharing.handleRemoteScreen()` or `VoiceChat.handleRemoteAudio()`

### Track Addition Check
```kotlin
private fun addTracksIfNotPresent(peerConnection: RTCPeerConnectionDecorator) {
    screenSharing.localScreenStream?.getTracks()?.forEach { track ->
        if (!peerConnection.hasTrack(track)) {
            peerConnection.addTrack(track, screenSharing.localScreenStream!!)
        }
    }
    voiceChat.localMicStream?.getTracks()?.forEach { track ->
        if (!peerConnection.hasTrack(track)) {
            peerConnection.addTrack(track, voiceChat.localMicStream!!)
        }
    }
}
```

### SDP Offer/Answer Exchange
Signaled via `Packet.SendDescription` / `Packet.DescriptionReceived`:
```kotlin
// Handled in SessionHandler.handleDescriptionReceived()
val sdp = RTCSessionDescription(json("type" to description["type"], "sdp" to description["sdp"]))
peerConnection.setRemoteDescription(sdp).await()

if (peerConnection.currentRemoteDescription != null) {
    val answer = peerConnection.createAnswer().await()
    peerConnection.setLocalDescription(answer).await()
    websocketService.sendDescription(roomId, description = ..., targetId = senderId)
}
```

### Recreating Peer Connections
Called after media streams change (new mic or screen share):
```kotlin
fun recreatePeerConnections(websocketService, roomId, isInitiator, coroutineScope) {
    peers.forEach { (peerId, peerConnection) ->
        addTracksIfNotPresent(peerConnection)
        if (isInitiator) {
            // create offer and send via websocket
        }
    }
}
```

### ICE Candidate Update
```kotlin
suspend fun updateIceCandidate(senderId: String, candidate: String) {
    peers[senderId]?.let { peerConnectionDecorator ->
        val candidateAsJson = JSON.parse<Json>(candidate)
        val rtcIceCandidate = createRTCIceCandidate(
            candidate = candidateAsJson["candidate"] as String,
            sdpMid = candidateAsJson["sdpMid"] as String,
            sdpMLineIndex = candidateAsJson["sdpMLineIndex"] as Int,
        )
        peerConnectionDecorator.addIceCandidate(RTCIceCandidate(rtcIceCandidate)).await()
    }
}
```
