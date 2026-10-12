package services

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class SharePresetTest {
    @Test
    fun legacyFrameRateMigratesToItsPreset() {
        assertEquals(SharePreset.GAME.profile, shareProfileFrom("60", null, null))
        assertEquals(SharePreset.VIDEO.profile, shareProfileFrom("30", null, null))
        assertEquals(SharePreset.VIDEO.profile, shareProfileFrom(null, null, null))
        assertEquals(SharePreset.VIDEO.profile, shareProfileFrom("999", "unknown", "abc"))
    }

    @Test
    fun customProfileRoundTripsAndMatchesNoPreset() {
        val custom = shareProfileFrom("30", "SHARPNESS", "8000000")
        assertEquals(ShareProfile(30, SharePriority.SHARPNESS, 8_000_000), custom)
        assertNull(SharePreset.matching(custom))
        assertEquals(SharePreset.TEXT, SharePreset.matching(shareProfileFrom("10", "SHARPNESS", "0")))
    }

    @Test
    fun manualBitrateWinsOverAutomatic() {
        assertEquals(6_000_000, shareBitrate(SharePreset.TEXT.profile, 1080))
        assertEquals(9_000_000, shareBitrate(SharePreset.GAME.profile, 1080))
        assertEquals(22_500_000, shareBitrate(SharePreset.GAME.profile, 2160))
        assertEquals(4_000_000, shareBitrate(ShareProfile(60, SharePriority.SMOOTHNESS, 4_000_000), 2160))
    }
}
