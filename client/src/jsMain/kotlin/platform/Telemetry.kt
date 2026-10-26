package platform

import kotlinx.browser.window

@JsModule("@sentry/browser")
@JsNonModule
private external object Sentry {
    fun init(options: dynamic)

    fun replayIntegration(options: dynamic): dynamic

    fun consoleLoggingIntegration(options: dynamic): dynamic

    fun captureFeedback(params: dynamic): dynamic

    val logger: dynamic
}

// A DSN is public by design. Only the hosted site sends: local builds, self-hosts and the Tauri app don't.
private const val DSN =
    "https://ec23e4d71c94e8883a8c82dcaa1d9592@o4512308528087040.ingest.us.sentry.io/4512308563017728"
private const val HOST = "screen-share.fly.dev"

/** Errors, warnings and a few replays of sessions with an error (docs/native-desktop-plan.md, F7). */
object Telemetry {
    // Only where init ran: the feedback button stays hidden elsewhere.
    var enabled = false
        private set

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
        enabled = true
    }
}

/** "Algo deu errado?": the user's words, with Sentry's usual breadcrumbs and no name or room. */
fun sendFeedback(message: String) {
    if (!Telemetry.enabled) return
    val params: dynamic = js("({})")
    params.message = message.take(2_000)
    Sentry.captureFeedback(params)
}

private const val METRICS_KEY = "screenshare.usageMetrics"

/** "Enviar estatísticas de uso" (the output picker): on unless turned off; errors are sent either way. */
fun usageMetricsOn(): Boolean = runCatching { window.localStorage.getItem(METRICS_KEY) != "false" }.getOrDefault(true)

fun setUsageMetricsOn(on: Boolean) {
    runCatching { window.localStorage.setItem(METRICS_KEY, on.toString()) }
}

/**
 * A usage event (the "Métricas do Screenshare" design): a name and a few buckets, never a name,
 * room, address or window title.
 */
fun metric(
    name: String,
    vararg attributes: Pair<String, String>,
) {
    if (!Telemetry.enabled || !usageMetricsOn()) return
    val fields: dynamic = js("({})")
    fields.platform = "web"
    attributes.forEach { (key, value) -> fields[key] = value }
    runCatching { Sentry.logger.info(name, fields) }
}

/** Buckets, not exact values: they answer the questions without making anyone recognizable. */
fun peopleBucket(count: Int): String =
    when {
        count <= 1 -> "1"
        count == 2 -> "2"
        count <= 5 -> "3-5"
        else -> "6+"
    }

fun minutesBucket(milliseconds: Double): String =
    when {
        milliseconds < 60_000 -> "<1"
        milliseconds < 5 * 60_000 -> "1-5"
        milliseconds < 15 * 60_000 -> "5-15"
        milliseconds < 60 * 60_000 -> "15-60"
        else -> "60+"
    }

fun secondsBucket(milliseconds: Double): String =
    when {
        milliseconds < 5_000 -> "<5"
        milliseconds < 30_000 -> "5-30"
        else -> "30+"
    }
