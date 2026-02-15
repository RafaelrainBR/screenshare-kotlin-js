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
    data class JoinRoom(
        @SerialName("rid")
        val roomId: String,
        val username: String,
    ) : Packet()

    @Serializable
    @SerialName("send-message")
    data class SendChatMessage(
        @SerialName("rid")
        val roomId: String,
        @SerialName("msg")
        val message: String,
    ) : Packet()

    @Serializable
    @SerialName("list-users")
    data class ListUsers(
        @SerialName("rid")
        val roomId: String,
    ) : Packet()

    @Serializable
    @SerialName("send-ice-candidate")
    data class SendIceCandidate(
        @SerialName("rid")
        val roomId: String,
        @SerialName("ice")
        val candidate: String,
        @SerialName("tid")
        val targetId: String,
    ) : Packet()

    @Serializable
    @SerialName("send-description")
    data class SendDescription(
        @SerialName("rid")
        val roomId: String,
        @SerialName("tid")
        val targetId: String,
        val description: Map<String, String>,
    ) : Packet()

    @Serializable
    @SerialName("start-screen-share")
    data class StartScreenShare(
        @SerialName("rid")
        val roomId: String,
    ) : Packet()

    @Serializable
    @SerialName("stop-screen-share")
    data class StopScreenShare(
        @SerialName("rid")
        val roomId: String,
    ) : Packet()

    @Serializable
    @SerialName("send-muted")
    data class SendMuted(
        @SerialName("rid")
        val roomId: String,
    ) : Packet()

    @Serializable
    @SerialName("send-unmuted")
    data class SendUnmuted(
        @SerialName("rid")
        val roomId: String,
    ) : Packet()

    // Server messages
    @Serializable
    @SerialName("user-connected")
    data class UserConnected(
        @SerialName("rid")
        val roomId: String,
        @SerialName("sid")
        val socketId: String,
        val username: String,
    ) : Packet()

    @Serializable
    @SerialName("user-disconnected")
    data class UserDisconnected(
        @SerialName("rid")
        val roomId: String,
        @SerialName("sid")
        val socketId: String,
        val username: String,
    ) : Packet()

    @Serializable
    @SerialName("chat-message")
    data class ChatMessageReceived(
        @SerialName("rid")
        val roomId: String,
        @SerialName("msg")
        val message: ChatMessage,
    ) : Packet()

    @Serializable
    @SerialName("user-list")
    data class UserList(
        @SerialName("rid")
        val roomId: String,
        @SerialName("users")
        val users: List<SocketUser>,
    ) : Packet()

    @Serializable
    @SerialName("ice-candidate-received")
    data class IceCandidateReceived(
        @SerialName("rid")
        val roomId: String,
        @SerialName("ice")
        val candidate: String,
        @SerialName("sid")
        val senderId: String,
    ) : Packet()

    @Serializable
    @SerialName("description-received")
    data class DescriptionReceived(
        @SerialName("rid")
        val roomId: String,
        @SerialName("sdp")
        val description: Map<String, String>,
        @SerialName("sid")
        val senderId: String,
    ) : Packet()

    @Serializable
    @SerialName("screen-share-started")
    data class ScreenShareStarted(
        @SerialName("rid")
        val roomId: String,
        @SerialName("sid")
        val senderId: String,
    ) : Packet()

    @Serializable
    @SerialName("screen-share-stopped")
    data class ScreenShareStopped(
        @SerialName("rid")
        val roomId: String,
        @SerialName("sid")
        val senderId: String,
    ) : Packet()

    @Serializable
    @SerialName("user-muted")
    data class UserMuted(
        @SerialName("rid")
        val roomId: String,
        @SerialName("sid")
        val socketId: String,
    ) : Packet()

    @Serializable
    @SerialName("user-unmuted")
    data class UserUnmuted(
        @SerialName("rid")
        val roomId: String,
        @SerialName("sid")
        val socketId: String,
    ) : Packet()

    fun getSide(): PacketSide =
        when (this) {
            is JoinRoom,
            is SendChatMessage,
            is ListUsers,
            is SendIceCandidate,
            is SendDescription,
            is StartScreenShare,
            is StopScreenShare,
            is SendMuted,
            is SendUnmuted,
            -> CLIENT

            is UserConnected,
            is UserDisconnected,
            is ChatMessageReceived,
            is UserList,
            is IceCandidateReceived,
            is DescriptionReceived,
            is ScreenShareStarted,
            is ScreenShareStopped,
            is UserMuted,
            is UserUnmuted,
            -> SERVER
        }
}

@Serializable
data class SocketUser(
    val socketId: String,
    val username: String,
    val roomId: String,
    val isMuted: Boolean = true,
)
