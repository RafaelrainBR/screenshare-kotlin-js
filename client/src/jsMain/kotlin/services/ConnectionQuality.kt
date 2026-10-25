package services

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.await
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import ui.InterfaceMutations
import kotlin.js.Date

/**
 * "Por que está travando?": what holds a screen's quality back, as far as this side can measure.
 * The sharer sees why its encoder backed off; a viewer sees loss and its own skipped frames.
 */
enum class QualityProblem(
    val label: String,
) {
    SENDER_CPU("Seu PC no limite"),
    SENDER_BANDWIDTH("Sua internet no limite"),
    UNSTABLE_CONNECTION("Conexão instável"),
    VIEWER_DECODE("Seu PC no limite"),
}

fun qualityExplanation(
    problem: QualityProblem,
    peerName: String,
    viewers: Int,
): String =
    when (problem) {
        QualityProblem.SENDER_CPU -> {
            "Seu PC não está dando conta de codificar o vídeo, então a qualidade da sua tela caiu. " +
                "Feche programas pesados ou escolha um preset mais leve."
        }

        QualityProblem.SENDER_BANDWIDTH -> {
            "Sua internet não está dando conta de enviar nessa qualidade, então a qualidade da sua tela caiu. " +
                "Escolha um preset mais leve." +
                if (viewers > 1) " Cada pessoa assistindo usa mais da sua internet." else ""
        }

        QualityProblem.UNSTABLE_CONNECTION -> {
            "A conexão entre você e $peerName está perdendo dados, por isso o vídeo trava. " +
                "Pode ser a sua internet ou a de $peerName."
        }

        QualityProblem.VIEWER_DECODE -> {
            "Seu PC não está dando conta de mostrar o vídeo e está pulando quadros. Feche programas pesados."
        }
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
) {
    // Keyed by "peer:track", so a new share or a reconnection starts from fresh counters.
    private val sendSamples = mutableMapOf<String, SendSample>()
    private val receiveSamples = mutableMapOf<String, ReceiveSample>()
    private val signals = mutableMapOf<String, QualitySignal>()
    private var job: Job? = null

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
    }

    private suspend fun tick() {
        val now = Date.now()
        val tiles = mutableMapOf<String, Pair<QualityProblem?, (QualityProblem) -> String>>()

        val localTrack = screenSharing.localScreenStream?.getVideoTracks()?.firstOrNull()
        val seenSends = mutableSetOf<String>()
        if (localTrack != null) {
            val problems =
                // A copy: a peer can leave while a getStats is pending.
                peerConnections.peers.toList().mapNotNull { (peerId, peer) ->
                    val stats = peer.trackStats(localTrack.id)?.await() ?: return@mapNotNull null
                    val sample = sendSample(stats, now) ?: return@mapNotNull null
                    val key = "$peerId:${localTrack.id}"
                    seenSends += key
                    sendSamples.put(key, sample)?.let { sendProblem(it, sample) }
                }
            // The CPU holds every viewer back, so it explains more than one slow link.
            val worst = problems.minByOrNull { it.ordinal }
            val viewers = peerConnections.peers.size
            tiles[ScreenSharing.LOCAL_TILE_ID] = worst to { p: QualityProblem -> qualityExplanation(p, "", viewers) }
        }
        sendSamples.keys.retainAll(seenSends)

        val seenReceives = mutableSetOf<String>()
        screenSharing.remoteScreenStreams.toList().forEach { (peerId, stream) ->
            val track = stream.getVideoTracks().firstOrNull() ?: return@forEach
            val stats = peerConnections.peers[peerId]?.trackStats(track.id)?.await() ?: return@forEach
            val sample = receiveSample(stats) ?: return@forEach
            val key = "$peerId:${track.id}"
            seenReceives += key
            val problem = receiveSamples.put(key, sample)?.let { receiveProblem(it, sample) }
            tiles[peerId] = problem to { p: QualityProblem -> qualityExplanation(p, resolveUsername(peerId), 0) }
        }
        receiveSamples.keys.retainAll(seenReceives)

        // A tile that stopped reporting must not keep a stale chip.
        signals.keys.filter { it !in tiles }.forEach { tileId ->
            if (signals.remove(tileId)?.shown != null) InterfaceMutations.setTileQuality(tileId, null, null)
        }
        tiles.forEach { (tileId, measured) ->
            val signal = signals.getOrPut(tileId) { QualitySignal() }
            val before = signal.shown
            val shown = signal.update(measured.first)
            if (shown == before) return@forEach
            // Sentry keeps these as logs: which problem, never who or where.
            if (shown != null) console.warn("Qualidade da tela: ${shown.name}")
            InterfaceMutations.setTileQuality(tileId, shown?.label, shown?.let(measured.second))
        }
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
