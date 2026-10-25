package services

import decorators.RTCIceCandidate
import decorators.RTCPeerConnectionDecorator
import decorators.RTCSessionDescription
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.await
import kotlinx.coroutines.delay
import kotlinx.coroutines.promise
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ConnectionQualityTest {
    private fun send(
        atMs: Double,
        cpu: Double?,
        bandwidth: Double?,
        reason: String? = null,
    ) = SendSample(atMs, cpu, bandwidth, reason)

    @Test
    fun theSharerSeesWhatLimitedItsEncoderForHalfTheWindow() {
        assertEquals(QualityProblem.SENDER_CPU, sendProblem(send(0.0, 10.0, 0.0), send(3_000.0, 11.5, 0.0)))
        assertEquals(QualityProblem.SENDER_BANDWIDTH, sendProblem(send(0.0, 0.0, 4.0), send(3_000.0, 0.0, 6.0)))
        assertNull(sendProblem(send(0.0, 0.0, 0.0), send(3_000.0, 1.0, 1.0)))
        // Both: the CPU holds every viewer back.
        assertEquals(QualityProblem.SENDER_CPU, sendProblem(send(0.0, 0.0, 0.0), send(3_000.0, 3.0, 3.0)))
    }

    @Test
    fun withoutDurationsTheCurrentReasonDecides() {
        assertEquals(
            QualityProblem.SENDER_BANDWIDTH,
            sendProblem(send(0.0, null, null), send(3_000.0, null, null, "bandwidth")),
        )
        assertNull(sendProblem(send(0.0, null, null), send(3_000.0, null, null, "none")))
        assertNull(sendProblem(send(3_000.0, 0.0, 0.0), send(3_000.0, 9.0, 9.0)))
    }

    @Test
    fun theViewerSeesLossFirstAndThenItsOwnSkippedFrames() {
        val start = ReceiveSample(1_000.0, 10.0, 500.0, 0.0)
        assertEquals(
            QualityProblem.UNSTABLE_CONNECTION,
            receiveProblem(start, ReceiveSample(1_180.0, 30.0, 560.0, 40.0)),
        )
        assertEquals(QualityProblem.VIEWER_DECODE, receiveProblem(start, ReceiveSample(1_300.0, 11.0, 560.0, 20.0)))
        assertNull(receiveProblem(start, ReceiveSample(1_300.0, 12.0, 680.0, 2.0)))
        // A static screen sends a handful of packets: one lost is not a trend.
        assertNull(receiveProblem(start, ReceiveSample(1_010.0, 12.0, 503.0, 0.0)))
        // Duplicates lower packetsLost.
        assertNull(receiveProblem(start, ReceiveSample(1_300.0, 5.0, 680.0, 0.0)))
    }

    @Test
    fun aProblemShowsAfterTwoWindowsAndGoesAfterThree() {
        val signal = QualitySignal()
        assertNull(signal.update(QualityProblem.SENDER_CPU))
        assertNull(signal.update(null))
        assertNull(signal.update(QualityProblem.SENDER_CPU))
        assertEquals(QualityProblem.SENDER_CPU, signal.update(QualityProblem.SENDER_CPU))
        assertEquals(QualityProblem.SENDER_CPU, signal.update(null))
        assertEquals(QualityProblem.SENDER_CPU, signal.update(null))
        assertNull(signal.update(null))
    }

    @Test
    fun explanationsNameThePeerAndMentionEachViewer() {
        val unstable = qualityExplanation(QualityProblem.UNSTABLE_CONNECTION, "Ana", 0)
        assertTrue("Ana" in unstable && "sua internet" in unstable)
        assertTrue("Cada pessoa" in qualityExplanation(QualityProblem.SENDER_BANDWIDTH, "", 2))
        assertTrue("Cada pessoa" !in qualityExplanation(QualityProblem.SENDER_BANDWIDTH, "", 1))
    }

    @Test
    fun aRealConnectionGivesBothSidesTheirCounters() =
        MainScope().promise {
            val sender = js("new RTCPeerConnection()")
            val viewer = js("new RTCPeerConnection()")
            val canvas = document.createElement("canvas").asDynamic()
            canvas.width = 640
            canvas.height = 360
            val context = canvas.getContext("2d")
            var frame = 0
            val timer =
                window.setInterval({
                    context.fillStyle = if (frame++ % 2 == 0) "red" else "blue"
                    context.fillRect(0, 0, 640, 360)
                }, 33)
            val stream = canvas.captureStream(30)
            val track = stream.getVideoTracks()[0]
            sender.addTrack(track, stream)
            val local = RTCPeerConnectionDecorator(sender)
            val remote = RTCPeerConnectionDecorator(viewer)
            local.onIceCandidateAdd { it?.let { candidate -> remote.addIceCandidate(RTCIceCandidate(candidate)) } }
            remote.onIceCandidateAdd { it?.let { candidate -> local.addIceCandidate(RTCIceCandidate(candidate)) } }
            try {
                val offer = local.createOffer().await()
                local.setLocalDescription(offer).await()
                remote.setRemoteDescription(RTCSessionDescription(offer)).await()
                val answer = remote.createAnswer().await()
                remote.setLocalDescription(answer).await()
                local.setRemoteDescription(RTCSessionDescription(answer)).await()
                val received = viewer.getReceivers()[0].track.id as String
                withTimeout(5_000) {
                    while ((receiveSample(remote.trackStats(received)!!.await())?.framesDecoded ?: 0.0) ==
                        0.0
                    ) {
                        delay(50)
                    }
                }
                val sent = assertNotNull(sendSample(local.trackStats(track.id as String)!!.await(), 0.0))
                // Chrome reports durations; Firefox only the reason.
                assertTrue(sent.cpuSeconds != null || sent.reason != null)
            } finally {
                window.clearInterval(timer)
                track.stop()
                sender.close()
                viewer.close()
            }
        }
}
