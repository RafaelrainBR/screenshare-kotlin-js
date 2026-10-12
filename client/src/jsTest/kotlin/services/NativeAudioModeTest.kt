package services

import platform.DesktopCaptureSourceKind
import kotlin.test.Test
import kotlin.test.assertEquals

class NativeAudioModeTest {
    @Test
    fun windowSendsItsAppAudioAndScreenTheWholeOutput() {
        assertEquals(ScreenSharing.DesktopAudioMode.PROCESS, nativeAudioMode(true, DesktopCaptureSourceKind.WINDOW))
        assertEquals(ScreenSharing.DesktopAudioMode.SYSTEM, nativeAudioMode(true, DesktopCaptureSourceKind.SCREEN))
        assertEquals(ScreenSharing.DesktopAudioMode.NONE, nativeAudioMode(false, DesktopCaptureSourceKind.WINDOW))
        assertEquals(ScreenSharing.DesktopAudioMode.NONE, nativeAudioMode(false, DesktopCaptureSourceKind.SCREEN))
    }
}
