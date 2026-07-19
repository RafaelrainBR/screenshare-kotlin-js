package screenshare.clientkmp.services

enum class Resolution(val width: Int, val height: Int, val label: String) {
    P480(854, 480, "480p"),
    P720(1280, 720, "720p"),
    P1080(1920, 1080, "1080p"),
    P1440(2560, 1440, "1440p"),
}

enum class FrameRate(val fps: Int, val label: String) {
    FPS15(15, "15 fps"),
    FPS24(24, "24 fps"),
    FPS30(30, "30 fps"),
    FPS60(60, "60 fps"),
}

enum class DisplaySurface {
    MONITOR,
    WINDOW,
    BROWSER_TAB,
}

data class ScreenSource(
    val id: String,
    val title: String,
    val isMonitor: Boolean,
)

data class ScreenShareConfig(
    val displaySurface: DisplaySurface? = null,
    val resolution: Resolution = Resolution.P1080,
    val frameRate: FrameRate = FrameRate.FPS30,
    val captureAudio: Boolean = false,
    val sourceId: String? = null,
)
