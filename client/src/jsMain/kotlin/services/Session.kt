package services

import hideAfterLocalShare
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import platform.DesktopCaptureException
import platform.DesktopCaptureSource
import platform.DesktopEnvironment
import screenshare.common.ChatMessage
import screenshare.common.SocketUser
import ui.InterfaceMutations
import ui.mutations.UserListMutations
import kotlin.js.Date

class Session(
    var localUsername: String,
    var localRoomId: String,
    var userList: List<SocketUser> = emptyList(),
    var localSocketId: String? = null,
    val websocketService: WebsocketService,
    coroutineScope: CoroutineScope,
) : CoroutineScope by coroutineScope {
    private var disposed = false
    private var mediaAvailable =
        !DesktopEnvironment.isAvailable || window.asDynamic().__screenshareDesktopBridge.mediaAvailable != false
    private val mediaCaptureGate = MediaCaptureGate()

    // Incremented synchronously by every user action. A delayed display picker
    // result is therefore unable to publish a share after stop, room exit, or
    // a newer replacement request.
    private var screenShareGeneration = 0
    private var selectedCameraDeviceId: String? = null
    val voiceChat = VoiceChat()
    val screenSharing =
        ScreenSharing(
            resolveUsername = { socketId ->
                userList.firstOrNull { it.socketId == socketId }?.username ?: socketId.takeLast(6)
            },
            localUsername = localUsername,
            coroutineScope = this,
        )
    val cameraSharing =
        CameraSharing(
            resolveUsername = { socketId ->
                userList.firstOrNull { it.socketId == socketId }?.username ?: socketId.takeLast(6)
            },
            localUsername = localUsername,
        )
    val peerConnections = PeerConnections(voiceChat, screenSharing, cameraSharing)

    init {
        peerConnections.setVideoReception(
            platform.shouldReceiveVideo(
                DesktopEnvironment.isAvailable,
                DesktopEnvironment.viewerMode,
                kotlinx.browser.document
                    .asDynamic()
                    .hidden as Boolean,
            ),
        )
        mediaCaptureGate.setAvailable(false)
        UserListMutations.clearUserMediaSharing()
        launch {
            InterfaceMutations.navigateToRoomScreen(
                roomId = localRoomId,
                username = localUsername,
            )
            websocketService.joinRoom(
                roomId = localRoomId,
                username = localUsername,
            )
            runCatching {
                InterfaceMutations.populateAudioDevices()
            }.onFailure {
                console.warn("Não foi possível atualizar os dispositivos de áudio")
            }
        }
    }

    suspend fun handleMessageSend(message: String) {
        check(!disposed) { "Room was left" }
        websocketService.sendChatMessage(localRoomId, message)
    }

    fun confirmRoomJoined(socketId: String) {
        localSocketId = socketId
        mediaCaptureGate.setAvailable(mediaAvailable)
        InterfaceMutations.updateRoomConnection(true)
        InterfaceMutations.addMessageToChat(
            ChatMessage("Sistema", "Você entrou na sala $localRoomId", Date().getTime().toLong()),
            localSocketId,
        )
        launch { websocketService.requestMediaState(localRoomId) }
    }

    fun handleMicButtonToggle() =
        launch {
            if (disposed || !mediaCaptureGate.available) return@launch
            val request = mediaCaptureGate.request()
            if (voiceChat.localMicStream == null) {
                runCatching {
                    voiceChat.setupLocalMic(
                        isRequestCurrent = request,
                        recreatePeerConnections = { recreatePeerConnections() },
                        onMicTrackReplaced = { oldTrackId, newTrack ->
                            if (newTrack != null) {
                                peerConnections.replaceMicTrack(oldTrackId, newTrack)
                            }
                        },
                    )
                }.onFailure {
                    if (!request()) return@launch
                    console.warn("Não foi possível acessar o microfone")
                    runCatching { DesktopEnvironment.showMainWindow() }
                    InterfaceMutations.showPermissionRecovery("microphone", it.asDynamic().name as? String)
                    return@launch
                }
            }

            if (!request()) return@launch
            voiceChat.toggleMute(
                broadcastMuted = { isMuted ->
                    websocketService.sendToggleMute(roomId = localRoomId, isMuted = isMuted)
                },
            )
            if (!request()) return@launch
            runCatching { DesktopEnvironment.setTrayState("mic", !voiceChat.isMicMuted) }
        }

    fun handleStartScreenShare(
        width: Int,
        height: Int,
        profile: ShareProfile,
        useSourceResolution: Boolean,
        desktopAudioMode: ScreenSharing.DesktopAudioMode = ScreenSharing.DesktopAudioMode.NONE,
        nativeSource: DesktopCaptureSource? = null,
    ) {
        if (disposed || !mediaCaptureGate.available) return
        val request = mediaCaptureGate.request()
        val generation = ++screenShareGeneration
        launch {
            if (disposed || !request() || generation != screenShareGeneration) return@launch
            runCatching {
                screenSharing.setupLocalScreenStream(
                    isRequestCurrent = { request() && generation == screenShareGeneration },
                    width = width,
                    height = height,
                    profile = profile,
                    useSourceResolution = useSourceResolution,
                    desktopAudioMode = desktopAudioMode,
                    nativeSource = nativeSource,
                    onStreamEnd = { handleStopScreenShare() },
                    recreatePeerConnections = { recreatePeerConnections() },
                )
            }.onFailure {
                if (!request() || generation != screenShareGeneration) return@launch
                // The OS picker is allowed to be cancelled. Keep the user in the room
                // with a recovery path instead of leaving a partial share behind.
                InterfaceMutations.reopenDesktopShareWizard()
                if (it is DesktopCaptureException && it.code == "screen-recording-denied") {
                    InterfaceMutations.showError(it.message ?: "")
                } else if (it is DesktopCaptureException) {
                    // On Windows native capture needs no permission; it failed on the chosen source.
                    InterfaceMutations.showError(
                        "Não foi possível compartilhar ${nativeSource?.title ?: "a fonte"}. Escolha outra ou tente de novo.",
                    )
                } else {
                    InterfaceMutations.showPermissionRecovery("screen", it.asDynamic().name as? String)
                }
                hideAfterLocalShare = false
                return@launch
            }
            if (disposed || !request() || generation != screenShareGeneration) {
                screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
                hideAfterLocalShare = false
                return@launch
            }
            websocketService.startScreenSharing(localRoomId)
            if (!request()) {
                runCatching { websocketService.stopScreenSharing(localRoomId) }
                return@launch
            }
            runCatching { DesktopEnvironment.setTrayState("screen", true, detail = nativeSource?.title) }
            if (hideAfterLocalShare) {
                hideAfterLocalShare = false
                runCatching { DesktopEnvironment.hideMainWindow() }
            }
        }
    }

    /** Applies [profile] to the running share without restarting it. */
    fun updateShareProfile(profile: ShareProfile) =
        launch {
            if (screenSharing.updateProfile(profile)) peerConnections.applyShareEncoding()
        }

    fun handleStopScreenShare() {
        ++screenShareGeneration
        launch {
            screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
            runCatching { websocketService.stopScreenSharing(localRoomId) }
            runCatching { DesktopEnvironment.setTrayState("screen", false) }
            if (hideAfterLocalShare) {
                hideAfterLocalShare = false
                runCatching { DesktopEnvironment.hideMainWindow() }
            }
        }
    }

    fun handleStartCameraShare() =
        launch {
            if (disposed || !mediaCaptureGate.available) return@launch
            val request = mediaCaptureGate.request()
            runCatching {
                cameraSharing.setupLocalCameraStream(
                    isRequestCurrent = request,
                    recreatePeerConnections = { recreatePeerConnections() },
                    onStreamEnd = {
                        handleStopCameraShare()
                    },
                    deviceId = selectedCameraDeviceId,
                )
                if (!request()) return@launch
                websocketService.startCameraShare(localRoomId)
                if (!request()) {
                    runCatching { websocketService.stopCameraShare(localRoomId) }
                    return@launch
                }
                runCatching { DesktopEnvironment.setTrayState("camera", true) }
                runCatching { InterfaceMutations.populateAudioDevices() }
            }.onFailure {
                if (!request()) return@launch
                console.warn("Não foi possível acessar a câmera")
                runCatching { DesktopEnvironment.showMainWindow() }
                InterfaceMutations.showPermissionRecovery("camera", it.asDynamic().name as? String)
            }
        }

    fun handleStopCameraShare() =
        launch {
            cameraSharing.stopCameraSharing(recreatePeerConnections = { recreatePeerConnections() })
            runCatching { websocketService.stopCameraShare(localRoomId) }
            runCatching { DesktopEnvironment.setTrayState("camera", false) }
        }

    fun handleCameraDeviceChange(deviceId: String) =
        launch {
            if (disposed || !mediaCaptureGate.available) return@launch
            val request = mediaCaptureGate.request()
            val previous = selectedCameraDeviceId
            selectedCameraDeviceId = deviceId.takeIf { it.isNotBlank() }
            if (cameraSharing.localCameraStream == null) return@launch
            runCatching {
                cameraSharing.setupLocalCameraStream(
                    isRequestCurrent = request,
                    recreatePeerConnections = { recreatePeerConnections() },
                    onStreamEnd = { handleStopCameraShare() },
                    deviceId = selectedCameraDeviceId,
                )
                InterfaceMutations.populateAudioDevices()
            }.onFailure {
                if (!request()) return@launch
                selectedCameraDeviceId = previous
                InterfaceMutations.restoreCameraDevice(previous)
                InterfaceMutations.showError("Não foi possível trocar a câmera. Escolha outro dispositivo.")
            }
        }

    fun handleMicInputDeviceChange(deviceId: String) =
        launch {
            if (disposed || !mediaCaptureGate.available) return@launch
            val request = mediaCaptureGate.request()
            val requested = deviceId.takeIf { it.isNotBlank() }
            val previous = voiceChat.selectedInputDeviceId
            if (voiceChat.localMicStream == null) {
                voiceChat.selectedInputDeviceId = requested
                return@launch
            }

            runCatching {
                if (requested == null) voiceChat.selectedInputDeviceId = null
                voiceChat.setupLocalMic(
                    isRequestCurrent = request,
                    recreatePeerConnections = { recreatePeerConnections() },
                    deviceId = requested,
                    onMicTrackReplaced = { oldTrackId, newTrack ->
                        if (newTrack != null) {
                            peerConnections.replaceMicTrack(oldTrackId, newTrack)
                        }
                    },
                )
                if (voiceChat.isMicMuted) {
                    voiceChat.localMicStream?.getTracks()?.forEach { track -> track.enabled = false }
                }
            }.onFailure {
                if (!request()) return@launch
                voiceChat.selectedInputDeviceId = previous
                InterfaceMutations.restoreInputDevice(previous)
                console.warn("Não foi possível trocar o microfone")
                InterfaceMutations.showError("Não foi possível trocar o microfone. Escolha outro dispositivo.")
            }
        }

    fun handleSpeakerOutputDeviceChange(deviceId: String) {
        InterfaceMutations.setOutputDevice(deviceId)
    }

    fun setVideoReception(enabled: Boolean) = peerConnections.setVideoReception(enabled)

    fun prepareForReconnect() {
        mediaCaptureGate.setAvailable(false)
        stopSendingMedia(recreatePeers = false)
        peerConnections.closeAll()
        screenSharing.remoteScreenStreams.keys
            .toList()
            .forEach(screenSharing::stopRemoteScreen)
        cameraSharing.remoteCameraStreams.keys
            .toList()
            .forEach(cameraSharing::stopRemoteCamera)
        voiceChat.clearRemoteAudio()
        UserListMutations.clearUserMediaSharing()
        localSocketId = null
        userList = emptyList()
        UserListMutations.updateUserList(emptyList(), null)
        InterfaceMutations.updateRoomConnection(false, "Sem conexão. Tentando reconectar…")
        launch { runCatching { DesktopEnvironment.setTrayState("remote", false) } }
    }

    /** Releases local capture without attempting room signaling during page exit. */
    fun dispose() {
        if (disposed) return
        disposed = true
        mediaCaptureGate.setAvailable(false)
        ++screenShareGeneration
        screenSharing.stopScreenSharing(recreatePeerConnections = {})
        cameraSharing.stopCameraSharing(recreatePeerConnections = {})
        voiceChat.stopLocalMic()
        InterfaceMutations.updateAudioControls(isMicMuted = true)
        peerConnections.closeAll()
        screenSharing.remoteScreenStreams.keys
            .toList()
            .forEach(screenSharing::stopRemoteScreen)
        cameraSharing.remoteCameraStreams.keys
            .toList()
            .forEach(cameraSharing::stopRemoteCamera)
        voiceChat.clearRemoteAudio()
        UserListMutations.clearUserMediaSharing()
        hideAfterLocalShare = false
        launch {
            runCatching { DesktopEnvironment.setTrayState("mic", false) }
            runCatching { DesktopEnvironment.setTrayState("camera", false) }
            runCatching { DesktopEnvironment.setTrayState("screen", false) }
        }
    }

    fun setMediaAvailable(available: Boolean) {
        mediaAvailable = available
        mediaCaptureGate.setAvailable(available && websocketService.isRoomJoined)
        if (!available) stopSendingMedia()
    }

    private fun stopSendingMedia(recreatePeers: Boolean = true) {
        mediaCaptureGate.invalidate()
        ++screenShareGeneration
        hideAfterLocalShare = false
        // Stop tracks synchronously, before any network or tray update can suspend.
        screenSharing.stopScreenSharing(recreatePeerConnections = {})
        cameraSharing.stopCameraSharing(recreatePeerConnections = {})
        voiceChat.stopLocalMic()
        voiceChat.isMicMuted = true
        InterfaceMutations.updateAudioControls(isMicMuted = true)
        if (recreatePeers) recreatePeerConnections()
        launch {
            runCatching { websocketService.stopScreenSharing(localRoomId) }
            runCatching { websocketService.stopCameraShare(localRoomId) }
            runCatching { websocketService.sendToggleMute(localRoomId, true) }
            runCatching { DesktopEnvironment.setTrayState("mic", false) }
            runCatching { DesktopEnvironment.setTrayState("camera", false) }
            runCatching { DesktopEnvironment.setTrayState("screen", false) }
        }
    }

    private fun recreatePeerConnections() {
        peerConnections.recreatePeerConnections(
            websocketService = websocketService,
            roomId = localRoomId,
            isInitiator = true,
            coroutineScope = this,
        )
    }
}
