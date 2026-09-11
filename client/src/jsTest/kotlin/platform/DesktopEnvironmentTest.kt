package platform

import io.ktor.http.URLProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class DesktopEnvironmentTest {
    @Test
    fun browserUsesCurrentPageOrigin() {
        val endpoint =
            resolveWebSocketEndpoint(
                locationProtocol = "https:",
                locationHostname = "screen-share.example",
                locationPort = "",
                isDesktop = false,
                desktopServerUrl = "ws://ignored.example:9000",
            )

        assertEquals(URLProtocol.WSS, endpoint.protocol)
        assertEquals("screen-share.example", endpoint.host)
        assertEquals(443, endpoint.port)
    }

    @Test
    fun desktopUsesConfiguredKotlinServer() {
        val endpoint =
            resolveWebSocketEndpoint(
                locationProtocol = "http:",
                locationHostname = "tauri.localhost",
                locationPort = "",
                isDesktop = true,
                desktopServerUrl = "wss://screen-share.fly.dev",
            )

        assertEquals(URLProtocol.WSS, endpoint.protocol)
        assertEquals("screen-share.fly.dev", endpoint.host)
        assertEquals(443, endpoint.port)
    }

    @Test
    fun desktopRejectsNonWebSocketServerUrl() {
        assertFailsWith<IllegalArgumentException> {
            resolveWebSocketEndpoint(
                locationProtocol = "http:",
                locationHostname = "tauri.localhost",
                locationPort = "",
                isDesktop = true,
                desktopServerUrl = "https://screen-share.fly.dev",
            )
        }
    }

    @Test
    fun nativeAudioNeedsDesktopBridgeAndGenerator() {
        assertEquals(false, nativeSystemAudioSupported(false, true, true))
        assertEquals(false, nativeSystemAudioSupported(true, false, true))
        assertEquals(false, nativeSystemAudioSupported(true, true, false))
        assertEquals(true, nativeSystemAudioSupported(true, true, true))
    }
}
