package ui

import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLElement

@Serializable
data class RecentRoom(
    val id: String,
    val favorite: Boolean = false,
)

private const val MAX_RECENT = 5
private const val MAX_ID = 64

/** [id] goes to the front of the recent ones; favorites keep their place and never age out. */
internal fun rememberRoom(
    rooms: List<RecentRoom>,
    id: String,
): List<RecentRoom> {
    if (id.isBlank() || id.length > MAX_ID) return rooms
    val favorite = rooms.firstOrNull { it.id == id }?.favorite ?: false
    val others = rooms.filter { it.id != id }
    val favorites = others.filter { it.favorite }
    val recent = others.filter { !it.favorite }
    return if (favorite) {
        rooms
    } else {
        favorites + (listOf(RecentRoom(id)) + recent).take(MAX_RECENT)
    }
}

/** Unstarring keeps the room as the newest recent one. */
internal fun toggleFavorite(
    rooms: List<RecentRoom>,
    id: String,
): List<RecentRoom> {
    val room = rooms.firstOrNull { it.id == id } ?: return rooms
    val others = rooms.filter { it.id != id }
    return if (room.favorite) {
        rememberRoom(others, id)
    } else {
        others.filter { it.favorite } + RecentRoom(id, favorite = true) + others.filter { !it.favorite }
    }
}

/** The join screen's recent and favorite rooms (screenshare.recentRooms, this browser only). */
object RecentRooms {
    private const val KEY = "screenshare.recentRooms"

    var rooms: List<RecentRoom> =
        runCatching { Json.decodeFromString<List<RecentRoom>>(window.localStorage.getItem(KEY) ?: "[]") }
            .getOrDefault(emptyList())
        private set

    fun remember(id: String) = update(rememberRoom(rooms, id))

    private fun update(next: List<RecentRoom>) {
        rooms = next
        runCatching { window.localStorage.setItem(KEY, Json.encodeToString(next)) }
        render()
    }

    /** A click on a room fills the code and joins; the star keeps it at the top. */
    fun render() {
        val list = document.getElementById("recent-rooms") as? HTMLElement ?: return
        list.innerHTML = ""
        list.classList.toggle("hidden", rooms.isEmpty())
        rooms.forEach { room ->
            val item = document.createElement("span") as HTMLElement
            item.className = "recent-room"
            val join = document.createElement("button") as HTMLButtonElement
            join.type = "button"
            join.className = "recent-room-join"
            join.textContent = room.id
            join.title = "Entrar na sala ${room.id}"
            join.addEventListener("click", {
                Elements.roomIdInput.value = room.id
                (document.getElementById("join-form")?.asDynamic())?.requestSubmit()
            })
            val star = document.createElement("button") as HTMLButtonElement
            star.type = "button"
            star.className = "recent-room-star"
            star.textContent = if (room.favorite) "★" else "☆"
            star.setAttribute("aria-pressed", room.favorite.toString())
            star.setAttribute(
                "aria-label",
                if (room.favorite) "Tirar ${room.id} dos favoritos" else "Favoritar ${room.id}",
            )
            star.title = star.getAttribute("aria-label")!!
            star.addEventListener("click", { update(toggleFavorite(rooms, room.id)) })
            item.appendChild(join)
            item.appendChild(star)
            list.appendChild(item)
        }
    }
}
