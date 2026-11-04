package app.pulpit.mobile.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import app.pulpit.mobile.R

/** Board-side colors. Tile faces keep the values they always had, so a
 *  board looks the same on the tablet as in the desktop editor. */
object DeckColors {
    val background = Color(0xFF12171B)
    val boardBg = Color(0xFF171C21)
    val tileFallback = Color(0xFF34495E)
    val accent = Color(0xFF1ABC9C)
}

/** Chrome palette shared with the desktop (DESIGN.md): the connect
 *  screen, the board chip and the link overlay. */
object PulpitColors {
    val bg = Color(0xFF12171B)
    val ground = Color(0xFF171C21)
    val tile = Color(0xFF222A31)
    val tile2 = Color(0xFF2C353E)
    val lift = Color(0xFF3A4753)
    val line = Color(0xFF252F38)
    val well = Color(0xFF13181C)
    val ink = Color(0xFFECF0F1)
    val ink2 = Color(0xFFA3B0B8)
    val ink3 = Color(0xFF7D8A94)
    val accent = Color(0xFF1ABC9C)
    val accentInk = Color(0xFF0B2A23)
    val accentSoft = Color(0x241ABC9C)
    val danger = Color(0xFFF19488)
}

/** Inter for chrome text, like the desktop. Tile labels do not use it:
 *  they keep the platform Roboto the desktop also pins for tile faces. */
val Inter = FontFamily(
    Font(R.font.inter_regular, FontWeight.Normal),
    Font(R.font.inter_medium, FontWeight.Medium),
    Font(R.font.inter_semibold, FontWeight.SemiBold),
)

private val DarkScheme = darkColorScheme(
    primary = PulpitColors.accent,
    onPrimary = PulpitColors.accentInk,
    secondary = PulpitColors.accent,
    onSecondary = PulpitColors.accentInk,
    background = PulpitColors.bg,
    onBackground = PulpitColors.ink,
    surface = PulpitColors.tile,
    onSurface = PulpitColors.ink,
    surfaceVariant = PulpitColors.tile2,
    onSurfaceVariant = PulpitColors.ink2,
    surfaceContainer = PulpitColors.tile,
    surfaceContainerHigh = PulpitColors.tile2,
    outline = PulpitColors.lift,
    outlineVariant = PulpitColors.line,
    error = PulpitColors.danger,
)

private fun Typography.withFamily(family: FontFamily): Typography {
    fun TextStyle.f() = copy(fontFamily = family)
    return copy(
        displayLarge = displayLarge.f(),
        displayMedium = displayMedium.f(),
        displaySmall = displaySmall.f(),
        headlineLarge = headlineLarge.f(),
        headlineMedium = headlineMedium.f(),
        headlineSmall = headlineSmall.f(),
        titleLarge = titleLarge.f(),
        titleMedium = titleMedium.f(),
        titleSmall = titleSmall.f(),
        bodyLarge = bodyLarge.f(),
        bodyMedium = bodyMedium.f(),
        bodySmall = bodySmall.f(),
        labelLarge = labelLarge.f(),
        labelMedium = labelMedium.f(),
        labelSmall = labelSmall.f(),
    )
}

private val PulpitTypography = Typography().withFamily(Inter).let {
    it.copy(
        headlineMedium = it.headlineMedium.copy(fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp),
        titleMedium = it.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        titleSmall = it.titleSmall.copy(fontWeight = FontWeight.SemiBold),
        labelLarge = it.labelLarge.copy(fontWeight = FontWeight.SemiBold),
    )
}

@Composable
fun DeckboardTheme(content: @Composable () -> Unit) {
    // dark-only, like the desktop
    MaterialTheme(colorScheme = DarkScheme, typography = PulpitTypography, content = content)
}
