package ui

import kotlinx.browser.document
import kotlinx.browser.window
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLVideoElement
import kotlin.js.json
import kotlin.math.abs
import kotlin.math.sqrt

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

    /** Slides each of [elements] from where it was to where [change] puts it (FLIP). */
    fun flip(
        elements: List<HTMLElement>,
        change: () -> Unit,
    ) {
        if (reduced) return change()
        val before = elements.map { it to it.getBoundingClientRect() }
        change()
        for ((element, from) in before) {
            // A fullscreen tile has no on-page position to slide from.
            if (!element.isConnected || element == document.fullscreenElement) continue
            val to = element.getBoundingClientRect()
            if (from.width == 0.0 || to.width == 0.0) continue
            val dx = from.left + from.width / 2 - (to.left + to.width / 2)
            val dy = from.top + from.height / 2 - (to.top + to.height / 2)
            // One scale for both axes: grid cells and rail previews differ in shape, stretching distorts up to 7x.
            val scale = sqrt(from.width * from.height / (to.width * to.height))
            if (abs(dx) < 1 && abs(dy) < 1 && abs(scale - 1) < 0.01) continue
            val first = json("translate" to "${dx}px ${dy}px", "scale" to "$scale")
            val last = json("translate" to "0 0", "scale" to "1")
            element.asDynamic().animate(
                arrayOf(first, last),
                json("duration" to 220, "easing" to "cubic-bezier(.2, 0, 0, 1)"),
            )
        }
    }

    /** The first frame fades in over the tile (ui.css `video.ready`). */
    fun revealOnFirstFrame(video: HTMLVideoElement) {
        video.addEventListener("loadeddata", { video.classList.add("ready") })
    }
}
