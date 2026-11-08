package com.rafaelrain.screenshare

import org.json.JSONArray
import org.json.JSONObject

data class Person(val id: String, val name: String, val muted: Boolean)

data class Message(val name: String, val text: String, val mine: Boolean)

/** The engine's SessionView (desktop-native/src/session.rs), only what the app shows. */
data class RoomView(
    val generation: Long = 0,
    val localId: String? = null,
    val people: List<Person> = emptyList(),
    val screenSharers: Set<String> = emptySet(),
    val cameraSharers: Set<String> = emptySet(),
    val messages: List<Message> = emptyList(),
    val messageCount: Long = 0,
    val error: String? = null,
    val screenState: String = "",
    val screenError: String? = null,
    val cameraState: String = "",
    val microphoneEnabled: Boolean = false,
    val outputEnabled: Boolean = false,
    val audioPending: Int = 0,
    val speaking: Set<String> = emptySet(),
    /** Our share's limit as the room hears it: "SenderCpu" or "SenderBandwidth" (quality.rs Problem). */
    val localShareProblem: String? = null,
    /** Each sharer's reported limit by socket id: "cpu" or "bandwidth". */
    val shareReports: Map<String, String> = emptyMap(),
    val roomSound: Pair<Long, String?> = 0L to null,
) {
    val joined get() = localId != null
    val sharing get() = screenState == "active"

    fun name(id: String) = people.firstOrNull { it.id == id }?.name ?: ""

    /** Every shown video: (socket id, camera), screens before cameras, like the web stage. */
    val videos: List<Pair<String, Boolean>>
        get() {
            val local = localId
            val screens = screenSharers.filter { it != local } + listOfNotNull(local.takeIf { sharing })
            val cameras = cameraSharers.filter { it != local } + listOfNotNull(local.takeIf { cameraState == "active" })
            return screens.map { it to false } + cameras.map { it to true }
        }

    companion object {
        fun parse(json: String): RoomView {
            val o = JSONObject(json)
            val local = o.optStringOrNull("localSocketId")
            val people = o.optJSONArray("participants").objects().map {
                Person(it.getString("socketId"), it.optString("username"), it.optBoolean("isMuted", true))
            }
            val share = o.optJSONArray("localShare")
            val sound = o.optJSONArray("roomSound")
            return RoomView(
                generation = o.optLong("generation"),
                localId = local,
                people = people,
                screenSharers = o.optJSONArray("screenSharers").strings().toSet(),
                cameraSharers = o.optJSONArray("cameraSharers").strings().toSet(),
                messages = o.optJSONArray("messages").objects().map {
                    Message(it.optString("username"), it.optString("content"), local != null && it.optStringOrNull("senderSocketId") == local)
                },
                messageCount = o.optLong("messageCount"),
                error = o.optStringOrNull("error"),
                screenState = o.optString("screenState"),
                screenError = o.optStringOrNull("screenError"),
                cameraState = o.optString("cameraState"),
                microphoneEnabled = o.optBoolean("microphoneEnabled"),
                outputEnabled = o.optBoolean("outputEnabled"),
                audioPending = o.optInt("audioPending"),
                speaking = o.optJSONArray("speaking").strings().toSet(),
                localShareProblem = share?.optString(0)?.takeIf { it.isNotEmpty() },
                shareReports = o.optJSONObject("shareReports")?.let { r -> r.keys().asSequence().associateWith { r.getString(it) } } ?: emptyMap(),
                roomSound = (sound?.optLong(0) ?: 0L) to sound?.let { if (it.isNull(1)) null else it.optString(1) },
            )
        }

        private fun JSONObject.optStringOrNull(key: String) = if (isNull(key)) null else optString(key)

        private fun JSONArray?.strings() = if (this == null) emptyList() else (0 until length()).map { getString(it) }

        private fun JSONArray?.objects() = if (this == null) emptyList() else (0 until length()).map { getJSONObject(it) }
    }
}

/** The quality.rs phrases for this device; other people stay "PC" (the protocol does not say which). */
fun shareBadge(problem: String?): String? = when (problem) {
    "SenderCpu" -> "Seu celular no limite"
    "SenderBandwidth" -> "Sua internet no limite"
    else -> null
}

fun peerBadge(report: String?, name: String): String? = when (report) {
    "cpu" -> "PC de $name no limite"
    "bandwidth" -> "Internet de $name no limite"
    else -> null
}
