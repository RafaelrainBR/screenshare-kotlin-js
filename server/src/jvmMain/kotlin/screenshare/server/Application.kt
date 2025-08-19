package screenshare.server

import io.ktor.serialization.kotlinx.json.json
import io.ktor.server.application.Application
import io.ktor.server.application.install
import io.ktor.server.http.content.staticResources
import io.ktor.server.plugins.contentnegotiation.ContentNegotiation
import io.ktor.server.routing.routing
import io.ktor.server.websocket.WebSockets
import io.ktor.server.websocket.pingPeriod
import io.ktor.server.websocket.timeout
import io.ktor.server.websocket.webSocket
import io.ktor.websocket.Frame.Close
import io.ktor.websocket.Frame.Text
import io.ktor.websocket.readText
import kotlinx.coroutines.channels.consumeEach
import kotlinx.serialization.json.Json
import org.slf4j.LoggerFactory
import screenshare.common.Packet
import screenshare.common.Packet.JoinRoom
import kotlin.time.Duration.Companion.seconds

private val logger = LoggerFactory.getLogger("Server")

fun main(args: Array<String>) = io.ktor.server.cio.EngineMain.main(args)

fun Application.module() {
    install(ContentNegotiation) {
        json()
    }

    install(WebSockets) {
        pingPeriod = 5.seconds
        timeout = 10.seconds
        maxFrameSize = Long.MAX_VALUE
        masking = false
    }

    val rooms = mutableMapOf<String, Room>()

    routing {
        webSocket("/") {
            var roomUser: RoomUser? = null
            var roomId: String? = null

            logger.info("New connection: ${this.call.request.local.remoteHost}")

            val connectUser: suspend (JoinRoom) -> Unit = { joinRoomPacket ->
                roomUser = RoomUser.create(session = this, username = joinRoomPacket.username)
                roomId = joinRoomPacket.roomId

                val room = rooms.computeIfAbsent(roomId!!) { Room.create(roomId!!) }
                room.addUser(roomUser!!)
            }

            val disconnectUser =
                suspend {
                    val room = roomId?.let { rooms[it] }
                    val roomUser = roomUser
                    if (roomUser != null && room != null) {
                        room.removeUser(roomUser)
                        if (room.isEmpty) {
                            rooms.remove(room.id)
                        }
                    }
                }

            val handlePacket: suspend (Packet) -> Unit = { packet ->
                val room = roomId?.let { rooms[it] }
                val roomUser = roomUser
                if (roomUser != null && room != null) {
                    room.consumePacket(roomUser, packet)
                }
            }

            this.incoming.consumeEach { frame ->
                when (frame) {
                    is Text -> {
                        val packet = Json.decodeFromString<Packet>(frame.readText())
                        when (packet) {
                            is JoinRoom -> connectUser(packet)
                            else -> handlePacket(packet)
                        }
                    }

                    is Close -> disconnectUser()

                    else -> {}
                }
            }

            logger.info("Connection closed: ${this.call.request.local.remoteHost}")
            disconnectUser()
        }

        staticResources("/", "static")
    }
}
