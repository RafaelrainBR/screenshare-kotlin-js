package services

import decorators.RTCIceCandidate
import decorators.RTCPeerConnectionDecorator
import decorators.RTCSessionDescription
import decorators.improveVideoStartupSdp
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.await
import kotlinx.coroutines.delay
import kotlinx.coroutines.promise
import kotlinx.coroutines.withTimeout
import kotlin.js.Promise
import kotlin.js.json
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class WebRtcStartupTest {
    @Test
    fun rejectedCaptureFpsCanBeRetriedAndIsReported() =
        MainScope().promise {
            val notice = document.createElement("div")
            notice.id = "ui-notice"
            document.body!!.appendChild(notice)
            val track: dynamic = js("({})")
            track.getConstraints = { json("width" to json("ideal" to 1920)) }
            track.applyConstraints = { _: dynamic -> Promise.reject(Throwable("unsupported FPS")) }
            val tracks = { arrayOf(track) }
            val stream = js("({ getVideoTracks: tracks })")
            val screen = ScreenSharing({ it }, "local", MainScope())
            screen.localScreenStream = stream.unsafeCast<org.w3c.dom.mediacapture.MediaStream>()
            try {
                screen.updateProfile(SharePreset.GAME.profile)
                assertEquals(30, screen.lastShareProfile.fps)
                assertTrue(notice.textContent!!.contains("quadros por segundo"))
                track.applyConstraints = { constraints: dynamic ->
                    assertEquals(1920, constraints.width.ideal as Int)
                    assertEquals(60, constraints.frameRate.max as Int)
                    Promise.resolve(null)
                }
                screen.updateProfile(SharePreset.GAME.profile)
                assertEquals(60, screen.lastShareProfile.fps)
            } finally {
                notice.remove()
            }
        }

    @Test
    fun startupHintPreservesAudioRetransmissionAndExistingCodecOptions() {
        val audio = "m=audio 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 opus/48000/2\r\na=fmtp:96 stereo=1\r\n"
        val video =
            "m=video 9 UDP/TLS/RTP/SAVPF 96 97\r\na=rtpmap:96 VP9/90000\r\na=fmtp:96 profile-id=0\r\n" +
                "a=rtpmap:97 rtx/90000\r\na=fmtp:97 apt=96\r\n"
        val expected = audio + video.replace("profile-id=0", "profile-id=0;x-google-start-bitrate=1500")
        assertEquals(expected, improveVideoStartupSdp(audio + video))
        assertEquals(expected, improveVideoStartupSdp(expected))
        val explicit = expected.replace("x-google-start-bitrate=1500", "x-google-start-bitrate=500")
        assertEquals(explicit, improveVideoStartupSdp(explicit))
    }

    @Test
    fun initialVideoBitrateDoesNotStartAtThumbnailQuality() =
        MainScope().promise {
            val sender = js("new RTCPeerConnection()")
            val viewer = js("new RTCPeerConnection()")
            val canvas = document.createElement("canvas").asDynamic()
            canvas.width = 1920
            canvas.height = 1080
            val context = canvas.getContext("2d")
            var frame = 0
            val timer =
                window.setInterval({
                    context.fillStyle = if (frame++ % 2 == 0) "red" else "blue"
                    context.fillRect(0, 0, 1920, 1080)
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
                val initialBitrate =
                    withTimeout(5_000) {
                        var bitrate = 0.0
                        while (bitrate == 0.0) {
                            val stats = (sender.getStats() as Promise<dynamic>).await()
                            stats.forEach { stat: dynamic ->
                                if (stat.type == "outbound-rtp" && stat.kind == "video" && stat.framesEncoded > 0) {
                                    bitrate = (stat.targetBitrate as? Double) ?: 0.0
                                }
                            }
                            if (bitrate == 0.0) delay(20)
                        }
                        bitrate
                    }
                assertTrue(initialBitrate >= 1_000_000, "Initial bitrate was $initialBitrate bps")
            } finally {
                window.clearInterval(timer)
                track.stop()
                sender.close()
                viewer.close()
            }
        }

    @Test
    fun encodingSelectedBeforeNegotiationIsAppliedWhenTheSenderIsReady() =
        MainScope().promise {
            val pc = js("new RTCPeerConnection()")
            val canvas =
                kotlinx.browser.document
                    .createElement("canvas")
                    .asDynamic()
            val stream = canvas.captureStream(30)
            val track = stream.getVideoTracks()[0]
            pc.addTrack(track, stream)
            val peer = RTCPeerConnectionDecorator(pc)
            try {
                peer.applyVideoEncoding(track.id as String, 8_000_000, 30, "balanced")
                val offer = peer.createOffer().await()
                peer.setLocalDescription(offer).await()
                val params = pc.getSenders()[0].getParameters()
                assertEquals(8_000_000, params.encodings[0].maxBitrate as? Int)
                assertEquals(30, params.encodings[0].maxFramerate as? Int)
            } finally {
                track.stop()
                pc.close()
            }
        }

    @Test
    fun candidatesWaitUntilRemoteDescriptionFinishes() =
        MainScope().promise {
            val calls = mutableListOf<String>()
            var finish: (dynamic) -> Unit = {}
            val pending = Promise<dynamic> { resolve, _ -> finish = resolve }
            val pc: dynamic = js("({})")
            pc.setRemoteDescription = { _: dynamic -> pending }
            pc.addIceCandidate = { _: dynamic ->
                calls += "candidate"
                Promise.resolve(null)
            }
            val peer = RTCPeerConnectionDecorator(pc)
            val applied = peer.setRemoteDescription(RTCSessionDescription(json("type" to "offer", "sdp" to "")))
            peer.addIceCandidate(RTCIceCandidate(json("candidate" to "", "sdpMid" to "0"))).await()
            assertTrue(calls.isEmpty(), "ICE must wait for the pending SDP")
            finish(null)
            applied.await()
            assertEquals(listOf("candidate"), calls)
        }

    @Test
    fun codecPreferenceReachesTheActualVideoTransceiver() =
        MainScope().promise {
            val pc = js("new RTCPeerConnection()")
            val peer = RTCPeerConnectionDecorator(pc)
            val canvas =
                kotlinx.browser.document
                    .createElement("canvas")
                    .asDynamic()
            val stream = canvas.captureStream(10)
            val track = stream.getVideoTracks()[0]
            pc.addTrack(track, stream)
            try {
                peer.preferVideoCodecs()
                val offer = (pc.createOffer() as Promise<dynamic>).await()
                val sdp = offer.sdp as String
                val payload = Regex("m=video [^\\r\\n]+").find(sdp)!!.value.split(" ")[3]
                assertTrue(sdp.contains("a=rtpmap:$payload VP9/90000"), "VP9 should be preferred: $sdp")
            } finally {
                track.stop()
                pc.close()
            }
        }
}
