package com.rafaelrain.screenshare

import io.sentry.Sentry
import io.sentry.SentryLevel

/**
 * What happens in the room, for Sentry: a log line and a breadcrumb for each step, and an event
 * for each problem the person sees, so nobody has to report it. No names and no room codes.
 * Without a DSN (local and PR builds) Sentry is off and every call does nothing.
 */
object Telemetry {
    fun info(category: String, message: String) {
        Sentry.addBreadcrumb(message, category)
        Sentry.logger().info("%s: %s", category, message)
    }

    /** A problem the person sees (or that silently breaks something): its own issue in Sentry. */
    fun problem(category: String, message: String) {
        Sentry.addBreadcrumb(message, category)
        Sentry.logger().warn("%s: %s", category, message)
        Sentry.withScope { scope ->
            scope.setTag("problem", category)
            Sentry.captureMessage("$category: $message", SentryLevel.WARNING)
        }
    }

    /**
     * A new room error, unless it is why a joined room dropped: the engine reconnects on its own
     * (session.rs retries forever), so that drop is only logged with its reason above.
     */
    fun roomProblem(before: RoomView, next: RoomView): String? =
        next.error?.takeIf { it != before.error && !(before.joined && !next.joined) }

    /** The room's state changes between two polls of the engine. */
    fun diff(before: RoomView, next: RoomView) {
        if (!before.joined && next.joined) info("room", "connected (connection ${next.generation}, ${next.people.size} people)")
        if (before.joined && !next.joined) info("room", "disconnected, reconnecting${next.error?.let { " ($it)" }.orEmpty()}")
        if (before.people.size != next.people.size && next.joined) info("room", "${next.people.size} people, ${next.screenSharers.size} sharing screen, ${next.cameraSharers.size} camera")
        if (before.screenState != next.screenState) info("share", "screen ${before.screenState.ifEmpty { "off" }} -> ${next.screenState.ifEmpty { "off" }}")
        if (before.cameraState != next.cameraState) info("camera", "camera ${before.cameraState.ifEmpty { "off" }} -> ${next.cameraState.ifEmpty { "off" }}")
        if (before.microphoneEnabled != next.microphoneEnabled) info("voice", "microphone ${if (next.microphoneEnabled) "on" else "off"}")
        if (before.localShareProblem != next.localShareProblem) {
            next.localShareProblem?.let { problem("share-limit", "our share is limited by $it") } ?: info("share", "share limit cleared")
        }
        roomProblem(before, next)?.let { problem("room-error", it) }
        if (next.screenError != null && next.screenError != before.screenError) problem("share-error", next.screenError)
    }
}
