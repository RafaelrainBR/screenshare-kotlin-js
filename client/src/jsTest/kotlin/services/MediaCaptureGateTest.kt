package services

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MediaCaptureGateTest {
    @Test
    fun unlockAndReconnectNeverRevivePendingCapture() {
        val gate = MediaCaptureGate()
        val pending = gate.request()
        assertTrue(pending())
        gate.setAvailable(false)
        assertFalse(pending())
        assertFalse(gate.request()())
        gate.setAvailable(true)
        assertFalse(pending())
        val fresh = gate.request()
        assertTrue(fresh())
        gate.invalidate()
        assertFalse(fresh())
        assertTrue(gate.request()())
    }
}
