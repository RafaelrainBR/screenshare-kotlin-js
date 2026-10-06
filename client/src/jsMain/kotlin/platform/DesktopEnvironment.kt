package platform

import io.ktor.http.URLProtocol
import io.ktor.http.Url
import kotlinx.coroutines.await
import kotlinx.browser.window
import org.w3c.dom.url.URLSearchParams

data class WebSocketEndpoint(
    val protocol: URLProtocol,
    val host: String,
    val port: Int,
)

object DesktopEnvironment {
    private const val SERVER_URL_STORAGE_KEY = "screenshare.desktop.serverUrl"
    private const val DEFAULT_SERVER_URL = "wss://screen-share.fly.dev"

    val isAvailable: Boolean
        get() = jsTypeOf(window.asDynamic().__TAURI_INTERNALS__) != "undefined"

    suspend fun autoStartEnabled(): Boolean =
        (window.asDynamic().__screenshareDesktopBridge.autoStartEnabled() as kotlin.js.Promise<Boolean>).await()

    suspend fun setAutoStart(enabled: Boolean) {
        (window.asDynamic().__screenshareDesktopBridge.setAutoStart(enabled) as kotlin.js.Promise<dynamic>).await()
    }

    suspend fun hideMainWindow() {
        (window.asDynamic().__screenshareDesktopBridge.hideMainWindow() as kotlin.js.Promise<dynamic>).await()
    }

    suspend fun showMainWindow() {
        (window.asDynamic().__screenshareDesktopBridge.showMainWindow() as kotlin.js.Promise<dynamic>).await()
    }

    suspend fun setTrayState(kind: String, active: Boolean, participants: Int? = null) {
        if (!isAvailable) return
        (window.asDynamic().__screenshareDesktopBridge.setTrayState(kind, active, participants) as kotlin.js.Promise<dynamic>).await()
    }

    val serverUrl: String?
        get() {
            if (!isAvailable) return null

            val configuredUrl = URLSearchParams(window.location.search).get("serverUrl")
            if (!configuredUrl.isNullOrBlank()) {
                window.sessionStorage.setItem(SERVER_URL_STORAGE_KEY, configuredUrl)
                return configuredUrl
            }

            return window.sessionStorage.getItem(SERVER_URL_STORAGE_KEY) ?: DEFAULT_SERVER_URL
        }
}

fun resolveWebSocketEndpoint(
    locationProtocol: String,
    locationHostname: String,
    locationPort: String,
    isDesktop: Boolean,
    desktopServerUrl: String?,
): WebSocketEndpoint {
    if (isDesktop && !desktopServerUrl.isNullOrBlank()) {
        val url = Url(desktopServerUrl)
        require(url.protocol == URLProtocol.WS || url.protocol == URLProtocol.WSS) {
            "Desktop server URL must use ws:// or wss://"
        }
        return WebSocketEndpoint(url.protocol, url.host, url.port)
    }

    val protocol = if (locationProtocol == "https:") URLProtocol.WSS else URLProtocol.WS
    val port = locationPort.ifBlank { if (protocol == URLProtocol.WSS) "443" else "80" }
    return WebSocketEndpoint(protocol, locationHostname, port.toInt())
}
