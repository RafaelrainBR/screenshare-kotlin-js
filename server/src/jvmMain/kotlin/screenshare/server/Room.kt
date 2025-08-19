package screenshare.server

import io.ktor.websocket.Frame.Text
import kotlinx.serialization.json.Json
import org.slf4j.LoggerFactory
import screenshare.common.ChatMessage
import screenshare.common.Packet
import screenshare.common.Packet.ChatMessageReceived
import screenshare.common.Packet.DescriptionReceived
import screenshare.common.Packet.IceCandidateReceived
import screenshare.common.Packet.ListUsers
import screenshare.common.Packet.ScreenShareStarted
import screenshare.common.Packet.ScreenShareStopped
import screenshare.common.Packet.SendChatMessage
import screenshare.common.Packet.SendDescription
import screenshare.common.Packet.SendIceCandidate
import screenshare.common.Packet.StartScreenShare
import screenshare.common.Packet.UserConnected
import screenshare.common.Packet.UserDisconnected
import screenshare.common.Packet.UserList
import screenshare.common.PacketSide.CLIENT
import screenshare.common.SocketUser

class Room(
    val id: String,
    private val users: MutableMap<String, RoomUser> = mutableMapOf(),
) {
    private val logger = LoggerFactory.getLogger(Room::class.java)

    val isEmpty: Boolean
        get() = users.isEmpty()

    val allUsers: List<RoomUser>
        get() = users.values.toList()

    suspend fun addUser(user: RoomUser) {
        users[user.id] = user
        notifyUserJoin(user)
    }

    suspend fun removeUser(user: RoomUser) {
        users.remove(user.id)
        notifyUserLeave(user)
    }

    suspend fun consumePacket(
        user: RoomUser,
        packet: Packet,
    ) {
        if (packet.getSide() != CLIENT) return

        logger.info("Room [$id] received from [${user.username}] the packet [$packet]")

        when (packet) {
            is SendChatMessage -> {
                val chatMessage =
                    ChatMessage(
                        username = user.username,
                        content = packet.message,
                        timestamp = System.currentTimeMillis(),
                    )
                broadcast(ChatMessageReceived(roomId = id, message = chatMessage))
            }

            is ListUsers -> {
                user.sendPacket(createUserListPacket())
            }

            is SendIceCandidate -> {
                getUserById(packet.targetId)
                    ?.sendPacket(
                        IceCandidateReceived(
                            roomId = id,
                            candidate = packet.candidate,
                            senderId = user.id,
                        ),
                    )
            }

            is SendDescription -> {
                getUserById(packet.targetId)
                    ?.sendPacket(
                        DescriptionReceived(
                            roomId = id,
                            description = packet.description,
                            senderId = user.id,
                        ),
                    )
            }

            is StartScreenShare -> {
                broadcast(ScreenShareStarted(roomId = id, senderId = user.id))
            }

            is Packet.StopScreenShare -> {
                broadcast(ScreenShareStopped(roomId = id, senderId = user.id))
            }

            else -> {}
        }
    }

    private fun getUserById(id: String): RoomUser? {
        return users[id]
    }

    private suspend fun notifyUserJoin(user: RoomUser) {
        broadcast(UserConnected(roomId = id, socketId = user.id, username = user.username))
        broadcast(createUserListPacket())
        logger.info("Room [$id] user joined: ${user.username}")
    }

    private suspend fun notifyUserLeave(user: RoomUser) {
        broadcast(UserDisconnected(roomId = id, socketId = user.id, username = user.username))
        broadcast(createUserListPacket())
        logger.info("Room [$id] user left: ${user.username}")
    }

    private suspend fun broadcast(packet: Packet) {
        users.values.forEach { user ->
            user.session.send(Text(Json.encodeToString(packet)))
        }
    }

    companion object {
        fun create(roomId: String): Room {
            return Room(id = roomId)
        }
    }
}

fun Room.createUserListPacket(): UserList {
    val userList =
        allUsers.map {
            SocketUser(socketId = it.id, username = it.username, roomId = id)
        }
    return UserList(roomId = id, users = userList)
}
