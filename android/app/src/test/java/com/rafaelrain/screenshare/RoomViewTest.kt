package com.rafaelrain.screenshare

import org.junit.Assert.assertEquals
import org.junit.Test

class RoomViewTest {
    @Test
    fun tellsTheRoomsOwnNotesFromPeopleTalking() {
        val view = RoomView.parse(
            """{"localSocketId":"me","messages":[
                {"username":"Sistema","content":"Você entrou na sala x"},
                {"username":"Ana","content":"oi","senderSocketId":"ana"},
                {"username":"Eu","content":"olá","senderSocketId":"me"}
            ]}""",
        )
        assertEquals(listOf(true to false, false to false, false to true), view.messages.map { it.system to it.mine })
    }
}
