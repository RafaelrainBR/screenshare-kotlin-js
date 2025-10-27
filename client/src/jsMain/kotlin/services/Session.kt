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
    val peerConnections = PeerConnections(voiceChat = voiceChat)

    init {
        launch {
            InterfaceMutations.navigateToRoomScreen(
                roomId = localRoomId,
                username = localUsername
            )
            websocketService.joinRoom(
                roomId = localRoomId,
                username = localUsername,
            )
            InterfaceMutations.addMessageToChat(
                ChatMessage(
                    username = "Sistema",
                    content = "Você entrou na sala $localRoomId",
                    Date().getTime().toLong()
                ),
                localUsername = localUsername
            )
        }
    }

    fun handleMessageSend(message: String) = launch {
        websocketService.sendChatMessage(
            roomId = localRoomId,
            message = message,
        )
    }

    fun handleMicButtonToggle() = launch {
        if (voiceChat.localMicStream == null) {
            runCatching {
                voiceChat.setupLocalMic(
                    recreatePeerConnections = {
                        peerConnections.recreatePeerConnections(
                            websocketService = websocketService,
                            roomId = localRoomId,
                            isInitiator = true,
                            coroutineScope = this
                        )
                    }
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
            }
        )
    }
}
