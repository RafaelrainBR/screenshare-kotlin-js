# Domain: media-capture

## Overview
Media capture covers acquisition of the local screen stream (`getDisplayMedia`) and the local microphone stream (`getUserMedia`), plus audio-level monitoring for speaking indicators. These are implemented in `ScreenSharing` and `VoiceChat` respectively.

---

## Files
- `client/src/jsMain/kotlin/services/ScreenSharing.kt`
- `client/src/jsMain/kotlin/services/VoiceChat.kt`
- `client/src/jsMain/kotlin/decorators/DisplayMediaDecorator.kt`

---

## Screen Sharing (ScreenSharing.kt)

### Capturing the Local Screen
`getDisplayMedia` is not exposed by Kotlin Wrappers, so it is added via an extension function in `DisplayMediaDecorator.kt`:
```kotlin
fun MediaDevices.getDisplayMedia(constraints: MediaStreamConstraints): Promise<MediaStream> =
    this.asDynamic().getDisplayMedia(constraints) as Promise<MediaStream>
```

Usage in `ScreenSharing.setupLocalScreenStream()`:
```kotlin
localScreenStream = window.navigator.mediaDevices
    .getDisplayMedia(buildMediaStreamConstraints())
    .await()
```

### Media Constraints
```kotlin
private fun buildMediaStreamConstraints(): MediaStreamConstraints =
    MediaStreamConstraints(
        video = mapOf(
            "cursor" to "always",
            "frameRate" to mapOf("ideal" to 30, "max" to 60),
            "width" to mapOf("ideal" to 1920),
            "height" to mapOf("ideal" to 1080),
            "resizeMode" to "crop-and-scale",
        ),
        audio = mapOf(
            "sampleSize" to 32, "sampleRate" to 48000,
            "echoCancellation" to false, "noiseSuppression" to false,
            "autoGainControl" to false, "channelCount" to 2, "latency" to 0,
        ),
    )
```

### Track End Handling
```kotlin
val videoTrack = localScreenStream?.getVideoTracks()?.firstOrNull()
if (videoTrack != null) {
    videoTrack.onended = {
        localScreenStream = null
        onStreamEnd()   // triggers handleStopScreenShare() in Session
    }
    recreatePeerConnections()
}
```

### Remote Screen Display
```kotlin
fun handleRemoteScreen(socketId: String, stream: MediaStream) {
    remoteScreenStreams[socketId] = stream
    InterfaceMutations.updateScreenContainer(stream, isInitiator = false)
}
```

---

## Voice Chat (VoiceChat.kt)

### Capturing the Microphone
```kotlin
localMicStream = window.navigator.mediaDevices
    .getUserMedia(buildMediaStreamConstraints())
    .await()
```

Mic constraints target high-quality audio suitable for music / screen share content:
```kotlin
private fun buildMediaStreamConstraints(): MediaStreamConstraints =
    MediaStreamConstraints(
        audio = mapOf(
            "echoCancellation" to false,
            "noiseSuppression" to false,
            "autoGainControl" to false,
            "sampleRate" to 48000,
            "sampleSize" to 16,
            "channelCount" to 2,
            "latency" to 0,
        ),
    )
```

### Mute Toggle
```kotlin
suspend fun toggleMute(broadcastMuted: suspend (isMuted: Boolean) -> Unit) {
    isMicMuted = !isMicMuted
    localMicStream?.let { stream ->
        stream.getTracks().forEach { track -> track.enabled = !isMicMuted }
    }
    InterfaceMutations.updateAudioControls(isMicMuted = isMicMuted)
    broadcastMuted(isMicMuted)
}
```

### Audio Level Monitoring (Speaking Indicator)
The Web Audio API is used to detect speaking:
```kotlin
private fun monitorAudioLevel(stream, socketId, isEnable, onSpeakingChange) {
    val audioContext = js("new AudioContext()").unsafeCast<dynamic>()
    val source = audioContext.createMediaStreamSource(stream)
    val analyser = audioContext.createAnalyser()
    analyser.fftSize = 512
    analyser.smoothingTimeConstant = 0.8
    source.connect(analyser)
    // Polls analyser every animation frame, compares RMS to SPEAKING_THRESHOLD (30)
    // Calls onSpeakingChange(true/false) on state transitions
}
```

`SPEAKING_THRESHOLD = 30` is a private constant in `VoiceChat.kt`.

`onSpeakingChange` callback calls `UserListMutations.setUserSpeaking(socketId, isSpeaking)` which toggles DaisyUI ring classes on the user avatar.

---

## Remote Audio

```kotlin
fun handleRemoteAudio(socketId: String, stream: MediaStream) {
    remoteAudioStreams[socketId] = stream
    InterfaceMutations.addAudioElementForUser(userId = socketId, stream = stream)
    monitorAudioLevel(stream = stream, socketId = socketId, isEnable = { remoteAudioStreams.containsKey(socketId) }) { isSpeaking ->
        UserListMutations.setUserSpeaking(socketId, isSpeaking)
    }
}
```

`addAudioElementForUser` creates an `<audio autoplay>` element and appends it to `document.body`.
