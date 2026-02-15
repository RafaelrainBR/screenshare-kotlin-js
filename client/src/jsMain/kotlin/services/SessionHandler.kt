package services

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import screenshare.common.ChatMessage
import screenshare.common.Packet
import ui.InterfaceMutations
import ui.mutations.UserListMutations
import kotlin.js.Date
import kotlin.js.json

fun handlePacket(
    session: Session,
    packet: Packet,
    coroutineScope: CoroutineScope,
) {
    println("Received packet: $packet")
    runCatching {
        when (packet) {
            is Packet.UserConnected -> {
                handleUserConnected(session, packet, coroutineScope)
            }

            is Packet.UserDisconnected -> {
                handleUserDisconnected(session, packet)
            }

            is Packet.ChatMessageReceived -> {
                handleChatMessageReceived(session, packet)
            }

            is Packet.UserList -> {
                handleUserList(session, packet)
            }

            is Packet.IceCandidateReceived -> {
                handleIceCandidateReceived(session, packet, coroutineScope)
            }

            is Packet.DescriptionReceived -> {
                handleDescriptionReceived(session, packet, coroutineScope)
            }

            is Packet.ScreenShareStarted -> {}

            is Packet.ScreenShareStopped -> {}

            is Packet.UserMuted, is Packet.UserUnmuted -> {
                handleUserMuted(packet)
            }

            else -> {
                println("Unknown packet type: ${packet::class.simpleName}")
            }
        }
    }.onFailure { error ->
        println("Error handling packet [$packet]: ${error.message}")
        println(error.stackTraceToString())
        println(error)
    }
}

private fun handleUserConnected(
    session: Session,
    packet: Packet.UserConnected,
    coroutineScope: CoroutineScope,
) {
    val isLocalUser = packet.username == session.localUsername
    if (!isLocalUser) {
        InterfaceMutations.addMessageToChat(
            message =
                ChatMessage(
                    username = "Sistema",
                    content = "${packet.username} entrou na sala",
                    timestamp = Date().getTime().toLong(),
                ),
            localUsername = session.localUsername,
        )

        session.peerConnections.createPeerConnection(
            websocketService = session.websocketService,
            socketId = packet.socketId,
            roomId = session.localRoomId,
            isInitiator = true,
            coroutineScope = coroutineScope,
        )
    }
}

private fun handleUserDisconnected(
    session: Session,
    packet: Packet.UserDisconnected,
) {
    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${packet.username} saiu da sala",
                timestamp = Date().getTime().toLong(),
            ),
        localUsername = session.localUsername,
    )

    session.peerConnections.closePeerConnection(packet.socketId)

    val currentSharer = session.currentSharerSocketId
    if (packet.socketId == currentSharer) {
        InterfaceMutations.endScreenSharing()
        session.currentSharerSocketId = null
    }
}

private fun handleChatMessageReceived(
    session: Session,
    packet: Packet.ChatMessageReceived,
) {
    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = packet.message.username,
                content = packet.message.content,
                timestamp = packet.message.timestamp,
            ),
        localUsername = session.localUsername,
    )
}

private fun handleUserList(
    session: Session,
    packet: Packet.UserList,
) {
    session.userList = packet.users
    UserListMutations.updateUserList(packet.users, session.localUsername)
}

private fun handleIceCandidateReceived(
    session: Session,
    packet: Packet.IceCandidateReceived,
    coroutineScope: CoroutineScope,
) = coroutineScope.launch {
    runCatching {
        session.peerConnections.updateIceCandidate(
            senderId = packet.senderId,
            candidate = packet.candidate,
        )
    }.onFailure {
        println("Failed to add ICE candidate: ${it.message}")
    }
}

private fun handleDescriptionReceived(
    session: Session,
    packet: Packet.DescriptionReceived,
    coroutineScope: CoroutineScope,
) = coroutineScope.launch {
    runCatching {
        val type = packet.description["type"] as String
        val sdp = packet.description["sdp"] as String

        if (type == "offer") {
            if (!session.peerConnections.contains(packet.senderId)) {
                session.peerConnections.createPeerConnection(
                    websocketService = session.websocketService,
                    socketId = packet.senderId,
                    roomId = session.localRoomId,
                    isInitiator = false,
                    coroutineScope = coroutineScope,
                )
            }

            val descriptionJson = json("type" to type, "sdp" to sdp)
            session.peerConnections.updateDescriptionFromOffer(
                websocketService = session.websocketService,
                roomId = session.localRoomId,
                senderId = packet.senderId,
                descriptionJson = descriptionJson,
            )
        }

        if (type == "answer") {
            val descriptionJson = json("type" to type, "sdp" to sdp)
            session.peerConnections.updateDescriptionFromAnswer(
                senderId = packet.senderId,
                descriptionJson = descriptionJson,
            )
        }
    }.onFailure {
        println("Failed to set remote description from packet [$packet]: ${it.message}")
    }
}

private fun handleUserMuted(packet: Packet) {
    val (isMuted, socketId) =
        when (packet) {
            is Packet.UserMuted -> Pair(true, packet.socketId)
            is Packet.UserUnmuted -> Pair(false, packet.socketId)
            else -> return
        }

    UserListMutations.updateUserMuted(socketId, isMuted)
}
