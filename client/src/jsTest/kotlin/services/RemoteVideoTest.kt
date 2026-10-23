package services

import org.w3c.dom.mediacapture.MediaStream
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class RemoteVideoTest {
    @Test
    fun aReFiredTrackKeepsItsPlace() {
        // Coming back to the tab re-fires the screen's `track` while the camera is on.
        assertFalse(
            classify(known = false, liveCamera = true, sharingScreen = true, sharingCamera = true, earlierVideos = 2),
        )
        assertTrue(
            classify(known = true, liveScreen = true, sharingScreen = true, sharingCamera = true, earlierVideos = 2),
        )
    }

    @Test
    fun aSecondVideoIsTheOtherKindWhateverArrivedFirst() {
        // The camera's offer before its start-camera-share packet, with the screen playing.
        assertTrue(classify(liveScreen = true, sharingScreen = true, earlierVideos = 1))
        // The screen's offer before its start-screen-share packet, with the camera playing.
        assertFalse(classify(liveCamera = true, sharingCamera = true, earlierVideos = 1))
    }

    @Test
    fun withoutAnotherLiveVideoTheShareFlagsDecide() {
        assertFalse(classify(sharingScreen = true))
        assertTrue(classify(sharingCamera = true))
        assertFalse(classify(sharingScreen = true, sharingCamera = true))
        assertTrue(classify(sharingScreen = true, sharingCamera = true, earlierVideos = 1))
        assertFalse(classify())
    }

    @Test
    fun replacingAStreamKeepsTheTracksItStillHas() {
        val shared = fakeTrack("video")
        val audio = fakeTrack("audio")
        stopReplacedTracks(fakeStream(shared, audio), fakeStream(shared))
        assertEquals(false, shared.stopped)
        assertEquals(true, audio.stopped)
    }

    private fun classify(
        known: Boolean? = null,
        liveScreen: Boolean = false,
        liveCamera: Boolean = false,
        sharingScreen: Boolean = false,
        sharingCamera: Boolean = false,
        earlierVideos: Int = 0,
    ) = isCameraTrack(known, liveScreen, liveCamera, sharingScreen, sharingCamera, earlierVideos)

    // Plain objects instead of js("...") strings: the IR compiler may rename locals those strings reference.
    private fun fakeTrack(id: String): dynamic {
        val track: dynamic = js("({})")
        track.id = id
        track.stopped = false
        track.stop = { track.stopped = true }
        return track
    }

    private fun fakeStream(vararg tracks: dynamic): MediaStream {
        val stream: dynamic = js("({})")
        stream.getTracks = { tracks }
        return stream.unsafeCast<MediaStream>()
    }
}
