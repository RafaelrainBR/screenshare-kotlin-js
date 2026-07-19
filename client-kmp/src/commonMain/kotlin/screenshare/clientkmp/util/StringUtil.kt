package screenshare.clientkmp.util

import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

@OptIn(ExperimentalUuidApi::class)
fun generateRandomRoomId(): String = Uuid.random().toString().take(8)

fun String.getUsernameInitials(): String =
    this
        .split(" ")
        .mapNotNull { it.firstOrNull()?.toString()?.uppercase() }
        .take(2)
        .joinToString("")
