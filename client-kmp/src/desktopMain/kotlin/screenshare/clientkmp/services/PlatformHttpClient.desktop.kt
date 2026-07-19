package screenshare.clientkmp.services

import io.ktor.client.HttpClient
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.websocket.WebSockets

actual fun createPlatformHttpClient(): HttpClient =
    HttpClient(OkHttp) {
        install(WebSockets)
    }
