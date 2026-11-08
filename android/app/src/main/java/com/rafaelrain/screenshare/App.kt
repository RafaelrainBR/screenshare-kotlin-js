package com.rafaelrain.screenshare

import android.app.Application
import com.rafaelrain.screenshare.engine.Engine
import io.sentry.android.core.SentryAndroid

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        // Without a DSN (local and pull request builds) nothing is sent.
        if (BuildConfig.SENTRY_DSN.isNotEmpty()) {
            SentryAndroid.init(this) { options ->
                options.dsn = BuildConfig.SENTRY_DSN
                options.release = "screenshare-android@${BuildConfig.VERSION_NAME}"
                options.isSendDefaultPii = false
                options.setTag("platform", "native-android")
            }
        }
        Engine.init(this, BuildConfig.SENTRY_DSN, "screenshare-android@${BuildConfig.VERSION_NAME}")
        Engine.send("""{"metrics":${Settings.metrics(this)}}""")
    }
}
