package screenshare.clientkmp.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val ScreenShareColorScheme =
    darkColorScheme(
        primary = Color(0xFF38BDF8),
        onPrimary = Color(0xFF00344E),
        primaryContainer = Color(0xFF004B6F),
        onPrimaryContainer = Color(0xFFBFE9FF),
        secondary = Color(0xFF818CF8),
        onSecondary = Color(0xFF1A1A6E),
        secondaryContainer = Color(0xFF2E3057),
        onSecondaryContainer = Color(0xFFDDDFFF),
        tertiary = Color(0xFF2DD4BF),
        onTertiary = Color(0xFF003732),
        error = Color(0xFFFB7185),
        onError = Color(0xFF5C0020),
        background = Color(0xFF0F1729),
        onBackground = Color(0xFFE2E8F0),
        surface = Color(0xFF0F1729),
        onSurface = Color(0xFFE2E8F0),
        surfaceVariant = Color(0xFF1E293B),
        onSurfaceVariant = Color(0xFF94A3B8),
        outline = Color(0xFF334155),
    )

@Composable
fun ScreenShareTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = ScreenShareColorScheme,
        content = content,
    )
}
