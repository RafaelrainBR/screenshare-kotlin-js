package services

import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.async
import kotlinx.coroutines.promise
import org.w3c.dom.mediacapture.MediaStream
import ui.InterfaceMutations
import kotlin.js.Promise
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MediaLifecycleTest {
    @Test
    fun replacingRemoteAudioDoesNotPlayTheParticipantTwice() {
        val stream = js("new MediaStream() ").unsafeCast<MediaStream>()
        try {
            InterfaceMutations.addAudioElementForUser("repeated-peer", stream)
            InterfaceMutations.addAudioElementForUser("repeated-peer", stream)
            assertEquals(1, document.querySelectorAll("#remote-audio-repeated-peer").length)
        } finally {
            InterfaceMutations.removeRemoteAudioElements()
        }
    }

    @Test
    fun leavingParticipantRemovesAudioAndMonitoringState() {
        val stream = js("new MediaStream()").unsafeCast<MediaStream>()
        val voice = VoiceChat()
        voice.remoteAudioStreams["departed-peer"] = stream
        InterfaceMutations.addAudioElementForUser("departed-peer", stream)
        voice.stopRemoteAudio("departed-peer")
        assertFalse(voice.remoteAudioStreams.containsKey("departed-peer"))
        assertEquals(null, document.getElementById("remote-audio-departed-peer"))
    }

    @Test
    fun mutedMicrophoneReplacementIsMutedBeforeReachingPeers() =
        MainScope().promise {
            val devices = window.navigator.mediaDevices.asDynamic()
            val original = devices.getUserMedia
            val track = fakeTrack()
            track.enabled = true
            val stream = fakeStream(track)
            devices.getUserMedia = { _: dynamic -> Promise.resolve(stream.unsafeCast<MediaStream>()) }
            var reachedPeer = false
            try {
                val voice = VoiceChat()
                runCatching {
                    voice.setupLocalMic(
                        isRequestCurrent = { true },
                        recreatePeerConnections = {},
                        onMicTrackReplaced = { _, replacement ->
                            reachedPeer = true
                            assertFalse(replacement!!.enabled)
                            // Stop at the peer boundary: DOM/device enumeration is unrelated.
                            error("peer boundary checked")
                        },
                    )
                }
                assertTrue(reachedPeer)
                assertFalse(track.enabled as Boolean)
            } finally {
                devices.getUserMedia = original
            }
        }

    @Test
    fun olderCameraPermissionCannotOverrideANewerRequest() =
        MainScope().promise {
            val devices = window.navigator.mediaDevices.asDynamic()
            val original = devices.getUserMedia
            val pending = CompletableDeferred<MediaStream>()
            val track = fakeTrack()
            val stream = fakeStream(track).unsafeCast<MediaStream>()
            devices.getUserMedia = { _: dynamic -> MainScope().promise { pending.await() } }
            val camera = CameraSharing({ it }, "local")
            try {
                val first =
                    async(start = CoroutineStart.UNDISPATCHED) {
                        runCatching { camera.setupLocalCameraStream({ true }, {}, {}) }
                    }
                devices.getUserMedia = { _: dynamic -> Promise.reject(IllegalStateException("permission denied")) }
                runCatching { camera.setupLocalCameraStream({ true }, {}, {}) }
                pending.complete(stream)
                assertTrue(first.await().isFailure)
                assertTrue(track.stopped as Boolean)
                assertEquals(null, camera.localCameraStream)
            } finally {
                devices.getUserMedia = original
            }
        }

    // Plain objects instead of js("...") strings: the IR compiler may rename locals those strings reference.
    private fun fakeTrack(): dynamic {
        val track: dynamic = js("({})")
        track.id = "replacement"
        track.stopped = false
        track.stop = { track.stopped = true }
        return track
    }

    private fun fakeStream(track: dynamic): dynamic {
        val stream: dynamic = js("({})")
        stream.getAudioTracks = { arrayOf(track) }
        stream.getVideoTracks = { emptyArray<dynamic>() }
        stream.getTracks = { arrayOf(track) }
        return stream
    }
}
