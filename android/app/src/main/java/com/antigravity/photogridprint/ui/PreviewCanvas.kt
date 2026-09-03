package com.antigravity.photogridprint.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import coil.request.ImageRequest
import com.antigravity.photogridprint.models.FitMode
import com.antigravity.photogridprint.models.GridConfig
import com.antigravity.photogridprint.models.PhotoItem
import com.antigravity.photogridprint.models.UiTheme

@Composable
fun LiveSheetPreview(
    items: List<PhotoItem>,
    config: GridConfig,
    theme: UiTheme,
    modifier: Modifier = Modifier
) {
    val (paperW, paperH) = config.paperSize.dimensions(config.isPortrait)
    val sheetAspect = paperW / paperH

    val expandedItems = mutableListOf<PhotoItem>()
    for (item in items) {
        repeat(item.copies) {
            expandedItems.add(item)
        }
    }

    val cols = maxOf(1, config.cols)
    val rows = maxOf(1, config.rows)
    val perPage = cols * rows

    BoxWithConstraints(
        modifier = modifier
            .fillMaxWidth()
            .padding(12.dp),
        contentAlignment = Alignment.Center
    ) {
        val maxAvailableW = maxWidth
        val maxAvailableH = maxHeight

        val displayW = if (maxAvailableW / maxAvailableH > sheetAspect) {
            maxAvailableH * sheetAspect
        } else {
            maxAvailableW
        }

        Box(
            modifier = Modifier
                .size(width = displayW, height = displayW / sheetAspect)
                .shadow(12.dp, shape = RoundedCornerShape(4.dp))
                .background(Color.White, shape = RoundedCornerShape(4.dp))
                .border(1.dp, Color(theme.borderHex), shape = RoundedCornerShape(4.dp))
        ) {
            BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
                val sheetWidthPx = constraints.maxWidth.toFloat()
                val sheetHeightPx = constraints.maxHeight.toFloat()

                val scale = sheetWidthPx / paperW

                val marginX = if (config.isBorderless) 0f else (config.marginPx / 300f * 25.4f * scale)
                val marginY = if (config.isBorderless) 0f else (config.marginPx * 0.85f / 300f * 25.4f * scale)
                val gap = if (config.isBorderless) 0f else (config.gapPx / 300f * 25.4f * scale)

                val availW = maxOf(1f, sheetWidthPx - 2f * marginX)
                val availH = maxOf(1f, sheetHeightPx - 2f * marginY)

                val cellW = (availW - (cols - 1) * gap) / cols
                val cellH = (availH - (rows - 1) * gap) / rows

                val density = LocalContext.current.resources.displayMetrics.density

                val displayedSlots = minOf(perPage, expandedItems.size)
                for (i in 0 until displayedSlots) {
                    val col = i % cols
                    val row = i / cols

                    val x = marginX + col * (cellW + gap)
                    val y = marginY + row * (cellH + gap)

                    val item = expandedItems[i]

                    Box(
                        modifier = Modifier
                            .offset(
                                x = (x / density).dp,
                                y = (y / density).dp
                            )
                            .size(
                                width = (cellW / density).dp,
                                height = (cellH / density).dp
                            )
                            .border(0.5.dp, Color(0xFFD1D5DB))
                            .clip(RoundedCornerShape(1.dp))
                    ) {
                        AsyncImage(
                            model = ImageRequest.Builder(LocalContext.current)
                                .data(item.uri)
                                .crossfade(true)
                                .build(),
                            contentDescription = null,
                            contentScale = if (config.fitMode == FitMode.Fill) ContentScale.Crop else ContentScale.Fit,
                            modifier = Modifier.fillMaxSize()
                        )
                    }
                }

                if (expandedItems.isEmpty()) {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Text(
                            text = "Tap '+ Add Photos' to preview sheet",
                            color = Color.Gray,
                            fontSize = 13.sp
                        )
                    }
                }
            }
        }
    }
}
