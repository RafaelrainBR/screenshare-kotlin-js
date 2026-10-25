package ui

import kotlinx.browser.window
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class RoomSoundsTest {
    @Test
    fun theChoiceIsSavedAndJoinAndLeaveMirrorEachOther() {
        RoomSounds.setEnabled(false)
        assertFalse(RoomSounds.enabled)
        assertEquals("false", window.localStorage.getItem("screenshare.roomSounds"))
        RoomSounds.setEnabled(true)
        assertTrue(RoomSounds.enabled)
        assertEquals(RoomSound.JOIN.notes.reversed(), RoomSound.LEAVE.notes)
    }

    @Test
    fun aSoundStartsOneOscillatorPerNoteAndABurstPlaysOnce() {
        val started = mutableListOf<Double>()
        val record = { frequency: Double -> started += frequency }
        val real = window.asDynamic().AudioContext
        // Records each oscillator's frequency instead of making noise.
        window.asDynamic().AudioContext =
            js(
                """function() {
                  var param = function() { return { value: 0, setValueAtTime: function() {}, exponentialRampToValueAtTime: function() {} }; };
                  var node = function() { return { connect: function() {} }; };
                  this.currentTime = 0; this.destination = {}; this.resume = function() {};
                  this.createGain = function() { var n = node(); n.gain = param(); return n; };
                  this.createOscillator = function() {
                    var n = node(); n.frequency = param();
                    n.start = function() { record(n.frequency.value); }; n.stop = function() {};
                    return n;
                  };
                }""",
            )
        try {
            RoomSounds.setEnabled(true)
            RoomSounds.play(RoomSound.SHARE)
            RoomSounds.play(RoomSound.JOIN)
            assertEquals(RoomSound.SHARE.notes, started)
        } finally {
            window.asDynamic().AudioContext = real
        }
    }
}
