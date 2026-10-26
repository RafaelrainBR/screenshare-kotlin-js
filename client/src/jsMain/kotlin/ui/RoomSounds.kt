package ui

import kotlinx.browser.window
import kotlin.js.Date

/** Someone else joined, left or started sharing: a short tone, so a hidden window still tells. */
enum class RoomSound(
    /** Notes in Hz, played one after the other. */
    val notes: List<Double>,
) {
    JOIN(listOf(659.25, 880.0)),
    LEAVE(listOf(880.0, 659.25)),
    SHARE(listOf(784.0, 1046.5)),
}

/** "Sons da sala" (the output picker): synthesized here, no audio files, through the chosen speaker. */
object RoomSounds {
    private const val KEY = "screenshare.roomSounds"
    private const val NOTE_SECONDS = 0.09

    // After a server deploy everyone rejoins at once: one tone stands for the burst.
    private const val QUIET_MS = 1_500.0

    var enabled = runCatching { window.localStorage.getItem(KEY) != "false" }.getOrDefault(true)
        private set
    private var context: dynamic = null
    private var sinkId: String? = null
    private var lastAt = 0.0

    fun setEnabled(on: Boolean) {
        enabled = on
        runCatching { window.localStorage.setItem(KEY, on.toString()) }
        if (!on) platform.featureUsed("sounds_off")
    }

    fun play(sound: RoomSound) {
        val now = Date.now()
        if (!enabled || InterfaceMutations.isOutputMuted || now - lastAt < QUIET_MS) return
        lastAt = now
        runCatching {
            if (context == null) context = js("new (window.AudioContext || window.webkitAudioContext)()")
            val audio = context
            val device = InterfaceMutations.selectedOutputDeviceId.orEmpty()
            if (device != sinkId && audio.setSinkId != undefined) {
                sinkId = device
                audio.setSinkId(device)
            }
            audio.resume()
            var at = (audio.currentTime as Double) + 0.02
            sound.notes.forEach { frequency ->
                val oscillator = audio.createOscillator()
                val gain = audio.createGain()
                oscillator.type = "sine"
                oscillator.frequency.value = frequency
                // A soft attack and decay: no click, well under the voices.
                gain.gain.setValueAtTime(0.0001, at)
                gain.gain.exponentialRampToValueAtTime(0.06, at + 0.01)
                gain.gain.exponentialRampToValueAtTime(0.0001, at + NOTE_SECONDS)
                oscillator.connect(gain)
                gain.connect(audio.destination)
                oscillator.start(at)
                oscillator.stop(at + NOTE_SECONDS)
                at += NOTE_SECONDS
            }
        }
    }
}
