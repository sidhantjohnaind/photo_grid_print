package com.antigravity.photogridprint.models

import android.net.Uri

enum class PaperSize(val displayName: String, val widthMm: Float, val heightMm: Float) {
    A4("A4 (210 x 297 mm)", 210f, 297f),
    Letter("US Letter (8.5 x 11 in)", 215.9f, 279.4f),
    Legal("US Legal (8.5 x 14 in)", 215.9f, 355.6f),
    Photo4x6("4 x 6\" Photo (10x15cm)", 101.6f, 152.4f),
    Photo5x7("5 x 7\" Photo (13x18cm)", 127f, 177.8f),
    A3("A3 (297 x 420 mm)", 297f, 420f),
    A5("A5 (148 x 210 mm)", 148f, 210f);

    fun dimensions(isPortrait: Boolean): Pair<Float, Float> {
        return if (isPortrait) {
            minOf(widthMm, heightMm) to maxOf(widthMm, heightMm)
        } else {
            maxOf(widthMm, heightMm) to minOf(widthMm, heightMm)
        }
    }
}

enum class FitMode {
    Fill, Contain
}

enum class ColorTone {
    Original, Grayscale, HighContrast
}

enum class UiTheme(
    val title: String,
    val emoji: String,
    val bgHex: Long,
    val cardHex: Long,
    val borderHex: Long,
    val accentHex: Long,
    val secondaryHex: Long,
    val isDark: Boolean
) {
    CyberNeon("Cyber Neon", "⚡", 0xFF0B0F19, 0xFF111827, 0xFF1E3A56, 0xFF06B6D4, 0xFF3B82F6, true),
    TokyoPurple("Tokyo Purple", "🔮", 0xFF130F1E, 0xFF1D172E, 0xFF3C285F, 0xFFA855F7, 0xFFEC4899, true),
    ForestEmerald("Forest Emerald", "🌲", 0xFF0C1815, 0xFF132420, 0xFF1E4438, 0xFF10B981, 0xFF34D399, true),
    SunsetAmber("Sunset Amber", "🌅", 0xFF1A1210, 0xFF261A18, 0xFF482A24, 0xFFF97316, 0xFFF43F5E, true),
    DarkSlate("Dark Slate", "🖤", 0xFF101218, 0xFF161921, 0xFF262B38, 0xFF3B82F6, 0xFF60A5FA, true),
    StudioLight("Studio Light", "☀️", 0xFFF8FAFC, 0xFFFFFFFF, 0xFFE2E8F0, 0xFF2563EB, 0xFF0EA5E9, false);
}

data class PhotoItem(
    val id: String = java.util.UUID.randomUUID().toString(),
    val uri: Uri,
    var copies: Int = 1
)

data class GridConfig(
    val paperSize: PaperSize = PaperSize.A4,
    val cols: Int = 4,
    val rows: Int = 4,
    val gapPx: Int = 24,
    val marginPx: Int = 50,
    val isBorderless: Boolean = false,
    val isPortrait: Boolean = false,
    val fitMode: FitMode = FitMode.Fill,
    val colorTone: ColorTone = ColorTone.Original,
    val showCutMarks: Boolean = false
) {
    fun calculateCellDimensionsMm(): Pair<Float, Float> {
        val (paperW, paperH) = paperSize.dimensions(isPortrait)
        val dpi = 300f
        val marginMm = if (isBorderless) 0f else (marginPx / dpi * 25.4f)
        val gapMm = if (isBorderless) 0f else (gapPx / dpi * 25.4f)

        val availW = maxOf(1f, paperW - 2f * marginMm)
        val availH = maxOf(1f, paperH - 2f * (marginMm * 0.85f))

        val cellW = maxOf(1f, (availW - (cols - 1) * gapMm) / cols)
        val cellH = maxOf(1f, (availH - (rows - 1) * gapMm) / rows)
        return cellW to cellH
    }
}
