package platform

import kotlinx.browser.window

@JsModule("@sentry/browser")
@JsNonModule
private external object Sentry {
    fun init(options: dynamic)

    fun replayIntegration(options: dynamic): dynamic

    fun consoleLoggingIntegration(options: dynamic): dynamic
}

// A DSN is public by design. Only the hosted site sends: local builds, self-hosts and the Tauri app don't.
private const val DSN =
    "https://ec23e4d71c94e8883a8c82dcaa1d9592@o4512308528087040.ingest.us.sentry.io/4512308563017728"
private const val HOST = "screen-share.fly.dev"

/** Errors, warnings and a few replays of sessions with an error (docs/native-desktop-plan.md, F7). */
object Telemetry {
    fun init() {
        if (window.location.hostname != HOST) return
        val options: dynamic = js("({})")
        options.dsn = DSN
        options.sendDefaultPii = false
        options.enableLogs = true
        // The free plan has 50 replays a month: only sessions that hit an error, and few of those.
        options.replaysSessionSampleRate = 0
        options.replaysOnErrorSampleRate = 0.1
        options.integrations =
            arrayOf(
                Sentry.replayIntegration(js("({maskAllText: true, maskAllInputs: true, blockAllMedia: true})")),
                Sentry.consoleLoggingIntegration(js("({levels: ['warn', 'error']})")),
            )
        // The room code rides in ?roomId=; names and rooms never leave the browser.
        options.beforeSend = { event: dynamic ->
            event.user = null
            val url = event.request?.url as? String
            if (url != null) event.request.url = url.substringBefore('?')
            event
        }
        Sentry.init(options)
    }
}
