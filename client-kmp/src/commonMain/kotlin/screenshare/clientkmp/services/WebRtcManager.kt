package screenshare.clientkmp.services

import kotlinx.coroutines.flow.StateFlow
import screenshare.common.Packet

/**
 * Platform-agnostic WebRTC manager.
 * wasmJs: browser RTCPeerConnection
 * Desktop: stub until Phase 0 fork is complete
 */
interface WebRtcManager {
    /**
     * The video track currently active for display (local screen share or remote share).
     * The concrete type is platform-specific (VideoStreamTrack on desktop, null on wasmJs).
     * UI uses an expect/actual VideoView composable to render it.
     */
    val currentVideoTrack: StateFlow<Any?>

    fun createPeerConnection(
        socketId: String,
        isInitiator: Boolean,
    )

    fun closePeerConnection(socketId: String)

    /** Clears the active video track so the UI stops rendering the last frame. */
    fun clearVideoTrack()

    suspend fun handleIceCandidate(packet: Packet.IceCandidateReceived)

    suspend fun handleDescription(packet: Packet.DescriptionReceived)

    fun recreateAllConnections()

    // Media
    suspend fun startLocalMic(): Boolean

    fun toggleMic(isMuted: Boolean)

    /** Starts screen share using [config] and calls [onStreamEnd] when the track ends. */
    suspend fun startScreenShare(config: ScreenShareConfig, onStreamEnd: () -> Unit): Boolean

    /** Convenience overload with default config — keeps existing callers compiling. */
    suspend fun startScreenShare(onStreamEnd: () -> Unit): Boolean = startScreenShare(ScreenShareConfig(), onStreamEnd)

    fun stopScreenShare()

    // Device enumeration
    suspend fun enumerateScreenSources(): List<ScreenSource>

    suspend fun enumerateAudioInputs(): List<AudioDevice>

    suspend fun enumerateAudioOutputs(): List<AudioDevice>

    suspend fun applyDeviceSettings(settings: DeviceSettings)
}

/** Factory — implemented per-platform via expect/actual. */
expect fun createWebRtcManager(
    appStateHolder: WebRtcAppStateHolder,
    websocketService: WebsocketService,
): WebRtcManager
