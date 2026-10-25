package screenshare.common

import io.kotest.core.spec.style.FunSpec
import io.kotest.datatest.withData
import io.kotest.matchers.shouldBe
import kotlinx.serialization.json.Json
import screenshare.common.Packet.ChatMessageReceived
import screenshare.common.Packet.JoinRoom
import screenshare.common.Packet.ListUsers
import screenshare.common.Packet.RequestMediaState
import screenshare.common.Packet.RoomMediaState
import screenshare.common.Packet.SendChatMessage
import screenshare.common.Packet.SendIceCandidate
import screenshare.common.Packet.UserConnected
import screenshare.common.Packet.UserDisconnected
import screenshare.common.Packet.UserList

class MessageSpec : FunSpec({
    withData(
        nameFn = { "should deserialize json into Message [${it.second}]" },
        joinRoomJson("testRoomId", "username") to JoinRoom("testRoomId", "username"),
        """{"type":"join-room","rid":"room","username":"same","clientVersion":2}""" to JoinRoom("room", "same", 2),
        """{"type":"room-joined","rid":"room","sid":"socket"}""" to Packet.RoomJoined("room", "socket"),
        ICE_JSON to ICE_PACKET,
        SEND_QUALITY_JSON to SEND_QUALITY,
        CLEAR_QUALITY_JSON to CLEAR_QUALITY,
        QUALITIES_JSON to QUALITIES,
        QUALITY_JSON to QUALITY,
        QUALITY_CLEARED_JSON to QUALITY_CLEARED,
        """{"type":"send-message","rid":"room","msg":"Hello","clientMessageId":"receipt"}""" to
            SendChatMessage("room", "Hello", "receipt"),
        (
            """{"type":"chat-message","rid":"room","msg":{"username":"same","content":"Hello","timestamp":123,""" +
                """"senderSocketId":"socket","clientMessageId":"receipt"}}"""
        ) to
            ChatMessageReceived("room", ChatMessage("same", "Hello", 123, "socket", "receipt")),
        sendMessageJson("testRoomId", "Hello") to SendChatMessage("testRoomId", "Hello"),
        listUsersJson("testRoomId") to ListUsers("testRoomId"),
        """{"type":"request-media-state","rid":"testRoomId"}""" to RequestMediaState("testRoomId"),
        """{"type":"room-media-state","rid":"testRoomId","screenSharers":["screen"],"cameraSharers":["camera"]}""" to
            RoomMediaState("testRoomId", listOf("screen"), listOf("camera")),
        sendIceCandidateJson("testRoomId", "candidate", "targetId") to
            SendIceCandidate("testRoomId", "candidate", "targetId"),
        userConnectedJson("testRoomId", "socketId", "username") to UserConnected("testRoomId", "socketId", "username"),
        userDisconnectedJson("testRoomId", "socketId", "username") to
            UserDisconnected("testRoomId", "socketId", "username"),
        messageReceivedJson("testRoomId", "username", "Hello", 1234567890) to
            ChatMessageReceived("testRoomId", ChatMessage("username", "Hello", 1234567890)),
        userListJson("testRoomId", listOf(SocketUser("socketId", "username", "testRoomId"))) to
            UserList(roomId = "testRoomId", users = listOf(SocketUser("socketId", "username", "testRoomId"))),
    ) { (json, expected) ->
        Json.decodeFromString<Packet>(json) shouldBe expected
    }

    withData(
        nameFn = { "should serialize Message into json [${it.second}]" },
        JoinRoom("testRoomId", "username") to joinRoomJson("testRoomId", "username"),
        JoinRoom("room", "same", 2) to """{"type":"join-room","rid":"room","username":"same","clientVersion":2}""",
        Packet.RoomJoined("room", "socket") to """{"type":"room-joined","rid":"room","sid":"socket"}""",
        // Empty username/credential are left out (defaults), as STUN entries need neither.
        ICE_PACKET to ICE_JSON,
        // A cleared limit leaves "problem" out, as a default.
        SEND_QUALITY to SEND_QUALITY_JSON,
        CLEAR_QUALITY to CLEAR_QUALITY_JSON,
        QUALITIES to QUALITIES_JSON,
        QUALITY to QUALITY_JSON,
        QUALITY_CLEARED to QUALITY_CLEARED_JSON,
        SendChatMessage("room", "Hello", "receipt") to
            """{"type":"send-message","rid":"room","msg":"Hello","clientMessageId":"receipt"}""",
        ChatMessageReceived("room", ChatMessage("same", "Hello", 123, "socket", "receipt")) to
            """{"type":"chat-message","rid":"room","msg":{"username":"same","content":"Hello","timestamp":123,"senderSocketId":"socket","clientMessageId":"receipt"}}""",
        SendChatMessage("testRoomId", "Hello") to sendMessageJson("testRoomId", "Hello"),
        ListUsers("testRoomId") to listUsersJson("testRoomId"),
        RequestMediaState("testRoomId") to """{"type":"request-media-state","rid":"testRoomId"}""",
        RoomMediaState("testRoomId", listOf("screen"), listOf("camera")) to
            """{"type":"room-media-state","rid":"testRoomId","screenSharers":["screen"],"cameraSharers":["camera"]}""",
        UserConnected("testRoomId", "socketId", "username") to userConnectedJson("testRoomId", "socketId", "username"),
        UserDisconnected("testRoomId", "socketId", "username") to
            userDisconnectedJson("testRoomId", "socketId", "username"),
        ChatMessageReceived("testRoomId", ChatMessage("username", "Hello", 1234567890)) to
            messageReceivedJson("testRoomId", "username", "Hello", 1234567890),
        UserList("testRoomId", listOf(SocketUser("socketId", "username", "testRoomId"))) to
            userListJson("testRoomId", listOf(SocketUser("socketId", "username", "testRoomId"))),
    ) { (message, expected) ->
        Json.encodeToString(message) shouldBe expected
    }
}) {
    private companion object {
        fun joinRoomJson(
            roomId: String,
            username: String,
        ) = """{"type":"join-room","rid":"$roomId","username":"$username"}"""

        fun sendMessageJson(
            roomId: String,
            message: String,
        ) = """{"type":"send-message","rid":"$roomId","msg":"$message"}"""

        fun listUsersJson(roomId: String) = """{"type":"list-users","rid":"$roomId"}"""

        fun sendIceCandidateJson(
            roomId: String,
            candidate: String,
            targetId: String,
        ) = """{"type":"send-ice-candidate","rid":"$roomId","ice":"$candidate","tid":"$targetId"}"""

        fun userConnectedJson(
            roomId: String,
            socketId: String,
            username: String,
        ) = """{"type":"user-connected","rid":"$roomId","sid":"$socketId","username":"$username"}"""

        fun userDisconnectedJson(
            roomId: String,
            socketId: String,
            username: String,
        ) = """{"type":"user-disconnected","rid":"$roomId","sid":"$socketId","username":"$username"}"""

        fun messageReceivedJson(
            roomId: String,
            username: String,
            content: String,
            timestamp: Long,
        ) =
            """{"type":"chat-message","rid":"$roomId","msg":{"username":"$username","content":"$content","timestamp":$timestamp}}"""

        fun userListJson(
            roomId: String,
            users: List<SocketUser>,
        ) = """{"type":"user-list","rid":"$roomId","users":${Json.encodeToString(users)}}"""

        const val ICE_JSON =
            """{"type":"ice-servers","rid":"room","iceServers":[{"urls":["stun:h:3478"]},""" +
                """{"urls":["turn:h:3478"],"username":"u","credential":"c"}]}"""
        val ICE_PACKET =
            Packet.IceServers(
                "room",
                listOf(IceServer(listOf("stun:h:3478")), IceServer(listOf("turn:h:3478"), "u", "c")),
            )

        // The same JSON is in desktop-native/src/session.rs.
        const val SEND_QUALITY_JSON = """{"type":"send-share-quality","rid":"room","problem":"cpu"}"""
        val SEND_QUALITY = Packet.SendShareQuality("room", ShareProblem.CPU)
        const val CLEAR_QUALITY_JSON = """{"type":"send-share-quality","rid":"room"}"""
        val CLEAR_QUALITY = Packet.SendShareQuality("room")
        const val QUALITIES_JSON = """{"type":"share-qualities","rid":"room","qualities":{"b":"bandwidth"}}"""
        val QUALITIES = Packet.ShareQualities("room", mapOf("b" to ShareProblem.BANDWIDTH))
        const val QUALITY_JSON = """{"type":"share-quality","rid":"room","sid":"b","problem":"cpu"}"""
        val QUALITY = Packet.ShareQuality("room", "b", ShareProblem.CPU)
        const val QUALITY_CLEARED_JSON = """{"type":"share-quality","rid":"room","sid":"b"}"""
        val QUALITY_CLEARED = Packet.ShareQuality("room", "b")
    }
}
