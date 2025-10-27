import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

@OptIn(ExperimentalUuidApi::class)
fun generateRandomRoomId(): String {
    return Uuid.random().toString()
}

fun String.getUsernameInitials(): String {
    return this.split(" ")
        .mapNotNull { it.firstOrNull()?.toString()?.uppercase() }
        .take(2).joinToString("")
}
