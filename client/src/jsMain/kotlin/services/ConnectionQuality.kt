package services

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.await
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import screenshare.common.ShareProblem
import ui.InterfaceMutations
import kotlin.js.Date

/**
 * "Por que está travando?": what holds a screen's quality back. The sharer sees why its encoder
 * backed off and, through the server (Packet.ShareQuality), so does whoever watches it; a viewer
 * also sees loss and its own skipped frames.
 */
enum class QualityProblem {
    // The sharer's own report: it explains a remote screen first.
    PEER_CPU,
    PEER_BANDWIDTH,
    SENDER_CPU,
    SENDER_BANDWIDTH,
    UNSTABLE_CONNECTION,
    VIEWER_DECODE,
}

fun qualityLabel(
    problem: QualityProblem,
    peerName: String,
): String =
    when (problem) {
        QualityProblem.PEER_CPU -> "PC de $peerName no limite"
        QualityProblem.PEER_BANDWIDTH -> "Internet de $peerName no limite"
        QualityProblem.SENDER_CPU, QualityProblem.VIEWER_DECODE -> "Seu PC no limite"
        QualityProblem.SENDER_BANDWIDTH -> "Sua internet no limite"
        QualityProblem.UNSTABLE_CONNECTION -> "Conexão instável"
    }

fun qualityExplanation(
    problem: QualityProblem,
    peerName: String,
    viewers: Int,
): String =
    when (problem) {
        QualityProblem.PEER_CPU -> {
            "O PC de $peerName não está dando conta de codificar o vídeo, então a qualidade da tela caiu. " +
                "Não é a sua conexão."
        }

        QualityProblem.PEER_BANDWIDTH -> {
            "A internet de $peerName não está dando conta de enviar nessa qualidade, então a qualidade da tela caiu." +
                if (viewers == 1) " Como só você está assistindo, pode ser também a sua internet." else ""
        }

        QualityProblem.SENDER_CPU -> {
            "Seu PC não está dando conta de codificar o vídeo, então a qualidade da sua tela caiu. " +
                "Feche programas pesados ou escolha um preset mais leve."
        }

        QualityProblem.SENDER_BANDWIDTH -> {
            "Sua internet não está dando conta de enviar nessa qualidade, então a qualidade da sua tela caiu. " +
                "Escolha um preset mais leve." +
                // One viewer: the congestion control can't tell our upload from their download.
                if (viewers > 1) {
                    " Cada pessoa assistindo usa mais da sua internet."
                } else {
                    " Como só uma pessoa está assistindo, pode ser também a internet dela."
                }
        }

        QualityProblem.UNSTABLE_CONNECTION -> {
            "A conexão entre você e $peerName está perdendo dados, por isso o vídeo trava. " +
                "Pode ser a sua internet ou a de $peerName."
        }

        QualityProblem.VIEWER_DECODE -> {
            "Seu PC não está dando conta de mostrar o vídeo e está pulando quadros. Feche programas pesados."
        }
    }

/** What our shown problem tells the room; only the encoder's limits are the sharer's to report. */
internal fun shareProblemOf(problem: QualityProblem?): String? =
    when (problem) {
        QualityProblem.SENDER_CPU -> ShareProblem.CPU
        QualityProblem.SENDER_BANDWIDTH -> ShareProblem.BANDWIDTH
        else -> null
    }

/** A sharer's report; a value this version does not know shows nothing. */
internal fun peerProblemOf(problem: String?): QualityProblem? =
    when (problem) {
        ShareProblem.CPU -> QualityProblem.PEER_CPU
        ShareProblem.BANDWIDTH -> QualityProblem.PEER_BANDWIDTH
        else -> null
    }

/** Cumulative outbound video counters: seconds the encoder was limited by the CPU and by bandwidth. */
data class SendSample(
    val atMs: Double,
    val cpuSeconds: Double?,
    val bandwidthSeconds: Double?,
    val reason: String?,
)

/** Cumulative inbound video counters. */
data class ReceiveSample(
    val packetsReceived: Double,
    val packetsLost: Double,
    val framesDecoded: Double,
    val framesDropped: Double,
)

internal const val QUALITY_WINDOW_MS = 3_000L

// Limited for half the window; NACK hides up to a few percent of loss; a static screen sends few packets.
private const val LIMITED_SHARE = 0.5
private const val LOSS_SHARE = 0.05
private const val MIN_PACKETS = 60.0
private const val DROPPED_SHARE = 0.15
private const val MIN_FRAMES = 30.0

internal fun sendProblem(
    previous: SendSample,
    current: SendSample,
): QualityProblem? {
    val elapsed = (current.atMs - previous.atMs) / 1000
    if (elapsed <= 0) return null

    // Firefox has no durations, only the reason at this instant.
    fun limited(
        before: Double?,
        now: Double?,
        reason: String,
    ): Double =
        if (before != null && now != null) {
            (now - before) / elapsed
        } else if (current.reason == reason) {
            1.0
        } else {
            0.0
        }
    return when {
        limited(previous.cpuSeconds, current.cpuSeconds, "cpu") >= LIMITED_SHARE -> {
            QualityProblem.SENDER_CPU
        }

        limited(previous.bandwidthSeconds, current.bandwidthSeconds, "bandwidth") >= LIMITED_SHARE -> {
            QualityProblem.SENDER_BANDWIDTH
        }

        else -> {
            null
        }
    }
}

internal fun receiveProblem(
    previous: ReceiveSample,
    current: ReceiveSample,
): QualityProblem? {
    val received = current.packetsReceived - previous.packetsReceived
    // Duplicates make packetsLost go down.
    val lost = (current.packetsLost - previous.packetsLost).coerceAtLeast(0.0)
    val decoded = current.framesDecoded - previous.framesDecoded
    val dropped = current.framesDropped - previous.framesDropped
    val lossy = received + lost >= MIN_PACKETS && lost / (received + lost) >= LOSS_SHARE
    val skipping = decoded + dropped >= MIN_FRAMES && dropped / (decoded + dropped) >= DROPPED_SHARE
    return when {
        lossy -> QualityProblem.UNSTABLE_CONNECTION
        skipping -> QualityProblem.VIEWER_DECODE
        else -> null
    }
}

/** A problem shows after two bad windows in a row and goes after three good ones, so a blip never flashes. */
internal class QualitySignal {
    var shown: QualityProblem? = null
        private set
    private var pending: QualityProblem? = null
    private var streak = 0

    fun update(window: QualityProblem?): QualityProblem? {
        if (window == shown) {
            pending = null
            streak = 0
            return shown
        }
        if (window != pending) {
            pending = window
            streak = 0
        }
        streak++
        if (streak >= if (window == null) 3 else 2) {
            shown = window
            pending = null
            streak = 0
        }
        return shown
    }
}

/** Reads each screen's stats every few seconds and puts the problem, if any, on its tile. */
class ConnectionQuality(
    private val peerConnections: PeerConnections,
    private val screenSharing: ScreenSharing,
    private val resolveUsername: (String) -> String,
    /** Sends what limits our share (a ShareProblem, or null); called only while the server relays it. */
    private val reportShare: (String?) -> Unit,
) {
    // Keyed by "peer:track", so a new share or a reconnection starts from fresh counters.
    private val sendSamples = mutableMapOf<String, SendSample>()
    private val receiveSamples = mutableMapOf<String, ReceiveSample>()
    private val signals = mutableMapOf<String, QualitySignal>()

    // What each sharer reported, and what each tile shows now.
    private val peerReports = mutableMapOf<String, QualityProblem>()
    private val displayed = mutableMapOf<String, Pair<QualityProblem, String?>>()

    // Servers before ShareQualities reject the packet, so nothing is sent until it arrives.
    private var relayAvailable = false
    private var reported: String? = null
    private var job: Job? = null

    /** The encoder our screen last went out on (outbound-rtp encoderImplementation), for share_ended. */
    var encoder: String? = null
        private set

    fun start(scope: CoroutineScope) {
        if (job != null) return
        job =
            scope.launch {
                while (isActive) {
                    delay(QUALITY_WINDOW_MS)
                    runCatching { tick() }
                }
            }
    }

    fun stop() {
        job?.cancel()
        job = null
        sendSamples.clear()
        receiveSamples.clear()
        signals.clear()
        displayed.clear()
        relayLost()
    }

    /** Packet.ShareQualities, on each join: every sharer's limit, and this server relays ours. */
    fun setPeerReports(qualities: Map<String, String>) {
        relayAvailable = true
        // A rejoin starts the server from nothing, so our limit goes again.
        reported = null
        peerReports.clear()
        qualities.forEach { (peer, problem) -> peerProblemOf(problem)?.let { peerReports[peer] = it } }
        screenSharing.remoteScreenStreams.keys
            .toList()
            .forEach(::show)
    }

    /** Packet.ShareQuality, or null when that peer stops sharing or leaves. */
    fun setPeerReport(
        peer: String,
        problem: String?,
    ) {
        val known = peerProblemOf(problem)
        if (known == null) peerReports.remove(peer) else peerReports[peer] = known
        show(peer)
    }

    /** The socket dropped: the next join's ShareQualities brings everything back. */
    fun relayLost() {
        relayAvailable = false
        reported = null
        peerReports.clear()
    }

    private suspend fun tick() {
        val now = Date.now()
        val tiles = mutableMapOf<String, QualityProblem?>()

        val localTrack = screenSharing.localScreenStream?.getVideoTracks()?.firstOrNull()
        val seenSends = mutableSetOf<String>()
        if (localTrack != null) {
            val problems =
                // A copy: a peer can leave while a getStats is pending.
                peerConnections.peers.toList().mapNotNull { (peerId, peer) ->
                    val stats = peer.trackStats(localTrack.id)?.await() ?: return@mapNotNull null
                    encoderName(stats)?.let { encoder = it }
                    val sample = sendSample(stats, now) ?: return@mapNotNull null
                    val key = "$peerId:${localTrack.id}"
                    seenSends += key
                    sendSamples.put(key, sample)?.let { sendProblem(it, sample) }
                }
            // The CPU holds every viewer back, so it explains more than one slow link.
            tiles[ScreenSharing.LOCAL_TILE_ID] = problems.minByOrNull { it.ordinal }
        }
        sendSamples.keys.retainAll(seenSends)

        val seenReceives = mutableSetOf<String>()
        screenSharing.remoteScreenStreams.toList().forEach { (peerId, stream) ->
            val track = stream.getVideoTracks().firstOrNull() ?: return@forEach
            val stats = peerConnections.peers[peerId]?.trackStats(track.id)?.await() ?: return@forEach
            val sample = receiveSample(stats) ?: return@forEach
            val key = "$peerId:${track.id}"
            seenReceives += key
            tiles[peerId] = receiveSamples.put(key, sample)?.let { receiveProblem(it, sample) }
        }
        receiveSamples.keys.retainAll(seenReceives)

        signals.keys.retainAll(tiles.keys)
        tiles.forEach { (tileId, measured) ->
            val signal = signals.getOrPut(tileId) { QualitySignal() }
            val before = signal.shown
            val shown = signal.update(measured)
            // quality_problem: which problem, never who or where.
            if (shown != before && shown != null) platform.metric("quality_problem", "problem" to shown.name)
        }
        // A tile that stopped reporting must not keep a stale chip; a sharer's report still holds.
        (displayed.keys + tiles.keys + peerReports.keys).forEach(::show)

        if (localTrack == null) {
            // The server already forgot it with stop-screen-share.
            reported = null
        } else {
            report(shareProblemOf(signals[ScreenSharing.LOCAL_TILE_ID]?.shown))
        }
    }

    private fun report(problem: String?) {
        if (!relayAvailable || problem == reported) return
        reported = problem
        reportShare(problem)
    }

    private fun show(tileId: String) {
        val remote = tileId != ScreenSharing.LOCAL_TILE_ID
        val problem =
            peerReports[tileId]?.takeIf { remote && tileId in screenSharing.remoteScreenStreams }
                ?: signals[tileId]?.shown
        // Our own encoder at its limit: one step lighter, a click away.
        val sending = problem == QualityProblem.SENDER_CPU || problem == QualityProblem.SENDER_BANDWIDTH
        val action = if (!remote && sending) ui.lighterShareFps()?.let { "Usar $it qps" } else null
        if (displayed[tileId]?.first == problem && displayed[tileId]?.second == action) return
        val name = if (remote) resolveUsername(tileId) else ""
        val applied =
            InterfaceMutations.setTileQuality(
                tileId,
                problem?.let { qualityLabel(it, name) },
                problem?.let { qualityExplanation(it, name, peerConnections.peers.size) },
                action,
            ) {
                ui.lightenShare()
                show(tileId)
            }
        // A tile not on the stage yet gets its chip on a later tick.
        if (!applied) return
        if (problem == null) displayed.remove(tileId) else displayed[tileId] = problem to action
    }
}

internal fun sendSample(
    stats: dynamic,
    atMs: Double,
): SendSample? {
    var sample: SendSample? = null
    stats.forEach { stat: dynamic ->
        if (stat.type == "outbound-rtp" && stat.kind == "video") {
            val durations = stat.qualityLimitationDurations
            sample =
                SendSample(
                    atMs,
                    durations?.cpu as? Double,
                    durations?.bandwidth as? Double,
                    stat.qualityLimitationReason as? String,
                )
        }
    }
    return sample
}

internal fun receiveSample(stats: dynamic): ReceiveSample? {
    var sample: ReceiveSample? = null
    stats.forEach { stat: dynamic ->
        if (stat.type == "inbound-rtp" && stat.kind == "video") {
            sample =
                ReceiveSample(
                    (stat.packetsReceived as? Double) ?: 0.0,
                    (stat.packetsLost as? Double) ?: 0.0,
                    (stat.framesDecoded as? Double) ?: 0.0,
                    (stat.framesDropped as? Double) ?: 0.0,
                )
        }
    }
    return sample
}

internal fun encoderName(stats: dynamic): String? {
    var name: String? = null
    stats.forEach { stat: dynamic ->
        if (stat.type == "outbound-rtp" && stat.kind == "video") name = stat.encoderImplementation as? String
    }
    return name
}
