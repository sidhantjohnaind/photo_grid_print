use crate::grid::{ColorFilter, FitMode, PaperSize};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_out = dirs_desktop()
            .map(|d| d.join("Photo_Grid_Print.pdf").to_string_lossy().to_string());

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
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
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
