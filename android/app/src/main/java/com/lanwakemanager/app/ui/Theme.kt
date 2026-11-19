package com.lanwakemanager.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

val Graphite = Color(0xFF121417)
val GraphiteSurface = Color(0xFF1B1F24)
val GraphiteRaised = Color(0xFF242A31)
val Green = Color(0xFF2EE59D)
val Cyan = Color(0xFF22D3EE)

private val DarkScheme = darkColorScheme(
    primary = Green, onPrimary = Color(0xFF00281A), primaryContainer = Color(0xFF0F3D2D), onPrimaryContainer = Color(0xFFB5F5DB),
    secondary = Cyan, onSecondary = Color(0xFF00313A), secondaryContainer = Color(0xFF0B3A44), onSecondaryContainer = Color(0xFFB8EEF8),
    tertiary = Color(0xFFF5B942), onTertiary = Color(0xFF3A2800),
    background = Graphite, onBackground = Color(0xFFE6EAEE),
    surface = GraphiteSurface, onSurface = Color(0xFFE6EAEE),
    surfaceVariant = GraphiteRaised, onSurfaceVariant = Color(0xFFA7B0BA),
    surfaceContainer = GraphiteSurface, surfaceContainerHigh = GraphiteRaised, surfaceContainerHighest = Color(0xFF2D343C),
    surfaceContainerLow = Color(0xFF171B1F), surfaceContainerLowest = Graphite,
    outline = Color(0xFF3A434D), outlineVariant = Color(0xFF2B323A),
    error = Color(0xFFFF6B6B), onError = Color(0xFF3A0000), errorContainer = Color(0xFF4A1B1B), onErrorContainer = Color(0xFFFFD9D9),
)

private val LightScheme = lightColorScheme(
    primary = Color(0xFF008F5D), onPrimary = Color.White, primaryContainer = Color(0xFFC8F7E2), onPrimaryContainer = Color(0xFF00382A),
    secondary = Color(0xFF0A7E96), onSecondary = Color.White, secondaryContainer = Color(0xFFCDF1F9), onSecondaryContainer = Color(0xFF00313A),
    tertiary = Color(0xFF8A5A00), onTertiary = Color.White,
    background = Color(0xFFF4F6F8), onBackground = Color(0xFF14181C),
    surface = Color.White, onSurface = Color(0xFF14181C),
    surfaceVariant = Color(0xFFE8ECF0), onSurfaceVariant = Color(0xFF4A545E),
    surfaceContainer = Color(0xFFEFF2F5), surfaceContainerHigh = Color(0xFFE8ECF0), surfaceContainerHighest = Color(0xFFE0E5EA),
    surfaceContainerLow = Color(0xFFF7F9FA), surfaceContainerLowest = Color.White,
    outline = Color(0xFF8A949E), outlineVariant = Color(0xFFD0D6DC),
    error = Color(0xFFBA1A1A), onError = Color.White, errorContainer = Color(0xFFFFDAD6), onErrorContainer = Color(0xFF410002),
)

/** Semantic status colours that adapt to the theme. */
@Immutable
data class StatusColors(val online: Color, val offline: Color, val warn: Color, val neutral: Color, val info: Color)

val LocalStatusColors = staticCompositionLocalOf {
    StatusColors(Green, Color(0xFFFF6B6B), Color(0xFFF5B942), Color(0xFF8C97A3), Cyan)
}

private val DarkStatus = StatusColors(Green, Color(0xFFFF6B6B), Color(0xFFF5B942), Color(0xFF8C97A3), Cyan)
private val LightStatus = StatusColors(Color(0xFF008F5D), Color(0xFFC62828), Color(0xFF9A6500), Color(0xFF66707A), Color(0xFF0A7E96))


/** [theme]: "system" | "dark" | "light". */
@Composable
fun LwmTheme(theme: String = "dark", content: @Composable () -> Unit) {
    val dark = when (theme) { "light" -> false; "dark" -> true; else -> isSystemInDarkTheme() }
    androidx.compose.runtime.CompositionLocalProvider(LocalStatusColors provides if (dark) DarkStatus else LightStatus) {
        MaterialTheme(
            colorScheme = if (dark) DarkScheme else LightScheme,
            shapes = Shapes(
                extraSmall = RoundedCornerShape(6.dp), small = RoundedCornerShape(10.dp),
                medium = RoundedCornerShape(14.dp), large = RoundedCornerShape(18.dp), extraLarge = RoundedCornerShape(24.dp),
            ),
            content = content,
        )
    }
}
