use crate::config::{AppConfig, UiTheme};
use crate::grid::{
    render_all_preview_pages_with_copies, render_images_with_copies_to_pdf_pages, ColorFilter, FitMode, GridConfig, PaperSize,
};
use crate::pdf;
use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, CursorIcon, Frame, Margin, Pos2, Rect, RichText, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use image::DynamicImage;
use rayon::prelude::*;
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SidebarTab {
    #[default]
    Photos,
    Layout,
    Settings,
}

pub struct PhotoItem {
    pub path: PathBuf,
    pub image: DynamicImage,
    pub preview_cache: DynamicImage, // Downscaled 1200px cache for ultra-fast silky 60fps UI live preview
    pub copies: usize,
    pub thumbnail_texture: Option<TextureHandle>,
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
    pub sidebar_tab: SidebarTab,
    pub theme: UiTheme,

    // Multi-Page Live Preview State
    pub preview_textures: Vec<TextureHandle>,
    pub preview_page_idx: usize,
    pub total_pages: usize,
    pub preview_dirty: bool,
    pub view_mode: PreviewViewMode,

    // Clickable Individual Photo Modal / Popup
    pub selected_item_idx: Option<usize>,
    pub selected_popup_pos: Option<Pos2>,

    // Live View Interactive Drag & Drop Reordering State
    pub dragged_item_idx: Option<usize>,
    pub drag_start_pos: Option<Pos2>,
    pub is_actively_dragging: bool,
    pub drag_target_idx: Option<usize>,
}

impl Default for PhotoGridApp {
    fn default() -> Self {
        let cfg = AppConfig::load();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_out = cfg.output_path.clone().unwrap_or_else(|| {
            dirs_desktop().unwrap_or_else(|| PathBuf::from(".")).join(format!("Photo_Grid_Print_{}.pdf", timestamp)).to_string_lossy().to_string()
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
            sidebar_tab: SidebarTab::Photos,
            theme: cfg.theme,
            preview_textures: Vec::new(),
            preview_page_idx: 0,
            total_pages: 0,
            preview_dirty: true,
            view_mode: PreviewViewMode::AllPages,
            selected_item_idx: None,
            selected_popup_pos: None,
            dragged_item_idx: None,
            drag_start_pos: None,
            is_actively_dragging: false,
            drag_target_idx: None,
        }
    }
}

impl PhotoGridApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let app = Self::default();
        apply_modern_theme(&cc.egui_ctx, app.theme);

        let mut app = app;

        // 1. Try restoring the user's last used folder from config
        if let Some(last) = &app.last_folder {
            let p = PathBuf::from(last);
            if p.exists() {
                app.load_folder(&p, 16);
                return app;
            }
        }

        // 2. Check dynamic user Downloads or Pictures folder
        if let Some(dl) = dirs_downloads() {
            let browser = dl.join("Browser");
            if browser.exists() {
                app.load_folder(&browser, 16);
                return app;
            } else if dl.exists() {
                let has_images = WalkDir::new(&dl).max_depth(1).into_iter().filter_map(|e| e.ok()).any(|e| {
                    e.path().extension().and_then(|ext| ext.to_str()).map_or(false, |ext| {
                        ["jpg", "jpeg", "png", "webp", "bmp"].contains(&ext.to_lowercase().as_str())
                    })
                });
                if has_images {
                    app.load_folder(&dl, 16);
                    return app;
                }
            }
        }

        if let Some(pics) = dirs_pictures() {
            if pics.exists() {
                app.load_folder(&pics, 16);
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
            theme: self.theme,
        };
        cfg.save();
    }

    /// Parallel multi-core image decoding & downscaled preview caching
    fn add_paths(&mut self, paths: &[PathBuf]) {
        let copies = self.global_copies;
        let loaded: Vec<PhotoItem> = paths
            .par_iter()
            .filter_map(|p| {
                if let Ok(img) = image::open(p) {
                    let preview = if img.width() > 1200 || img.height() > 1200 {
                        img.thumbnail(1200, 1200)
                    } else {
                        img.clone()
                    };

                    Some(PhotoItem {
                        path: p.clone(),
                        image: img,
                        preview_cache: preview,
                        copies,
                        thumbnail_texture: None,
                    })
                } else {
                    None
                }
            })
            .collect();

        if !loaded.is_empty() {
            self.items.extend(loaded);
            self.preview_dirty = true;
        }
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

    fn prepare_preview_items(&self) -> Vec<(&DynamicImage, usize)> {
        self.items
            .iter()
            .map(|item| {
                let copies = match self.copies_mode {
                    CopiesMode::SameForAll => self.global_copies,
                    CopiesMode::Individual => item.copies,
                };
                (&item.preview_cache, copies)
            })
            .collect()
    }

    fn prepare_full_render_items(&self) -> Vec<(&DynamicImage, usize)> {
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

        let render_items = self.prepare_preview_items();

        // Multithreaded parallel rendering across all CPU cores
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

        let render_items = self.prepare_full_render_items();
        let total_photos: usize = render_items.iter().map(|(_, c)| *c).sum();
        let config = self.current_config();

        match render_images_with_copies_to_pdf_pages(&render_items, &config) {
            Ok(pdf_pages) => {
                let (paper_w_mm, paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);
                let page_w_pt = paper_w_mm / 25.4 * 72.0;
                let page_h_pt = paper_h_mm / 25.4 * 72.0;

                let pdf_bytes = pdf::create_pdf(&pdf_pages, page_w_pt, page_h_pt);

                // Add timestamp date at end so multiple PDF files exist without overwriting
                let raw_out = PathBuf::from(&self.output_path);
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                let out = if raw_out.exists() || raw_out.file_name().and_then(|f| f.to_str()) == Some("Photo_Grid_Print.pdf") {
                    let parent = raw_out.parent().unwrap_or_else(|| Path::new("."));
                    let stem = raw_out.file_stem().and_then(|s| s.to_str()).unwrap_or("Photo_Grid_Print");
                    let base_stem = if let Some(idx) = stem.rfind('_') {
                        if stem[idx + 1..].chars().all(|c| c.is_ascii_digit()) && stem.len() - idx > 6 {
                            &stem[..idx]
                        } else {
                            stem
                        }
                    } else {
                        stem
                    };
                    let new_path = parent.join(format!("{}_{}.pdf", base_stem, timestamp));
                    self.output_path = new_path.to_string_lossy().to_string();
                    new_path
                } else {
                    raw_out
                };

                if let Some(parent) = out.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                match fs::write(&out, &pdf_bytes) {
                    Ok(_) => {
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

/// 100% Dynamic Cross-Platform Direct Printing handler
pub fn send_to_printer(pdf_path: &Path) {
    let pdf_str = pdf_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        let candidates = [
            "msedge",
            "chrome",
            "brave",
            "SumatraPDF",
            "AcroRd32",
            "FoxitPDFReader",
        ];

        for cmd in &candidates {
            if let Ok(child) = Command::new(cmd).args(["/p", &pdf_str]).spawn() {
                if child.id() > 0 {
                    return;
                }
            }
        }

        let mut search_dirs = Vec::new();
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            search_dirs.push(PathBuf::from(pf));
        }
        if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
            search_dirs.push(PathBuf::from(pf86));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            search_dirs.push(PathBuf::from(local));
        }

        let rel_paths = [
            r"Microsoft\Edge\Application\msedge.exe",
            r"Google\Chrome\Application\chrome.exe",
            r"BraveSoftware\Brave-Browser\Application\brave.exe",
            r"SumatraPDF\SumatraPDF.exe",
            r"Adobe\Acrobat Reader DC\Reader\AcroRd32.exe",
        ];

        for base in &search_dirs {
            for rel in &rel_paths {
                let candidate = base.join(rel);
                if candidate.exists() {
                    if let Ok(child) = Command::new(&candidate).args(["/p", &pdf_str]).spawn() {
                        if child.id() > 0 {
                            return;
                        }
                    }
                }
            }
        }

        let ps_res = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!("Start-Process -FilePath '{}' -Verb Print", pdf_str),
            ])
            .spawn();

        if ps_res.is_ok() {
            return;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if Command::new("lpr").arg(&pdf_str).spawn().is_ok() {
            return;
        }
        if Command::new("lp").arg(&pdf_str).spawn().is_ok() {
            return;
        }
    }

    let _ = open::that(pdf_path);
}

// -----------------------------------------------------------------------------
// Dynamic Themes & Color Engine
// -----------------------------------------------------------------------------

impl UiTheme {
    pub fn name(&self) -> &'static str {
        match self {
            UiTheme::CyberNeon => "Cyber Neon",
            UiTheme::TokyoNight => "Tokyo Purple",
            UiTheme::ForestEmerald => "Forest Emerald",
            UiTheme::SunsetAmber => "Sunset Amber",
            UiTheme::DarkSlate => "Dark Slate",
            UiTheme::StudioLight => "Studio Light",
        }
    }

    pub fn emoji(&self) -> &'static str {
        "●"
    }

    pub fn is_dark(&self) -> bool {
        *self != UiTheme::StudioLight
    }

    pub fn accent_color(&self) -> Color32 {
        match self {
            UiTheme::CyberNeon => Color32::from_rgb(6, 182, 212),     // Electric Cyan
            UiTheme::TokyoNight => Color32::from_rgb(168, 85, 247),   // Vibrant Purple
            UiTheme::ForestEmerald => Color32::from_rgb(16, 185, 129),// Neon Emerald
            UiTheme::SunsetAmber => Color32::from_rgb(249, 115, 22),  // Glowing Orange/Amber
            UiTheme::DarkSlate => Color32::from_rgb(59, 130, 246),    // Azure Blue
            UiTheme::StudioLight => Color32::from_rgb(37, 99, 235),   // Crisp Sapphire
        }
    }

    pub fn secondary_accent(&self) -> Color32 {
        match self {
            UiTheme::CyberNeon => Color32::from_rgb(59, 130, 246),
            UiTheme::TokyoNight => Color32::from_rgb(236, 72, 153),
            UiTheme::ForestEmerald => Color32::from_rgb(52, 211, 153),
            UiTheme::SunsetAmber => Color32::from_rgb(244, 63, 94),
            UiTheme::DarkSlate => Color32::from_rgb(96, 165, 250),
            UiTheme::StudioLight => Color32::from_rgb(14, 165, 233),
        }
    }

    pub fn panel_bg(&self) -> Color32 {
        match self {
            UiTheme::CyberNeon => Color32::from_rgb(11, 15, 25),      // Deep navy obsidian
            UiTheme::TokyoNight => Color32::from_rgb(19, 15, 30),     // Night violet
            UiTheme::ForestEmerald => Color32::from_rgb(12, 24, 21),  // Dark pine
            UiTheme::SunsetAmber => Color32::from_rgb(26, 18, 16),    // Dark burnt charcoal
            UiTheme::DarkSlate => Color32::from_rgb(16, 18, 24),      // Classic slate
            UiTheme::StudioLight => Color32::from_rgb(248, 250, 252), // Clean paper white
        }
    }

    pub fn canvas_bg(&self) -> Color32 {
        match self {
            UiTheme::CyberNeon => Color32::from_rgb(26, 32, 46),      // Balanced studio navy slate
            UiTheme::TokyoNight => Color32::from_rgb(32, 26, 48),     // Balanced studio violet
            UiTheme::ForestEmerald => Color32::from_rgb(24, 36, 32),  // Balanced studio pine
            UiTheme::SunsetAmber => Color32::from_rgb(36, 28, 26),    // Warm studio charcoal
            UiTheme::DarkSlate => Color32::from_rgb(28, 32, 42),      // Classic neutral studio slate
            UiTheme::StudioLight => Color32::from_rgb(228, 234, 242), // Clean paper desk
        }
    }


    pub fn card_bg(&self) -> Color32 {
        match self {
            UiTheme::CyberNeon => Color32::from_rgb(17, 24, 39),
            UiTheme::TokyoNight => Color32::from_rgb(29, 23, 46),
            UiTheme::ForestEmerald => Color32::from_rgb(19, 36, 32),
            UiTheme::SunsetAmber => Color32::from_rgb(38, 26, 24),
            UiTheme::DarkSlate => Color32::from_rgb(22, 25, 33),
            UiTheme::StudioLight => Color32::from_rgb(255, 255, 255),
        }
    }

    pub fn card_border(&self) -> Color32 {
        match self {
            UiTheme::CyberNeon => Color32::from_rgb(30, 58, 86),
            UiTheme::TokyoNight => Color32::from_rgb(60, 40, 95),
            UiTheme::ForestEmerald => Color32::from_rgb(30, 68, 56),
            UiTheme::SunsetAmber => Color32::from_rgb(72, 42, 36),
            UiTheme::DarkSlate => Color32::from_rgb(38, 43, 56),
            UiTheme::StudioLight => Color32::from_rgb(226, 232, 240),
        }
    }

    pub fn text_primary(&self) -> Color32 {
        if self.is_dark() {
            Color32::from_rgb(243, 244, 246)
        } else {
            Color32::from_rgb(15, 23, 42)
        }
    }

    pub fn text_muted(&self) -> Color32 {
        if self.is_dark() {
            Color32::from_rgb(148, 163, 184)
        } else {
            Color32::from_rgb(100, 116, 139)
        }
    }
}

pub fn modern_card(theme: UiTheme) -> Frame {
    Frame {
        inner_margin: Margin::same(12),
        corner_radius: CornerRadius::same(8),
        fill: theme.card_bg(),
        stroke: Stroke::new(1.0, theme.card_border()),
        ..Default::default()
    }
}

pub fn modern_card_highlight(theme: UiTheme) -> Frame {
    let highlight_border = theme.accent_color();
    Frame {
        inner_margin: Margin::same(12),
        corner_radius: CornerRadius::same(8),
        fill: theme.card_bg(),
        stroke: Stroke::new(1.5, highlight_border),
        ..Default::default()
    }
}

pub fn apply_modern_theme(ctx: &egui::Context, theme: UiTheme) {
    let mut visuals = if theme.is_dark() {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = theme.panel_bg();
    visuals.window_fill = theme.card_bg();
    visuals.extreme_bg_color = theme.canvas_bg();
    visuals.override_text_color = Some(theme.text_primary());

    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);
    visuals.widgets.open.corner_radius = CornerRadius::same(6);

    let inactive_bg = if theme.is_dark() {
        Color32::from_rgb(26, 31, 44)
    } else {
        Color32::from_rgb(241, 245, 249)
    };
    visuals.widgets.inactive.bg_fill = inactive_bg;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.card_border());

    visuals.widgets.hovered.bg_fill = theme.card_border();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.accent_color());

    visuals.widgets.active.bg_fill = theme.accent_color();
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, theme.secondary_accent());

    visuals.selection.bg_fill = theme.accent_color();
    visuals.selection.stroke = Stroke::new(1.0, theme.secondary_accent());

    ctx.set_visuals(visuals);
}

fn themed_tab_btn(ui: &mut egui::Ui, theme: UiTheme, selected: bool, text: &str, min_w: f32) -> egui::Response {
    let (bg, text_color, stroke) = if selected {
        (theme.accent_color(), Color32::WHITE, Stroke::new(1.0, theme.secondary_accent()))
    } else {
        (theme.panel_bg(), theme.text_muted(), Stroke::new(1.0, Color32::TRANSPARENT))
    };
    let btn = egui::Button::new(RichText::new(text).size(12.5).strong().color(text_color))
        .fill(bg)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(6))
        .min_size(egui::vec2(min_w, 32.0));
    ui.add(btn)
}

fn themed_chip_btn(ui: &mut egui::Ui, theme: UiTheme, active: bool, text: &str) -> egui::Response {
    let (bg, text_color, stroke) = if active {
        (theme.accent_color(), Color32::WHITE, Stroke::new(1.0, theme.secondary_accent()))
    } else {
        (theme.card_bg(), theme.text_muted(), Stroke::new(1.0, theme.card_border()))
    };
    let btn = egui::Button::new(RichText::new(text).size(11.5).strong().color(text_color))
        .fill(bg)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(5));
    ui.add(btn)
}

// -----------------------------------------------------------------------------
// App UI Loop
// -----------------------------------------------------------------------------

impl eframe::App for PhotoGridApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let theme = self.theme;

        // Drag and drop file handler from outside OS
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

        // ==========================================
        // TOP APP BAR: Vibrant Colorful Header
        // ==========================================
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.heading(
                RichText::new("PHOTO GRID PRINT")
                    .size(17.0)
                    .strong()
                    .color(theme.accent_color()),
            );

            let core_text = format!("⚡ {} Threads", rayon::current_num_threads());
            ui.label(
                RichText::new(core_text)
                    .size(11.0)
                    .color(theme.secondary_accent()),
            );

            let cfg = self.current_config();
            let summary_chip = format!(
                "{} • {}x{} Grid • 300 DPI",
                cfg.paper_size.name().split(' ').next().unwrap_or("A4"),
                cfg.cols,
                cfg.rows
            );
            ui.label(
                RichText::new(format!("|  {}", summary_chip))
                    .size(11.5)
                    .color(theme.text_muted()),
            );

            // Dynamic Theme Switcher - Clean & Sleek
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                if ui.button(RichText::new("⚙ Config").size(11.5)).clicked() {
                    let cfg_path = AppConfig::config_path();
                    let _ = open::that(cfg_path.parent().unwrap_or(&cfg_path));
                }

                ui.add_space(8.0);
                egui::ComboBox::from_id_salt("top_theme_select")
                    .selected_text(
                        RichText::new(format!("{} {}", self.theme.emoji(), self.theme.name()))
                            .size(12.0)
                            .strong()
                            .color(self.theme.accent_color()),
                    )
                    .show_ui(ui, |ui| {
                        for t in [
                            UiTheme::CyberNeon,
                            UiTheme::TokyoNight,
                            UiTheme::ForestEmerald,
                            UiTheme::SunsetAmber,
                            UiTheme::DarkSlate,
                            UiTheme::StudioLight,
                        ] {
                            let is_active = self.theme == t;
                            let text = RichText::new(format!("{} {}", t.emoji(), t.name()))
                                .size(12.0)
                                .strong()
                                .color(if is_active { t.accent_color() } else { theme.text_primary() });
                            if ui.selectable_label(is_active, text).clicked() {
                                self.theme = t;
                                apply_modern_theme(&ctx, self.theme);
                                self.save_config();
                            }
                        }
                    });
                ui.label(RichText::new("🎨 Theme:").size(12.0).color(theme.text_muted()));
            });
        });

        ui.add_space(3.0);
        ui.separator();
        ui.add_space(3.0);

        let total_avail = ui.available_size();
        let sidebar_width = (480.0_f32).min(total_avail.x * 0.48).max(420.0);

        ui.horizontal(|ui| {
            // ==========================================
            // LEFT SIDEBAR: Pinned Actions + Clean Tabs
            // ==========================================
            ui.allocate_ui_with_layout(
                egui::vec2(sidebar_width, total_avail.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let sidebar_avail = ui.available_size();
                    let bottom_reserved = if self.status_message.is_some() { 110.0 } else { 68.0 };
                    let scroll_height = (sidebar_avail.y - bottom_reserved).max(80.0);

                    // --- Tab Bar ---
                    Frame::default()
                        .fill(theme.canvas_bg())
                        .stroke(Stroke::new(1.0, theme.card_border()))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let tab_w = (sidebar_width - 32.0) / 3.0;

                                let photos_label = format!("Photos ({})", self.items.len());
                                if themed_tab_btn(ui, theme, self.sidebar_tab == SidebarTab::Photos, &photos_label, tab_w)
                                    .on_hover_text("Manage imported photos, copies, and order")
                                    .clicked()
                                {
                                    self.sidebar_tab = SidebarTab::Photos;
                                }

                                if themed_tab_btn(ui, theme, self.sidebar_tab == SidebarTab::Layout, "Layout & Grid", tab_w)
                                    .on_hover_text("Paper sizes, passport presets, grid rows & cols")
                                    .clicked()
                                {
                                    self.sidebar_tab = SidebarTab::Layout;
                                }

                                if themed_tab_btn(ui, theme, self.sidebar_tab == SidebarTab::Settings, "Themes & Style", tab_w)
                                    .on_hover_text("Themes, color palettes, margins, trimmer marks")
                                    .clicked()
                                {
                                    self.sidebar_tab = SidebarTab::Settings;
                                }
                            });
                        });

                    ui.add_space(6.0);

                    // --- Scrollable Tab Content ---
                    egui::ScrollArea::vertical()
                        .id_salt("sidebar_tab_scroll")
                        .max_height(scroll_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                            let content_w = sidebar_width - 18.0;

                            match self.sidebar_tab {
                                // -------------------------------------------------------------
                                // TAB 1: PHOTOS
                                // -------------------------------------------------------------
                                SidebarTab::Photos => {
                                    // 1. Quick Add / Import Card
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("Import Photos").strong().color(theme.text_primary()));
                                            ui.label(RichText::new("(or drag & drop files)").size(11.0).color(theme.text_muted()));
                                        });

                                        ui.add_space(4.0);
                                        ui.horizontal_wrapped(|ui| {
                                            if ui.add(egui::Button::new(RichText::new("+ Select Files...").strong().color(Color32::WHITE)).fill(theme.accent_color())).clicked() {
                                                if let Some(files) = rfd::FileDialog::new()
                                                    .add_filter("Images", &["jpg", "jpeg", "png", "webp", "bmp"])
                                                    .pick_files()
                                                {
                                                    self.add_paths(&files);
                                                }
                                            }

                                            if ui.button("📁 Folder...").clicked() {
                                                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                                    self.load_folder(&dir, 16);
                                                }
                                            }

                                            if let Some(dl) = dirs_downloads() {
                                                let browser = dl.join("Browser");
                                                if browser.exists() && ui.button("Browser (16)").clicked() {
                                                    self.load_folder(&browser, 16);
                                                } else if dl.exists() && ui.button("Downloads").clicked() {
                                                    self.load_folder(&dl, 16);
                                                }
                                            }

                                            if let Some(pics) = dirs_pictures() {
                                                if pics.exists() && ui.button("Pictures").clicked() {
                                                    self.load_folder(&pics, 16);
                                                }
                                            }

                                            if !self.items.is_empty() && ui.button(RichText::new("Clear All").color(Color32::from_rgb(248, 113, 113))).clicked() {
                                                self.items.clear();
                                                self.selected_item_idx = None;
                                                self.preview_dirty = true;
                                                self.status_message = None;
                                            }
                                        });
                                    });

                                    // 2. Copies Settings Card
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("Copies Mode:").strong().color(theme.text_primary()));

                                            let is_all = self.copies_mode == CopiesMode::SameForAll;
                                            let is_ind = self.copies_mode == CopiesMode::Individual;

                                            if themed_chip_btn(ui, theme, is_all, "Same for All").clicked() {
                                                self.copies_mode = CopiesMode::SameForAll;
                                                self.save_config();
                                                self.preview_dirty = true;
                                            }
                                            if themed_chip_btn(ui, theme, is_ind, "Individual per Photo").clicked() {
                                                self.copies_mode = CopiesMode::Individual;
                                                self.save_config();
                                                self.preview_dirty = true;
                                            }
                                        });

                                        ui.add_space(4.0);
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

                                                ui.add_space(4.0);
                                                for c in [1, 2, 3, 4, 6, 8, 16] {
                                                    if themed_chip_btn(ui, theme, self.global_copies == c, &format!("{}x", c)).clicked() {
                                                        self.global_copies = c;
                                                    }
                                                }
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
                                                ui.label("Set all to:");
                                                for c in [1, 2, 3, 4, 6, 8, 16] {
                                                    if ui.button(format!("{}x", c)).clicked() {
                                                        for item in &mut self.items {
                                                            item.copies = c;
                                                        }
                                                        self.preview_dirty = true;
                                                    }
                                                }
                                            });
                                        }
                                    });

                                    // 3. Photo List Cards
                                    if self.items.is_empty() {
                                        modern_card(theme).show(ui, |ui| {
                                            ui.set_width(content_w);
                                            ui.vertical_centered(|ui| {
                                                ui.add_space(30.0);
                                                ui.label(RichText::new("No photos loaded yet").size(14.0).color(theme.text_muted()));
                                                ui.label(RichText::new("Click '+ Select Files...' or drag & drop images here").size(11.5).color(theme.text_muted()));
                                                ui.add_space(30.0);
                                            });
                                        });
                                    } else {
                                        let mut to_remove = None;
                                        let mut to_rotate = None;
                                        let mut to_swap = None;
                                        let items_len = self.items.len();

                                        for (idx, item) in self.items.iter_mut().enumerate() {
                                            let is_selected = self.selected_item_idx == Some(idx);

                                            // Ensure thumbnail texture exists
                                            if item.thumbnail_texture.is_none() {
                                                let thumb = item.preview_cache.thumbnail(64, 64);
                                                let raw = thumb.to_rgb8().into_raw();
                                                let img = ColorImage::from_rgb([thumb.width() as usize, thumb.height() as usize], &raw);
                                                item.thumbnail_texture = Some(ctx.load_texture(format!("thumb_{}", idx), img, TextureOptions::LINEAR));
                                            }

                                            let frame = if is_selected { modern_card_highlight(theme) } else { modern_card(theme) };
                                            frame.show(ui, |ui| {
                                                ui.set_width(content_w);

                                                let usable_w = (content_w - 24.0).max(100.0);
                                                let right_controls_w = 172.0_f32;
                                                let left_info_w = (usable_w - right_controls_w).max(80.0);

                                                ui.horizontal(|ui| {
                                                    // 1. Thumbnail + Info Column
                                                    ui.allocate_ui_with_layout(
                                                        egui::vec2(left_info_w, 42.0),
                                                        egui::Layout::left_to_right(egui::Align::Center),
                                                        |ui| {
                                                            if let Some(tex) = &item.thumbnail_texture {
                                                                ui.image((tex.id(), egui::vec2(38.0, 38.0)));
                                                            }

                                                            ui.vertical(|ui| {
                                                                ui.horizontal(|ui| {
                                                                    let num_label = format!("#{}", idx + 1);
                                                                    ui.label(RichText::new(num_label).strong().color(theme.accent_color()));

                                                                    let name = item.path.file_name().unwrap_or_default().to_string_lossy();
                                                                    let text_limit_w = (left_info_w - 60.0).max(40.0);
                                                                    ui.add_sized(
                                                                        [text_limit_w, 16.0],
                                                                        egui::Label::new(RichText::new(name).size(12.0).strong().color(theme.text_primary())).truncate(),
                                                                    );
                                                                });

                                                                let dims = format!("{} × {} px", item.image.width(), item.image.height());
                                                                ui.label(RichText::new(dims).size(10.5).color(theme.text_muted()));
                                                            });
                                                        },
                                                    );

                                                    // 2. Action Controls Column (Fixed width, never overlaps text)
                                                    ui.allocate_ui_with_layout(
                                                        egui::vec2(right_controls_w, 42.0),
                                                        egui::Layout::right_to_left(egui::Align::Center),
                                                        |ui| {
                                                            if ui.small_button(RichText::new("X").color(Color32::from_rgb(248, 113, 113))).on_hover_text("Delete photo").clicked() {
                                                                to_remove = Some(idx);
                                                            }

                                                            if idx + 1 < items_len && ui.small_button("v").on_hover_text("Move Down").clicked() {
                                                                to_swap = Some((idx, idx + 1));
                                                            }
                                                            if idx > 0 && ui.small_button("^").on_hover_text("Move Up").clicked() {
                                                                to_swap = Some((idx, idx - 1));
                                                            }

                                                            if ui.small_button("Rot 90").on_hover_text("Rotate 90 degrees").clicked() {
                                                                to_rotate = Some(idx);
                                                            }

                                                            // Copies stepper
                                                            let prev_c = item.copies;
                                                            if ui.small_button("+").clicked() && item.copies < 100 {
                                                                item.copies += 1;
                                                            }
                                                            ui.add(egui::DragValue::new(&mut item.copies).range(1..=100));
                                                            if ui.small_button("-").clicked() && item.copies > 1 {
                                                                item.copies -= 1;
                                                            }

                                                            if item.copies != prev_c {
                                                                self.preview_dirty = true;
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        }

                                        if let Some(idx) = to_rotate {
                                            self.items[idx].image = self.items[idx].image.rotate90();
                                            self.items[idx].preview_cache = self.items[idx].preview_cache.rotate90();
                                            self.items[idx].thumbnail_texture = None;
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
                                    }
                                }

                                // -------------------------------------------------------------
                                // TAB 2: LAYOUT & GRID
                                // -------------------------------------------------------------
                                SidebarTab::Layout => {
                                    // Paper Size & Orientation
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.label(RichText::new("Paper Size & Orientation").strong().color(theme.text_primary()));
                                        ui.add_space(4.0);

                                        ui.horizontal(|ui| {
                                            ui.label("Paper:");
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

                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.label("Orientation:");
                                            let prev_port = self.is_portrait;
                                            if themed_chip_btn(ui, theme, !self.is_portrait, "Landscape").clicked() {
                                                self.is_portrait = false;
                                            }
                                            if themed_chip_btn(ui, theme, self.is_portrait, "Portrait").clicked() {
                                                self.is_portrait = true;
                                            }
                                            if self.is_portrait != prev_port {
                                                self.save_config();
                                                self.preview_dirty = true;
                                            }
                                        });
                                    });

                                    // Standard Grid Presets
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.label(RichText::new("Grid Presets").strong().color(theme.text_primary()));
                                        ui.add_space(4.0);

                                        let prev_cols = self.cols;
                                        let prev_rows = self.rows;

                                        ui.horizontal_wrapped(|ui| {
                                            if themed_chip_btn(ui, theme, self.cols == 4 && self.rows == 4, "16 (4x4)").clicked() { self.cols = 4; self.rows = 4; }
                                            if themed_chip_btn(ui, theme, self.cols == 3 && self.rows == 3, "9 (3x3)").clicked() { self.cols = 3; self.rows = 3; }
                                            if themed_chip_btn(ui, theme, self.cols == 4 && self.rows == 2, "8 (4x2)").clicked() { self.cols = 4; self.rows = 2; }
                                            if themed_chip_btn(ui, theme, self.cols == 3 && self.rows == 2, "6 (3x2)").clicked() { self.cols = 3; self.rows = 2; }
                                            if themed_chip_btn(ui, theme, self.cols == 2 && self.rows == 3, "6 (2x3)").clicked() { self.cols = 2; self.rows = 3; }
                                            if themed_chip_btn(ui, theme, self.cols == 2 && self.rows == 2, "4 (2x2)").clicked() { self.cols = 2; self.rows = 2; }
                                        });

                                        if self.cols != prev_cols || self.rows != prev_rows {
                                            self.save_config();
                                            self.preview_dirty = true;
                                        }

                                        ui.add_space(6.0);
                                        ui.label(RichText::new("ID / Passport Photo Sizes:").size(11.5).strong().color(theme.text_muted()));
                                        ui.add_space(2.0);

                                        let prev_cols = self.cols;
                                        let prev_rows = self.rows;

                                        ui.horizontal_wrapped(|ui| {
                                            if themed_chip_btn(ui, theme, self.cols == 3 && self.rows == 2, "US Passport 2x2\" (6)").clicked() {
                                                self.cols = 3; self.rows = 2; self.fit_mode = FitMode::Fill;
                                            }
                                            if themed_chip_btn(ui, theme, self.cols == 4 && self.rows == 2, "Passport 35x45mm (8)").clicked() {
                                                self.cols = 4; self.rows = 2; self.fit_mode = FitMode::Fill;
                                            }
                                            if themed_chip_btn(ui, theme, self.cols == 4 && self.rows == 3, "Stamp 30x40mm (12)").clicked() {
                                                self.cols = 4; self.rows = 3; self.fit_mode = FitMode::Fill;
                                            }
                                        });

                                        if self.cols != prev_cols || self.rows != prev_rows {
                                            self.save_config();
                                            self.preview_dirty = true;
                                        }
                                    });

                                    // Custom Columns & Rows
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.label(RichText::new("Custom Grid & Fit").strong().color(theme.text_primary()));
                                        ui.add_space(4.0);

                                        let prev_cols = self.cols;
                                        let prev_rows = self.rows;
                                        let prev_fit = self.fit_mode;

                                        ui.horizontal(|ui| {
                                            ui.label("Columns:");
                                            ui.add(egui::Slider::new(&mut self.cols, 1..=8));
                                            ui.label("Rows:");
                                            ui.add(egui::Slider::new(&mut self.rows, 1..=8));
                                        });

                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.label("Image Fit:");
                                            if themed_chip_btn(ui, theme, self.fit_mode == FitMode::Fill, "Fill (Crop to Cell)").clicked() {
                                                self.fit_mode = FitMode::Fill;
                                            }
                                            if themed_chip_btn(ui, theme, self.fit_mode == FitMode::Contain, "Fit (Preserve Full)").clicked() {
                                                self.fit_mode = FitMode::Contain;
                                            }
                                        });

                                        if self.cols != prev_cols || self.rows != prev_rows || self.fit_mode != prev_fit {
                                            self.save_config();
                                            self.preview_dirty = true;
                                        }
                                    });
                                }

                                // -------------------------------------------------------------
                                // TAB 3: THEMES & STYLE
                                // -------------------------------------------------------------
                                SidebarTab::Settings => {
                                    // Visual Theme Gallery Card
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("Color Themes").strong().color(theme.accent_color()));
                                            ui.label(RichText::new("(Instant preview)").size(11.0).color(theme.text_muted()));
                                        });
                                        ui.add_space(4.0);

                                        ui.horizontal_wrapped(|ui| {
                                            for t in [
                                                UiTheme::CyberNeon,
                                                UiTheme::TokyoNight,
                                                UiTheme::ForestEmerald,
                                                UiTheme::SunsetAmber,
                                                UiTheme::DarkSlate,
                                                UiTheme::StudioLight,
                                            ] {
                                                let is_active = self.theme == t;
                                                let (bg, text_c, stroke) = if is_active {
                                                    (t.accent_color(), Color32::WHITE, Stroke::new(1.5, t.secondary_accent()))
                                                } else {
                                                    (theme.card_bg(), t.accent_color(), Stroke::new(1.0, theme.card_border()))
                                                };

                                                let btn = egui::Button::new(RichText::new(format!("{} {}", t.emoji(), t.name())).size(12.0).strong().color(text_c))
                                                    .fill(bg)
                                                    .stroke(stroke)
                                                    .corner_radius(CornerRadius::same(6))
                                                    .min_size(egui::vec2(130.0, 32.0));

                                                if ui.add(btn).clicked() {
                                                    self.theme = t;
                                                    apply_modern_theme(&ctx, self.theme);
                                                    self.save_config();
                                                }
                                            }
                                        });
                                    });

                                    // Spacing & Margins
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.label(RichText::new("Spacing & Bleed").strong().color(theme.text_primary()));
                                        ui.add_space(4.0);

                                        ui.horizontal(|ui| {
                                            let prev_b = self.is_borderless;
                                            if themed_chip_btn(ui, theme, !self.is_borderless, "Spaced Margins").clicked() {
                                                self.is_borderless = false;
                                            }
                                            if themed_chip_btn(ui, theme, self.is_borderless, "100% Full-Bleed").clicked() {
                                                self.is_borderless = true;
                                            }
                                            if self.is_borderless != prev_b {
                                                self.save_config();
                                                self.preview_dirty = true;
                                            }
                                        });

                                        if !self.is_borderless {
                                            ui.add_space(4.0);
                                            ui.horizontal(|ui| {
                                                let prev_margin = self.margin;
                                                ui.label("Margin:");
                                                ui.add(egui::Slider::new(&mut self.margin, 0..=150).suffix(" px"));
                                                if ui.small_button("0px").clicked() { self.margin = 0; }
                                                if ui.small_button("50px").clicked() { self.margin = 50; }

                                                if self.margin != prev_margin {
                                                    self.save_config();
                                                    self.preview_dirty = true;
                                                }
                                            });

                                            ui.horizontal(|ui| {
                                                let prev_gap = self.gap;
                                                ui.label("Photo Gap:");
                                                ui.add(egui::Slider::new(&mut self.gap, 0..=60).suffix(" px"));
                                                if ui.small_button("0px").clicked() { self.gap = 0; }
                                                if ui.small_button("24px").clicked() { self.gap = 24; }

                                                if self.gap != prev_gap {
                                                    self.save_config();
                                                    self.preview_dirty = true;
                                                }
                                            });

                                            ui.add_space(2.0);
                                            let prev_cut = self.show_cut_marks;
                                            ui.checkbox(&mut self.show_cut_marks, "Trimmer / Cutting Corner Guides");
                                            if self.show_cut_marks != prev_cut {
                                                self.save_config();
                                                self.preview_dirty = true;
                                            }
                                        }
                                    });

                                    // Color Tone Filters
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.label(RichText::new("Color Filter").strong().color(theme.text_primary()));
                                        ui.add_space(4.0);

                                        let prev_filter = self.color_filter;
                                        ui.horizontal(|ui| {
                                            if themed_chip_btn(ui, theme, self.color_filter == ColorFilter::Original, "Color").clicked() {
                                                self.color_filter = ColorFilter::Original;
                                            }
                                            if themed_chip_btn(ui, theme, self.color_filter == ColorFilter::Grayscale, "Grayscale (B&W)").clicked() {
                                                self.color_filter = ColorFilter::Grayscale;
                                            }
                                            if themed_chip_btn(ui, theme, self.color_filter == ColorFilter::HighContrast, "High Contrast").clicked() {
                                                self.color_filter = ColorFilter::HighContrast;
                                            }
                                        });

                                        if self.color_filter != prev_filter {
                                            self.save_config();
                                            self.preview_dirty = true;
                                        }
                                    });

                                    // Output Save Path
                                    modern_card(theme).show(ui, |ui| {
                                        ui.set_width(content_w);
                                        ui.label(RichText::new("Output Destination").strong().color(theme.text_primary()));
                                        ui.add_space(4.0);

                                        ui.horizontal(|ui| {
                                            let prev_out = self.output_path.clone();
                                            ui.text_edit_singleline(&mut self.output_path);

                                            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                                            if ui.button("Browse...").clicked() {
                                                let default_name = format!("Photo_Grid_Print_{}.pdf", timestamp);
                                                if let Some(dest) = rfd::FileDialog::new()
                                                    .set_file_name(&default_name)
                                                    .add_filter("PDF Document", &["pdf"])
                                                    .save_file()
                                                {
                                                    self.output_path = dest.to_string_lossy().to_string();
                                                }
                                            }

                                            if ui.button("New Timestamp").on_hover_text("Refresh to current date and time").clicked() {
                                                let p = PathBuf::from(&self.output_path);
                                                let parent = p.parent().unwrap_or_else(|| std::path::Path::new("."));
                                                self.output_path = parent.join(format!("Photo_Grid_Print_{}.pdf", timestamp)).to_string_lossy().to_string();
                                            }

                                            if self.output_path != prev_out {
                                                self.save_config();
                                            }
                                        });
                                    });
                                }
                            }
                        });

                    // ==============================================================
                    // PINNED BOTTOM ACTION BAR (Always visible in sidebar!)
                    // ==============================================================
                    ui.separator();
                    ui.add_space(2.0);

                    let btn_w = (sidebar_width - 24.0) / 2.0;
                    ui.horizontal(|ui| {
                        let print_btn = egui::Button::new(
                            RichText::new("Print Now")
                                .size(14.5)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(theme.accent_color())
                        .stroke(Stroke::new(1.5, theme.secondary_accent()))
                        .corner_radius(CornerRadius::same(8))
                        .min_size(egui::vec2(btn_w, 42.0));

                        if ui.add(print_btn).clicked() {
                            self.generate_and_handle_pdf(true);
                        }

                        let pdf_btn = egui::Button::new(
                            RichText::new("Save & Open PDF")
                                .size(14.5)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(45, 54, 74))
                        .stroke(Stroke::new(1.5, Color32::from_rgb(80, 96, 130)))
                        .corner_radius(CornerRadius::same(8))
                        .min_size(egui::vec2(btn_w, 42.0));

                        if ui.add(pdf_btn).clicked() {
                            self.generate_and_handle_pdf(false);
                        }
                    });

                    if let Some((msg, is_err)) = &self.status_message {
                        ui.add_space(3.0);
                        let (bg, stroke, text_c) = if *is_err {
                            (Color32::from_rgba_premultiplied(239, 68, 68, 35), Color32::from_rgb(239, 68, 68), Color32::from_rgb(252, 165, 165))
                        } else {
                            (Color32::from_rgba_premultiplied(34, 197, 94, 35), Color32::from_rgb(34, 197, 94), Color32::from_rgb(134, 239, 172))
                        };

                        Frame::default()
                            .fill(bg)
                            .stroke(Stroke::new(1.0, stroke))
                            .corner_radius(CornerRadius::same(5))
                            .inner_margin(Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.set_width(sidebar_width - 24.0);
                                ui.label(RichText::new(msg).size(11.5).strong().color(text_c));
                            });
                    }
                },
            );

            ui.separator();

            // =========================================================================
            // RIGHT PANEL: Expanded Multi-Page Interactive Live Sheet Preview
            // =========================================================================
            Frame::default()
                .fill(theme.canvas_bg())
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        ui.available_size(),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                    let per_page = self.cols * self.rows;
                    let total_photos: usize = self.prepare_preview_items().iter().map(|(_, c)| *c).sum();
                    let config = self.current_config();
                    let (cell_w_mm, cell_h_mm) = config.cell_dimensions_mm();
                    let cell_w_in = cell_w_mm / 25.4;
                    let cell_h_in = cell_h_mm / 25.4;

                    // Multi-page header bar ribbon
                    Frame::default()
                        .fill(theme.card_bg())
                        .stroke(Stroke::new(1.0, theme.card_border()))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Sheet Preview")
                                        .size(13.5)
                                        .strong()
                                        .color(theme.text_primary()),
                                );

                                ui.separator();

                                let dim_text = format!("{:.1} × {:.1} mm ({:.2} × {:.2} in)", cell_w_mm, cell_h_mm, cell_w_in, cell_h_in);
                                ui.label(RichText::new("Cell:").size(11.5).color(theme.text_muted()));
                                ui.label(RichText::new(dim_text).size(11.5).strong().color(theme.accent_color()));

                                ui.separator();

                                ui.label(
                                    RichText::new(format!("{} photos ({} / sheet)", total_photos, per_page))
                                        .size(11.5)
                                        .color(theme.text_muted()),
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // View Mode Toggle Buttons
                                    let is_all = self.view_mode == PreviewViewMode::AllPages;
                                    let is_single = self.view_mode == PreviewViewMode::SinglePage;

                                    if themed_chip_btn(ui, theme, is_all, "All Sheets").clicked() {
                                        self.view_mode = PreviewViewMode::AllPages;
                                    }

                                    if themed_chip_btn(ui, theme, is_single, "Single Page").clicked() {
                                        self.view_mode = PreviewViewMode::SinglePage;
                                    }

                                    // Paging controls in Single Page mode
                                    if self.view_mode == PreviewViewMode::SinglePage && self.total_pages > 1 {
                                        ui.separator();
                                        if ui.button("Next >").clicked() {
                                            if self.preview_page_idx + 1 < self.total_pages {
                                                self.preview_page_idx += 1;
                                            }
                                        }

                                        ui.label(
                                            RichText::new(format!("{}/{}", self.preview_page_idx + 1, self.total_pages))
                                                .strong()
                                                .color(theme.accent_color()),
                                        );

                                        if ui.button("< Prev").clicked() {
                                            if self.preview_page_idx > 0 {
                                                self.preview_page_idx -= 1;
                                            }
                                        }
                                    }
                                });
                            });
                        });

                    ui.add_space(4.0);

                    if self.preview_textures.is_empty() {
                        let (paper_w_mm, paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);
                        let paper_aspect = (paper_w_mm / paper_h_mm) as f32;

                        let avail_space = ui.available_size();
                        let max_sheet_w = (avail_space.x - 32.0).clamp(240.0, 780.0);
                        let max_sheet_h = (avail_space.y - 48.0).clamp(200.0, 850.0);

                        let (sheet_w, sheet_h) = if max_sheet_w / max_sheet_h > paper_aspect {
                            (max_sheet_h * paper_aspect, max_sheet_h)
                        } else {
                            (max_sheet_w, max_sheet_w / paper_aspect)
                        };

                        ui.add_space(20.0);
                        let (rect, _response) = ui.allocate_exact_size(egui::vec2(sheet_w, sheet_h), egui::Sense::hover());
                        let painter = ui.painter_at(rect);

                        // Soft realistic drop shadows
                        let shadow_alpha = if theme.is_dark() { 90 } else { 35 };
                        painter.rect_filled(rect.expand(6.0), 8.0, Color32::from_black_alpha(shadow_alpha));

                        // Crisp paper surface
                        painter.rect_filled(rect, 6.0, Color32::WHITE);
                        painter.rect_stroke(rect, 6.0, Stroke::new(1.0, Color32::from_gray(215)), StrokeKind::Outside);

                        // Draw live grid cells on the blank sheet
                        let cols = config.cols.max(1) as f32;
                        let rows = config.rows.max(1) as f32;
                        let full_dpi_w = paper_w_mm / 25.4 * 300.0;
                        let scale = sheet_w as f64 / full_dpi_w;
                        let margin_x = if self.is_borderless { 0.0 } else { (config.margin_x as f64 * scale) as f32 };
                        let margin_y = if self.is_borderless { 0.0 } else { (config.margin_y as f64 * scale) as f32 };
                        let gap = if self.is_borderless { 0.0 } else { (config.gap as f64 * scale).max(0.0) as f32 };

                        let grid_avail_w = sheet_w - 2.0 * margin_x;
                        let grid_avail_h = sheet_h - 2.0 * margin_y;
                        let cell_w = (grid_avail_w - (cols - 1.0) * gap) / cols;
                        let cell_h = (grid_avail_h - (rows - 1.0) * gap) / rows;

                        let cell_bg = Color32::from_rgb(248, 250, 252);
                        let cell_stroke = Stroke::new(1.0, Color32::from_gray(225));

                        for r in 0..config.rows {
                            for c in 0..config.cols {
                                let cx = rect.min.x + margin_x + c as f32 * (cell_w + gap);
                                let cy = rect.min.y + margin_y + r as f32 * (cell_h + gap);
                                let cell_rect = Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(cell_w, cell_h));

                                painter.rect_filled(cell_rect, 3.0, cell_bg);
                                painter.rect_stroke(cell_rect, 3.0, cell_stroke, StrokeKind::Inside);

                                let slot_num = r * config.cols + c + 1;
                                painter.text(
                                    cell_rect.center(),
                                    Align2::CENTER_CENTER,
                                    format!("#{}", slot_num),
                                    egui::FontId::proportional(13.0),
                                    Color32::from_gray(175),
                                );
                            }
                        }

                        // Center hero call-to-action floating card
                        let card_w = (sheet_w * 0.62).clamp(260.0, 360.0);
                        let card_h = 125.0;
                        let cta_rect = Rect::from_center_size(rect.center(), Vec2::new(card_w, card_h));

                        painter.rect_filled(cta_rect.expand(4.0), 12.0, Color32::from_black_alpha(100));
                        painter.rect_filled(cta_rect, 10.0, theme.card_bg());
                        painter.rect_stroke(cta_rect, 10.0, Stroke::new(1.5, theme.accent_color()), StrokeKind::Outside);

                        let mut cta_ui = ui.new_child(egui::UiBuilder::new().max_rect(cta_rect));
                        cta_ui.vertical_centered(|ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new("📄 Ready to Print").size(15.0).strong().color(theme.text_primary()));
                            ui.label(RichText::new(format!("{} • {}x{} Grid ({} photos/sheet)", config.paper_size.name(), config.cols, config.rows, config.cols * config.rows)).size(11.0).color(theme.text_muted()));
                            ui.add_space(8.0);

                            let btn = egui::Button::new(RichText::new("+ Select Photos to Start").size(13.0).strong().color(Color32::WHITE))
                                .fill(theme.accent_color())
                                .stroke(Stroke::new(1.0, theme.secondary_accent()))
                                .corner_radius(CornerRadius::same(6))
                                .min_size(Vec2::new(190.0, 32.0));

                            if ui.add(btn).clicked() {
                                if let Some(files) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
                                    .pick_files()
                                {
                                    self.add_paths(&files);
                                }
                            }
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
                    let (paper_w_mm, _paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);
                    let full_dpi_w = paper_w_mm / 25.4 * 300.0;

                    let avail_space = ui.available_size();
                    let preview_area_w = (avail_space.x - 24.0).max(200.0);
                    let img_aspect = self.preview_textures[0].aspect_ratio();

                    let sheet_w = preview_area_w.min(780.0);
                    let sheet_h = sheet_w / img_aspect;

                    let mut clicked_cell: Option<(usize, Pos2)> = None;
                    let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
                    let is_mouse_down = ctx.input(|i| i.pointer.primary_down());
                    let is_mouse_clicked = ctx.input(|i| i.pointer.primary_clicked());
                    let is_mouse_released = ctx.input(|i| i.pointer.any_released());

                    // Check if drag threshold exceeded
                    if let (Some(origin), Some(curr_pos)) = (self.drag_start_pos, mouse_pos) {
                        if is_mouse_down && origin.distance(curr_pos) > 6.0 {
                            self.is_actively_dragging = true;
                        }
                    }

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

                                ui.add_space(8.0);
                                if self.total_pages > 1 && self.view_mode == PreviewViewMode::AllPages {
                                    let avail_w = ui.available_width();
                                    let left_pad = ((avail_w - sheet_w) / 2.0).max(8.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(left_pad);
                                        Frame::default()
                                            .fill(theme.card_bg())
                                            .stroke(Stroke::new(1.0, theme.card_border()))
                                            .corner_radius(CornerRadius::same(6))
                                            .inner_margin(Margin::symmetric(10, 4))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(format!("Sheet {} of {}", p_idx + 1, self.total_pages))
                                                        .size(12.0)
                                                        .strong()
                                                        .color(theme.accent_color()),
                                                );
                                            });
                                    });
                                    ui.add_space(4.0);
                                }

                                let texture = &self.preview_textures[p_idx];

                                let img_resp = ui.add(
                                    egui::Image::new(texture)
                                        .fit_to_exact_size(egui::vec2(sheet_w, sheet_h))
                                        .corner_radius(6),
                                );
                                let sheet_rect = img_resp.rect;

                                let painter = ui.painter();
                                let shadow_rect = sheet_rect.expand(4.0);
                                let shadow_alpha = if theme.is_dark() { 110 } else { 40 };
                                painter.rect_filled(shadow_rect, 8.0, Color32::from_black_alpha(shadow_alpha));
                                painter.rect_stroke(sheet_rect, 6.0, Stroke::new(1.0, theme.card_border()), StrokeKind::Outside);

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
                                    let is_dragged = self.is_actively_dragging && self.dragged_item_idx == Some(item_idx);
                                    let is_drop_target = self.is_actively_dragging && self.drag_target_idx == Some(item_idx);

                                    if self.is_actively_dragging {
                                        if is_hovered {
                                            self.drag_target_idx = Some(item_idx);
                                        }

                                        if is_dragged {
                                            painter.rect_filled(cell_rect, 4.0, Color32::from_black_alpha(160));
                                            painter.rect_stroke(cell_rect, 4.0, Stroke::new(2.0, Color32::from_rgb(100, 110, 130)), StrokeKind::Inside);
                                        } else if is_drop_target {
                                            painter.rect_filled(cell_rect, 4.0, Color32::from_rgba_premultiplied(theme.accent_color().r(), theme.accent_color().g(), theme.accent_color().b(), 70));
                                            painter.rect_stroke(cell_rect, 4.0, Stroke::new(3.0, theme.accent_color()), StrokeKind::Inside);

                                            let badge = format!("Drop to move #{} here", self.dragged_item_idx.unwrap_or(0) + 1);
                                            let badge_rect = Rect::from_min_size(cell_rect.min + Vec2::new(4.0, 4.0), Vec2::new(140.0, 20.0));
                                            painter.rect_filled(badge_rect, 4.0, Color32::from_black_alpha(230));
                                            painter.text(badge_rect.center(), Align2::CENTER_CENTER, badge, egui::FontId::proportional(10.0), theme.accent_color());
                                        }
                                    } else {
                                        if is_hovered {
                                            ctx.set_cursor_icon(CursorIcon::Grab);
                                            painter.rect_filled(cell_rect, 3.0, Color32::from_rgba_premultiplied(theme.accent_color().r(), theme.accent_color().g(), theme.accent_color().b(), 45));
                                            painter.rect_stroke(cell_rect, 3.0, Stroke::new(2.5, theme.accent_color()), StrokeKind::Inside);

                                            let badge_text = format!("#{}: Drag to move | Click to edit", item_idx + 1);
                                            let badge_rect = Rect::from_min_size(cell_rect.min + Vec2::new(4.0, 4.0), Vec2::new(165.0, 20.0));
                                            painter.rect_filled(badge_rect, 4.0, Color32::from_black_alpha(220));
                                            painter.text(badge_rect.center(), Align2::CENTER_CENTER, badge_text, egui::FontId::proportional(9.5), Color32::WHITE);
                                        } else if is_selected {
                                            painter.rect_stroke(cell_rect, 3.0, Stroke::new(2.5, theme.secondary_accent()), StrokeKind::Inside);
                                        }

                                        if is_hovered && is_mouse_down && self.dragged_item_idx.is_none() {
                                            self.dragged_item_idx = Some(item_idx);
                                            self.drag_start_pos = mouse_pos;
                                        }

                                        if is_hovered && is_mouse_clicked {
                                            clicked_cell = Some((item_idx, cell_rect.center_bottom()));
                                        }
                                    }
                                }

                                ui.add_space(20.0);
                            }
                        });

                    // Handle Drag & Drop Drop Release
                    if is_mouse_released {
                        if self.is_actively_dragging {
                            if let (Some(src), Some(dst)) = (self.dragged_item_idx, self.drag_target_idx) {
                                if src != dst && src < self.items.len() && dst < self.items.len() {
                                    let item = self.items.remove(src);
                                    self.items.insert(dst, item);
                                    self.selected_item_idx = Some(dst);
                                    self.preview_dirty = true;
                                    self.status_message = Some((
                                        format!("Reordered: moved photo #{} to position #{}.", src + 1, dst + 1),
                                        false,
                                    ));
                                }
                            }
                        }
                        self.dragged_item_idx = None;
                        self.drag_start_pos = None;
                        self.is_actively_dragging = false;
                        self.drag_target_idx = None;
                    }

                    // Floating cursor badge when dragging
                    if self.is_actively_dragging {
                        if let (Some(drag_idx), Some(cur_pos)) = (self.dragged_item_idx, mouse_pos) {
                            ctx.set_cursor_icon(CursorIcon::Grabbing);
                            let ghost_rect = Rect::from_center_size(cur_pos + Vec2::new(25.0, 25.0), Vec2::new(125.0, 30.0));
                            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("drag_ghost")));
                            painter.rect_filled(ghost_rect, 6.0, Color32::from_black_alpha(230));
                            painter.rect_stroke(ghost_rect, 6.0, Stroke::new(1.5, theme.accent_color()), StrokeKind::Outside);
                            
                            let label = format!("Moving Photo #{}", drag_idx + 1);
                            painter.text(ghost_rect.center(), Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), Color32::WHITE);
                        }
                    }

                    if let Some((item_idx, pos)) = clicked_cell {
                        if !self.is_actively_dragging {
                            self.selected_item_idx = Some(item_idx);
                            self.selected_popup_pos = Some(pos);
                            self.copies_mode = CopiesMode::Individual;
                        }
                    }
                },
            );
        });
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

                        ui.label(RichText::new(filename).strong().color(theme.text_primary()));
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
                            if ui.button("Rotate 90").clicked() {
                                should_rotate = true;
                            }
                            if ui.button("Flip Mirror").clicked() {
                                should_flip = true;
                            }
                            if selected_idx > 0 && ui.button("^ Move Up").clicked() {
                                should_swap = Some((selected_idx, selected_idx - 1));
                            }
                            if selected_idx + 1 < self.items.len() && ui.button("v Move Down").clicked() {
                                should_swap = Some((selected_idx, selected_idx + 1));
                            }
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Remove Photo").color(Color32::from_rgb(248, 113, 113))).clicked() {
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
                    self.items[selected_idx].preview_cache = self.items[selected_idx].preview_cache.rotate90();
                    self.items[selected_idx].thumbnail_texture = None;
                    self.preview_dirty = true;
                }

                if should_flip {
                    self.items[selected_idx].image = self.items[selected_idx].image.fliph();
                    self.items[selected_idx].preview_cache = self.items[selected_idx].preview_cache.fliph();
                    self.items[selected_idx].thumbnail_texture = None;
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
            .with_inner_size([1260.0, 860.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title("Photo Grid Print - Sheet Generator & Direct Print"),
        ..Default::default()
    };
    eframe::run_native(
        "Photo Grid Print",
        native_options,
        Box::new(|cc| Ok(Box::new(PhotoGridApp::new(cc)))),
    )
}

pub fn dirs_downloads() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|p| PathBuf::from(p).join("Downloads"))
}

pub fn dirs_pictures() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|p| PathBuf::from(p).join("Pictures"))
}

pub fn dirs_desktop() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|p| PathBuf::from(p).join("Desktop"))
}
