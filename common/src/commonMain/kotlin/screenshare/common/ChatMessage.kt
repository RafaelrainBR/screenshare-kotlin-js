package screenshare.common

import kotlinx.serialization.Serializable

@Serializable
data class ChatMessage(
    val username: String,
    val content: String,
    val timestamp: Long,
)
