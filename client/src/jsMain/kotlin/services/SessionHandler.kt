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

            is Packet.ScreenShareStarted -> {
                handleScreenShareStarted(session, packet)
            }

            is Packet.ScreenShareStopped -> {
                handleScreenShareStopped(session, packet)
            }

            is Packet.CameraShareStarted -> {
                handleCameraShareStarted(session, packet)
            }

            is Packet.CameraShareStopped -> {
                handleCameraShareStopped(session, packet)
            }

            is Packet.UserMuted, is Packet.UserUnmuted -> {
                handleUserMuted(packet)
            }

            else -> Unit
        }
    }.onFailure {
        console.warn("Não foi possível processar uma atualização da sala")
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

        // Um novo participante precisa saber o estado de compartilhamento atual
        // para rotear os tracks de vídeo (tela vs câmera). Re-transmite os
        // sinais Start do que está ativo; peers já conectados deduplicam.
        session.launch {
            if (session.screenSharing.localScreenStream != null) {
                session.websocketService.startScreenSharing(session.localRoomId)
            }
            if (session.cameraSharing.localCameraStream != null) {
                session.websocketService.startCameraShare(session.localRoomId)
            }
        }
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
    session.screenSharing.stopRemoteScreen(packet.socketId)
    session.cameraSharing.stopRemoteCamera(packet.socketId)
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
    }.onFailure { console.warn("Não foi possível aplicar uma atualização de conexão") }
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
    }.onFailure { console.warn("Não foi possível atualizar a conexão de mídia") }
}

private fun handleScreenShareStarted(
    session: Session,
    packet: Packet.ScreenShareStarted,
) {
    val wasSharing = session.peerConnections.isScreenSharing(packet.senderId)
    session.peerConnections.markScreenSharing(packet.senderId, true)
    if (wasSharing) return

    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} começou a compartilhar a tela",
                timestamp = Date().getTime().toLong(),
            ),
        localUsername = session.localUsername,
    )
}

private fun handleScreenShareStopped(
    session: Session,
    packet: Packet.ScreenShareStopped,
) {
    session.peerConnections.markScreenSharing(packet.senderId, false)
    session.screenSharing.stopRemoteScreen(packet.senderId)
    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} parou de compartilhar a tela",
                timestamp = Date().getTime().toLong(),
            ),
        localUsername = session.localUsername,
    )
}

private fun handleCameraShareStarted(
    session: Session,
    packet: Packet.CameraShareStarted,
) {
    val wasSharing = session.peerConnections.isCameraSharing(packet.senderId)
    session.peerConnections.markCameraSharing(packet.senderId, true)
    if (wasSharing) return

    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} ativou a câmera",
                timestamp = Date().getTime().toLong(),
            ),
        localUsername = session.localUsername,
    )
}

private fun handleCameraShareStopped(
    session: Session,
    packet: Packet.CameraShareStopped,
) {
    session.peerConnections.markCameraSharing(packet.senderId, false)
    session.cameraSharing.stopRemoteCamera(packet.senderId)
    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} desativou a câmera",
                timestamp = Date().getTime().toLong(),
            ),
        localUsername = session.localUsername,
    )
}

private fun screenShareAuthorName(
    session: Session,
    socketId: String,
): String {
    val isLocal = session.userList.firstOrNull { it.username == session.localUsername }?.socketId == socketId
    if (isLocal) return "Você"
    return session.userList.firstOrNull { it.socketId == socketId }?.username ?: socketId.takeLast(6)
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
