package platform

import kotlin.test.Test
import kotlin.test.assertEquals

class TelemetryTest {
    @Test
    fun metricsCarryBucketsNotExactValues() {
        assertEquals(listOf("1", "2", "3-5", "3-5", "6+"), listOf(1, 2, 3, 5, 6).map(::peopleBucket))
        assertEquals(
            listOf("<1", "1-5", "5-15", "15-60", "60+"),
            listOf(59_000.0, 60_000.0, 5 * 60_000.0, 15 * 60_000.0, 60 * 60_000.0).map(::minutesBucket),
        )
        assertEquals(listOf("<5", "5-30", "30+"), listOf(4_999.0, 5_000.0, 30_000.0).map(::secondsBucket))
    }
}
