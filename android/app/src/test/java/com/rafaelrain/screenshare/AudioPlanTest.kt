package com.rafaelrain.screenshare

import android.media.AudioManager
import org.junit.Assert.assertEquals
import org.junit.Test

class AudioPlanTest {
    @Test
    fun microphoneOffKeepsTheMediaMode() {
        assertEquals(AudioPlan(AudioManager.MODE_NORMAL, speaker = true), audioPlan(microphone = false, headset = false, speakerWanted = true))
    }

    @Test
    fun microphoneOnUsesTheCallModeForEchoCancelling() {
        assertEquals(AudioManager.MODE_IN_COMMUNICATION, audioPlan(microphone = true, headset = false, speakerWanted = true).mode)
    }

    @Test
    fun aHeadsetNeverPlaysOnTheLoudspeaker() {
        assertEquals(false, audioPlan(microphone = true, headset = true, speakerWanted = true).speaker)
        assertEquals(false, audioPlan(microphone = true, headset = false, speakerWanted = false).speaker)
    }
}
