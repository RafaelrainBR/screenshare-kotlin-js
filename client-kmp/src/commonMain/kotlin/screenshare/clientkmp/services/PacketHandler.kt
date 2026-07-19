package screenshare.clientkmp.services

import screenshare.clientkmp.state.AppState
import screenshare.clientkmp.util.currentTimeMillis
import screenshare.common.ChatMessage
import screenshare.common.Packet

class PacketHandler(
    private val appState: AppState,
    private val webRtcManager: WebRtcManager? = null,
) {
    suspend fun handle(packet: Packet) {
        println("Received packet: $packet")
        runCatching {
            when (packet) {
                is Packet.UserConnected -> handleUserConnected(packet)
                is Packet.UserDisconnected -> handleUserDisconnected(packet)
                is Packet.ChatMessageReceived -> handleChatMessage(packet)
                is Packet.UserList -> handleUserList(packet)
                is Packet.IceCandidateReceived -> webRtcManager?.handleIceCandidate(packet)
                is Packet.DescriptionReceived -> webRtcManager?.handleDescription(packet)
                is Packet.UserMuted -> handleMuteChange(packet.socketId, isMuted = true)
                is Packet.UserUnmuted -> handleMuteChange(packet.socketId, isMuted = false)
                is Packet.ScreenShareStarted -> handleScreenShareStarted(packet)
                is Packet.ScreenShareStopped -> handleScreenShareStopped(packet)
                else -> println("Unknown packet: ${packet::class.simpleName}")
            }
        }.onFailure { error ->
            println("Error handling packet [$packet]: ${error.message}")
            println(error.stackTraceToString())
        }
    }

    private fun handleUserConnected(packet: Packet.UserConnected) {
        val isLocalUser = packet.username == appState.currentRoom?.username
        if (!isLocalUser) {
            appState.updateRoom { room ->
                room.copy(
                    chatMessages = room.chatMessages + systemMessage("${packet.username} entrou na sala"),
                )
            }
            webRtcManager?.createPeerConnection(socketId = packet.socketId, isInitiator = true)
        }
    }

    private fun handleUserDisconnected(packet: Packet.UserDisconnected) {
        val wasSharing = appState.currentRoom?.currentSharerSocketId == packet.socketId
        appState.updateRoom { room ->
            val messages = room.chatMessages + systemMessage("${packet.username} saiu da sala")
            val newSharerSocketId =
                if (room.currentSharerSocketId ==
                    packet.socketId
                ) {
                    null
                } else {
                    room.currentSharerSocketId
                }
            room.copy(chatMessages = messages, currentSharerSocketId = newSharerSocketId)
        }
        if (wasSharing) webRtcManager?.clearVideoTrack()
        webRtcManager?.closePeerConnection(packet.socketId)
    }

    private fun handleChatMessage(packet: Packet.ChatMessageReceived) {
        appState.updateRoom { room ->
            room.copy(chatMessages = room.chatMessages + packet.message)
        }
    }

    private fun handleUserList(packet: Packet.UserList) {
        appState.updateRoom { room ->
            room.copy(users = packet.users)
        }
    }

    private fun handleMuteChange(
        socketId: String,
        isMuted: Boolean,
    ) {
        appState.updateRoom { room ->
            val updatedUsers =
                room.users.map { user ->
                    if (user.socketId == socketId) user.copy(isMuted = isMuted) else user
                }
            room.copy(users = updatedUsers)
        }
    }

    private fun handleScreenShareStarted(packet: Packet.ScreenShareStarted) {
        appState.updateRoom { room ->
            room.copy(currentSharerSocketId = packet.senderId)
        }
    }

    private fun handleScreenShareStopped(packet: Packet.ScreenShareStopped) {
        if (appState.currentRoom?.currentSharerSocketId == packet.senderId) {
            webRtcManager?.clearVideoTrack()
        }
        appState.updateRoom { room ->
            if (room.currentSharerSocketId == packet.senderId) {
                room.copy(currentSharerSocketId = null)
            } else {
                room
            }
        }
    }

    private fun systemMessage(content: String) =
        ChatMessage(
            username = "Sistema",
            content = content,
            timestamp = currentTimeMillis(),
        )
}
