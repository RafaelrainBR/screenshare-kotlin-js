package com.rafaelrain.screenshare

import android.content.Context

/** Name, recent and favorite rooms and the switches, in SharedPreferences (a handful of values). */
object Settings {
    private fun prefs(context: Context) = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    fun name(context: Context) = prefs(context).getString("name", "").orEmpty()

    fun setName(context: Context, name: String) = prefs(context).edit().putString("name", name).apply()

    fun sounds(context: Context) = prefs(context).getBoolean("sounds", true)

    fun setSounds(context: Context, on: Boolean) = prefs(context).edit().putBoolean("sounds", on).apply()

    fun metrics(context: Context) = prefs(context).getBoolean("metrics", true)

    fun setMetrics(context: Context, on: Boolean) = prefs(context).edit().putBoolean("metrics", on).apply()

    /** Most recent first, at most 8, like the web's recent rooms. */
    fun recents(context: Context) = prefs(context).getString("recents", "").orEmpty().split('\n').filter { it.isNotBlank() }

    fun remember(context: Context, room: String) =
        prefs(context).edit().putString("recents", (listOf(room) + recents(context).filter { it != room }).take(8).joinToString("\n")).apply()

    fun favorites(context: Context) = prefs(context).getStringSet("favorites", emptySet()).orEmpty()

    fun toggleFavorite(context: Context, room: String) {
        val current = favorites(context)
        prefs(context).edit().putStringSet("favorites", if (room in current) current - room else current + room).apply()
    }

    fun lastShare(context: Context): ShareChoice = prefs(context).run {
        ShareChoice(getString("sharePreset", "video")!!, getInt("shareSide", 720), getBoolean("shareAudio", true))
    }

    fun setLastShare(context: Context, choice: ShareChoice) =
        prefs(context).edit().putString("sharePreset", choice.preset).putInt("shareSide", choice.shortSide).putBoolean("shareAudio", choice.audio).apply()
}
