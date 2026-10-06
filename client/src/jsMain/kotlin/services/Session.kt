package services

import hideAfterLocalShare
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import screenshare.common.ChatMessage
import screenshare.common.SocketUser
import platform.DesktopEnvironment
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
            websocketService.requestMediaState(localRoomId)
            runCatching {
                InterfaceMutations.populateAudioDevices()
            }.onFailure {
                console.warn("Não foi possível atualizar os dispositivos de áudio")
            }
            InterfaceMutations.addMessageToChat(
                ChatMessage(
                    username = "Sistema",
                    content = "Você entrou na sala $localRoomId",
                    Date().getTime().toLong(),
                ),
                localUsername = localUsername,
            )
        }
    }

    fun handleMessageSend(message: String) =
        launch {
            websocketService.sendChatMessage(
                roomId = localRoomId,
                message = message,
            )
        }

    fun handleMicButtonToggle() =
        launch {
            if (voiceChat.localMicStream == null) {
                runCatching {
                    voiceChat.setupLocalMic(
                        recreatePeerConnections = { recreatePeerConnections() },
                        onMicTrackReplaced = { oldTrackId, newTrack ->
                            if (newTrack != null) {
                                peerConnections.replaceMicTrack(oldTrackId, newTrack)
                            }
                        },
                    )
                }.onFailure {
                    console.warn("Não foi possível acessar o microfone")
                    runCatching { DesktopEnvironment.showMainWindow() }
                    InterfaceMutations.showPermissionRecovery("microphone", it.asDynamic().name as? String)
                    return@launch
                }
            }

            voiceChat.toggleMute(
                broadcastMuted = { isMuted ->
                    websocketService.sendToggleMute(roomId = localRoomId, isMuted = isMuted)
                },
            )
            runCatching { DesktopEnvironment.setTrayState("mic", !voiceChat.isMicMuted) }
        }

    fun handleStartScreenShare(
        width: Int,
        height: Int,
        fps: Int,
        useSourceResolution: Boolean,
        desktopAudioMode: ScreenSharing.DesktopAudioMode = ScreenSharing.DesktopAudioMode.NONE,
        audioSourceProcessId: Int? = null,
    ) {
        val generation = ++screenShareGeneration
        launch {
            if (disposed) return@launch
            if (screenSharing.localScreenStream != null) {
                screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
                websocketService.stopScreenSharing(localRoomId)
            }
            runCatching {
                screenSharing.setupLocalScreenStream(
                    width = width,
                    height = height,
                    frameRate = fps,
                    useSourceResolution = useSourceResolution,
                    desktopAudioMode = desktopAudioMode,
                    audioSourceProcessId = audioSourceProcessId,
                    onStreamEnd = { handleStopScreenShare() },
                    recreatePeerConnections = { recreatePeerConnections() },
                )
            }.onFailure {
                // The OS picker is allowed to be cancelled. Keep the user in the room
                // with a recovery path instead of leaving a partial share behind.
                InterfaceMutations.reopenDesktopShareWizard()
                InterfaceMutations.showPermissionRecovery("screen", it.asDynamic().name as? String)
                hideAfterLocalShare = false
                return@launch
            }
            if (disposed || generation != screenShareGeneration) {
                screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
                hideAfterLocalShare = false
                return@launch
            }
            websocketService.startScreenSharing(localRoomId)
            runCatching { DesktopEnvironment.setTrayState("screen", true) }
            if (hideAfterLocalShare) {
                hideAfterLocalShare = false
                runCatching { DesktopEnvironment.hideMainWindow() }
            }
        }
    }

    fun handleStopScreenShare() {
        ++screenShareGeneration
        launch {
            screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
            websocketService.stopScreenSharing(localRoomId)
            runCatching { DesktopEnvironment.setTrayState("screen", false) }
            if (hideAfterLocalShare) {
                hideAfterLocalShare = false
                runCatching { DesktopEnvironment.hideMainWindow() }
            }
        }
    }

    fun handleStartCameraShare() =
        launch {
            runCatching {
                cameraSharing.setupLocalCameraStream(
                    recreatePeerConnections = { recreatePeerConnections() },
                    onStreamEnd = {
                        handleStopCameraShare()
                    },
                    deviceId = selectedCameraDeviceId,
                )
                websocketService.startCameraShare(localRoomId)
                runCatching { DesktopEnvironment.setTrayState("camera", true) }
                if (hideAfterLocalShare) {
                    hideAfterLocalShare = false
                    runCatching { DesktopEnvironment.hideMainWindow() }
                }
                runCatching { InterfaceMutations.populateAudioDevices() }
            }.onFailure {
                console.warn("Não foi possível acessar a câmera")
                InterfaceMutations.showPermissionRecovery("camera", it.asDynamic().name as? String)
                hideAfterLocalShare = false
            }
        }

    fun handleStopCameraShare() =
        launch {
            cameraSharing.stopCameraSharing(recreatePeerConnections = { recreatePeerConnections() })
            websocketService.stopCameraShare(localRoomId)
            runCatching { DesktopEnvironment.setTrayState("camera", false) }
            if (hideAfterLocalShare) {
                hideAfterLocalShare = false
                runCatching { DesktopEnvironment.hideMainWindow() }
            }
        }

    fun handleCameraDeviceChange(deviceId: String) = launch {
        val previous = selectedCameraDeviceId
        selectedCameraDeviceId = deviceId.takeIf { it.isNotBlank() }
        if (cameraSharing.localCameraStream == null) return@launch
        runCatching {
            cameraSharing.setupLocalCameraStream(
                recreatePeerConnections = { recreatePeerConnections() },
                onStreamEnd = { handleStopCameraShare() },
                deviceId = selectedCameraDeviceId,
            )
            InterfaceMutations.populateAudioDevices()
        }.onFailure {
            selectedCameraDeviceId = previous
            InterfaceMutations.restoreCameraDevice(previous)
            InterfaceMutations.showError("Não foi possível trocar a câmera. Escolha outro dispositivo.")
        }
    }

    fun handleMicInputDeviceChange(deviceId: String) =
        launch {
            val requested = deviceId.takeIf { it.isNotBlank() }
            val previous = voiceChat.selectedInputDeviceId
            if (voiceChat.localMicStream == null) {
                voiceChat.selectedInputDeviceId = requested
                return@launch
            }

            runCatching {
                if (requested == null) voiceChat.selectedInputDeviceId = null
                voiceChat.setupLocalMic(
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
                voiceChat.selectedInputDeviceId = previous
                InterfaceMutations.restoreInputDevice(previous)
                console.warn("Não foi possível trocar o microfone")
                InterfaceMutations.showError("Não foi possível trocar o microfone. Escolha outro dispositivo.")
            }
        }

    fun handleSpeakerOutputDeviceChange(deviceId: String) {
        InterfaceMutations.setOutputDevice(deviceId)
    }

    fun setVideoReception(enabled: Boolean) =
        peerConnections.setVideoReception(enabled)

    fun prepareForReconnect() {
        peerConnections.closeAll()
        screenSharing.remoteScreenStreams.keys.toList().forEach(screenSharing::stopRemoteScreen)
        cameraSharing.remoteCameraStreams.keys.toList().forEach(cameraSharing::stopRemoteCamera)
        voiceChat.clearRemoteAudio()
        UserListMutations.clearUserMediaSharing()
    }

    /** Releases local capture without attempting room signaling during page exit. */
    fun dispose() {
        if (disposed) return
        disposed = true
        ++screenShareGeneration
        screenSharing.stopScreenSharing(recreatePeerConnections = {})
        cameraSharing.stopCameraSharing(recreatePeerConnections = {})
        voiceChat.stopLocalMic()
        peerConnections.closeAll()
        launch {
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
