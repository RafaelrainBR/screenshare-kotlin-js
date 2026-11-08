package com.rafaelrain.screenshare

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class UpdaterTest {
    private fun release(tag: String, prerelease: Boolean = false, assets: List<String> = listOf("Screenshare-android_x.apk", "SHA256SUMS")) =
        JSONObject().put("tag_name", tag).put("prerelease", prerelease).put("draft", false)
            .put("assets", JSONArray(assets.map { JSONObject().put("name", it).put("browser_download_url", "https://example/$tag/$it") }))

    @Test
    fun picksTheHighestFinalAndroidRelease() {
        val releases = JSONArray(
            listOf(
                release("native-v1.0.9"),
                release("android-v1.2.0"),
                release("android-v1.10.0"),
                release("android-v2.0.0-rc.1", prerelease = true),
                release("android-v1.11.0", assets = listOf("SHA256SUMS")),
            ),
        )
        assertEquals("1.10.0", newestUpdate(releases, current = 1_020_099)?.version)
    }

    @Test
    fun nothingWhenAlreadyNewest() {
        assertNull(newestUpdate(JSONArray(listOf(release("android-v1.0.0"))), current = 1_000_099))
    }

    @Test
    fun aReleaseCandidateInstallUpdatesToItsFinal() {
        assertEquals("1.0.0", newestUpdate(JSONArray(listOf(release("android-v1.0.0"))), current = 1_000_003)?.version)
    }

    @Test
    fun onlyFinalTagsHaveACode() {
        assertEquals(1_020_399, finalVersionCode("android-v1.2.3"))
        assertNull(finalVersionCode("android-v1.2.3-rc.1"))
        assertNull(finalVersionCode("native-v1.2.3"))
    }
}
