import org.w3c.dom.Document
import org.w3c.dom.Element
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

@OptIn(ExperimentalUuidApi::class)
fun generateRandomRoomId(): String = Uuid.random().toString()

fun String.getUsernameInitials(): String =
    this
        .split(" ")
        .mapNotNull { it.firstOrNull()?.toString()?.uppercase() }
        .take(2)
        .joinToString("")

fun String.sanitizeHTML(): String =
    this
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&#39;")

fun Document.createSafeElement(tag: String): Element = this.createElement(tag)
