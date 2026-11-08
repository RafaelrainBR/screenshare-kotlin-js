package com.rafaelrain.screenshare

import android.content.Context
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build

/**
 * How the phone plays the room (docs/design/android-plan.md, "Voz e rotas de áudio"):
 * microphone off keeps the normal media mode (Bluetooth stays A2DP, volume keys move media);
 * microphone on needs the call mode for the hardware echo canceller.
 */
data class AudioPlan(val mode: Int, val speaker: Boolean)

fun audioPlan(microphone: Boolean, headset: Boolean, speakerWanted: Boolean): AudioPlan =
    AudioPlan(
        mode = if (microphone) AudioManager.MODE_IN_COMMUNICATION else AudioManager.MODE_NORMAL,
        // Like Discord: no headset means the loudspeaker, unless the person picked the earpiece.
        speaker = !headset && speakerWanted,
    )

private val HEADSETS = setOf(
    AudioDeviceInfo.TYPE_WIRED_HEADSET,
    AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
    AudioDeviceInfo.TYPE_USB_HEADSET,
    AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
    AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
    AudioDeviceInfo.TYPE_BLE_HEADSET,
)

class AudioRoutes(context: Context) {
    private val audio = context.getSystemService(AudioManager::class.java)

    fun headset() = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS).any { it.type in HEADSETS }

    /** Never asks for audio focus: the game keeps playing at its own volume (no ducking). */
    fun apply(microphone: Boolean, speakerWanted: Boolean) {
        val plan = audioPlan(microphone, headset(), speakerWanted)
        audio.mode = plan.mode
        if (Build.VERSION.SDK_INT >= 31) {
            if (plan.mode == AudioManager.MODE_IN_COMMUNICATION) {
                val wanted = audio.availableCommunicationDevices.firstOrNull {
                    if (plan.speaker) it.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER else it.type in HEADSETS
                } ?: audio.availableCommunicationDevices.firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
                if (wanted != null) audio.setCommunicationDevice(wanted)
            } else {
                audio.clearCommunicationDevice()
            }
        } else {
            @Suppress("DEPRECATION")
            audio.isSpeakerphoneOn = plan.mode == AudioManager.MODE_IN_COMMUNICATION && plan.speaker
        }
    }

    fun release() {
        if (Build.VERSION.SDK_INT >= 31) audio.clearCommunicationDevice()
        audio.mode = AudioManager.MODE_NORMAL
    }
}
