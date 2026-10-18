package services

import decorators.RTCPeerConnectionDecorator
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.async
import kotlinx.coroutines.await
import kotlinx.coroutines.delay
import kotlinx.coroutines.promise
import kotlin.js.Promise
import kotlin.js.json
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class NegotiationTest {
    @Test
    fun incomingOfferWaitsForAnOfferAlreadyBeingCreated() =
        MainScope().promise {
            var finishOffer: (kotlin.js.Json) -> Unit = {}
            val fake = FakePeer()
            fake.offer = Promise { resolve, _ -> finishOffer = resolve }
            val (peers, _) = peersWith(isInitiator = false, fake)
            fake.negotiate()
            delay(10)
            val incoming = async { offer(peers) }
            delay(10)
            assertTrue(fake.calls.isEmpty(), "Remote SDP must not race an unfinished local offer")
            finishOffer(json("type" to "offer", "sdp" to ""))
            incoming.await()
            assertEquals(listOf("sld:offer", "sld:rollback", "srd:offer", "sld:answer"), fake.calls)
        }

    @Test
    fun rollbackReturnsARealPeerConnectionToStable() =
        MainScope().promise {
            val pc = js("new RTCPeerConnection()")
            try {
                pc.addTransceiver("video")
                val offer = (pc.createOffer() as Promise<dynamic>).await()
                (pc.setLocalDescription(offer) as Promise<dynamic>).await()
                RTCPeerConnectionDecorator(pc).rollback().await()
                assertEquals("stable", pc.signalingState as String)
            } finally {
                pc.close()
            }
        }

    @Test
    fun impolitePeerIgnoresOnlyTheCollidingOffer() =
        MainScope().promise {
            val (peers, fake) = peersWith(isInitiator = true)
            fake.signalingState = "have-local-offer"
            offer(peers)
            assertEquals(emptyList(), fake.calls)

            fake.signalingState = "stable"
            offer(peers)
            assertEquals(listOf("srd:offer", "sld:answer"), fake.calls)
        }

    @Test
    fun politePeerRollsBackAndAnswersTheCollidingOffer() =
        MainScope().promise {
            val (peers, fake) = peersWith(isInitiator = false)
            fake.signalingState = "have-local-offer"
            offer(peers)
            assertEquals(listOf("sld:rollback", "srd:offer", "sld:answer"), fake.calls)
        }

    @Test
    fun stoppedShareIsRemovedFromTheConnection() =
        MainScope().promise {
            val fake = FakePeer()
            fake.senders =
                arrayOf(
                    js("({ track: { id: 'dead', readyState: 'ended' } })"),
                    js("({ track: { id: 'live', readyState: 'live' } })"),
                    js("({ track: null })"),
                )
            peersWith(isInitiator = false, fake)
            assertEquals(listOf("remove:dead"), fake.calls)
        }

    @Test
    fun stoppedRemoteTrackDoesNotComeBackAsAScreen() =
        MainScope().promise {
            val screen = ScreenSharing({ it }, "local", MainScope())
            val (_, fake) = peersWith(isInitiator = false, screen = screen)
            val track = js("({ kind: 'video', readyState: 'ended' })")
            val tracks = { arrayOf(track) }
            val stream = js("({ getTracks: tracks, getVideoTracks: tracks })")
            fake.onTrack(js("({ streams: [stream] })"))
            assertTrue(screen.remoteScreenStreams.isEmpty())
        }

    private class FakePeer {
        var signalingState = "stable"
        val calls = mutableListOf<String>()
        var senders = emptyArray<dynamic>()
        var onTrack: (dynamic) -> Unit = {}
        var offer: Promise<kotlin.js.Json>? = null
        var negotiate: () -> Unit = {}
    }

    private suspend fun peersWith(
        isInitiator: Boolean,
        fake: FakePeer = FakePeer(),
        screen: ScreenSharing = ScreenSharing({ it }, "local", MainScope()),
    ): Pair<PeerConnections, FakePeer> {
        val pc: dynamic = js("({})")
        val state: () -> String = { fake.signalingState }
        js("Object.defineProperty(pc, 'signalingState', { get: state })")
        pc.getTransceivers = { emptyArray<dynamic>() }
        pc.getSenders = { fake.senders }
        pc.removeTrack = { sender: dynamic -> fake.calls += "remove:${sender.track.id}" }
        pc.addEventListener = { _: dynamic, listener: (dynamic) -> Unit -> fake.onTrack = listener }
        pc.createOffer = { fake.offer ?: Promise.reject(Throwable("offer not under test")) }
        fake.negotiate = { pc.onnegotiationneeded() }
        pc.createAnswer = { Promise.resolve(json("type" to "answer", "sdp" to "")) }
        pc.setRemoteDescription = { desc: dynamic ->
            fake.calls += "srd:${desc.type}"
            fake.signalingState = "have-remote-offer"
            Promise.resolve(null)
        }
        pc.setLocalDescription = { desc: dynamic ->
            fake.calls += "sld:${desc.type}"
            fake.signalingState = if (desc.type == "offer") "have-local-offer" else "stable"
            Promise.resolve(null)
        }
        val scope = MainScope()
        val peers =
            PeerConnections(
                VoiceChat(),
                screen,
                CameraSharing({ it }, "local"),
            )
        peers.createPeerConnection(
            websocketService = WebsocketService(handler = { _, _, _ -> }),
            socketId = "remote",
            roomId = "room",
            isInitiator = isInitiator,
            coroutineScope = scope,
            peerConnection = RTCPeerConnectionDecorator(pc),
        )
        delay(10) // let the initiator's (rejected) first offer settle
        return peers to fake
    }

    private suspend fun offer(peers: PeerConnections) {
        // Sending the answer fails without a websocket; the negotiation steps are what matter.
        runCatching {
            peers.updateDescriptionFromOffer(
                websocketService = WebsocketService(handler = { _, _, _ -> }),
                roomId = "room",
                senderId = "remote",
                descriptionJson = json("type" to "offer", "sdp" to ""),
            )
        }
    }
}
