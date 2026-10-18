package ui

import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.promise
import kotlin.js.Promise
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class SourcePermissionTest {
    @Test
    fun deniedPermissionDoesNotPromptAgainOnRefreshTicks() =
        MainScope().promise {
            val fixture = document.createElement("div")
            // Elements resolves the whole page when the wizard first opens.
            val controls =
                mapOf(
                    "input" to "username roomId message-input",
                    "select" to
                        "inputDevices outputDevices cameraDevices share-resolution share-fps share-priority share-bitrate",
                    "button" to
                        "joinBtn send-message-btn micToggle shareScreenBtn stopSharingBtn cameraBtn stopCameraBtn " +
                        "outputToggle confirm-share cancel-share share-source-refresh share-audio-on " +
                        "share-menu-back share-menu-source",
                    "div" to
                        "join-screen app-screen chat-messages current-room-id no-screen-message " +
                        "screens-stage screens-main " +
                        "screens-rail user-list participant-count quality-modal share-wizard-desc " +
                        "share-source-step share-source-grid share-source-state share-audio-step share-quality " +
                        "share-quality-summary share-preset-grid resolution-section share-bitrate-hint share-picker " +
                        "share-menu-state share-menu-custom share-menu-main share-menu-options " +
                        "share-menu-options-title share-menu-list",
                )
            fixture.innerHTML =
                controls.entries.joinToString("") { (tag, ids) ->
                    ids.split(" ").joinToString("") { "<$tag id=\"$it\"></$tag>" }
                }
            document.body!!.appendChild(fixture)
            val host = window.asDynamic()
            val originalBridge = host.__screenshareDesktopBridge
            val originalTauri = host.__TAURI_INTERNALS__
            val originalSetInterval = host.setInterval
            val originalClearInterval = host.clearInterval
            val timers = mutableMapOf<Int, () -> Unit>()
            var timerId = 0
            var prompts = 0
            val bridge: dynamic = js("({})")
            bridge.isAvailable = true
            bridge.enumerateSources = {
                prompts++
                val error = js("new Error('Allow Screen Recording')")
                error.code = "screen-recording-denied"
                Promise.reject(error)
            }
            host.__TAURI_INTERNALS__ = js("({})")
            host.__screenshareDesktopBridge = bridge
            host.setInterval = { tick: () -> Unit, _: Int -> (++timerId).also { timers[it] = tick } }
            host.clearInterval = { id: Int ->
                timers.remove(id)
                Unit
            }
            try {
                openNativeShareWizard()
                // Retain a queued callback too: clearing the timer must invalidate it.
                val queuedTick = timers.values.single()
                delay(50)
                assertEquals("Allow Screen Recording", Elements.shareSourceState.textContent)
                repeat(3) {
                    queuedTick()
                    delay(10)
                }
                assertEquals(1, prompts, "permission denial must not cause automatic permission requests")
                assertTrue(timers.isEmpty(), "the permission error must stop polling")

                // Reopening after granting permission must still enumerate and poll.
                var enumerations = 0
                bridge.enumerateSources = {
                    enumerations++
                    Promise.resolve(emptyArray<dynamic>())
                }
                openNativeShareWizard()
                delay(50)
                timers.values.single()()
                delay(50)
                assertEquals(2, enumerations)
            } finally {
                timers.clear()
                host.setInterval = originalSetInterval
                host.clearInterval = originalClearInterval
                host.__screenshareDesktopBridge = originalBridge
                host.__TAURI_INTERNALS__ = originalTauri
                fixture.remove()
            }
        }
}
