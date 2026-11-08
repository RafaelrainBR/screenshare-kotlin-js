package com.rafaelrain.screenshare

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// The web's ui.css tokens ("Sala noturna v4"), the same as the desktop's ui/theme.rs.
val Bg = Color(0xFF101318)
val Surface = Color(0xFF171D26)
val Surface2 = Color(0xFF222B38)
val Hover = Color(0xFF2B3546)
val VideoBg = Color(0xFF13171E)
val Line = Color(0xFF354151)
val Line2 = Color(0xFF4A586C)
val TextColor = Color(0xFFEEF2FA)
val Dim = Color(0xFFAAB6C8)
val Faint = Color(0xFF8592A6)
val Accent = Color(0xFF8EB4FA)
val OnAccent = Color(0xFF0C1022)
val Green = Color(0xFF60C99B)
val Red = Color(0xFFEF8791)
val RedLine = Color(0xFFA8525E)
val Amber = Color(0xFFF2C46D)
val AmberLine = Color(0xFF8A6D33)
val Glass = Color(0xEB101318)

val Barlow = FontFamily(Font(R.font.barlow_regular, FontWeight.Normal), Font(R.font.barlow_bold, FontWeight.Bold))
val Body = TextStyle(fontFamily = Barlow, fontSize = 15.sp, lineHeight = 22.sp, color = TextColor)
val Mono = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp, color = TextColor)
val Small = RoundedCornerShape(8.dp)
val Regular = RoundedCornerShape(10.dp)

@Composable
fun ScreenshareTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Accent, onPrimary = OnAccent, background = Bg, surface = Surface, onSurface = TextColor,
            surfaceContainerLow = Surface, outline = Line, secondaryContainer = Surface2, onSecondaryContainer = TextColor,
        ),
        typography = MaterialTheme.typography.run {
            copy(
                bodyLarge = bodyLarge.copy(fontFamily = Barlow), bodyMedium = bodyMedium.copy(fontFamily = Barlow),
                labelLarge = labelLarge.copy(fontFamily = Barlow, fontWeight = FontWeight.Bold), titleLarge = titleLarge.copy(fontFamily = Barlow),
            )
        },
        content = content,
    )
}

/** The web's line icons (24×24, stroke 1.8, round caps), drawn from their SVG paths. */
private fun lineIcon(name: String, vararg paths: String): ImageVector =
    ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f).apply {
        paths.forEach {
            addPath(PathParser().parsePathString(it).toNodes(), stroke = SolidColor(Color.White), strokeLineWidth = 1.8f, strokeLineCap = StrokeCap.Round, strokeLineJoin = StrokeJoin.Round)
        }
    }.build()

private const val SCREEN = "M5 4h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"
private const val CAMERA = "M5 6h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2z"
private const val MIC = "M9 5a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0V5ZM5 11a7 7 0 0 0 14 0M12 18v3M8 21h8"
private const val SPEAKER = "M11 5 6 9H3v6h3l5 4V5Z"
private const val SLASH = "M3 3l18 18"

object Glyphs {
    val Screen = lineIcon("screen", SCREEN, "M8 21h8M12 18v3")
    val StopScreen = lineIcon("stopScreen", SCREEN, "M9 8h6v6H9z")
    val Camera = lineIcon("camera", CAMERA, "m15 10 6-3v10l-6-3")
    val CameraOff = lineIcon("cameraOff", CAMERA, "m15 10 6-3v10l-6-3", SLASH)
    val Mic = lineIcon("mic", MIC)
    val MicOff = lineIcon("micOff", MIC, SLASH)
    val Speaker = lineIcon("speaker", SPEAKER, "M15 9a4 4 0 0 1 0 6M18 6a8 8 0 0 1 0 12")
    val Earpiece = lineIcon("earpiece", SPEAKER, "M15 9a4 4 0 0 1 0 6")
    val Leave = lineIcon("leave", "M14 4h4a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-4M10 16l-4-4 4-4M6 12h10")
    val People = lineIcon("people", "M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM2 21a7 7 0 0 1 14 0M16 3.5a4 4 0 0 1 0 7.5M18 14a6 6 0 0 1 4 7")
    val Person = lineIcon("person", "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0")
    val Hash = lineIcon("hash", "M5 9h15M4 15h15M10 3 8 21M16 3l-2 18")
    val Arrow = lineIcon("arrow", "M5 12h14M13 6l6 6-6 6")
    val Copy = lineIcon("copy", "M9 9h10a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H11a2 2 0 0 1-2-2V9zM5 15H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1")
    val Chat = lineIcon("chat", "M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z")
    val Alert = lineIcon("alert", "M12 3 2 20h20L12 3zM12 10v4M12 17h.01")
}

@Composable
fun Label(text: String, modifier: Modifier = Modifier) = Text(text, style = Body.copy(fontSize = 14.sp), modifier = modifier)

@Composable
fun Hint(text: String, color: Color = Faint, modifier: Modifier = Modifier) = Text(text, style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp, color = color), modifier = modifier)

/** `.input-icon .input`: 48 dp, bg fill, line border that turns accent on focus, icon at the left. */
@Composable
fun IconField(
    value: String, onValueChange: (String) -> Unit, icon: ImageVector, placeholder: String, mono: Boolean = false, modifier: Modifier = Modifier,
    imeAction: ImeAction = ImeAction.Default, onIme: (() -> Unit)? = null, capitalization: KeyboardCapitalization = KeyboardCapitalization.Words,
) {
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val style = if (mono) Mono.copy(fontSize = 15.sp) else Body
    BasicTextField(
        value, onValueChange, singleLine = true, textStyle = style, cursorBrush = SolidColor(Accent), interactionSource = interaction,
        keyboardOptions = KeyboardOptions(capitalization = if (mono) KeyboardCapitalization.None else capitalization, autoCorrectEnabled = !mono, imeAction = imeAction),
        keyboardActions = if (onIme == null) KeyboardActions.Default else KeyboardActions(onAny = { onIme() }),
        modifier = modifier.fillMaxWidth().height(48.dp).background(Bg, Small).border(1.dp, if (focused) Accent else Line, Small),
        decorationBox = { field ->
            Row(Modifier.padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(icon, null, tint = Dim, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(10.dp))
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) Text(placeholder, style = style.copy(color = Faint))
                    field()
                }
            }
        },
    )
}

enum class Kind { Primary, Secondary, Ghost, Danger }

/** `.btn`: radius 8, bold 14 sp; primary is accent, danger is red text on a red line. */
@Composable
fun ActionButton(text: String, kind: Kind, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, trailing: ImageVector? = null, height: Dp = 48.dp, leading: ImageVector? = null) {
    val (bg, fg, line) = when (kind) {
        Kind.Primary -> Triple(Accent, OnAccent, Accent)
        Kind.Secondary -> Triple(Surface2, TextColor, Line)
        Kind.Ghost -> Triple(Color.Transparent, Dim, Line)
        Kind.Danger -> Triple(Color.Transparent, Red, RedLine)
    }
    Button(
        onClick, enabled = enabled, shape = Small, border = BorderStroke(1.dp, if (enabled) line else Line), contentPadding = PaddingValues(horizontal = 14.dp),
        colors = ButtonDefaults.buttonColors(containerColor = bg, contentColor = fg, disabledContainerColor = if (kind == Kind.Primary) Accent.copy(alpha = .45f) else bg, disabledContentColor = fg.copy(alpha = .6f)),
        modifier = modifier.height(height),
    ) {
        if (leading != null) {
            Icon(leading, null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
        }
        Text(text, style = Body.copy(fontWeight = FontWeight.Bold, color = fg))
        if (trailing != null) {
            Spacer(Modifier.width(8.dp))
            Icon(trailing, null, modifier = Modifier.size(18.dp))
        }
    }
}

/**
 * A dock control (`.toolgroup .icon-button`): radius 10, a short caption under it so the phone's dock reads
 * without guessing; active is accent, muted draws the glyph red. [badge]: "" is a dot, text is a count.
 */
@Composable
fun DockButton(icon: ImageVector, label: String, caption: String?, modifier: Modifier = Modifier, active: Boolean = false, muted: Boolean = false, danger: Boolean = false, badge: String? = null, height: Dp = 46.dp, onClick: () -> Unit) {
    val fg = when {
        active -> OnAccent
        muted || danger -> Red
        else -> TextColor
    }
    Column(
        modifier.clickable(onClickLabel = label, onClick = onClick).semantics(mergeDescendants = true) { contentDescription = label },
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            Modifier.fillMaxWidth().height(height).background(if (active) Accent else if (danger) Color.Transparent else Surface2, Regular)
                .border(1.dp, if (active) Accent else if (danger) RedLine else Line, Regular),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, null, tint = fg, modifier = Modifier.size(22.dp))
            when {
                badge == "" -> Box(Modifier.align(Alignment.TopEnd).padding(7.dp).size(8.dp).background(Accent, RoundedCornerShape(50)))
                badge != null -> Text(badge, style = Body.copy(fontSize = 11.sp, lineHeight = 12.sp, fontWeight = FontWeight.Bold, color = Dim), modifier = Modifier.align(Alignment.TopEnd).padding(top = 4.dp, end = 6.dp))
            }
        }
        if (caption != null) Text(caption, style = Body.copy(fontSize = 11.sp, lineHeight = 14.sp, color = Dim), maxLines = 1, modifier = Modifier.padding(top = 4.dp))
    }
}

/** `.glass-chip` / `.tile-name`: dark glass with the line-2 border. [wrap]: a notice is read whole, a tile's name stays on one line. */
@Composable
fun GlassChip(text: String, color: Color = TextColor, border: Color = Line2, icon: ImageVector? = null, modifier: Modifier = Modifier, wrap: Boolean = false) {
    Row(
        modifier.background(Glass, Small).border(1.dp, border, Small).padding(horizontal = 9.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (icon != null) Icon(icon, null, tint = color, modifier = Modifier.size(15.dp))
        Text(text, style = Body.copy(fontSize = 13.sp, lineHeight = 16.sp, color = color, fontWeight = if (color == TextColor) FontWeight.Normal else FontWeight.Bold), maxLines = if (wrap) 4 else 1)
    }
}

/** The web's 720p | 1080p segmented control: the chosen one filled with accent. */
@Composable
fun <T> Segmented(options: List<Pair<T, String>>, selected: T, onSelect: (T) -> Unit) {
    Row(Modifier.fillMaxWidth().background(Bg, Small).border(1.dp, Line, Small).padding(3.dp), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
        options.forEach { (value, label) ->
            val on = value == selected
            Box(
                Modifier.weight(1f).height(40.dp).background(if (on) Accent else Color.Transparent, RoundedCornerShape(6.dp)).clickable { onSelect(value) },
                contentAlignment = Alignment.Center,
            ) {
                Text(label, style = Body.copy(fontSize = 14.sp, fontWeight = FontWeight.Bold, color = if (on) OnAccent else TextColor))
            }
        }
    }
}

/** `.avatar-circle`: initials on line, a green ring while speaking. */
@Composable
fun Avatar(name: String, speaking: Boolean, size: Dp = 32.dp) {
    val initials = name.trim().split(Regex("\\s+")).filter { it.isNotEmpty() }.take(2).joinToString("") { it.take(1).uppercase() }.ifEmpty { "?" }
    Box(
        Modifier.size(size).background(Surface2, RoundedCornerShape(50)).border(2.dp, if (speaking) Green else Color.Transparent, RoundedCornerShape(50)),
        contentAlignment = Alignment.Center,
    ) {
        Text(initials, style = Body.copy(fontSize = (size.value * .36f).sp, fontWeight = FontWeight.Bold))
    }
}
