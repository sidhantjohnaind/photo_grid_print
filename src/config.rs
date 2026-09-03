use crate::grid::{ColorFilter, FitMode, PaperSize};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum UiTheme {
    #[default]
    CyberNeon,      // Neon cyan + deep midnight
    TokyoNight,     // Synthwave purple + hot pink
    ForestEmerald,  // Radiant emerald + dark pine
    SunsetAmber,    // Warm sunset amber + coral red
    DarkSlate,      // Classic obsidian studio slate
    StudioLight,    // Clean designer bright mode
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub paper_size: PaperSize,
    pub cols: usize,
    pub rows: usize,
    pub global_copies: usize,
    pub is_individual_copies: bool,
    pub gap: u32,
    pub margin: u32,
    pub is_borderless: bool,
    pub is_portrait: bool,
    pub fit_mode: FitMode,
    pub color_filter: ColorFilter,
    pub show_cut_marks: bool,
    pub output_path: Option<String>,
    pub last_folder: Option<String>,
    #[serde(default)]
    pub theme: UiTheme,
    #[serde(default = "default_fps_cap")]
    pub fps_cap: u32,
}

fn default_fps_cap() -> u32 {
    30
}

impl Default for AppConfig {
    fn default() -> Self {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_out = dirs_desktop()
            .map(|d| d.join(format!("Photo_Grid_Print_{}.pdf", timestamp)).to_string_lossy().to_string());

        Self {
            paper_size: PaperSize::A4,
            cols: 4,
            rows: 4,
            global_copies: 1,
            is_individual_copies: false,
            gap: 24,
            margin: 50,
            is_borderless: false,
            is_portrait: false,
            fit_mode: FitMode::Fill,
            color_filter: ColorFilter::Original,
            show_cut_marks: false,
            output_path: default_out,
            last_folder: None,
            theme: UiTheme::CyberNeon,
            fps_cap: 30,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));

        base.join("PhotoGridPrint").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut cfg = if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(c) = serde_json::from_str::<AppConfig>(&content) {
                    c
                } else {
                    Self::default()
                }
            } else {
                Self::default()
            }
        } else {
            Self::default()
        };

        // Always ensure a fresh timestamp date is present in the output filename
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let parent = cfg
            .output_path
            .as_ref()
            .and_then(|p| PathBuf::from(p).parent().map(|p| p.to_path_buf()))
            .or_else(dirs_desktop)
            .unwrap_or_else(|| PathBuf::from("."));

        cfg.output_path = Some(
            parent
                .join(format!("Photo_Grid_Print_{}.pdf", timestamp))
                .to_string_lossy()
                .to_string(),
        );
        cfg
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, json);
        }
    }
}

fn dirs_desktop() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Desktop"))
}
