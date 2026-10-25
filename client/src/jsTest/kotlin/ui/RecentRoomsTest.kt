package ui

import kotlin.test.Test
import kotlin.test.assertEquals

class RecentRoomsTest {
    private fun ids(rooms: List<RecentRoom>) = rooms.map { if (it.favorite) "*${it.id}" else it.id }

    @Test
    fun theNewestRoomLeadsTheRecentOnesAndOnlyFiveStay() {
        val rooms = listOf("a", "b", "c", "d", "e", "f").fold(emptyList<RecentRoom>(), ::rememberRoom)
        assertEquals(listOf("f", "e", "d", "c", "b"), ids(rooms))
        assertEquals(listOf("c", "f", "e", "d", "b"), ids(rememberRoom(rooms, "c")))
        assertEquals(ids(rooms), ids(rememberRoom(rooms, " ")))
        assertEquals(ids(rooms), ids(rememberRoom(rooms, "x".repeat(65))))
    }

    @Test
    fun favoritesStayAtTheFrontAndNeverAgeOut() {
        var rooms = listOf("a", "b", "c").fold(emptyList<RecentRoom>(), ::rememberRoom)
        rooms = toggleFavorite(rooms, "a")
        assertEquals(listOf("*a", "c", "b"), ids(rooms))
        rooms = listOf("d", "e", "f", "g", "h", "a").fold(rooms, ::rememberRoom)
        assertEquals(listOf("*a", "h", "g", "f", "e", "d"), ids(rooms))
        assertEquals(listOf("a", "h", "g", "f", "e"), ids(toggleFavorite(rooms, "a")))
    }
}
