package services

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import platform.DesktopEnvironment
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
            is Packet.RoomJoined -> {
                if (packet.roomId == session.localRoomId) session.confirmRoomJoined(packet.socketId)
            }

            is Packet.IceServers -> {
                if (packet.roomId == session.localRoomId) session.peerConnections.iceServers = packet.iceServers
            }

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

            is Packet.RoomMediaState -> {
                packet.screenSharers.forEach { session.peerConnections.markScreenSharing(it, true) }
                packet.cameraSharers.forEach { session.peerConnections.markCameraSharing(it, true) }
                UserListMutations.restoreUserMediaSharing(packet.screenSharers, packet.cameraSharers)
                updateRemoteTrayState(session)
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

            is Packet.ShareQualities -> {
                if (packet.roomId == session.localRoomId) session.connectionQuality.setPeerReports(packet.qualities)
            }

            is Packet.ShareQuality -> {
                if (packet.roomId == session.localRoomId) {
                    session.connectionQuality.setPeerReport(packet.socketId, packet.problem)
                }
            }

            else -> {
                Unit
            }
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
    val isLocalUser = packet.socketId == session.localSocketId
    if (isLocalUser) {
        session.localSocketId = packet.socketId
        session.launch {
            if (session.screenSharing.localScreenStream !=
                null
            ) {
                session.websocketService.startScreenSharing(session.localRoomId)
            }
            if (session.cameraSharing.localCameraStream !=
                null
            ) {
                session.websocketService.startCameraShare(session.localRoomId)
            }
            // The server starts everyone muted; after a reconnect the mic may still be open.
            if (session.voiceChat.localMicStream != null && !session.voiceChat.isMicMuted) {
                session.websocketService.sendToggleMute(session.localRoomId, isMuted = false)
            }
        }
    } else {
        InterfaceMutations.addMessageToChat(
            message =
                ChatMessage(
                    username = "Sistema",
                    content = "${packet.username} entrou na sala",
                    timestamp = Date().getTime().toLong(),
                ),
            localSocketId = session.localSocketId,
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
        localSocketId = session.localSocketId,
    )

    session.peerConnections.closePeerConnection(packet.socketId)
    session.voiceChat.stopRemoteAudio(packet.socketId)
    session.connectionQuality.setPeerReport(packet.socketId, null)
    UserListMutations.setUserMediaSharing(packet.socketId, camera = false, active = false)
    UserListMutations.setUserMediaSharing(packet.socketId, camera = true, active = false)
    updateRemoteTrayState(session)
    session.screenSharing.stopRemoteScreen(packet.socketId)
    session.cameraSharing.stopRemoteCamera(packet.socketId)
}

private fun handleChatMessageReceived(
    session: Session,
    packet: Packet.ChatMessageReceived,
) {
    InterfaceMutations.addMessageToChat(
        message = packet.message,
        localSocketId = session.localSocketId,
    )
}

private fun handleUserList(
    session: Session,
    packet: Packet.UserList,
) {
    session.userList = packet.users
    UserListMutations.updateUserList(packet.users, session.localSocketId)
    packet.users.filter { it.socketId != session.localSocketId }.forEach { user ->
        if (!session.peerConnections.contains(user.socketId)) {
            session.peerConnections.createPeerConnection(
                websocketService = session.websocketService,
                socketId = user.socketId,
                roomId = session.localRoomId,
                isInitiator = false,
                coroutineScope = session,
            )
        }
    }
    session.launch {
        runCatching { DesktopEnvironment.setTrayState("participants", false, packet.users.size) }
    }
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
    UserListMutations.setUserMediaSharing(packet.senderId, camera = false, active = true)
    updateRemoteTrayState(session)
    if (wasSharing) return

    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} começou a compartilhar a tela",
                timestamp = Date().getTime().toLong(),
            ),
        localSocketId = session.localSocketId,
    )
}

private fun handleScreenShareStopped(
    session: Session,
    packet: Packet.ScreenShareStopped,
) {
    session.peerConnections.markScreenSharing(packet.senderId, false)
    session.connectionQuality.setPeerReport(packet.senderId, null)
    UserListMutations.setUserMediaSharing(packet.senderId, camera = false, active = false)
    updateRemoteTrayState(session)
    session.screenSharing.stopRemoteScreen(packet.senderId)
    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} parou de compartilhar a tela",
                timestamp = Date().getTime().toLong(),
            ),
        localSocketId = session.localSocketId,
    )
}

private fun handleCameraShareStarted(
    session: Session,
    packet: Packet.CameraShareStarted,
) {
    val wasSharing = session.peerConnections.isCameraSharing(packet.senderId)
    session.peerConnections.markCameraSharing(packet.senderId, true)
    UserListMutations.setUserMediaSharing(packet.senderId, camera = true, active = true)
    updateRemoteTrayState(session)
    if (wasSharing) return

    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} ativou a câmera",
                timestamp = Date().getTime().toLong(),
            ),
        localSocketId = session.localSocketId,
    )
}

private fun handleCameraShareStopped(
    session: Session,
    packet: Packet.CameraShareStopped,
) {
    session.peerConnections.markCameraSharing(packet.senderId, false)
    UserListMutations.setUserMediaSharing(packet.senderId, camera = true, active = false)
    updateRemoteTrayState(session)
    session.cameraSharing.stopRemoteCamera(packet.senderId)
    InterfaceMutations.addMessageToChat(
        message =
            ChatMessage(
                username = "Sistema",
                content = "${screenShareAuthorName(session, packet.senderId)} desativou a câmera",
                timestamp = Date().getTime().toLong(),
            ),
        localSocketId = session.localSocketId,
    )
}

private fun screenShareAuthorName(
    session: Session,
    socketId: String,
): String {
    val isLocal = session.localSocketId == socketId
    if (isLocal) return "Você"
    return session.userList.firstOrNull { it.socketId == socketId }?.username ?: socketId.takeLast(6)
}

private fun updateRemoteTrayState(session: Session) =
    session.launch {
        runCatching {
            DesktopEnvironment.setTrayState(
                "remote",
                UserListMutations.hasRemoteShares(session.localSocketId),
            )
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
