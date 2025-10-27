package screenshare.common

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import screenshare.common.PacketSide.CLIENT
import screenshare.common.PacketSide.SERVER

enum class PacketSide {
    CLIENT,
    SERVER,
}

@Serializable
sealed class Packet {
    // Client messages
    @Serializable
    @SerialName("join-room")
    data class JoinRoom(val roomId: String, val username: String) : Packet()

    @Serializable
    @SerialName("send-message")
    data class SendChatMessage(val roomId: String, val message: String) : Packet()

    @Serializable
    @SerialName("list-users")
    data class ListUsers(val roomId: String) : Packet()

    @Serializable
    @SerialName("send-ice-candidate")
    data class SendIceCandidate(val roomId: String, val candidate: String, val targetId: String) : Packet()

    @Serializable
    @SerialName("send-description")
    data class SendDescription(
        val roomId: String,
        val targetId: String,
        val description: Map<String, String>,
    ) : Packet()

    @Serializable
    @SerialName("start-screen-share")
    data class StartScreenShare(val roomId: String) : Packet()

    @Serializable
    @SerialName("stop-screen-share")
    data class StopScreenShare(val roomId: String) : Packet()

    // Server messages
    @Serializable
    @SerialName("user-connected")
    data class UserConnected(val roomId: String, val socketId: String, val username: String) : Packet()

    @Serializable
    @SerialName("user-disconnected")
    data class UserDisconnected(val roomId: String, val socketId: String, val username: String) : Packet()

    @Serializable
    @SerialName("chat-message")
    data class ChatMessageReceived(val roomId: String, val message: ChatMessage) : Packet()

    @Serializable
    @SerialName("user-list")
    data class UserList(val roomId: String, val users: List<SocketUser>) : Packet()

    @Serializable
    @SerialName("ice-candidate-received")
    data class IceCandidateReceived(val roomId: String, val candidate: String, val senderId: String) : Packet()

    @Serializable
    @SerialName("description-received")
    data class DescriptionReceived(val roomId: String, val description: Map<String, String>, val senderId: String) :
        Packet()

    @Serializable
    @SerialName("screen-share-started")
    data class ScreenShareStarted(val roomId: String, val senderId: String) : Packet()

    @Serializable
    @SerialName("screen-share-stopped")
    data class ScreenShareStopped(val roomId: String, val senderId: String) : Packet()

    fun getSide(): PacketSide {
        return when (this) {
            is JoinRoom,
            is SendChatMessage,
            is ListUsers,
            is SendIceCandidate,
            is SendDescription,
            is StartScreenShare,
            is StopScreenShare,
            -> CLIENT

            is UserConnected,
            is UserDisconnected,
            is ChatMessageReceived,
            is UserList,
            is IceCandidateReceived,
            is DescriptionReceived,
            is ScreenShareStarted,
            is ScreenShareStopped,
            -> SERVER
        }
    }
}

@Serializable
data class SocketUser(
    val socketId: String,
    val username: String,
    val roomId: String,
)
