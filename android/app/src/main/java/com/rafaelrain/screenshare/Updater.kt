package com.rafaelrain.screenshare

import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import org.json.JSONArray
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

data class Update(val version: String, val apk: String, val sums: String)

/** android-vX.Y.Z (no suffix: release candidates are installed by hand) → its versionCode, else null. */
fun finalVersionCode(tag: String): Int? {
    val match = Regex("""^android-v(\d+)\.(\d{1,2})\.(\d{1,2})$""").matchEntire(tag) ?: return null
    val (major, minor, patch) = match.destructured.toList().map { it.toInt() }
    return major * 1_000_000 + minor * 10_000 + patch * 100 + 99
}

/** The newest final android-v* release above [current] with its APK and SHA256SUMS, from GitHub's release list JSON. */
fun newestUpdate(releases: JSONArray, current: Int): Update? =
    (0 until releases.length()).map { releases.getJSONObject(it) }
        .filter { !it.optBoolean("draft") && !it.optBoolean("prerelease") }
        .mapNotNull { release ->
            val code = finalVersionCode(release.optString("tag_name")) ?: return@mapNotNull null
            val assets = release.optJSONArray("assets") ?: return@mapNotNull null
            val urls = (0 until assets.length()).map { assets.getJSONObject(it) }.associate { it.optString("name") to it.optString("browser_download_url") }
            val apk = urls.entries.firstOrNull { it.key.endsWith(".apk") }?.value ?: return@mapNotNull null
            val sums = urls["SHA256SUMS"] ?: return@mapNotNull null
            Triple(code, release.optString("tag_name").removePrefix("android-v"), Update(release.optString("tag_name").removePrefix("android-v"), apk, sums))
        }
        .filter { it.first > current }
        .maxByOrNull { it.first }?.third

/** Updates from GitHub Releases (android-plan.md, Atualização): download, check SHA256SUMS, open the system installer. */
object Updater {
    private const val RELEASES = "https://api.github.com/repos/RafaelrainBR/screenshare/releases?per_page=100"

    private fun get(url: String): ByteArray {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.setRequestProperty("User-Agent", "screenshare-android")
        connection.setRequestProperty("Accept", "application/vnd.github+json")
        connection.connectTimeout = 15_000
        connection.readTimeout = 60_000
        try {
            if (connection.responseCode !in 200..299) error("HTTP ${connection.responseCode}")
            return connection.inputStream.use { it.readBytes() }
        } finally {
            connection.disconnect()
        }
    }

    /** Blocking: call off the main thread. Errors (offline, rate limit) mean "try later". */
    fun check(): Update? = runCatching {
        // Paged like the desktop: android and native releases share the repository.
        val all = JSONArray()
        for (page in 1..3) {
            val batch = JSONArray(String(get("$RELEASES&page=$page")))
            for (i in 0 until batch.length()) all.put(batch.get(i))
            if (batch.length() < 100) break
        }
        newestUpdate(all, BuildConfig.VERSION_CODE)
    }.getOrNull()

    /** Blocking: downloads and verifies the APK, then returns the installer intent. */
    fun download(context: Context, update: Update): Intent {
        val apk = get(update.apk)
        val name = update.apk.substringAfterLast('/')
        val expected = String(get(update.sums)).lines().firstOrNull { it.trim().endsWith(name) }?.substringBefore(' ')
            ?: error("SHA256SUMS sem $name")
        val actual = MessageDigest.getInstance("SHA-256").digest(apk).joinToString("") { "%02x".format(it) }
        check(actual.equals(expected, ignoreCase = true)) { "o arquivo baixado não confere" }
        val file = File(context.cacheDir, "updates").apply { mkdirs() }.resolve("screenshare.apk")
        file.writeBytes(apk)
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        // The system installer checks the signature: only an APK signed with our key installs over this one.
        return Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    }
}
