use crate::config::AppConfig;
use crate::grid::{
    render_all_preview_pages_with_copies, render_images_with_copies_to_pdf_pages, ColorFilter, FitMode, GridConfig, PaperSize,
};
use crate::pdf;
use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, CursorIcon, Frame, Margin, Pos2, Rect, RichText, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use image::DynamicImage;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CopiesMode {
    SameForAll,
    Individual,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreviewViewMode {
    AllPages,
    SinglePage,
}

pub struct PhotoItem {
    pub path: PathBuf,
    pub image: DynamicImage,
    pub copies: usize,
}

pub struct PhotoGridApp {
    pub items: Vec<PhotoItem>,
    pub paper_size: PaperSize,
    pub cols: usize,
    pub rows: usize,
    pub global_copies: usize,
    pub copies_mode: CopiesMode,
    pub border_width: u32,
    pub gap: u32,
    pub margin: u32,
    pub is_borderless: bool,
    pub is_portrait: bool,
    pub fit_mode: FitMode,
    pub color_filter: ColorFilter,
    pub show_cut_marks: bool,
    pub output_path: String,
    pub last_folder: Option<String>,
    pub status_message: Option<(String, bool)>,

    // Multi-Page Live Preview State
    pub preview_textures: Vec<TextureHandle>,
    pub preview_page_idx: usize,
    pub total_pages: usize,
    pub preview_dirty: bool,
    pub view_mode: PreviewViewMode,

    // Clickable Individual Photo Modal / Popup
    pub selected_item_idx: Option<usize>,
    pub selected_popup_pos: Option<Pos2>,
}

impl Default for PhotoGridApp {
    fn default() -> Self {
        let cfg = AppConfig::load();
        let default_out = cfg.output_path.clone().unwrap_or_else(|| {
            dirs_desktop().unwrap_or_else(|| PathBuf::from(".")).join("Photo_Grid_Print.pdf").to_string_lossy().to_string()
        });

        Self {
            items: Vec::new(),
            paper_size: cfg.paper_size,
            cols: cfg.cols,
            rows: cfg.rows,
            global_copies: cfg.global_copies,
            copies_mode: if cfg.is_individual_copies { CopiesMode::Individual } else { CopiesMode::SameForAll },
            border_width: 0,
            gap: cfg.gap,
            margin: cfg.margin,
            is_borderless: cfg.is_borderless,
            is_portrait: cfg.is_portrait,
            fit_mode: cfg.fit_mode,
            color_filter: cfg.color_filter,
            show_cut_marks: cfg.show_cut_marks,
            output_path: default_out,
            last_folder: cfg.last_folder,
            status_message: None,
            preview_textures: Vec::new(),
            preview_page_idx: 0,
            total_pages: 0,
            preview_dirty: true,
            view_mode: PreviewViewMode::AllPages,
            selected_item_idx: None,
            selected_popup_pos: None,
        }
    }
}

impl PhotoGridApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        let browser_dir = PathBuf::from(r"D:\Downloads\Browser");
        if browser_dir.exists() {
            app.load_folder(&browser_dir, 16);
        } else if let Some(last) = &app.last_folder {
            let p = PathBuf::from(last);
            if p.exists() {
                app.load_folder(&p, 16);
            }
        }
        app
    }

    pub fn save_config(&self) {
        let cfg = AppConfig {
            paper_size: self.paper_size,
            cols: self.cols,
            rows: self.rows,
            global_copies: self.global_copies,
            is_individual_copies: self.copies_mode == CopiesMode::Individual,
            gap: self.gap,
            margin: self.margin,
            is_borderless: self.is_borderless,
            is_portrait: self.is_portrait,
            fit_mode: self.fit_mode,
            color_filter: self.color_filter,
            show_cut_marks: self.show_cut_marks,
            output_path: Some(self.output_path.clone()),
            last_folder: self.last_folder.clone(),
        };
        cfg.save();
    }

    fn add_paths(&mut self, paths: &[PathBuf]) {
        for p in paths {
            if let Ok(img) = image::open(p) {
                self.items.push(PhotoItem {
                    path: p.clone(),
                    image: img,
                    copies: self.global_copies,
                });
            }
        }
        self.preview_dirty = true;
    }

    fn load_folder(&mut self, dir: &PathBuf, max_count: usize) {
        let valid_exts = ["jpg", "jpeg", "png", "webp", "bmp"];
        let mut files = Vec::new();
        for entry in WalkDir::new(dir).max_depth(1).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if valid_exts.contains(&ext.to_lowercase().as_str()) {
                        files.push(p);
                    }
                }
            }
        }
        files.sort_by(|a, b| {
            let m_a = fs::metadata(a).and_then(|m| m.modified()).ok();
            let m_b = fs::metadata(b).and_then(|m| m.modified()).ok();
            m_b.cmp(&m_a)
        });

        if max_count > 0 && files.len() > max_count {
            files.truncate(max_count);
        }

        self.last_folder = Some(dir.to_string_lossy().to_string());
        self.save_config();

        self.items.clear();
        self.add_paths(&files);
        self.status_message = Some((
            format!("Loaded {} images from folder.", self.items.len()),
            false,
        ));
    }

    fn current_config(&self) -> GridConfig {
        let actual_margin = if self.is_borderless { 0 } else { self.margin };
        let actual_gap = if self.is_borderless { 0 } else { self.gap };

        GridConfig {
            paper_size: self.paper_size,
            cols: self.cols,
            rows: self.rows,
            border_width: if self.is_borderless { 0 } else { self.border_width },
            border_color: [60, 60, 60],
            gap: actual_gap,
            margin_x: actual_margin,
            margin_y: (actual_margin as f64 * 0.85) as u32,
            is_portrait: self.is_portrait,
            fit_mode: self.fit_mode,
            color_filter: self.color_filter,
            show_cut_marks: self.show_cut_marks && !self.is_borderless,
            dpi: 300,
        }
    }

    fn prepare_render_items(&self) -> Vec<(&DynamicImage, usize)> {
        self.items
            .iter()
            .map(|item| {
                let copies = match self.copies_mode {
                    CopiesMode::SameForAll => self.global_copies,
                    CopiesMode::Individual => item.copies,
                };
                (&item.image, copies)
            })
            .collect()
    }

    fn update_preview(&mut self, ctx: &egui::Context) {
        if self.items.is_empty() {
            self.preview_textures.clear();
            self.total_pages = 0;
            self.preview_dirty = false;
            return;
        }

        let config = self.current_config();
        let total_photos: usize = self
            .items
            .iter()
            .map(|item| match self.copies_mode {
                CopiesMode::SameForAll => self.global_copies,
                CopiesMode::Individual => item.copies,
            })
            .sum();

        let per_page = (config.cols * config.rows).max(1);
        self.total_pages = (total_photos + per_page - 1) / per_page;

        if self.preview_page_idx >= self.total_pages {
            self.preview_page_idx = self.total_pages.saturating_sub(1);
        }

        let render_items: Vec<(&DynamicImage, usize)> = self
            .items
            .iter()
            .map(|item| {
                let copies = match self.copies_mode {
                    CopiesMode::SameForAll => self.global_copies,
                    CopiesMode::Individual => item.copies,
                };
                (&item.image, copies)
            })
            .collect();

        let rgb_pages = render_all_preview_pages_with_copies(&render_items, &config, 1000);
        self.preview_textures.clear();

        for (p_idx, rgb_canvas) in rgb_pages.into_iter().enumerate() {
            let width = rgb_canvas.width() as usize;
            let height = rgb_canvas.height() as usize;
            let raw_pixels = rgb_canvas.into_raw();
            let color_image = ColorImage::from_rgb([width, height], &raw_pixels);
            let tex = ctx.load_texture(
                format!("live_page_preview_{}", p_idx),
                color_image,
                TextureOptions::LINEAR,
            );
            self.preview_textures.push(tex);
        }

        self.preview_dirty = false;
    }

    fn generate_and_handle_pdf(&mut self, direct_print: bool) {
        if self.items.is_empty() {
            self.status_message = Some(("Please select at least one image file.".to_string(), true));
            return;
        }

        self.save_config();

        let render_items = self.prepare_render_items();
        let config = self.current_config();

        match render_images_with_copies_to_pdf_pages(&render_items, &config) {
            Ok(pdf_pages) => {
                let (paper_w_mm, paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);
                let page_w_pt = paper_w_mm / 25.4 * 72.0;
                let page_h_pt = paper_h_mm / 25.4 * 72.0;

                let pdf_bytes = pdf::create_pdf(&pdf_pages, page_w_pt, page_h_pt);
                let out = PathBuf::from(&self.output_path);

                if let Some(parent) = out.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                match fs::write(&out, &pdf_bytes) {
                    Ok(_) => {
                        let total_photos: usize = render_items.iter().map(|(_, c)| *c).sum();
                        if direct_print {
                            self.status_message = Some((
                                format!(
                                    "Sent to printer! Generated {} page(s) ({} photos).",
                                    pdf_pages.len(),
                                    total_photos
                                ),
                                false,
                            ));
                            send_to_printer(&out);
                        } else {
                            self.status_message = Some((
                                format!(
                                    "Success! Generated {} page(s) ({} photos) -> {}",
                                    pdf_pages.len(),
                                    total_photos,
                                    out.display()
                                ),
                                false,
                            ));
                            let _ = open::that(&out);
                        }
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Failed to write PDF: {}", e), true));
                    }
                }
            }
            Err(e) => {
                self.status_message = Some((format!("Error generating PDF: {}", e), true));
            }
        }
    }
}

pub fn send_to_printer(pdf_path: &Path) {
    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];

    for path in &edge_paths {
        if Path::new(path).exists() {
            let res = Command::new(path)
                .args(["/p", &pdf_path.to_string_lossy()])
                .spawn();
            if res.is_ok() {
                return;
            }
        }
    }

    let _ = open::that(pdf_path);
}

fn card_frame() -> Frame {
    Frame {
        inner_margin: Margin::same(12),
        corner_radius: CornerRadius::same(6),
        fill: Color32::from_rgb(26, 28, 34),
        stroke: Stroke::new(1.0, Color32::from_rgb(45, 48, 58)),
        ..Default::default()
    }
}

impl eframe::App for PhotoGridApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Drag and drop handler
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                let mut added_paths = Vec::new();
                for f in &i.raw.dropped_files {
                    let p = f.path().to_path_buf();
                    if p.is_dir() {
                        self.load_folder(&p, 16);
                        return;
                    } else if p.is_file() {
                        added_paths.push(p);
                    }
                }
                if !added_paths.is_empty() {
                    self.add_paths(&added_paths);
                    self.status_message = Some((
                        format!("Added {} image(s).", added_paths.len()),
                        false,
                    ));
                }
            }
        });

        if self.preview_dirty {
            self.update_preview(&ctx);
        }

        // Top App Bar
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.heading(RichText::new("Photo Grid Print").size(19.0).strong().color(Color32::from_rgb(240, 240, 245)));
            ui.label(RichText::new("|  High-Res 300 DPI Multi-Sheet Generator & Direct Print").size(12.0).color(Color32::from_rgb(150, 155, 170)));
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.small_button("⚙ Config").clicked() {
                    let cfg_path = AppConfig::config_path();
                    let _ = open::that(cfg_path.parent().unwrap_or(&cfg_path));
                }
            });
        });

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(4.0);

        let total_avail = ui.available_size();
        let sidebar_width = (520.0_f32).min(total_avail.x * 0.48).max(440.0);

        ui.horizontal(|ui| {
            // ==========================================
            // LEFT SIDEBAR: Controls (Full Height Scroll)
            // ==========================================
            ui.allocate_ui_with_layout(
                egui::vec2(sidebar_width, total_avail.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("sidebar_scroll_area")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                            let content_w = sidebar_width - 18.0;

                            // 1. Source Photos Card
                            card_frame().show(ui, |ui| {
                                ui.set_width(content_w);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("1. Photos").strong().color(Color32::from_rgb(220, 225, 240)));
                                    ui.label(RichText::new("(Drag & drop anywhere)").size(11.0).color(Color32::GRAY));
                                });

                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("Select Files...").clicked() {
                                        if let Some(files) = rfd::FileDialog::new()
                                            .add_filter("Images", &["jpg", "jpeg", "png", "webp", "bmp"])
                                            .pick_files()
                                        {
                                            self.add_paths(&files);
                                        }
                                    }

                                    if ui.button("Select Folder...").clicked() {
                                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                            self.load_folder(&dir, 16);
                                        }
                                    }

                                    let browser_dir = PathBuf::from(r"D:\Downloads\Browser");
                                    if browser_dir.exists() && ui.button("WhatsApp (16)").clicked() {
                                        self.load_folder(&browser_dir, 16);
                                    }

                                    if !self.items.is_empty() && ui.button("Clear").clicked() {
                                        self.items.clear();
                                        self.selected_item_idx = None;
                                        self.preview_dirty = true;
                                        self.status_message = None;
                                    }
                                });

                                if self.items.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(RichText::new("No photos loaded. Drag & drop images here.").size(11.5).color(Color32::from_rgb(130, 135, 150)));
                                } else {
                                    ui.add_space(4.0);
                                    let count = self.items.len();
                                    ui.label(RichText::new(format!("✓ {} photo(s) ready", count)).color(Color32::from_rgb(70, 200, 120)).strong());

                                    if self.copies_mode == CopiesMode::Individual {
                                        ui.add_space(3.0);
                                        ui.label(RichText::new("Individual Copies per Photo:").size(11.5).strong().color(Color32::from_rgb(190, 195, 210)));
                                        
                                        egui::ScrollArea::vertical()
                                            .id_salt("individual_copies_scroll")
                                            .max_height(140.0)
                                            .show(ui, |ui| {
                                                let mut to_remove = None;
                                                let mut to_rotate = None;
                                                let mut to_swap = None;
                                                let items_len = self.items.len();

                                                for (idx, item) in self.items.iter_mut().enumerate() {
                                                    let is_selected = self.selected_item_idx == Some(idx);

                                                    ui.horizontal_wrapped(|ui| {
                                                        let name = item.path.file_name().unwrap_or_default().to_string_lossy();
                                                        
                                                        let num_label = if is_selected {
                                                            RichText::new(format!("👉 {}.", idx + 1)).strong().color(Color32::from_rgb(0, 200, 255))
                                                        } else {
                                                            RichText::new(format!("{}.", idx + 1)).size(11.0).color(Color32::GRAY)
                                                        };
                                                        ui.label(num_label);
                                                        ui.add(egui::Label::new(RichText::new(name).size(11.0)).truncate());

                                                        let prev_c = item.copies;

                                                        if ui.small_button("-").clicked() && item.copies > 1 {
                                                            item.copies -= 1;
                                                        }
                                                        ui.add(egui::DragValue::new(&mut item.copies).range(1..=100));
                                                        if ui.small_button("+").clicked() && item.copies < 100 {
                                                            item.copies += 1;
                                                        }

                                                        if ui.small_button("1x").clicked() { item.copies = 1; }
                                                        if ui.small_button("2x").clicked() { item.copies = 2; }
                                                        if ui.small_button("4x").clicked() { item.copies = 4; }

                                                        if ui.small_button("↻ 90°").clicked() {
                                                            to_rotate = Some(idx);
                                                        }

                                                        if idx > 0 && ui.small_button("▲").clicked() {
                                                            to_swap = Some((idx, idx - 1));
                                                        }
                                                        if idx + 1 < items_len && ui.small_button("▼").clicked() {
                                                            to_swap = Some((idx, idx + 1));
                                                        }

                                                        if item.copies != prev_c {
                                                            self.preview_dirty = true;
                                                        }

                                                        if ui.small_button("x").clicked() {
                                                            to_remove = Some(idx);
                                                        }
                                                    });
                                                    ui.add_space(2.0);
                                                }

                                                if let Some(idx) = to_rotate {
                                                    self.items[idx].image = self.items[idx].image.rotate90();
                                                    self.preview_dirty = true;
                                                }

                                                if let Some((a, b)) = to_swap {
                                                    self.items.swap(a, b);
                                                    self.selected_item_idx = Some(b);
                                                    self.preview_dirty = true;
                                                }

                                                if let Some(idx) = to_remove {
                                                    self.items.remove(idx);
                                                    if self.selected_item_idx == Some(idx) {
                                                        self.selected_item_idx = None;
                                                    }
                                                    self.preview_dirty = true;
                                                }
                                            });
                                    }
                                }
                            });

                            // 2. Paper Size & Grid Presets Card (with Passport/ID Sizes!)
                            card_frame().show(ui, |ui| {
                                ui.set_width(content_w);
                                ui.label(RichText::new("2. Layout, Paper & Presets").strong().color(Color32::from_rgb(220, 225, 240)));
                                ui.add_space(4.0);

                                ui.horizontal(|ui| {
                                    ui.label("Paper Size:");
                                    let prev_paper = self.paper_size;
                                    egui::ComboBox::from_id_salt("paper_size_combo")
                                        .selected_text(self.paper_size.name())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.paper_size, PaperSize::A4, PaperSize::A4.name());
                                            ui.selectable_value(&mut self.paper_size, PaperSize::Letter, PaperSize::Letter.name());
                                            ui.selectable_value(&mut self.paper_size, PaperSize::Legal, PaperSize::Legal.name());
                                            ui.selectable_value(&mut self.paper_size, PaperSize::Photo4x6, PaperSize::Photo4x6.name());
                                            ui.selectable_value(&mut self.paper_size, PaperSize::Photo5x7, PaperSize::Photo5x7.name());
                                            ui.selectable_value(&mut self.paper_size, PaperSize::A3, PaperSize::A3.name());
                                            ui.selectable_value(&mut self.paper_size, PaperSize::A5, PaperSize::A5.name());
                                        });
                                    if self.paper_size != prev_paper {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });

                                ui.add_space(3.0);
                                ui.horizontal_wrapped(|ui| {
                                    let prev_cols = self.cols;
                                    let prev_rows = self.rows;

                                    ui.label("Grid Presets:");
                                    if ui.button(RichText::new("16 (4x4)").strong()).clicked() { self.cols = 4; self.rows = 4; }
                                    if ui.button(RichText::new("9 (3x3)").strong()).clicked() { self.cols = 3; self.rows = 3; }
                                    if ui.button(RichText::new("8 (4x2)").strong()).clicked() { self.cols = 4; self.rows = 2; }
                                    if ui.button(RichText::new("6 (3x2)").strong().color(Color32::from_rgb(0, 190, 230))).clicked() { self.cols = 3; self.rows = 2; }
                                    if ui.button(RichText::new("6 (2x3)").strong().color(Color32::from_rgb(0, 190, 230))).clicked() { self.cols = 2; self.rows = 3; }
                                    if ui.button(RichText::new("4 (2x2)").strong()).clicked() { self.cols = 2; self.rows = 2; }

                                    if self.cols != prev_cols || self.rows != prev_rows {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });

                                ui.add_space(2.0);
                                ui.horizontal_wrapped(|ui| {
                                    let prev_cols = self.cols;
                                    let prev_rows = self.rows;

                                    ui.label("ID / Passport Presets:");
                                    if ui.button(RichText::new("Passport 2x2\" (6)").strong().color(Color32::from_rgb(100, 220, 140))).clicked() {
                                        self.cols = 3; self.rows = 2; self.fit_mode = FitMode::Fill;
                                    }
                                    if ui.button(RichText::new("Passport 35x45mm (8)").strong().color(Color32::from_rgb(100, 220, 140))).clicked() {
                                        self.cols = 4; self.rows = 2; self.fit_mode = FitMode::Fill;
                                    }
                                    if ui.button(RichText::new("Stamp 30x40mm (12)").strong().color(Color32::from_rgb(100, 220, 140))).clicked() {
                                        self.cols = 4; self.rows = 3; self.fit_mode = FitMode::Fill;
                                    }

                                    if self.cols != prev_cols || self.rows != prev_rows {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });

                                ui.add_space(3.0);
                                ui.horizontal(|ui| {
                                    let prev_cols = self.cols;
                                    let prev_rows = self.rows;

                                    ui.label("Cols:");
                                    ui.add(egui::Slider::new(&mut self.cols, 1..=8));
                                    ui.label("Rows:");
                                    ui.add(egui::Slider::new(&mut self.rows, 1..=8));

                                    if self.cols != prev_cols || self.rows != prev_rows {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });
                            });

                            // 3. Copies Settings Card
                            card_frame().show(ui, |ui| {
                                ui.set_width(content_w);
                                ui.label(RichText::new("3. Copies Settings").strong().color(Color32::from_rgb(220, 225, 240)));
                                ui.add_space(4.0);

                                ui.horizontal(|ui| {
                                    let prev_mode = self.copies_mode;
                                    ui.radio_value(&mut self.copies_mode, CopiesMode::SameForAll, "All Same");
                                    ui.radio_value(&mut self.copies_mode, CopiesMode::Individual, "Individual per Photo");
                                    if self.copies_mode != prev_mode {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });

                                ui.add_space(3.0);
                                if self.copies_mode == CopiesMode::SameForAll {
                                    let prev_copies = self.global_copies;

                                    ui.horizontal(|ui| {
                                        ui.label("Copies each:");
                                        if ui.button("-").clicked() && self.global_copies > 1 {
                                            self.global_copies -= 1;
                                        }
                                        ui.add(egui::DragValue::new(&mut self.global_copies).range(1..=100));
                                        if ui.button("+").clicked() && self.global_copies < 100 {
                                            self.global_copies += 1;
                                        }
                                    });

                                    ui.add_space(2.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label("Quick set:");
                                        if ui.button("1x").clicked() { self.global_copies = 1; }
                                        if ui.button("2x").clicked() { self.global_copies = 2; }
                                        if ui.button("3x").clicked() { self.global_copies = 3; }
                                        if ui.button("4x").clicked() { self.global_copies = 4; }
                                        if ui.button("5x").clicked() { self.global_copies = 5; }
                                        if ui.button("6x").clicked() { self.global_copies = 6; }
                                        if ui.button("8x").clicked() { self.global_copies = 8; }
                                        if ui.button("16x").clicked() { self.global_copies = 16; }
                                    });

                                    if self.global_copies != prev_copies {
                                        for item in &mut self.items {
                                            item.copies = self.global_copies;
                                        }
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                } else {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label("Batch set all:");
                                        if ui.button("1x").clicked() {
                                            for item in &mut self.items { item.copies = 1; }
                                            self.preview_dirty = true;
                                        }
                                        if ui.button("2x").clicked() {
                                            for item in &mut self.items { item.copies = 2; }
                                            self.preview_dirty = true;
                                        }
                                        if ui.button("3x").clicked() {
                                            for item in &mut self.items { item.copies = 3; }
                                            self.preview_dirty = true;
                                        }
                                        if ui.button("4x").clicked() {
                                            for item in &mut self.items { item.copies = 4; }
                                            self.preview_dirty = true;
                                        }
                                        if ui.button("6x").clicked() {
                                            for item in &mut self.items { item.copies = 6; }
                                            self.preview_dirty = true;
                                        }
                                        if ui.button("8x").clicked() {
                                            for item in &mut self.items { item.copies = 8; }
                                            self.preview_dirty = true;
                                        }
                                        if ui.button("16x").clicked() {
                                            for item in &mut self.items { item.copies = 16; }
                                            self.preview_dirty = true;
                                        }
                                    });
                                }
                            });

                            // 4. Margins, Spacing & Color Tone Filters Card
                            card_frame().show(ui, |ui| {
                                ui.set_width(content_w);
                                ui.label(RichText::new("4. Spacing, Tone & Trimming").strong().color(Color32::from_rgb(220, 225, 240)));
                                ui.add_space(4.0);

                                ui.horizontal(|ui| {
                                    let prev_filter = self.color_filter;
                                    ui.label("Color Tone:");
                                    ui.radio_value(&mut self.color_filter, ColorFilter::Original, "Color");
                                    ui.radio_value(&mut self.color_filter, ColorFilter::Grayscale, "Grayscale (B&W)");
                                    ui.radio_value(&mut self.color_filter, ColorFilter::HighContrast, "High Contrast");

                                    if self.color_filter != prev_filter {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });

                                ui.add_space(3.0);
                                ui.horizontal(|ui| {
                                    let prev_b = self.is_borderless;
                                    ui.radio_value(&mut self.is_borderless, false, "Spaced Margins & Gaps");
                                    ui.radio_value(&mut self.is_borderless, true, "100% Full-Bleed");
                                    if self.is_borderless != prev_b {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });

                                ui.add_space(2.0);
                                if !self.is_borderless {
                                    ui.horizontal(|ui| {
                                        let prev_margin = self.margin;
                                        ui.label("Page Margin:");
                                        ui.add(egui::Slider::new(&mut self.margin, 0..=150).suffix(" px"));
                                        if ui.button("0px (No Margin)").clicked() { self.margin = 0; }
                                        if ui.button("50px").clicked() { self.margin = 50; }

                                        if self.margin != prev_margin {
                                            self.save_config();
                                            self.preview_dirty = true;
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        let prev_gap = self.gap;
                                        ui.label("Photo Gap:");
                                        ui.add(egui::Slider::new(&mut self.gap, 0..=60).suffix(" px"));
                                        if ui.button("0px (Touching)").clicked() { self.gap = 0; }
                                        if ui.button("24px").clicked() { self.gap = 24; }

                                        if self.gap != prev_gap {
                                            self.save_config();
                                            self.preview_dirty = true;
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        let prev_cut = self.show_cut_marks;
                                        ui.checkbox(&mut self.show_cut_marks, "Trimmer / Cutting Corner Guides");
                                        if self.show_cut_marks != prev_cut {
                                            self.save_config();
                                            self.preview_dirty = true;
                                        }
                                    });
                                }

                                ui.horizontal(|ui| {
                                    let prev_port = self.is_portrait;
                                    let prev_fit = self.fit_mode;

                                    ui.label("Orientation:");
                                    ui.radio_value(&mut self.is_portrait, false, "Landscape");
                                    ui.radio_value(&mut self.is_portrait, true, "Portrait");

                                    ui.label("Fit:");
                                    ui.radio_value(&mut self.fit_mode, FitMode::Fill, "Fill");
                                    ui.radio_value(&mut self.fit_mode, FitMode::Contain, "Fit");

                                    if self.is_portrait != prev_port || self.fit_mode != prev_fit {
                                        self.save_config();
                                        self.preview_dirty = true;
                                    }
                                });
                            });

                            // 5. Save Path Card
                            card_frame().show(ui, |ui| {
                                ui.set_width(content_w);
                                ui.label(RichText::new("5. Save Destination").strong().color(Color32::from_rgb(220, 225, 240)));
                                ui.add_space(3.0);
                                ui.horizontal(|ui| {
                                    let prev_out = self.output_path.clone();
                                    ui.text_edit_singleline(&mut self.output_path);
                                    if ui.button("Browse...").clicked() {
                                        if let Some(dest) = rfd::FileDialog::new()
                                            .set_file_name("Photo_Grid_Print.pdf")
                                            .add_filter("PDF Document", &["pdf"])
                                            .save_file()
                                        {
                                            self.output_path = dest.to_string_lossy().to_string();
                                        }
                                    }
                                    if self.output_path != prev_out {
                                        self.save_config();
                                    }
                                });
                            });

                            // 6. Primary Action Buttons
                            ui.add_space(4.0);
                            let btn_w = (content_w - 8.0) / 2.0;

                            ui.horizontal(|ui| {
                                let print_btn = egui::Button::new(
                                    RichText::new("Print Now")
                                        .size(15.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(0, 130, 220))
                                .min_size(egui::vec2(btn_w, 40.0));

                                if ui.add(print_btn).clicked() {
                                    self.generate_and_handle_pdf(true);
                                }

                                let pdf_btn = egui::Button::new(
                                    RichText::new("Save & Open PDF")
                                        .size(15.0)
                                        .strong(),
                                )
                                .min_size(egui::vec2(btn_w, 40.0));

                                if ui.add(pdf_btn).clicked() {
                                    self.generate_and_handle_pdf(false);
                                }
                            });

                            if let Some((msg, is_err)) = &self.status_message {
                                ui.add_space(3.0);
                                let color = if *is_err {
                                    Color32::from_rgb(230, 70, 70)
                                } else {
                                    Color32::from_rgb(60, 200, 100)
                                };
                                ui.label(RichText::new(msg).color(color).strong());
                            }
                            ui.add_space(12.0);
                        });
                },
            );

            ui.separator();

            // =========================================================================
            // RIGHT PANEL: Expanded Multi-Page Interactive Live Sheet Preview
            // =========================================================================
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    let per_page = self.cols * self.rows;
                    let total_photos: usize = self.prepare_render_items().iter().map(|(_, c)| *c).sum();

                    // Multi-page header bar
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let summary_txt = format!(
                            "Live Sheet Preview  •  {} Sheet{} ({} photos total • {}/page)",
                            self.total_pages,
                            if self.total_pages == 1 { "" } else { "s" },
                            total_photos,
                            per_page
                        );
                        ui.label(RichText::new(summary_txt).size(13.0).strong().color(Color32::from_rgb(220, 225, 240)));

                        ui.label(RichText::new("(Click any photo on any page to edit)").size(11.0).color(Color32::from_rgb(0, 190, 240)));

                        if self.total_pages > 1 {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(8.0);

                                ui.selectable_value(&mut self.view_mode, PreviewViewMode::SinglePage, "Single Page");
                                ui.selectable_value(&mut self.view_mode, PreviewViewMode::AllPages, "Scroll All Sheets");

                                if self.view_mode == PreviewViewMode::SinglePage {
                                    if ui.button("Next ▶").clicked() && self.preview_page_idx + 1 < self.total_pages {
                                        self.preview_page_idx += 1;
                                    }
                                    ui.label(RichText::new(format!("{}/{}", self.preview_page_idx + 1, self.total_pages)).strong());
                                    if ui.button("◀ Prev").clicked() && self.preview_page_idx > 0 {
                                        self.preview_page_idx -= 1;
                                    }
                                }
                            });
                        }
                    });

                    ui.add_space(4.0);

                    if self.preview_textures.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(160.0);
                            ui.label(RichText::new("No Images Loaded").size(18.0).color(Color32::from_rgb(140, 145, 160)));
                            ui.add_space(6.0);
                            ui.label(RichText::new("Drag & drop photos or click 'Select Files...' to see live sheet preview").size(13.0).color(Color32::from_rgb(100, 105, 120)));
                        });
                        return;
                    }

                    // Build global item index queue
                    let mut item_queue: Vec<usize> = Vec::new();
                    for (idx, item) in self.items.iter().enumerate() {
                        let copies = match self.copies_mode {
                            CopiesMode::SameForAll => self.global_copies,
                            CopiesMode::Individual => item.copies,
                        };
                        for _ in 0..copies {
                            item_queue.push(idx);
                        }
                    }

                    let items_per_page = (self.cols * self.rows).max(1);
                    let config = self.current_config();
                    let (paper_w_mm, _paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);
                    let full_dpi_w = paper_w_mm / 25.4 * 300.0;

                    let avail_space = ui.available_size();
                    let preview_area_w = (avail_space.x - 24.0).max(200.0);
                    let img_aspect = self.preview_textures[0].aspect_ratio();

                    let sheet_w = preview_area_w.min(780.0);
                    let sheet_h = sheet_w / img_aspect;

                    let mut clicked_cell: Option<(usize, Pos2)> = None;
                    let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
                    let is_mouse_clicked = ctx.input(|i| i.pointer.primary_clicked());

                    let scale = sheet_w as f64 / full_dpi_w;
                    let outer_margin_x = (config.margin_x as f64 * scale) as f32;
                    let outer_margin_y = (config.margin_y as f64 * scale) as f32;
                    let scaled_gap = (config.gap as f64 * scale).max(0.0) as f32;

                    let cols = config.cols.max(1) as f32;
                    let rows = config.rows.max(1) as f32;

                    let available_w = sheet_w - 2.0 * outer_margin_x;
                    let available_h = sheet_h - 2.0 * outer_margin_y;

                    let cell_w = (available_w - (cols - 1.0) * scaled_gap) / cols;
                    let cell_h = (available_h - (rows - 1.0) * scaled_gap) / rows;

                    let grid_w = cols * cell_w + (cols - 1.0) * scaled_gap;
                    let grid_h = rows * cell_h + (rows - 1.0) * scaled_gap;

                    // Smooth Interactive Vertical ScrollArea for sheets
                    egui::ScrollArea::vertical()
                        .id_salt("live_preview_scroll_area")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let pages_to_render: Vec<usize> = match self.view_mode {
                                PreviewViewMode::AllPages => (0..self.preview_textures.len()).collect(),
                                PreviewViewMode::SinglePage => vec![self.preview_page_idx.min(self.preview_textures.len().saturating_sub(1))],
                            };

                            for p_idx in pages_to_render {
                                if p_idx >= self.preview_textures.len() {
                                    continue;
                                }

                                ui.add_space(4.0);
                                if self.total_pages > 1 && self.view_mode == PreviewViewMode::AllPages {
                                    ui.horizontal(|ui| {
                                        ui.add_space(8.0);
                                        ui.label(RichText::new(format!("Sheet #{} of {}", p_idx + 1, self.total_pages))
                                            .size(12.5)
                                            .strong()
                                            .color(Color32::from_rgb(0, 190, 240)));
                                    });
                                    ui.add_space(2.0);
                                }

                                let texture = &self.preview_textures[p_idx];

                                let img_resp = ui.add(
                                    egui::Image::new(texture)
                                        .fit_to_exact_size(egui::vec2(sheet_w, sheet_h))
                                        .corner_radius(4),
                                );
                                let sheet_rect = img_resp.rect;

                                let painter = ui.painter();
                                let shadow_rect = sheet_rect.expand(3.0);
                                painter.rect_filled(shadow_rect, 6.0, Color32::from_black_alpha(80));
                                painter.rect_stroke(sheet_rect, 4.0, Stroke::new(1.0, Color32::from_rgb(200, 205, 215)), StrokeKind::Outside);

                                let start_x = sheet_rect.min.x + outer_margin_x + (available_w - grid_w) / 2.0;
                                let start_y = sheet_rect.min.y + outer_margin_y + (available_h - grid_h) / 2.0;

                                let page_start_idx = p_idx * items_per_page;
                                let page_end_idx = (page_start_idx + items_per_page).min(item_queue.len());

                                for (slot_idx, &item_idx) in item_queue[page_start_idx..page_end_idx].iter().enumerate() {
                                    let col = (slot_idx % config.cols) as f32;
                                    let row = (slot_idx / config.cols) as f32;

                                    let x = start_x + col * (cell_w + scaled_gap);
                                    let y = start_y + row * (cell_h + scaled_gap);

                                    let cell_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell_w, cell_h));

                                    let is_hovered = mouse_pos.map_or(false, |mp| cell_rect.contains(mp));
                                    let is_selected = self.selected_item_idx == Some(item_idx);

                                    if is_hovered {
                                        ctx.set_cursor_icon(CursorIcon::PointingHand);
                                        painter.rect_filled(cell_rect, 2.0, Color32::from_rgba_premultiplied(0, 160, 255, 40));
                                        painter.rect_stroke(cell_rect, 2.0, Stroke::new(2.5, Color32::from_rgb(0, 190, 255)), StrokeKind::Inside);

                                        let badge_text = format!("#{}: Click to edit", item_idx + 1);
                                        let badge_rect = Rect::from_min_size(cell_rect.min + Vec2::new(4.0, 4.0), Vec2::new(100.0, 20.0));
                                        painter.rect_filled(badge_rect, 4.0, Color32::from_black_alpha(200));
                                        painter.text(badge_rect.center(), Align2::CENTER_CENTER, badge_text, egui::FontId::proportional(10.0), Color32::WHITE);
                                    } else if is_selected {
                                        painter.rect_stroke(cell_rect, 2.0, Stroke::new(2.5, Color32::from_rgb(255, 180, 0)), StrokeKind::Inside);
                                    }

                                    if is_hovered && is_mouse_clicked {
                                        clicked_cell = Some((item_idx, cell_rect.center_bottom()));
                                    }
                                }

                                ui.add_space(20.0);
                            }
                        });

                    if let Some((item_idx, pos)) = clicked_cell {
                        self.selected_item_idx = Some(item_idx);
                        self.selected_popup_pos = Some(pos);
                        self.copies_mode = CopiesMode::Individual;
                    }
                },
            );
        });

        // =========================================================================
        // FLOATING POPUP FOR CLICKED INDIVIDUAL PHOTO ON PREVIEW
        // =========================================================================
        if let (Some(selected_idx), Some(popup_pos)) = (self.selected_item_idx, self.selected_popup_pos) {
            if selected_idx < self.items.len() {
                let mut should_close = false;
                let mut should_delete = false;
                let mut should_rotate = false;
                let mut should_flip = false;
                let mut should_swap = None;

                egui::Window::new(format!("Photo #{} Settings", selected_idx + 1))
                    .fixed_pos(popup_pos + Vec2::new(-110.0, 10.0))
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .show(&ctx, |ui| {
                        let item = &mut self.items[selected_idx];
                        let filename = item.path.file_name().unwrap_or_default().to_string_lossy();

                        ui.label(RichText::new(filename).strong().color(Color32::from_rgb(220, 225, 240)));
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("Copies:");
                            if ui.button("-").clicked() && item.copies > 1 {
                                item.copies -= 1;
                                self.preview_dirty = true;
                            }
                            let prev_c = item.copies;
                            ui.add(egui::DragValue::new(&mut item.copies).range(1..=100));
                            if item.copies != prev_c {
                                self.preview_dirty = true;
                            }
                            if ui.button("+").clicked() && item.copies < 100 {
                                item.copies += 1;
                                self.preview_dirty = true;
                            }
                        });

                        ui.add_space(2.0);
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("1x").clicked() { item.copies = 1; self.preview_dirty = true; }
                            if ui.button("2x").clicked() { item.copies = 2; self.preview_dirty = true; }
                            if ui.button("3x").clicked() { item.copies = 3; self.preview_dirty = true; }
                            if ui.button("4x").clicked() { item.copies = 4; self.preview_dirty = true; }
                            if ui.button("6x").clicked() { item.copies = 6; self.preview_dirty = true; }
                            if ui.button("8x").clicked() { item.copies = 8; self.preview_dirty = true; }
                            if ui.button("16x").clicked() { item.copies = 16; self.preview_dirty = true; }
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("↻ 90° Rotate").clicked() {
                                should_rotate = true;
                            }
                            if ui.button("⇄ Flip Mirror").clicked() {
                                should_flip = true;
                            }
                            if selected_idx > 0 && ui.button("▲ Move Up").clicked() {
                                should_swap = Some((selected_idx, selected_idx - 1));
                            }
                            if selected_idx + 1 < self.items.len() && ui.button("▼ Move Down").clicked() {
                                should_swap = Some((selected_idx, selected_idx + 1));
                            }
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Remove Photo").color(Color32::from_rgb(240, 80, 80))).clicked() {
                                should_delete = true;
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Done").clicked() {
                                    should_close = true;
                                }
                            });
                        });
                    });

                if should_rotate {
                    self.items[selected_idx].image = self.items[selected_idx].image.rotate90();
                    self.preview_dirty = true;
                }

                if should_flip {
                    self.items[selected_idx].image = self.items[selected_idx].image.fliph();
                    self.preview_dirty = true;
                }

                if let Some((a, b)) = should_swap {
                    self.items.swap(a, b);
                    self.selected_item_idx = Some(b);
                    self.preview_dirty = true;
                }

                if should_delete {
                    self.items.remove(selected_idx);
                    self.selected_item_idx = None;
                    self.selected_popup_pos = None;
                    self.preview_dirty = true;
                } else if should_close {
                    self.selected_item_idx = None;
                    self.selected_popup_pos = None;
                }
            } else {
                self.selected_item_idx = None;
                self.selected_popup_pos = None;
            }
        }
    }
}

pub fn run_gui() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1220.0, 840.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Photo Grid Print - Sheet Generator & Direct Print"),
        ..Default::default()
    };
    eframe::run_native(
        "Photo Grid Print",
        native_options,
        Box::new(|cc| Ok(Box::new(PhotoGridApp::new(cc)))),
    )
}

fn dirs_desktop() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Desktop"))
}
