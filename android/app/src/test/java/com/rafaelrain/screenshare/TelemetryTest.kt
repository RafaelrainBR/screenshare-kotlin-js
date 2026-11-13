package com.rafaelrain.screenshare

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TelemetryTest {
    private val joined = RoomView(localId = "me")

    @Test
    fun aDropTheEngineReconnectsFromIsNotAProblem() {
        assertNull(Telemetry.roomProblem(joined, RoomView(error = "websocket closed")))
    }

    @Test
    fun anErrorBeforeJoiningIsAProblem() {
        assertEquals("identity timeout", Telemetry.roomProblem(RoomView(), RoomView(error = "identity timeout")))
    }

    @Test
    fun anErrorInAJoinedRoomIsAProblemOnce() {
        val failed = joined.copy(error = "chat failed")
        assertEquals("chat failed", Telemetry.roomProblem(joined, failed))
        assertNull(Telemetry.roomProblem(failed, failed))
    }

    @Test
    fun aShareLimitIsAProblemOncePerKind() {
        val limited = joined.copy(localShareProblem = "SenderCpuTest")
        assertEquals("SenderCpuTest", Telemetry.shareLimitProblem(joined, limited))
        assertNull(Telemetry.shareLimitProblem(limited, joined))
        assertNull(Telemetry.shareLimitProblem(joined, limited))
        assertEquals("SenderBandwidthTest", Telemetry.shareLimitProblem(limited, joined.copy(localShareProblem = "SenderBandwidthTest")))
    }
}
