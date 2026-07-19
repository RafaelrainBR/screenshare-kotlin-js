package screenshare.clientkmp.services

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import screenshare.clientkmp.state.AppState
import screenshare.clientkmp.util.currentTimeMillis
import screenshare.common.ChatMessage

class SessionManager(
    private val appState: AppState,
    private val websocketService: WebsocketService,
    private val coroutineScope: CoroutineScope,
) {
    val webRtcManager: WebRtcManager =
        createWebRtcManager(
            appStateHolder = WebRtcAppStateHolder(appState, coroutineScope),
            websocketService = websocketService,
        )

    /** Current video track for display (remote or local screen share). Platform-typed as Any?. */
    val currentVideoTrack: StateFlow<Any?> get() = webRtcManager.currentVideoTrack

    init {
        websocketService.onClose = ::handleDisconnection
    }

    fun joinRoom(
        username: String,
        roomId: String,
    ) {
        coroutineScope.launch {
            appState.navigateToRoom(roomId, username)
            websocketService.joinRoom(roomId, username)
            appState.updateRoom { room ->
                room.copy(
                    chatMessages = room.chatMessages + systemMessage("Você entrou na sala "),
                )
            }
        }
    }

    fun sendMessage(message: String) {
        val room = appState.currentRoom ?: return
        coroutineScope.launch {
            websocketService.sendChatMessage(room.roomId, message)
        }
    }

    fun toggleMic() {
        val room = appState.currentRoom ?: return
        coroutineScope.launch {
            if (room.isMicMuted) {
                // Unmuting: ensure local mic stream is set up
                val started = webRtcManager.startLocalMic()
                if (!started) return@launch
            }
            val newMuted = !room.isMicMuted
            appState.updateRoom { it.copy(isMicMuted = newMuted) }
            webRtcManager.toggleMic(newMuted)
            websocketService.sendToggleMute(room.roomId, newMuted)
        }
    }

    fun startScreenShare(config: ScreenShareConfig = ScreenShareConfig()) {
        val room = appState.currentRoom ?: return
        coroutineScope.launch {
            val started = webRtcManager.startScreenShare(config) {
                stopScreenShare()
            }
            if (started) {
                websocketService.startScreenSharing(room.roomId)
            }
        }
    }

    suspend fun getScreenSources(): List<ScreenSource> = webRtcManager.enumerateScreenSources()

    suspend fun getAudioDevices(): Pair<List<AudioDevice>, List<AudioDevice>> {
        val inputs = webRtcManager.enumerateAudioInputs()
        val outputs = webRtcManager.enumerateAudioOutputs()
        return Pair(inputs, outputs)
    }

    fun applyDeviceSettings(settings: DeviceSettings) {
        coroutineScope.launch {
            webRtcManager.applyDeviceSettings(settings)
        }
    }

    fun stopScreenShare() {
        val room = appState.currentRoom ?: return
        coroutineScope.launch {
            webRtcManager.stopScreenShare()
            websocketService.stopScreenSharing(room.roomId)
            appState.updateRoom { it.copy(currentSharerSocketId = null) }
        }
    }

    private fun handleDisconnection() {
        coroutineScope.launch {
            appState.updateRoom { room ->
                room.copy(chatMessages = room.chatMessages + systemMessage("Conexão encerrada. Reconectando..."))
            }
            reconnectWithBackoff()
        }
    }

    private suspend fun reconnectWithBackoff() {
        val savedRoom = appState.currentRoom
        val backoffDelays = listOf(1_000L, 2_000L, 4_000L, 8_000L, 16_000L, 30_000L)
        for (attempt in 0 until 10) {
            val waitMs = backoffDelays.getOrElse(attempt) { 30_000L }
            delay(waitMs)
            val success =
                runCatching {
                    websocketService.connect(coroutineScope)
                    savedRoom?.let { room ->
                        websocketService.joinRoom(room.roomId, room.username)
                        appState.updateRoom { r ->
                            r.copy(chatMessages = r.chatMessages + systemMessage("Reconectado à sala."))
                        }
                    }
                }.isSuccess
            if (success) return
            println("[Session] reconexão tentativa ${attempt + 1} falhou")
        }
        appState.updateRoom { r ->
            r.copy(chatMessages = r.chatMessages + systemMessage("Não foi possível reconectar."))
        }
    }

    private fun systemMessage(content: String) =
        ChatMessage(
            username = "Sistema",
            content = content,
            timestamp = currentTimeMillis(),
        )
}
