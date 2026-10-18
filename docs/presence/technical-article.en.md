# Kotlin/JS, WebRTC, and native capture in the same room

**Draft for review — October 18, 2026. Not published.** Code reviewed at commit `5c8f4374dd85977477f99b3dffbc45272b0e0a4d`; source links are pinned to that commit. The distribution reference is [stable release v0.11.1](https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/tag/v0.11.1). This article describes the implementation; it is not a benchmark or hardware validation. [Portuguese version](technical-article.md).

Screenshare lets people join a room by code, share a screen, and talk. The client uses Kotlin/JS; on desktop, the same client receives native capture tracks through a Tauri bridge. The central boundary is an API WebRTC already understands: `MediaStreamTrack`.

## Signaling and transport have different jobs

The [shared protocol](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/common/src/commonMain/kotlin/screenshare/common/Packet.kt) defines serializable packets for joining, messages, SDP, ICE candidates, and sharing state. The server's [Room](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/server/src/jvmMain/kotlin/screenshare/server/Room.kt) forwards descriptions and candidates to the target participant and distributes chat and room changes.

Video and audio travel over the clients' WebRTC connections. When direct connectivity fails, a configured TURN server can relay media. This does not make Ktor a media server. The architecture also does not behave as an SFU: more peers increase client work and bandwidth. The [hosting guide](../self-hosting.md#stun-and-turn) explains configuration and operational limits.

This separation prevents treating chat and media as if they had the same guarantees. Chat stays in process memory and should not be presented as a durable archive. Knowing a room code grants access; the app does not implement accounts or room passwords.

## One track, two sources

In the browser, [ScreenSharing](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/client/src/jsMain/kotlin/services/ScreenSharing.kt) requests capture through `getDisplayMedia`. When a native source is selected, it calls `DesktopCaptureBridge.startVideoCapture` and builds a stream from the returned track. Source selection happens before the stream is added to the connection.

On desktop, the [TypeScript bridge](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/desktop-tauri/src/desktopBridge.ts) adapts frames to the mechanism available in the WebView. The Windows path uses BGRA frames in shared memory and `MediaStreamTrackGenerator`. The WebKit path uses a worker with `VideoTrackGenerator` for video and an `AudioWorklet` connected to an audio destination to create the audio track. Platform implementations live in the [Rust capture module](https://github.com/RafaelrainBR/screenshare-kotlin-js/tree/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/desktop-capture/src).

As a result, [PeerConnections](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/client/src/jsMain/kotlin/services/PeerConnections.kt) can add screen, microphone, and camera tracks without implementing a separate connection for each capture backend. Platform differences remain: consent, source selection, available APIs, and shutdown still require their own handling.

Native capture does not silently substitute the browser picker's audio when an explicit desktop choice fails. The selected audio mode determines the source, and a recoverable error keeps the video running without publishing an unselected audio track. This is an observable behavior decision, not a guarantee that every audio source will work.

## Sharing also changes an existing connection

A participant may start sharing after a connection already exists. Peer management adds missing tracks and triggers negotiation; it also maintains `makingOffer` and `isPolite` state. When offers collide, the polite side rolls back and answers, while the other side ignores the conflicting offer. There are [specific negotiation tests](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/client/src/jsTest/kotlin/services/NegotiationTest.kt) for this behavior. Having tests does not replace a real session across networks and WebViews; they were not run to write this article.

Sharing profiles apply frame rate, bitrate, and degradation preferences to senders. Setting up to 60 fps does not prove that the receiver will receive them. The source, encoder, network, and device still determine the outcome. A demonstration should identify its platform and show what actually happened.

## What desktop still requires

Current packages target Windows x64 and macOS Apple Silicon. Windows builds have no publisher signature; macOS uses ad hoc signing without notarization, with capture permissions that may need recovery after an update. Application audio on macOS requires 14.2 or later. These conditions should appear before the download, as explained in the [usage guide](../usage.md).

Keeping the client and connections shared preserves a small boundary: delivering tracks to the application. The cost remains in bridging the actual capabilities of WebView2, WebKit, and system capture. Useful feedback on this architecture concerns that boundary and its tests, without confusing portable Kotlin code with identical platform APIs.
