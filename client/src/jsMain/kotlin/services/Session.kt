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
    var currentSharerSocketId: String? = null,
    var userList: List<SocketUser> = emptyList(),
    val websocketService: WebsocketService,
    coroutineScope: CoroutineScope,
) : CoroutineScope by coroutineScope {
    val voiceChat = VoiceChat()
    val screenSharing = ScreenSharing()
    val peerConnections = PeerConnections(voiceChat, screenSharing)

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
            }.onFailure { error ->
                console.error("Error populating audio devices", error)
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
                }.onFailure { error ->
                    console.error("Error getting microphone", error)
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
    ) =
        launch {
            screenSharing.setupLocalScreenStream(
                width = width,
                height = height,
                frameRate = fps,
                useSourceResolution = useSourceResolution,
                onStreamEnd = {
                    handleStopScreenShare()
                },
                recreatePeerConnections = { recreatePeerConnections() },
            )
            websocketService.startScreenSharing(localRoomId)
        }

    fun handleStopScreenShare() =
        launch {
            screenSharing.stopScreenSharing(recreatePeerConnections = { recreatePeerConnections() })
            websocketService.stopScreenSharing(localRoomId)
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
            }.onFailure { error ->
                console.error("Error switching microphone device", error)
                window.alert("Erro ao trocar o microfone")
            }
        }

    fun handleSpeakerOutputDeviceChange(deviceId: String) {
        InterfaceMutations.setOutputDevice(deviceId)
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
