package decorators

import kotlin.test.Test
import kotlin.test.assertEquals

class CodecOrderTest {
    @Test
    fun hardwareCodecsGoFirstAndKeepTheDefaultOrder() {
        assertEquals(listOf("vp9", "h264", "vp8", "av1"), codecOrder(emptySet()))
        assertEquals(listOf("h264", "av1", "vp9", "vp8"), codecOrder(setOf("av1", "h264")))
    }
}
