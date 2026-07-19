package screenshare.clientkmp.state

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import screenshare.common.ChatMessage
import screenshare.common.SocketUser

data class RoomState(
    val roomId: String,
    val username: String,
    val localSocketId: String? = null,
    val users: List<SocketUser> = emptyList(),
    val chatMessages: List<ChatMessage> = emptyList(),
    val currentSharerSocketId: String? = null,
    val isMicMuted: Boolean = true,
    val speakingUsers: Set<String> = emptySet(),
)

sealed class Screen {
    data object Join : Screen()

    data class Room(val state: RoomState) : Screen()
}

class AppState(val initialRoomId: String? = null) {
    private val _screen = MutableStateFlow<Screen>(Screen.Join)
    val screen: StateFlow<Screen> = _screen.asStateFlow()

    fun navigateToRoom(
        roomId: String,
        username: String,
    ) {
        _screen.value = Screen.Room(RoomState(roomId = roomId, username = username))
    }

    fun updateRoom(transform: (RoomState) -> RoomState) {
        val current = _screen.value
        if (current is Screen.Room) {
            _screen.value = Screen.Room(transform(current.state))
        }
    }

    fun setSpeaking(
        socketId: String,
        isSpeaking: Boolean,
    ) {
        updateRoom { room ->
            val speaking =
                if (isSpeaking) {
                    room.speakingUsers + socketId
                } else {
                    room.speakingUsers - socketId
                }
            room.copy(speakingUsers = speaking)
        }
    }

    val currentRoom: RoomState?
        get() = (_screen.value as? Screen.Room)?.state
}
