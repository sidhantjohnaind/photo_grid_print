use crate::pdf::PdfPage;
use image::{codecs::jpeg::JpegEncoder, imageops, DynamicImage, ImageBuffer, Rgb, RgbImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum FitMode {
    #[default]
    Fill,    // Crop to fill cell completely (no white letterboxing)
    Contain, // Preserve full image (pad with white if needed)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ColorFilter {
    #[default]
    Original,
    Grayscale,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum PaperSize {
    #[default]
    A4,       // 210 x 297 mm
    A3,       // 297 x 420 mm
    A5,       // 148 x 210 mm
    Letter,   // 215.9 x 279.4 mm (8.5 x 11 in)
    Legal,    // 215.9 x 355.6 mm (8.5 x 14 in)
    Photo4x6, // 101.6 x 152.4 mm (4 x 6 in)
    Photo5x7, // 127 x 177.8 mm (5 x 7 in)
}

impl PaperSize {
    pub fn dimensions_mm(&self, is_portrait: bool) -> (f64, f64) {
        let (w, h) = match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::Photo4x6 => (101.6, 152.4),
            PaperSize::Photo5x7 => (127.0, 177.8),
        };
        if is_portrait {
            (w, h)
        } else {
            (h, w)
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PaperSize::A4 => "A4 (210 x 297 mm)",
            PaperSize::Letter => "US Letter (8.5 x 11 in)",
            PaperSize::Legal => "US Legal (8.5 x 14 in)",
            PaperSize::Photo4x6 => "4 x 6 in Photo Paper",
            PaperSize::Photo5x7 => "5 x 7 in Photo Paper",
            PaperSize::A3 => "A3 (297 x 420 mm)",
            PaperSize::A5 => "A5 (148 x 210 mm)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridConfig {
    pub paper_size: PaperSize,
    pub cols: usize,
    pub rows: usize,
    pub border_width: u32,
    pub border_color: [u8; 3],
    pub gap: u32,
    pub margin_x: u32,
    pub margin_y: u32,
    pub is_portrait: bool,
    pub fit_mode: FitMode,
    pub color_filter: ColorFilter,
    pub show_cut_marks: bool,
    pub dpi: u32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::A4,
            cols: 4,
            rows: 4,
            border_width: 0,
            border_color: [200, 200, 200],
            gap: 24,
            margin_x: 50,
            margin_y: 42,
            is_portrait: false,
            fit_mode: FitMode::Fill,
            color_filter: ColorFilter::Original,
            show_cut_marks: false,
            dpi: 300,
        }
    }
}

/// Multithreaded Parallel 300 DPI PDF Page Generator using Rayon across all CPU cores
pub fn render_images_with_copies_to_pdf_pages(
    items: &[(&DynamicImage, usize)],
    config: &GridConfig,
) -> anyhow::Result<Vec<PdfPage>> {
    let (paper_w_mm, paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);

    let page_w_px = (paper_w_mm / 25.4 * config.dpi as f64).round() as u32;
    let page_h_px = (paper_h_mm / 25.4 * config.dpi as f64).round() as u32;

    let outer_margin_x = config.margin_x;
    let outer_margin_y = config.margin_y;

    let cols = config.cols.max(1) as u32;
    let rows = config.rows.max(1) as u32;
    let items_per_page = (cols * rows) as usize;

    let available_w = page_w_px.saturating_sub(2 * outer_margin_x);
    let available_h = page_h_px.saturating_sub(2 * outer_margin_y);

    let cell_w = (available_w.saturating_sub((cols - 1) * config.gap)) / cols;
    let cell_h = (available_h.saturating_sub((rows - 1) * config.gap)) / rows;

    let total_grid_w = cols * cell_w + (cols - 1) * config.gap;
    let total_grid_h = rows * cell_h + (rows - 1) * config.gap;

    let start_x = outer_margin_x + (available_w.saturating_sub(total_grid_w)) / 2;
    let start_y = outer_margin_y + (available_h.saturating_sub(total_grid_h)) / 2;

    // Parallel processing of all distinct cells using all CPU cores
    let processed_cells: Vec<RgbImage> = items
        .par_iter()
        .map(|(img, _)| prepare_cell(img, cell_w, cell_h, config, false))
        .collect();

    let mut cell_queue: Vec<&RgbImage> = Vec::new();
    for (i, &(_, copies)) in items.iter().enumerate() {
        for _ in 0..copies {
            cell_queue.push(&processed_cells[i]);
        }
    }

    if cell_queue.is_empty() {
        return Ok(Vec::new());
    }

    let num_pages = (cell_queue.len() + items_per_page - 1) / items_per_page;

    // Parallel PDF page canvas rendering and JPEG encoding across all CPU cores
    let pdf_pages: Vec<PdfPage> = (0..num_pages)
        .into_par_iter()
        .map(|p| {
            let mut page_canvas: RgbImage = ImageBuffer::from_pixel(page_w_px, page_h_px, Rgb([255, 255, 255]));
            let start_idx = p * items_per_page;
            let end_idx = (start_idx + items_per_page).min(cell_queue.len());

            for (slot, &cell_img) in cell_queue[start_idx..end_idx].iter().enumerate() {
                let row = (slot as u32) / cols;
                let col = (slot as u32) % cols;
                let x = start_x + col * (cell_w + config.gap);
                let y = start_y + row * (cell_h + config.gap);

                imageops::overlay(&mut page_canvas, cell_img, x as i64, y as i64);

                if config.show_cut_marks && config.gap > 4 {
                    draw_cut_corner_marks(&mut page_canvas, x, y, cell_w, cell_h, 15);
                }
            }

            let mut jpeg_bytes = Vec::new();
            let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 95);
            let _ = encoder.encode_image(&page_canvas);

            PdfPage {
                jpeg_data: jpeg_bytes,
                width_px: page_w_px,
                height_px: page_h_px,
            }
        })
        .collect();

    Ok(pdf_pages)
}

/// Blazing-fast Multithreaded Parallel Live Preview Generator across all CPU cores
pub fn render_all_preview_pages_with_copies(
    items: &[(&DynamicImage, usize)],
    config: &GridConfig,
    max_dim: u32,
) -> Vec<RgbImage> {
    if items.is_empty() {
        return Vec::new();
    }

    let (paper_w_mm, paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);
    let aspect = paper_w_mm / paper_h_mm;

    let (preview_w, preview_h) = if aspect >= 1.0 {
        (max_dim, (max_dim as f64 / aspect).round() as u32)
    } else {
        ((max_dim as f64 * aspect).round() as u32, max_dim)
    };

    let scale = preview_w as f64 / (paper_w_mm / 25.4 * 300.0);

    let outer_margin_x = (config.margin_x as f64 * scale).round() as u32;
    let outer_margin_y = (config.margin_y as f64 * scale).round() as u32;
    let scaled_gap = (config.gap as f64 * scale).round().max(0.0) as u32;

    let cols = config.cols.max(1) as u32;
    let rows = config.rows.max(1) as u32;
    let items_per_page = (cols * rows) as usize;

    let available_w = preview_w.saturating_sub(2 * outer_margin_x);
    let available_h = preview_h.saturating_sub(2 * outer_margin_y);

    let cell_w = (available_w.saturating_sub((cols - 1) * scaled_gap)) / cols;
    let cell_h = (available_h.saturating_sub((rows - 1) * scaled_gap)) / rows;

    let total_grid_w = cols * cell_w + (cols - 1) * scaled_gap;
    let total_grid_h = rows * cell_h + (rows - 1) * scaled_gap;

    let start_x = outer_margin_x + (available_w.saturating_sub(total_grid_w)) / 2;
    let start_y = outer_margin_y + (available_h.saturating_sub(total_grid_h)) / 2;

    // Parallel fast preview processing with Triangle filter across all CPU cores
    let processed_cells: Vec<RgbImage> = items
        .par_iter()
        .map(|(img, _)| prepare_cell(img, cell_w, cell_h, config, true))
        .collect();

    let mut cell_queue: Vec<&RgbImage> = Vec::new();
    for (i, &(_, copies)) in items.iter().enumerate() {
        for _ in 0..copies {
            cell_queue.push(&processed_cells[i]);
        }
    }

    if cell_queue.is_empty() {
        return Vec::new();
    }

    let num_pages = (cell_queue.len() + items_per_page - 1) / items_per_page;

    // Parallel preview canvases rendering across all CPU cores
    (0..num_pages)
        .into_par_iter()
        .map(|p| {
            let mut canvas: RgbImage = ImageBuffer::from_pixel(preview_w, preview_h, Rgb([255, 255, 255]));
            let start_idx = p * items_per_page;
            let end_idx = (start_idx + items_per_page).min(cell_queue.len());

            for (slot, &cell_img) in cell_queue[start_idx..end_idx].iter().enumerate() {
                let row = (slot as u32) / cols;
                let col = (slot as u32) % cols;
                let x = start_x + col * (cell_w + scaled_gap);
                let y = start_y + row * (cell_h + scaled_gap);

                imageops::overlay(&mut canvas, cell_img, x as i64, y as i64);

                if config.show_cut_marks && scaled_gap > 3 {
                    draw_cut_corner_marks(&mut canvas, x, y, cell_w, cell_h, 6);
                }
            }

            canvas
        })
        .collect()
}

fn draw_cut_corner_marks(img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, len: u32) {
    let mark_color = Rgb([190, 195, 205]);
    let img_w = img.width();
    let img_h = img.height();

    for i in 0..len {
        if x >= i && y < img_h { img.put_pixel(x - i, y, mark_color); }
        if y >= i && x < img_w { img.put_pixel(x, y - i, mark_color); }
    }
    for i in 0..len {
        if x + w + i < img_w && y < img_h { img.put_pixel(x + w + i, y, mark_color); }
        if y >= i && x + w < img_w { img.put_pixel(x + w, y - i, mark_color); }
    }
    for i in 0..len {
        if x >= i && y + h < img_h { img.put_pixel(x - i, y + h, mark_color); }
        if y + h + i < img_h && x < img_w { img.put_pixel(x, y + h + i, mark_color); }
    }
    for i in 0..len {
        if x + w + i < img_w && y + h < img_h { img.put_pixel(x + w + i, y + h, mark_color); }
        if y + h + i < img_h && x + w < img_w { img.put_pixel(x + w, y + h + i, mark_color); }
    }
}

fn prepare_cell(
    img: &DynamicImage,
    target_w: u32,
    target_h: u32,
    config: &GridConfig,
    is_preview: bool,
) -> RgbImage {
    let mut rgb_img = img.to_rgb8();

    match config.color_filter {
        ColorFilter::Original => {}
        ColorFilter::Grayscale => {
            let gray = imageops::grayscale(&rgb_img);
            rgb_img = DynamicImage::ImageLuma8(gray).to_rgb8();
        }
        ColorFilter::HighContrast => {
            let gray = imageops::grayscale(&rgb_img);
            let mut contrast_img = DynamicImage::ImageLuma8(gray).to_rgb8();
            imageops::colorops::contrast_in_place(&mut contrast_img, 25.0);
            rgb_img = contrast_img;
        }
    }

    let (src_w, src_h) = rgb_img.dimensions();
    // Triangle filter for ultra-fast silky 60fps preview, Lanczos3 for final 300 DPI PDF export
    let filter = if is_preview {
        imageops::FilterType::Triangle
    } else {
        imageops::FilterType::Lanczos3
    };

    let mut cell = match config.fit_mode {
        FitMode::Fill => {
            let scale_x = target_w as f64 / src_w as f64;
            let scale_y = target_h as f64 / src_h as f64;
            let scale = scale_x.max(scale_y);

            let scaled_w = (src_w as f64 * scale).round().max(1.0) as u32;
            let scaled_h = (src_h as f64 * scale).round().max(1.0) as u32;

            let scaled = imageops::resize(&rgb_img, scaled_w, scaled_h, filter);

            let crop_x = (scaled_w.saturating_sub(target_w)) / 2;
            let crop_y = (scaled_h.saturating_sub(target_h)) / 2;

            imageops::crop_imm(&scaled, crop_x, crop_y, target_w, target_h).to_image()
        }
        FitMode::Contain => {
            let scale_x = target_w as f64 / src_w as f64;
            let scale_y = target_h as f64 / src_h as f64;
            let scale = scale_x.min(scale_y);

            let scaled_w = (src_w as f64 * scale).round().max(1.0) as u32;
            let scaled_h = (src_h as f64 * scale).round().max(1.0) as u32;

            let scaled = imageops::resize(&rgb_img, scaled_w, scaled_h, filter);

            let mut out = ImageBuffer::from_pixel(target_w, target_h, Rgb([255, 255, 255]));
            let off_x = (target_w.saturating_sub(scaled_w)) / 2;
            let off_y = (target_h.saturating_sub(scaled_h)) / 2;
            imageops::overlay(&mut out, &scaled, off_x as i64, off_y as i64);
            out
        }
    };

    if config.border_width > 0 {
        let border_c = Rgb(config.border_color);
        let bw = config.border_width;
        let w = cell.width();
        let h = cell.height();

        for b in 0..bw {
            for x in b..(w.saturating_sub(b)) {
                if b < h {
                    cell.put_pixel(x, b, border_c);
                }
                if h > 1 + b {
                    cell.put_pixel(x, h - 1 - b, border_c);
                }
            }
            for y in b..(h.saturating_sub(b)) {
                if b < w {
                    cell.put_pixel(b, y, border_c);
                }
                if w > 1 + b {
                    cell.put_pixel(w - 1 - b, y, border_c);
                }
            }
        }
    }

    cell
}
