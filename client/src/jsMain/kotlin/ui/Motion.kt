package ui

import kotlinx.browser.window
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLVideoElement
import kotlin.js.json

/** The enter and leave animations CSS cannot do on its own; ui.css has the rest. */
internal object Motion {
    // ui.css's reduced-motion rule does not reach the Web Animations API.
    private val reduced get() = window.matchMedia("(prefers-reduced-motion: reduce)").matches

    /** Fades and grows [element] in once: a CSS animation would replay each time the layout moves it. */
    fun enter(element: HTMLElement) {
        if (reduced) return
        element.asDynamic().animate(
            arrayOf(json("opacity" to 0, "scale" to "0.98"), json("opacity" to 1, "scale" to "1")),
            json("duration" to 180, "easing" to "cubic-bezier(.2, 0, 0, 1)"),
        )
    }

    /** Fades [element] out, then runs [done]. */
    fun leave(
        element: HTMLElement,
        done: () -> Unit,
    ) {
        if (reduced) return done()
        element.asDynamic().animate(
            arrayOf(json("opacity" to 1), json("opacity" to 0, "scale" to "0.98")),
            json("duration" to 150, "easing" to "ease-in", "fill" to "forwards"),
        )
        window.setTimeout({ done() }, 150)
    }

    /** The first frame fades in over the tile (ui.css `video.ready`). */
    fun revealOnFirstFrame(video: HTMLVideoElement) {
        video.addEventListener("loadeddata", { video.classList.add("ready") })
    }
}
