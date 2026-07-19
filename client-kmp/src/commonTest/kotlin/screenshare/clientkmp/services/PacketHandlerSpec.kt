package screenshare.clientkmp.services

import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.collections.shouldHaveSize
import io.kotest.matchers.nulls.shouldBeNull
import io.kotest.matchers.shouldBe
import screenshare.clientkmp.state.AppState
import screenshare.common.ChatMessage
import screenshare.common.Packet
import screenshare.common.SocketUser

class PacketHandlerSpec : FunSpec({

    fun appStateInRoom(
        roomId: String = "sala1",
        username: String = "Alice",
    ): AppState {
        val state = AppState()
        state.navigateToRoom(roomId, username)
        return state
    }

    test("ChatMessageReceived acrescenta mensagem ao histórico") {
        val appState = appStateInRoom()
        val handler = PacketHandler(appState, webRtcManager = null)
        val msg = ChatMessage(username = "Bob", content = "Olá!", timestamp = 0L)

        handler.handle(Packet.ChatMessageReceived("sala1", msg))

        appState.currentRoom
            ?.chatMessages
            ?.last()
            ?.content shouldBe "Olá!"
        appState.currentRoom
            ?.chatMessages
            ?.last()
            ?.username shouldBe "Bob"
    }

    test("UserList atualiza lista de usuários") {
        val appState = appStateInRoom()
        val handler = PacketHandler(appState, webRtcManager = null)
        val users =
            listOf(
                SocketUser(socketId = "s1", username = "Alice", roomId = "sala1", isMuted = true),
                SocketUser(socketId = "s2", username = "Bob", roomId = "sala1", isMuted = false),
            )

        handler.handle(Packet.UserList("sala1", users))

        appState.currentRoom?.users!! shouldHaveSize 2
        appState.currentRoom?.users?.map { it.username } shouldBe listOf("Alice", "Bob")
    }

    test("UserConnected de outro usuário adiciona mensagem de sistema") {
        val appState = appStateInRoom(username = "Alice")
        val handler = PacketHandler(appState, webRtcManager = null)

        handler.handle(Packet.UserConnected("sala1", socketId = "s2", username = "Bob"))

        val msgs = appState.currentRoom?.chatMessages.orEmpty()
        msgs.any { it.username == "Sistema" && it.content.contains("Bob") } shouldBe true
    }

    test("UserConnected do próprio usuário não gera mensagem de sistema") {
        val appState = appStateInRoom(username = "Alice")
        val handler = PacketHandler(appState, webRtcManager = null)
        val initialCount = appState.currentRoom?.chatMessages?.size ?: 0

        handler.handle(Packet.UserConnected("sala1", socketId = "s1", username = "Alice"))

        appState.currentRoom?.chatMessages?.size shouldBe initialCount
    }

    test("UserDisconnected de usuário que estava compartilhando limpa currentSharerSocketId") {
        val appState = appStateInRoom()
        appState.updateRoom { it.copy(currentSharerSocketId = "s2") }
        val handler = PacketHandler(appState, webRtcManager = null)

        handler.handle(Packet.UserDisconnected("sala1", socketId = "s2", username = "Bob"))

        appState.currentRoom?.currentSharerSocketId.shouldBeNull()
    }

    test("ScreenShareStarted define currentSharerSocketId") {
        val appState = appStateInRoom()
        val handler = PacketHandler(appState, webRtcManager = null)

        handler.handle(Packet.ScreenShareStarted("sala1", senderId = "s2"))

        appState.currentRoom?.currentSharerSocketId shouldBe "s2"
    }

    test("ScreenShareStopped limpa currentSharerSocketId") {
        val appState = appStateInRoom()
        appState.updateRoom { it.copy(currentSharerSocketId = "s2") }
        val handler = PacketHandler(appState, webRtcManager = null)

        handler.handle(Packet.ScreenShareStopped("sala1", senderId = "s2"))

        appState.currentRoom?.currentSharerSocketId.shouldBeNull()
    }

    test("ScreenShareStopped não altera se remetente não estava compartilhando") {
        val appState = appStateInRoom()
        appState.updateRoom { it.copy(currentSharerSocketId = "s3") }
        val handler = PacketHandler(appState, webRtcManager = null)

        handler.handle(Packet.ScreenShareStopped("sala1", senderId = "s2"))

        appState.currentRoom?.currentSharerSocketId shouldBe "s3"
    }

    test("UserMuted e UserUnmuted atualizam isMuted do usuário") {
        val appState = appStateInRoom()
        val users = listOf(SocketUser(socketId = "s1", username = "Alice", roomId = "sala1", isMuted = false))
        appState.updateRoom { it.copy(users = users) }
        val handler = PacketHandler(appState, webRtcManager = null)

        handler.handle(Packet.UserMuted("sala1", socketId = "s1"))
        appState.currentRoom
            ?.users
            ?.first()
            ?.isMuted shouldBe true

        handler.handle(Packet.UserUnmuted("sala1", socketId = "s1"))
        appState.currentRoom
            ?.users
            ?.first()
            ?.isMuted shouldBe false
    }
})
