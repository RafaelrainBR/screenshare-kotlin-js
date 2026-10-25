package screenshare.server

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.serialization.json.Json
import org.slf4j.LoggerFactory
import screenshare.common.ChatMessage
import screenshare.common.IceServer
import screenshare.common.Packet
import screenshare.common.Packet.CameraShareStarted
import screenshare.common.Packet.CameraShareStopped
import screenshare.common.Packet.ChatMessageReceived
import screenshare.common.Packet.DescriptionReceived
import screenshare.common.Packet.IceCandidateReceived
import screenshare.common.Packet.ListUsers
import screenshare.common.Packet.RequestMediaState
import screenshare.common.Packet.RoomMediaState
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
import screenshare.common.ShareProblem
import screenshare.common.SocketUser
import java.util.concurrent.ConcurrentHashMap

class Room(
    val id: String,
    private val users: MutableMap<String, RoomUser> = ConcurrentHashMap(),
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
) {
    private val logger = LoggerFactory.getLogger(Room::class.java)

    private val messages = mutableListOf<ChatMessage>()

    val isEmpty: Boolean
        get() = users.isEmpty()

    val allUsers: List<RoomUser>
        get() = users.values.toList()

    fun close() {
        scope.cancel()
    }

    suspend fun addUser(user: RoomUser) {
        user.start(scope) { removeUser(user) }
        users[user.id] = user
        if (user.clientVersion >= 2) user.sendPacket(Packet.RoomJoined(id, user.id))
        if (user.clientVersion >= 3 && iceServers.isNotEmpty()) user.sendPacket(Packet.IceServers(id, iceServers))
        if (user.clientVersion >= 4) {
            val qualities =
                allUsers.filter { it.id != user.id }.mapNotNull { other ->
                    other.shareProblem?.let {
                        other.id to
                            it
                    }
                }
            user.sendPacket(Packet.ShareQualities(id, qualities.toMap()))
        }
        sendMessageHistory(user)
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
                        senderSocketId = user.id,
                        clientMessageId = packet.clientMessageId,
                    )
                messages.add(chatMessage)
                broadcast(ChatMessageReceived(roomId = id, message = chatMessage))
            }

            is ListUsers -> {
                user.sendPacket(createUserListPacket())
            }

            is RequestMediaState -> {
                user.sendPacket(
                    RoomMediaState(
                        roomId = id,
                        screenSharers = allUsers.filter { it.isScreenSharing }.map { it.id },
                        cameraSharers = allUsers.filter { it.isCameraSharing }.map { it.id },
                    ),
                )
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
                user.isScreenSharing = true
                broadcast(ScreenShareStarted(roomId = id, senderId = user.id))
            }

            is Packet.StopScreenShare -> {
                user.isScreenSharing = false
                setShareProblem(user, null)
                broadcast(ScreenShareStopped(roomId = id, senderId = user.id))
            }

            is Packet.SendShareQuality -> {
                // Only known values. Not tied to isScreenSharing: on a rejoin the report can beat start-screen-share.
                val problem = packet.problem
                if (problem != null && problem !in ShareProblem.ALL) return
                setShareProblem(user, problem)
            }

            is Packet.SendMuted -> {
                handleToggleMute(socketId = user.id, isMuted = true)
            }

            is Packet.SendUnmuted -> {
                handleToggleMute(socketId = user.id, isMuted = false)
            }

            is Packet.StartCameraShare -> {
                user.isCameraSharing = true
                broadcast(CameraShareStarted(roomId = id, senderId = user.id))
            }

            is Packet.StopCameraShare -> {
                user.isCameraSharing = false
                broadcast(CameraShareStopped(roomId = id, senderId = user.id))
            }

            else -> {}
        }
    }

    private fun getUserById(id: String): RoomUser? = users[id]

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

    /** Older clients decode packets strictly, so only clientVersion 4+ hears of it. */
    private suspend fun setShareProblem(
        user: RoomUser,
        problem: String?,
    ) {
        if (user.shareProblem == problem) return
        user.shareProblem = problem
        users.values
            .filter { it.id != user.id && it.clientVersion >= 4 }
            .forEach { it.sendPacket(Packet.ShareQuality(id, user.id, problem)) }
    }

    private suspend fun broadcast(packet: Packet) {
        users.values.forEach { user ->
            user.sendPacket(packet)
        }
    }

    private suspend fun sendMessageHistory(toUser: RoomUser) {
        messages.forEach { message ->
            toUser.sendPacket(
                ChatMessageReceived(
                    roomId = id,
                    message = message,
                ),
            )
        }
    }

    private suspend fun handleToggleMute(
        socketId: String,
        isMuted: Boolean,
    ) {
        users[socketId]?.isMuted = isMuted

        val packet =
            if (isMuted) {
                Packet.UserMuted(roomId = id, socketId = socketId)
            } else {
                Packet.UserUnmuted(roomId = id, socketId = socketId)
            }
        broadcast(packet)
    }

    companion object {
        fun create(roomId: String): Room = Room(id = roomId)

        /** ICE_SERVERS holds the STUN/TURN list as JSON (a Fly secret), so credentials stay out of Git. */
        private val iceServers: List<IceServer> =
            System.getenv("ICE_SERVERS")?.takeIf { it.isNotBlank() }?.let { json ->
                runCatching { Json.decodeFromString<List<IceServer>>(json) }
                    .onFailure {
                        LoggerFactory
                            .getLogger(
                                Room::class.java,
                            ).error("ICE_SERVERS is not valid JSON; sending none", it)
                    }.getOrDefault(emptyList())
            } ?: emptyList()
    }
}

fun Room.createUserListPacket(): UserList {
    val userList =
        allUsers.map {
            SocketUser(
                socketId = it.id,
                username = it.username,
                roomId = id,
                isMuted = it.isMuted,
            )
        }
    return UserList(roomId = id, users = userList)
}
