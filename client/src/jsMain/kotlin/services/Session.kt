package services

import kotlinx.browser.window
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import screenshare.common.ChatMessage
import screenshare.common.SocketUser
import ui.InterfaceMutations
import kotlin.js.Date

class Session(
    var localUsername: String,
    var localRoomId: String,
    var userList: List<SocketUser> = emptyList(),
    val websocketService: WebsocketService,
    coroutineScope: CoroutineScope,
) : CoroutineScope by coroutineScope {
    private var disposed = false
    // Incremented synchronously by every user action. A delayed display picker
    // result is therefore unable to publish a share after stop, room exit, or
    // a newer replacement request.
    private var screenShareGeneration = 0
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
                    window.alert("Permissao de mic necessária")
                    return@launch
                }
            }

            voiceChat.toggleMute(
                broadcastMuted = { isMuted ->
                    websocketService.sendToggleMute(roomId = localRoomId, isMuted = isMuted)
                },
            )
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
                // The OS picker is allowed to be cancelled. No native worker
                // has started yet, so simply return the user to their local
                // audio selection instead of leaving a partial share behind.
                InterfaceMutations.reopenDesktopShareWizard()
                return@launch
            }
            if (disposed || generation != screenShareGeneration) {
                screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
                return@launch
            }
            websocketService.startScreenSharing(localRoomId)
        }
    }

    fun handleStopScreenShare() {
        ++screenShareGeneration
        launch {
            screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
            websocketService.stopScreenSharing(localRoomId)
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
                )
                websocketService.startCameraShare(localRoomId)
            }.onFailure {
                console.warn("Não foi possível acessar a câmera")
                window.alert("Permissao de câmera necessária")
            }
        }

    fun handleStopCameraShare() =
        launch {
            cameraSharing.stopCameraSharing(recreatePeerConnections = { recreatePeerConnections() })
            websocketService.stopCameraShare(localRoomId)
        }

    fun handleMicInputDeviceChange(deviceId: String) =
        launch {
            if (deviceId.isBlank()) return@launch

            runCatching {
                voiceChat.setupLocalMic(
                    recreatePeerConnections = { recreatePeerConnections() },
                    deviceId = deviceId,
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
                console.warn("Não foi possível trocar o microfone")
                window.alert("Erro ao trocar o microfone")
            }
        }

    fun handleSpeakerOutputDeviceChange(deviceId: String) {
        InterfaceMutations.setOutputDevice(deviceId)
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
