package screenshare.common

import io.kotest.core.spec.style.DescribeSpec
import io.kotest.core.spec.style.FunSpec
import io.kotest.datatest.withData
import io.kotest.matchers.shouldBe
import kotlinx.serialization.json.Json
import screenshare.common.Packet.ChatMessageReceived
import screenshare.common.Packet.JoinRoom
import screenshare.common.Packet.ListUsers
import screenshare.common.Packet.SendChatMessage
import screenshare.common.Packet.SendIceCandidate
import screenshare.common.Packet.UserConnected
import screenshare.common.Packet.UserDisconnected
import screenshare.common.Packet.UserList

class MessageSpec : FunSpec({
    withData(
        nameFn = { "should deserialize json into Message [${it.second}]" },
        joinRoomJson("testRoomId", "username") to JoinRoom("testRoomId", "username"),
        sendMessageJson("testRoomId", "Hello") to SendChatMessage("testRoomId", "Hello"),
        listUsersJson("testRoomId") to ListUsers("testRoomId"),
        sendIceCandidateJson("testRoomId", "candidate", "targetId") to
                SendIceCandidate("testRoomId", "candidate", "targetId"),
        userConnectedJson("testRoomId", "socketId", "username") to
                UserConnected("testRoomId", "socketId", "username"),
        userDisconnectedJson("testRoomId", "socketId", "username") to
                UserDisconnected("testRoomId", "socketId", "username"),
        messageReceivedJson("testRoomId", "username", "Hello", 1234567890) to
                ChatMessageReceived("testRoomId", ChatMessage("username", "Hello", 1234567890)),
        userListJson("testRoomId", listOf(SocketUser("socketId", "username", "testRoomId")))
                to
                UserList(roomId = "testRoomId", users = listOf(SocketUser("socketId", "username", "testRoomId"))),
    ) { (json, expected) ->
        Json.decodeFromString<Packet>(json) shouldBe expected
    }

    withData(
        nameFn = { "should serialize Message into json [${it.second}]" },
        JoinRoom("testRoomId", "username") to joinRoomJson("testRoomId", "username"),
        SendChatMessage("testRoomId", "Hello") to sendMessageJson("testRoomId", "Hello"),
        ListUsers("testRoomId") to listUsersJson("testRoomId"),
        UserConnected("testRoomId", "socketId", "username") to userConnectedJson("testRoomId", "socketId", "username"),
        UserDisconnected("testRoomId", "socketId", "username") to userDisconnectedJson(
            "testRoomId",
            "socketId",
            "username"
        ),
        ChatMessageReceived(
            "testRoomId",
            ChatMessage("username", "Hello", 1234567890)
        ) to messageReceivedJson("testRoomId", "username", "Hello", 1234567890),
        UserList("testRoomId", listOf(SocketUser("socketId", "username", "testRoomId"))) to userListJson(
            "testRoomId", listOf(SocketUser("socketId", "username", "testRoomId")),
        ),
    ) { (message, expected) ->
        Json.encodeToString(message) shouldBe expected
    }
}) {
    private companion object {
        fun joinRoomJson(
            roomId: String,
            username: String,
        ) = """{"type":"join-room","roomId":"$roomId","username":"$username"}"""

        fun sendMessageJson(
            roomId: String,
            message: String,
        ) = """{"type":"send-message","roomId":"$roomId","message":"$message"}"""

        fun listUsersJson(roomId: String) = """{"type":"list-users","roomId":"$roomId"}"""

        fun sendIceCandidateJson(
            roomId: String,
            candidate: String,
            targetId: String,
        ) = """{"type":"send-ice-candidate","roomId":"$roomId","candidate":"$candidate","targetId":"$targetId"}"""

        fun userConnectedJson(
            roomId: String,
            socketId: String,
            username: String,
        ) = """{"type":"user-connected","roomId":"$roomId","socketId":"$socketId","username":"$username"}"""

        fun userDisconnectedJson(
            roomId: String,
            socketId: String,
            username: String,
        ) = """{"type":"user-disconnected","roomId":"$roomId","socketId":"$socketId","username":"$username"}"""

        fun messageReceivedJson(
            roomId: String,
            username: String,
            content: String,
            timestamp: Long,
        ) =
            """{"type":"chat-message","roomId":"$roomId","message":{"username":"$username","content":"$content","timestamp":$timestamp}}"""

        fun userListJson(
            roomId: String,
            users: List<SocketUser>,
        ) = """{"type":"user-list","roomId":"$roomId","users":${Json.encodeToString(users)}}"""
    }
}
