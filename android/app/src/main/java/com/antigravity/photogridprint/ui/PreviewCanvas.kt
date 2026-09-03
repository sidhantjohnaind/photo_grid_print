package com.antigravity.photogridprint.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import coil.request.ImageRequest
import com.antigravity.photogridprint.models.FitMode
import com.antigravity.photogridprint.models.GridConfig
import com.antigravity.photogridprint.models.PhotoItem
import com.antigravity.photogridprint.models.UiTheme
import kotlin.math.roundToInt

@Composable
fun LiveSheetPreview(
    items: List<PhotoItem>,
    config: GridConfig,
    theme: UiTheme,
    currentPage: Int = 0,
    onReorder: (sourceSlotIdx: Int, targetSlotIdx: Int) -> Unit = { _, _ -> },
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

    val pageStartIdx = currentPage * perPage
    val pageEndIdx = minOf(pageStartIdx + perPage, expandedItems.size)
    val pageItems = if (pageStartIdx < expandedItems.size) {
        expandedItems.subList(pageStartIdx, pageEndIdx)
    } else {
        emptyList()
    }

    // Touch Drag-and-Drop state
    var draggingSlotIdx by remember { mutableStateOf<Int?>(null) }
    var currentTouchPos by remember { mutableStateOf<Offset?>(null) }
    var targetSlotIdx by remember { mutableStateOf<Int?>(null) }

    BoxWithConstraints(
        modifier = modifier
            .fillMaxWidth()
            .padding(8.dp),
        contentAlignment = Alignment.Center
    ) {
        val maxAvailableW = maxWidth
        val maxAvailableH = maxHeight

        val displayW = if (maxAvailableW / maxAvailableH > sheetAspect) {
            maxAvailableH * sheetAspect
        } else {
            maxAvailableW
        }
        val displayH = displayW / sheetAspect

        Box(
            modifier = Modifier
                .size(width = displayW, height = displayH)
                .shadow(10.dp, shape = RoundedCornerShape(4.dp))
                .background(Color.White, shape = RoundedCornerShape(4.dp))
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

                fun findSlotAt(px: Float, py: Float): Int? {
                    if (px < marginX || px > sheetWidthPx - marginX || py < marginY || py > sheetHeightPx - marginY) {
                        return null
                    }
                    val col = ((px - marginX) / (cellW + gap)).toInt().coerceIn(0, cols - 1)
                    val row = ((py - marginY) / (cellH + gap)).toInt().coerceIn(0, rows - 1)
                    val slot = row * cols + col
                    return if (slot in 0 until pageItems.size) slot else null
                }

                // Interactive touch gesture container for drag and drop
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .pointerInput(pageItems, cols, rows, marginX, marginY, cellW, cellH, gap) {
                            detectDragGesturesAfterLongPress(
                                onDragStart = { offset ->
                                    val slot = findSlotAt(offset.x, offset.y)
                                    if (slot != null && slot < pageItems.size) {
                                        draggingSlotIdx = slot
                                        currentTouchPos = offset
                                        targetSlotIdx = slot
                                    }
                                },
                                onDrag = { change, dragAmount ->
                                    change.consume()
                                    val newPos = (currentTouchPos ?: Offset.Zero) + dragAmount
                                    currentTouchPos = newPos
                                    val target = findSlotAt(newPos.x, newPos.y)
                                    if (target != null && target < pageItems.size) {
                                        targetSlotIdx = target
                                    }
                                },
                                onDragEnd = {
                                    val src = draggingSlotIdx
                                    val dst = targetSlotIdx
                                    if (src != null && dst != null && src != dst) {
                                        onReorder(src, dst)
                                    }
                                    draggingSlotIdx = null
                                    targetSlotIdx = null
                                    currentTouchPos = null
                                },
                                onDragCancel = {
                                    draggingSlotIdx = null
                                    targetSlotIdx = null
                                    currentTouchPos = null
                                }
                            )
                        }
                ) {
                    for (i in pageItems.indices) {
                        val col = i % cols
                        val row = i / cols

                        val x = marginX + col * (cellW + gap)
                        val y = marginY + row * (cellH + gap)

                        val isDraggingThis = draggingSlotIdx == i
                        val isTarget = draggingSlotIdx != null && targetSlotIdx == i

                        val item = pageItems[i]

                        val cellModifier = Modifier
                            .offset(
                                x = (x / density).dp,
                                y = (y / density).dp
                            )
                            .size(
                                width = (cellW / density).dp,
                                height = (cellH / density).dp
                            )
                            .clip(RoundedCornerShape(2.dp))
                            .alpha(if (isDraggingThis) 0.35f else 1.0f)
                            .then(
                                if (isTarget) {
                                    Modifier.border(2.dp, Color(theme.accentHex), RoundedCornerShape(2.dp))
                                } else {
                                    Modifier // NO black borders! Clean, seamless image placement
                                }
                            )

                        Box(modifier = cellModifier) {
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

                    // Floating ghost preview following finger when dragging
                    if (draggingSlotIdx != null && currentTouchPos != null) {
                        val dragItem = pageItems.getOrNull(draggingSlotIdx!!)
                        if (dragItem != null) {
                            val ghostW = (cellW / density).dp * 0.9f
                            val ghostH = (cellH / density).dp * 0.9f
                            val touch = currentTouchPos!!

                            Box(
                                modifier = Modifier
                                    .offset {
                                        IntOffset(
                                            (touch.x - (cellW * 0.45f)).roundToInt(),
                                            (touch.y - (cellH * 0.45f)).roundToInt()
                                        )
                                    }
                                    .size(ghostW, ghostH)
                                    .shadow(16.dp, RoundedCornerShape(6.dp))
                                    .border(2.dp, Color(theme.accentHex), RoundedCornerShape(6.dp))
                                    .background(Color.Black, RoundedCornerShape(6.dp))
                                    .clip(RoundedCornerShape(6.dp))
                            ) {
                                AsyncImage(
                                    model = ImageRequest.Builder(LocalContext.current)
                                        .data(dragItem.uri)
                                        .crossfade(true)
                                        .build(),
                                    contentDescription = null,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize()
                                )

                                Box(
                                    modifier = Modifier
                                        .align(Alignment.BottomCenter)
                                        .background(Color.Black.copy(alpha = 0.7f))
                                        .padding(horizontal = 6.dp, vertical = 2.dp)
                                ) {
                                    Text(
                                        text = "Moving Photo",
                                        fontSize = 10.sp,
                                        fontWeight = FontWeight.Bold,
                                        color = Color(theme.accentHex)
                                    )
                                }
                            }
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
}
