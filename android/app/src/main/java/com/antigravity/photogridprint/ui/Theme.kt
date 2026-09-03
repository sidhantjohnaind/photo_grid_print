package com.antigravity.photogridprint.ui

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import com.antigravity.photogridprint.models.UiTheme

@Composable
fun PhotoGridTheme(
    theme: UiTheme,
    content: @Composable () -> Unit
) {
    val colorScheme: ColorScheme = if (theme.isDark) {
        darkColorScheme(
            primary = Color(theme.accentHex),
            secondary = Color(theme.secondaryHex),
            background = Color(theme.bgHex),
            surface = Color(theme.cardHex),
            onPrimary = Color.White,
            onSecondary = Color.White,
            onBackground = Color(0xFFF3F4F6),
            onSurface = Color(0xFFF3F4F6)
        )
    } else {
        lightColorScheme(
            primary = Color(theme.accentHex),
            secondary = Color(theme.secondaryHex),
            background = Color(theme.bgHex),
            surface = Color(theme.cardHex),
            onPrimary = Color.White,
            onSecondary = Color.White,
            onBackground = Color(0xFF0F172A),
            onSurface = Color(0xFF0F172A)
        )
    }

    MaterialTheme(
        colorScheme = colorScheme,
        content = content
    )
}
