package services

/**
 * What the encoder gives up when the network tightens. The content hint goes
 * with it: `text` turns on the codec's screen-content tools so small fonts stay
 * legible, `motion` keeps movies and games from being encoded as a still desktop.
 */
enum class SharePriority(
    val contentHint: String,
    val degradationPreference: String,
) {
    SHARPNESS("text", "maintain-resolution"),
    BALANCED("motion", "balanced"),
    SMOOTHNESS("motion", "maintain-framerate"),
}

/** [maxBitrate] null means automatic, from the share height. */
data class ShareProfile(
    val fps: Int,
    val priority: SharePriority,
    val maxBitrate: Int? = null,
)

/** Starting points; the advanced settings edit the profile they fill in. */
enum class SharePreset(
    val profile: ShareProfile,
) {
    GAME(ShareProfile(60, SharePriority.SMOOTHNESS)),
    VIDEO(ShareProfile(30, SharePriority.BALANCED)),

    // A still screen costs almost nothing at any fps; 10 keeps scrolling and the cursor readable.
    TEXT(ShareProfile(10, SharePriority.SHARPNESS)),
    ;

    companion object {
        /** null when the profile was customized away from every preset. */
        fun matching(profile: ShareProfile): SharePreset? = entries.firstOrNull { it.profile == profile }
    }
}

/** Older versions stored only the frame rate: 60 fps was for games, anything else for video. */
fun shareProfileFrom(
    fps: String?,
    priority: String?,
    maxBitrate: String?,
): ShareProfile {
    val frameRate = fps?.toIntOrNull()?.takeIf { it in 1..60 } ?: SharePreset.VIDEO.profile.fps
    return ShareProfile(
        fps = frameRate,
        priority =
            SharePriority.entries.firstOrNull { it.name == priority }
                ?: if (frameRate >= 60) SharePriority.SMOOTHNESS else SharePriority.BALANCED,
        maxBitrate = maxBitrate?.toIntOrNull()?.takeIf { it > 0 },
    )
}

fun shareBitrate(
    profile: ShareProfile,
    height: Int,
): Int {
    profile.maxBitrate?.let { return it }
    // Only a ceiling: a still screen of text stays far below it.
    val base =
        when {
            height <= 720 -> 2_500_000
            height <= 1080 -> 6_000_000
            height <= 1440 -> 10_000_000
            else -> 15_000_000
        }
    return if (profile.fps >= 60) (base * 1.5).toInt() else base
}
