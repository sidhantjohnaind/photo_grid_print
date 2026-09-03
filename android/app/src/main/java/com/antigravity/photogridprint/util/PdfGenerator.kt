package com.antigravity.photogridprint.util

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ColorMatrix
import android.graphics.ColorMatrixColorFilter
import android.graphics.Paint
import android.graphics.Rect
import android.graphics.RectF
import android.graphics.pdf.PdfDocument
import com.antigravity.photogridprint.models.ColorTone
import com.antigravity.photogridprint.models.FitMode
import com.antigravity.photogridprint.models.GridConfig
import com.antigravity.photogridprint.models.PhotoItem
import java.io.File
import java.io.FileOutputStream

object PdfGenerator {

    fun generatePdf(
        context: Context,
        items: List<PhotoItem>,
        config: GridConfig,
        outputFile: File
    ): Int {
        val expandedItems = mutableListOf<PhotoItem>()
        for (item in items) {
            repeat(item.copies) {
                expandedItems.add(item)
            }
        }
        if (expandedItems.isEmpty()) return 0

        val (paperWMm, paperHMm) = config.paperSize.dimensions(config.isPortrait)
        // PDF points (72 points per inch; 1 inch = 25.4 mm)
        val pageWidthPt = (paperWMm / 25.4f * 72f).toInt()
        val pageHeightPt = (paperHMm / 25.4f * 72f).toInt()

        val dpi = 300f
        val scalePtPerPx = 72f / dpi

        val marginXPt = if (config.isBorderless) 0f else (config.marginPx * scalePtPerPx)
        val marginYPt = if (config.isBorderless) 0f else (config.marginPx * 0.85f * scalePtPerPx)
        val gapPt = if (config.isBorderless) 0f else (config.gapPx * scalePtPerPx)

        val cols = maxOf(1, config.cols)
        val rows = maxOf(1, config.rows)
        val perPage = cols * rows

        val availWPt = pageWidthPt - 2f * marginXPt
        val availHPt = pageHeightPt - 2f * marginYPt

        val cellWPt = (availWPt - (cols - 1) * gapPt) / cols
        val cellHPt = (availHPt - (rows - 1) * gapPt) / rows

        val totalPages = (expandedItems.size + perPage - 1) / perPage
        val pdfDoc = PdfDocument()

        val paint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG)
        when (config.colorTone) {
            ColorTone.Grayscale -> {
                val matrix = ColorMatrix().apply { setSaturation(0f) }
                paint.colorFilter = ColorMatrixColorFilter(matrix)
            }
            ColorTone.HighContrast -> {
                val matrix = ColorMatrix(
                    floatArrayOf(
                        1.4f, 0f, 0f, 0f, -30f,
                        0f, 1.4f, 0f, 0f, -30f,
                        0f, 0f, 1.4f, 0f, -30f,
                        0f, 0f, 0f, 1f, 0f
                    )
                )
                paint.colorFilter = ColorMatrixColorFilter(matrix)
            }
            ColorTone.Original -> paint.colorFilter = null
        }

        val cutMarkPaint = Paint().apply {
            color = Color.DKGRAY
            strokeWidth = 0.5f
        }

        for (pageIdx in 0 until totalPages) {
            val pageInfo = PdfDocument.PageInfo.Builder(pageWidthPt, pageHeightPt, pageIdx + 1).create()
            val page = pdfDoc.startPage(pageInfo)
            val canvas: Canvas = page.canvas

            // Pure white background
            canvas.drawColor(Color.WHITE)

            val startItemIdx = pageIdx * perPage
            val endItemIdx = minOf(startItemIdx + perPage, expandedItems.size)

            for (i in startItemIdx until endItemIdx) {
                val slot = i - startItemIdx
                val col = slot % cols
                val row = slot / cols

                val x = marginXPt + col * (cellWPt + gapPt)
                val y = marginYPt + row * (cellHPt + gapPt)
                val cellRect = RectF(x, y, x + cellWPt, y + cellHPt)

                val item = expandedItems[i]
                val bitmap = decodeSampledBitmapFromUri(context, item.uri, 1200, 1200)

                if (bitmap != null) {
                    drawBitmapInRect(canvas, bitmap, cellRect, config.fitMode, paint)
                    bitmap.recycle()
                }

                // Trimmer marks
                if (config.showCutMarks && !config.isBorderless) {
                    val markLen = 6f
                    canvas.drawLine(cellRect.left, cellRect.top - markLen, cellRect.left, cellRect.top, cutMarkPaint)
                    canvas.drawLine(cellRect.left - markLen, cellRect.top, cellRect.left, cellRect.top, cutMarkPaint)
                    canvas.drawLine(cellRect.right, cellRect.top - markLen, cellRect.right, cellRect.top, cutMarkPaint)
                    canvas.drawLine(cellRect.right + markLen, cellRect.top, cellRect.right, cellRect.top, cutMarkPaint)
                    canvas.drawLine(cellRect.left, cellRect.bottom, cellRect.left, cellRect.bottom + markLen, cutMarkPaint)
                    canvas.drawLine(cellRect.left - markLen, cellRect.bottom, cellRect.left, cellRect.bottom, cutMarkPaint)
                    canvas.drawLine(cellRect.right, cellRect.bottom, cellRect.right, cellRect.bottom + markLen, cutMarkPaint)
                    canvas.drawLine(cellRect.right + markLen, cellRect.bottom, cellRect.right, cellRect.bottom, cutMarkPaint)
                }
            }

            pdfDoc.finishPage(page)
        }

        FileOutputStream(outputFile).use { out ->
            pdfDoc.writeTo(out)
        }
        pdfDoc.close()
        return totalPages
    }

    private fun drawBitmapInRect(canvas: Canvas, bitmap: Bitmap, rect: RectF, fitMode: FitMode, paint: Paint) {
        val bw = bitmap.width.toFloat()
        val bh = bitmap.height.toFloat()
        val rw = rect.width()
        val rh = rect.height()

        when (fitMode) {
            FitMode.Fill -> {
                val scale = maxOf(rw / bw, rh / bh)
                val scaledW = bw * scale
                val scaledH = bh * scale
                val left = rect.left + (rw - scaledW) / 2f
                val top = rect.top + (rh - scaledH) / 2f

                canvas.save()
                canvas.clipRect(rect)
                canvas.drawBitmap(bitmap, null, RectF(left, top, left + scaledW, top + scaledH), paint)
                canvas.restore()
            }
            FitMode.Contain -> {
                val scale = minOf(rw / bw, rh / bh)
                val scaledW = bw * scale
                val scaledH = bh * scale
                val left = rect.left + (rw - scaledW) / 2f
                val top = rect.top + (rh - scaledH) / 2f

                canvas.drawBitmap(bitmap, null, RectF(left, top, left + scaledW, top + scaledH), paint)
            }
        }
    }

    fun decodeSampledBitmapFromUri(context: Context, uri: android.net.Uri, reqWidth: Int, reqHeight: Int): Bitmap? {
        return try {
            val options = BitmapFactory.Options().apply {
                inJustDecodeBounds = true
            }
            context.contentResolver.openInputStream(uri)?.use { stream ->
                BitmapFactory.decodeStream(stream, null, options)
            }

            options.inSampleSize = calculateInSampleSize(options, reqWidth, reqHeight)
            options.inJustDecodeBounds = false

            context.contentResolver.openInputStream(uri)?.use { stream ->
                BitmapFactory.decodeStream(stream, null, options)
            }
        } catch (e: Exception) {
            null
        }
    }

    private fun calculateInSampleSize(options: BitmapFactory.Options, reqWidth: Int, reqHeight: Int): Int {
        val (height: Int, width: Int) = options.run { outHeight to outWidth }
        var inSampleSize = 1
        if (height > reqHeight || width > reqWidth) {
            val halfHeight: Int = height / 2
            val halfWidth: Int = width / 2
            while (halfHeight / inSampleSize >= reqHeight && halfWidth / inSampleSize >= reqWidth) {
                inSampleSize *= 2
            }
        }
        return inSampleSize
    }
}
