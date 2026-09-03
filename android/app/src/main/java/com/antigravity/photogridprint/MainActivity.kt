package com.antigravity.photogridprint

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Print
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import coil.compose.AsyncImage
import com.antigravity.photogridprint.models.*
import com.antigravity.photogridprint.ui.LiveSheetPreview
import com.antigravity.photogridprint.ui.PhotoGridTheme
import com.antigravity.photogridprint.util.AndroidPrintHelper
import com.antigravity.photogridprint.util.PdfGenerator
import java.io.File

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            PhotoGridAppScreen()
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PhotoGridAppScreen() {
    val context = LocalContext.current

    var selectedTheme by remember { mutableStateOf(UiTheme.CyberNeon) }
    var selectedTab by remember { mutableStateOf(0) } // 0: Photos, 1: Layout, 2: Style
    var currentPage by remember { mutableStateOf(0) }

    var items by remember { mutableStateOf(listOf<PhotoItem>()) }
    var config by remember { mutableStateOf(GridConfig()) }

    // Native Android Photo Picker launcher
    val photoPickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.PickMultipleVisualMedia()
    ) { uris: List<Uri> ->
        if (uris.isNotEmpty()) {
            val newItems = uris.map { PhotoItem(uri = it, copies = 1) }
            items = items + newItems
            Toast.makeText(context, "Added ${uris.size} photo(s)", Toast.LENGTH_SHORT).show()
        }
    }

    PhotoGridTheme(theme = selectedTheme) {
        Surface(
            modifier = Modifier.fillMaxSize(),
            color = Color(selectedTheme.bgHex)
        ) {
            Column(modifier = Modifier.fillMaxSize()) {
                // Top App Bar
                TopAppBar(
                    title = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                text = "PHOTO GRID PRINT",
                                fontSize = 16.sp,
                                fontWeight = FontWeight.Bold,
                                color = Color(selectedTheme.accentHex)
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = "Mobile Studio",
                                fontSize = 11.sp,
                                color = Color.Gray
                            )
                        }
                    },
                    actions = {
                        // Quick theme cycle pill
                        Box(
                            modifier = Modifier
                                .padding(end = 8.dp)
                                .clip(RoundedCornerShape(6.dp))
                                .background(Color(selectedTheme.cardHex))
                                .border(1.dp, Color(selectedTheme.borderHex), RoundedCornerShape(6.dp))
                                .clickable {
                                    val themes = UiTheme.values()
                                    val nextIdx = (themes.indexOf(selectedTheme) + 1) % themes.size
                                    selectedTheme = themes[nextIdx]
                                }
                                .padding(horizontal = 8.dp, vertical = 4.dp)
                        ) {
                            Text(
                                text = "${selectedTheme.emoji} ${selectedTheme.title}",
                                fontSize = 11.sp,
                                color = Color(selectedTheme.accentHex),
                                fontWeight = FontWeight.SemiBold
                            )
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = Color(selectedTheme.bgHex)
                    )
                )

                // Multi-page Calculation
                val totalCopies = items.sumOf { it.copies }
                val perPage = (config.cols * config.rows).coerceAtLeast(1)
                val totalPages = maxOf(1, (totalCopies + perPage - 1) / perPage)
                if (currentPage >= totalPages) {
                    currentPage = totalPages - 1
                }

                // Upper Section: Live Sheet Preview Canvas with Page Selector & Dimensions
                val (cellW, cellH) = config.calculateCellDimensionsMm()
                val cellWIn = cellW / 25.4f
                val cellHIn = cellH / 25.4f

                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(Color(selectedTheme.cardHex))
                        .padding(bottom = 6.dp)
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 12.dp, vertical = 4.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "Cell: %.1f x %.1f mm (%.2f x %.2f in)".format(cellW, cellH, cellWIn, cellHIn),
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold,
                            color = Color(selectedTheme.accentHex)
                        )

                        // Multi-Page Navigation Controls
                        if (totalPages > 1) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                IconButton(
                                    onClick = { if (currentPage > 0) currentPage-- },
                                    enabled = currentPage > 0,
                                    modifier = Modifier.size(28.dp)
                                ) {
                                    Text(
                                        text = "< Prev",
                                        fontSize = 11.sp,
                                        fontWeight = FontWeight.Bold,
                                        color = if (currentPage > 0) Color(selectedTheme.accentHex) else Color.Gray
                                    )
                                }

                                Text(
                                    text = "${currentPage + 1} / $totalPages",
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold,
                                    color = Color.White,
                                    modifier = Modifier.padding(horizontal = 6.dp)
                                )

                                IconButton(
                                    onClick = { if (currentPage < totalPages - 1) currentPage++ },
                                    enabled = currentPage < totalPages - 1,
                                    modifier = Modifier.size(28.dp)
                                ) {
                                    Text(
                                        text = "Next >",
                                        fontSize = 11.sp,
                                        fontWeight = FontWeight.Bold,
                                        color = if (currentPage < totalPages - 1) Color(selectedTheme.accentHex) else Color.Gray
                                    )
                                }
                            }
                        } else {
                            Text(
                                text = "${items.size} photos ($totalCopies total)",
                                fontSize = 11.sp,
                                color = Color.Gray
                            )
                        }
                    }

                    // Drag & Drop Enabled Live Sheet Preview
                    LiveSheetPreview(
                        items = items,
                        config = config,
                        theme = selectedTheme,
                        currentPage = currentPage,
                        onReorder = { srcSlot, dstSlot ->
                            val expandedItems = mutableListOf<PhotoItem>()
                            for (item in items) {
                                repeat(item.copies) { expandedItems.add(item) }
                            }
                            val fromGlobal = currentPage * perPage + srcSlot
                            val toGlobal = currentPage * perPage + dstSlot

                            if (fromGlobal in expandedItems.indices && toGlobal in expandedItems.indices) {
                                val srcItem = expandedItems[fromGlobal]
                                val dstItem = expandedItems[toGlobal]

                                val srcIdx = items.indexOfFirst { it.id == srcItem.id }
                                val dstIdx = items.indexOfFirst { it.id == dstItem.id }

                                if (srcIdx != -1 && dstIdx != -1 && srcIdx != dstIdx) {
                                    val mutable = items.toMutableList()
                                    val moved = mutable.removeAt(srcIdx)
                                    mutable.add(dstIdx, moved)
                                    items = mutable
                                    Toast.makeText(context, "Reordered photo to position #${dstIdx + 1}", Toast.LENGTH_SHORT).show()
                                }
                            }
                        },
                        modifier = Modifier.height(210.dp)
                    )
                }

                // Middle Section: Tab Bar
                TabRow(
                    selectedTabIndex = selectedTab,
                    containerColor = Color(selectedTheme.cardHex),
                    contentColor = Color(selectedTheme.accentHex),
                    divider = {}
                ) {
                    Tab(
                        selected = selectedTab == 0,
                        onClick = { selectedTab = 0 },
                        text = { Text("Photos (${items.size})", fontWeight = FontWeight.SemiBold) }
                    )
                    Tab(
                        selected = selectedTab == 1,
                        onClick = { selectedTab = 1 },
                        text = { Text("Layout & Grid", fontWeight = FontWeight.SemiBold) }
                    )
                    Tab(
                        selected = selectedTab == 2,
                        onClick = { selectedTab = 2 },
                        text = { Text("Themes & Style", fontWeight = FontWeight.SemiBold) }
                    )
                }

                // Tab Content (Scrollable)
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 8.dp)
                ) {
                    when (selectedTab) {
                        0 -> PhotosTab(
                            items = items,
                            theme = selectedTheme,
                            onAddPhotos = {
                                photoPickerLauncher.launch(
                                    PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)
                                )
                            },
                            onClearAll = { items = emptyList() },
                            onItemCopiesChange = { idx, newCopies ->
                                items = items.toMutableList().also { it[idx] = it[idx].copy(copies = newCopies) }
                            },
                            onMoveItem = { fromIdx, toIdx ->
                                if (fromIdx in items.indices && toIdx in items.indices) {
                                    val mutable = items.toMutableList()
                                    val moved = mutable.removeAt(fromIdx)
                                    mutable.add(toIdx, moved)
                                    items = mutable
                                }
                            },
                            onRemoveItem = { idx ->
                                items = items.toMutableList().also { it.removeAt(idx) }
                            }
                        )
                        1 -> LayoutTab(
                            config = config,
                            theme = selectedTheme,
                            onConfigChange = { config = it }
                        )
                        2 -> StyleTab(
                            config = config,
                            theme = selectedTheme,
                            onConfigChange = { config = it },
                            onThemeChange = { selectedTheme = it }
                        )
                    }
                }

                // Pinned Bottom Action Bar
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    color = Color(selectedTheme.cardHex),
                    shadowElevation = 8.dp
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(12.dp),
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Button(
                            onClick = {
                                if (items.isEmpty()) {
                                    Toast.makeText(context, "Please select at least 1 photo", Toast.LENGTH_SHORT).show()
                                    return@Button
                                }
                                val timestamp = java.text.SimpleDateFormat("yyyyMMdd_HHmmss", java.util.Locale.getDefault()).format(java.util.Date())
                                val cachePdf = File(context.cacheDir, "Photo_Grid_Print_${timestamp}.pdf")
                                PdfGenerator.generatePdf(context, items, config, cachePdf)
                                AndroidPrintHelper.printPdf(context, cachePdf, "Photo Grid Print $timestamp")
                            },
                            modifier = Modifier
                                .weight(1f)
                                .height(48.dp),
                            shape = RoundedCornerShape(8.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = Color(selectedTheme.accentHex),
                                contentColor = Color.White
                            )
                        ) {
                            Icon(Icons.Default.Print, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(modifier = Modifier.width(6.dp))
                            Text("Print Now", fontWeight = FontWeight.Bold)
                        }

                        Button(
                            onClick = {
                                if (items.isEmpty()) {
                                    Toast.makeText(context, "Please select at least 1 photo", Toast.LENGTH_SHORT).show()
                                    return@Button
                                }
                                val timestamp = java.text.SimpleDateFormat("yyyyMMdd_HHmmss", java.util.Locale.getDefault()).format(java.util.Date())
                                val cachePdf = File(context.cacheDir, "Photo_Grid_Print_${timestamp}.pdf")
                                PdfGenerator.generatePdf(context, items, config, cachePdf)

                                val contentUri = FileProvider.getUriForFile(
                                    context,
                                    "${context.packageName}.fileprovider",
                                    cachePdf
                                )
                                val shareIntent = Intent(Intent.ACTION_SEND).apply {
                                    type = "application/pdf"
                                    putExtra(Intent.EXTRA_STREAM, contentUri)
                                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                }
                                context.startActivity(Intent.createChooser(shareIntent, "Share or Save PDF"))
                            },
                            modifier = Modifier
                                .weight(1f)
                                .height(48.dp),
                            shape = RoundedCornerShape(8.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = Color(selectedTheme.borderHex),
                                contentColor = Color.White
                            )
                        ) {
                            Icon(Icons.Default.Share, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(modifier = Modifier.width(6.dp))
                            Text("Share / Save PDF", fontWeight = FontWeight.Bold)
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun PhotosTab(
    items: List<PhotoItem>,
    theme: UiTheme,
    onAddPhotos: () -> Unit,
    onClearAll: () -> Unit,
    onItemCopiesChange: (Int, Int) -> Unit,
    onMoveItem: (fromIdx: Int, toIdx: Int) -> Unit,
    onRemoveItem: (Int) -> Unit
) {
    Column(modifier = Modifier.fillMaxSize()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Button(
                onClick = onAddPhotos,
                modifier = Modifier.weight(1f),
                shape = RoundedCornerShape(8.dp),
                colors = ButtonDefaults.buttonColors(containerColor = Color(theme.accentHex))
            ) {
                Icon(Icons.Default.Add, contentDescription = null)
                Spacer(modifier = Modifier.width(4.dp))
                Text("+ Add Photos", fontWeight = FontWeight.Bold)
            }

            if (items.isNotEmpty()) {
                OutlinedButton(
                    onClick = onClearAll,
                    shape = RoundedCornerShape(8.dp),
                    colors = ButtonDefaults.outlinedButtonColors(contentColor = Color(0xFFF87171))
                ) {
                    Text("Clear All")
                }
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        if (items.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("No photos selected yet. Tap '+ Add Photos' above.", color = Color.Gray)
            }
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                itemsIndexed(items) { index, item ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(8.dp))
                            .background(Color(theme.cardHex))
                            .border(1.dp, Color(theme.borderHex), RoundedCornerShape(8.dp))
                            .padding(8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        AsyncImage(
                            model = item.uri,
                            contentDescription = null,
                            modifier = Modifier
                                .size(40.dp)
                                .clip(RoundedCornerShape(4.dp)),
                            contentScale = ContentScale.Crop
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = "#${index + 1}",
                            fontWeight = FontWeight.Bold,
                            color = Color(theme.accentHex),
                            fontSize = 14.sp
                        )

                        Spacer(modifier = Modifier.weight(1f))

                        // Reorder buttons: Move Up and Move Down
                        IconButton(
                            onClick = { if (index > 0) onMoveItem(index, index - 1) },
                            enabled = index > 0,
                            modifier = Modifier.size(28.dp)
                        ) {
                            Icon(
                                Icons.Default.ArrowUpward,
                                contentDescription = "Move Up",
                                tint = if (index > 0) Color.White else Color.Gray,
                                modifier = Modifier.size(16.dp)
                            )
                        }

                        IconButton(
                            onClick = { if (index < items.size - 1) onMoveItem(index, index + 1) },
                            enabled = index < items.size - 1,
                            modifier = Modifier.size(28.dp)
                        ) {
                            Icon(
                                Icons.Default.ArrowDownward,
                                contentDescription = "Move Down",
                                tint = if (index < items.size - 1) Color.White else Color.Gray,
                                modifier = Modifier.size(16.dp)
                            )
                        }

                        Spacer(modifier = Modifier.width(4.dp))

                        // Copies stepper
                        IconButton(
                            onClick = { if (item.copies > 1) onItemCopiesChange(index, item.copies - 1) },
                            modifier = Modifier.size(28.dp)
                        ) {
                            Text("-", fontSize = 18.sp, fontWeight = FontWeight.Bold, color = Color.White)
                        }
                        Text(
                            text = "${item.copies}x",
                            modifier = Modifier.padding(horizontal = 4.dp),
                            fontWeight = FontWeight.Bold,
                            color = Color.White
                        )
                        IconButton(
                            onClick = { onItemCopiesChange(index, item.copies + 1) },
                            modifier = Modifier.size(28.dp)
                        ) {
                            Text("+", fontSize = 18.sp, fontWeight = FontWeight.Bold, color = Color.White)
                        }

                        IconButton(
                            onClick = { onRemoveItem(index) },
                            modifier = Modifier.size(28.dp)
                        ) {
                            Icon(Icons.Default.Delete, contentDescription = null, tint = Color(0xFFF87171), modifier = Modifier.size(18.dp))
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun LayoutTab(
    config: GridConfig,
    theme: UiTheme,
    onConfigChange: (GridConfig) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Paper Size Chips
        Text("Paper Size", fontWeight = FontWeight.Bold, color = Color.White)
        LazyRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            val sizes = listOf(PaperSize.A4, PaperSize.Letter, PaperSize.Photo4x6, PaperSize.Photo5x7, PaperSize.A3)
            items(sizes.size) { idx ->
                val size = sizes[idx]
                val selected = config.paperSize == size
                FilterChip(
                    selected = selected,
                    onClick = { onConfigChange(config.copy(paperSize = size)) },
                    label = { Text(size.name) },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = Color(theme.accentHex),
                        selectedLabelColor = Color.White
                    )
                )
            }
        }

        // Orientation
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = !config.isPortrait,
                onClick = { onConfigChange(config.copy(isPortrait = false)) },
                label = { Text("Landscape") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
            FilterChip(
                selected = config.isPortrait,
                onClick = { onConfigChange(config.copy(isPortrait = true)) },
                label = { Text("Portrait") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
        }

        // Presets
        Text("Grid Presets", fontWeight = FontWeight.Bold, color = Color.White)
        Row(modifier = Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            val presets = listOf(
                "16 (4x4)" to (4 to 4),
                "9 (3x3)" to (3 to 3),
                "8 (4x2)" to (4 to 2),
                "6 (3x2)" to (3 to 2),
                "4 (2x2)" to (2 to 2)
            )
            for ((label, pair) in presets) {
                val (c, r) = pair
                FilterChip(
                    selected = config.cols == c && config.rows == r,
                    onClick = { onConfigChange(config.copy(cols = c, rows = r)) },
                    label = { Text(label) },
                    colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
                )
            }
        }

        // Passport Presets
        Text("ID / Passport Presets", fontWeight = FontWeight.Bold, color = Color.Gray, fontSize = 12.sp)
        Row(modifier = Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            FilterChip(
                selected = config.cols == 3 && config.rows == 2,
                onClick = { onConfigChange(config.copy(cols = 3, rows = 2, fitMode = FitMode.Fill)) },
                label = { Text("US Passport 2x2\" (6)") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
            FilterChip(
                selected = config.cols == 4 && config.rows == 2,
                onClick = { onConfigChange(config.copy(cols = 4, rows = 2, fitMode = FitMode.Fill)) },
                label = { Text("Passport 35x45mm (8)") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
            FilterChip(
                selected = config.cols == 4 && config.rows == 3,
                onClick = { onConfigChange(config.copy(cols = 4, rows = 3, fitMode = FitMode.Fill)) },
                label = { Text("Stamp 30x40mm (12)") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
        }

        // Columns & Rows Sliders
        Text("Columns: ${config.cols}", color = Color.White)
        Slider(
            value = config.cols.toFloat(),
            onValueChange = { onConfigChange(config.copy(cols = it.toInt())) },
            valueRange = 1f..8f,
            steps = 6,
            colors = SliderDefaults.colors(thumbColor = Color(theme.accentHex), activeTrackColor = Color(theme.accentHex))
        )

        Text("Rows: ${config.rows}", color = Color.White)
        Slider(
            value = config.rows.toFloat(),
            onValueChange = { onConfigChange(config.copy(rows = it.toInt())) },
            valueRange = 1f..8f,
            steps = 6,
            colors = SliderDefaults.colors(thumbColor = Color(theme.accentHex), activeTrackColor = Color(theme.accentHex))
        )

        // Fit Mode
        Text("Image Fit", fontWeight = FontWeight.Bold, color = Color.White)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = config.fitMode == FitMode.Fill,
                onClick = { onConfigChange(config.copy(fitMode = FitMode.Fill)) },
                label = { Text("Fill (Crop to Cell)") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
            FilterChip(
                selected = config.fitMode == FitMode.Contain,
                onClick = { onConfigChange(config.copy(fitMode = FitMode.Contain)) },
                label = { Text("Fit (Preserve Full)") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
        }
    }
}

@Composable
fun StyleTab(
    config: GridConfig,
    theme: UiTheme,
    onConfigChange: (GridConfig) -> Unit,
    onThemeChange: (UiTheme) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Theme Selector Gallery
        Text("Visual Color Theme", fontWeight = FontWeight.Bold, color = Color.White)
        LazyRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            val themes = UiTheme.values()
            items(themes.size) { idx ->
                val t = themes[idx]
                FilterChip(
                    selected = theme == t,
                    onClick = { onThemeChange(t) },
                    label = { Text("${t.emoji} ${t.title}") },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = Color(t.accentHex),
                        selectedLabelColor = Color.White
                    )
                )
            }
        }

        // Bleed / Margins
        Text("Spacing & Margins", fontWeight = FontWeight.Bold, color = Color.White)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = !config.isBorderless,
                onClick = { onConfigChange(config.copy(isBorderless = false)) },
                label = { Text("Spaced Margins") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
            FilterChip(
                selected = config.isBorderless,
                onClick = { onConfigChange(config.copy(isBorderless = true)) },
                label = { Text("100% Full-Bleed") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
            )
        }

        if (!config.isBorderless) {
            Text("Page Margin: ${config.marginPx} px", color = Color.White)
            Slider(
                value = config.marginPx.toFloat(),
                onValueChange = { onConfigChange(config.copy(marginPx = it.toInt())) },
                valueRange = 0f..150f,
                colors = SliderDefaults.colors(thumbColor = Color(theme.accentHex), activeTrackColor = Color(theme.accentHex))
            )

            Text("Photo Gap: ${config.gapPx} px", color = Color.White)
            Slider(
                value = config.gapPx.toFloat(),
                onValueChange = { onConfigChange(config.copy(gapPx = it.toInt())) },
                valueRange = 0f..60f,
                colors = SliderDefaults.colors(thumbColor = Color(theme.accentHex), activeTrackColor = Color(theme.accentHex))
            )

            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(
                    checked = config.showCutMarks,
                    onCheckedChange = { onConfigChange(config.copy(showCutMarks = it)) },
                    colors = CheckboxDefaults.colors(checkedColor = Color(theme.accentHex))
                )
                Text("Trimmer / Cutting Corner Guides", color = Color.White)
            }
        }

        // Color Tone
        Text("Color Filter", fontWeight = FontWeight.Bold, color = Color.White)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (tone in ColorTone.values()) {
                FilterChip(
                    selected = config.colorTone == tone,
                    onClick = { onConfigChange(config.copy(colorTone = tone)) },
                    label = { Text(tone.name) },
                    colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color(theme.accentHex), selectedLabelColor = Color.White)
                )
            }
        }
    }
}
