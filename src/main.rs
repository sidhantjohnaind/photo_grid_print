#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod grid;
mod gui;
mod pdf;

use anyhow::{bail, Context, Result};
use clap::Parser;
use grid::{render_images_with_copies_to_pdf_pages, FitMode, GridConfig, PaperSize};
use image::{DynamicImage, GenericImageView};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(
    name = "photo_grid_print",
    author = "Antigravity",
    version = "1.0.0",
    about = "Lightning-fast Grid Photo Sheet & PDF Generator with Live Preview & Direct Print"
)]
struct Args {
    /// Input image file(s) or folder(s). If omitted, native GUI will launch.
    #[arg(short, long, num_args = 0..)]
    input: Vec<PathBuf>,

    /// Force interactive terminal CLI mode instead of GUI
    #[arg(long, default_value_t = false)]
    cli: bool,

    /// Output PDF path (defaults to Desktop/Photo_Grid_Print.pdf)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Paper size: a4, letter, legal, 4x6, 5x7, a3, a5 (default: a4)
    #[arg(long, default_value = "a4")]
    paper: String,

    /// Limit number of photos to process from input (e.g. 16 for top 16 latest)
    #[arg(short = 'l', long)]
    limit: Option<usize>,

    /// Number of columns in grid (default: 4)
    #[arg(short = 'c', long, default_value_t = 4)]
    cols: usize,

    /// Number of rows in grid (default: 4, giving 16 photos per page)
    #[arg(short = 'r', long, default_value_t = 4)]
    rows: usize,

    /// Number of copies per photo (e.g. 1 for unique, 28 for classroom batch)
    #[arg(short = 'n', long, default_value_t = 1)]
    copies: usize,

    /// Border width around each photo in pixels (default: 0 for clean gap)
    #[arg(short = 'b', long, default_value_t = 0)]
    border: u32,

    /// Gap between photos in pixels (default: 24)
    #[arg(short = 'g', long, default_value_t = 24)]
    gap: u32,

    /// Generate 100% borderless / edge-to-edge full-bleed sheet
    #[arg(long, default_value_t = false)]
    borderless: bool,

    /// Paper orientation: Portrait instead of Landscape
    #[arg(short = 'p', long, default_value_t = false)]
    portrait: bool,

    /// Fit mode: contain (no cropping, white padding) instead of fill (fill cell)
    #[arg(long, default_value_t = false)]
    contain: bool,

    /// Direct print to default printer
    #[arg(long, default_value_t = false)]
    print: bool,

    /// Do not automatically open PDF after generation
    #[arg(long, default_value_t = false)]
    no_open: bool,
}

fn parse_paper_size(s: &str) -> PaperSize {
    match s.to_lowercase().replace(['-', '_', ' '], "").as_str() {
        "letter" => PaperSize::Letter,
        "legal" => PaperSize::Legal,
        "4x6" | "photo4x6" | "4x6in" => PaperSize::Photo4x6,
        "5x7" | "photo5x7" | "5x7in" => PaperSize::Photo5x7,
        "a3" => PaperSize::A3,
        "a5" => PaperSize::A5,
        _ => PaperSize::A4,
    }
}

fn main() -> Result<()> {
    let mut args = Args::parse();

    // If no arguments given and not --cli, launch Native GUI!
    if args.input.is_empty() && !args.cli && std::env::args().len() <= 1 {
        if let Err(e) = gui::run_gui() {
            eprintln!("GUI Error: {}", e);
        }
        return Ok(());
    }

    if args.input.is_empty() {
        print_banner();
        if !run_interactive(&mut args)? {
            return Ok(());
        }
    }

    if args.borderless {
        args.border = 0;
        args.gap = 0;
    }

    let mut image_paths = collect_images(&args.input)?;
    if image_paths.is_empty() {
        bail!("No valid image files (JPG, PNG, WEBP, BMP) found in specified input path(s).");
    }

    if let Some(limit) = args.limit {
        if limit > 0 && limit < image_paths.len() {
            image_paths.truncate(limit);
        }
    }

    println!("Found {} image(s) to process.", image_paths.len());

    let mut images: Vec<DynamicImage> = Vec::new();
    for p in &image_paths {
        print!("Loading: {}... ", p.file_name().unwrap_or_default().to_string_lossy());
        io::stdout().flush().ok();
        match image::open(p) {
            Ok(img) => {
                println!("OK ({:?})", img.dimensions());
                images.push(img);
            }
            Err(e) => {
                println!("FAILED ({})", e);
            }
        }
    }

    if images.is_empty() {
        bail!("Could not load any images successfully.");
    }

    let paper_size = parse_paper_size(&args.paper);

    let config = GridConfig {
        paper_size,
        cols: args.cols,
        rows: args.rows,
        border_width: args.border,
        border_color: [60, 60, 60],
        gap: args.gap,
        margin_x: if args.borderless { 0 } else { 60 },
        margin_y: if args.borderless { 0 } else { 50 },
        is_portrait: args.portrait,
        fit_mode: if args.contain { FitMode::Contain } else { FitMode::Fill },
        show_cut_marks: false,
        dpi: 300,
    };

    let items: Vec<(&DynamicImage, usize)> = images.iter().map(|img| (img, args.copies)).collect();

    println!("\nRendering {} copies of {} image(s) at 300 DPI on {} ({}x{} grid = {} per page)...",
        args.copies,
        images.len(),
        config.paper_size.name(),
        config.cols,
        config.rows,
        config.cols * config.rows
    );

    let pdf_pages = render_images_with_copies_to_pdf_pages(&items, &config)?;
    println!("Generated {} page(s).", pdf_pages.len());

    let (paper_w_mm, paper_h_mm) = config.paper_size.dimensions_mm(config.is_portrait);
    let page_w_pt = paper_w_mm / 25.4 * 72.0;
    let page_h_pt = paper_h_mm / 25.4 * 72.0;

    let pdf_bytes = pdf::create_pdf(&pdf_pages, page_w_pt, page_h_pt);

    let output_path = match &args.output {
        Some(p) => p.clone(),
        None => {
            let desktop = dirs_desktop().unwrap_or_else(|| PathBuf::from("."));
            desktop.join("Photo_Grid_Print.pdf")
        }
    };

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).ok();
    }

    fs::write(&output_path, &pdf_bytes)
        .with_context(|| format!("Failed to write PDF to {}", output_path.display()))?;

    println!("\nSUCCESS: PDF saved to: {}", output_path.display());
    println!("Paper Size: {} ({}) | Total Photos: {}",
        config.paper_size.name(),
        if config.is_portrait { "Portrait" } else { "Landscape" },
        images.len() * args.copies
    );

    if args.print {
        println!("Sending to printer...");
        let _ = Command::new("powershell")
            .args(["-Command", &format!("Start-Process -FilePath '{}' -Verb Print", output_path.display())])
            .spawn();
    } else if !args.no_open {
        println!("Opening PDF...");
        let _ = open::that(&output_path);
    }

    if args.cli {
        println!("\nPress Enter to exit...");
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }

    Ok(())
}

fn print_banner() {
    println!("==========================================================");
    println!("   Photo Grid Print - High Resolution A4 Sheet Maker     ");
    println!("==========================================================");
}

fn run_interactive(args: &mut Args) -> Result<bool> {
    let default_folder = PathBuf::from(r"D:\Downloads\Browser");
    let initial_path = if default_folder.exists() {
        default_folder.to_string_lossy().to_string()
    } else {
        ".".to_string()
    };

    print!("Enter image file or folder path [default: {}]: ", initial_path);
    io::stdout().flush().ok();

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let trimmed = line.trim().trim_matches('"');

    let chosen_path = if trimmed.is_empty() {
        PathBuf::from(initial_path)
    } else {
        PathBuf::from(trimmed)
    };

    args.input = vec![chosen_path];

    print!("Number of latest photos to take (leave blank for all, or e.g. 16) [default: 16]: ");
    io::stdout().flush().ok();
    let mut limit_str = String::new();
    io::stdin().read_line(&mut limit_str)?;
    let trimmed_lim = limit_str.trim();
    if trimmed_lim.is_empty() {
        args.limit = Some(16);
    } else if let Ok(l) = trimmed_lim.parse::<usize>() {
        if l > 0 {
            args.limit = Some(l);
        }
    }

    print!("Copies per photo [default: 1]: ");
    io::stdout().flush().ok();
    let mut copies_str = String::new();
    io::stdin().read_line(&mut copies_str)?;
    if let Ok(c) = copies_str.trim().parse::<usize>() {
        if c > 0 {
            args.copies = c;
        }
    }

    print!("Layout style (1 = Spaced clean gap, 2 = 100% Borderless) [default: 1]: ");
    io::stdout().flush().ok();
    let mut style_str = String::new();
    io::stdin().read_line(&mut style_str)?;
    if style_str.trim() == "2" {
        args.borderless = true;
    }

    Ok(true)
}

fn collect_images(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut images = Vec::new();
    let valid_exts = ["jpg", "jpeg", "png", "webp", "bmp"];

    for input in inputs {
        if input.is_file() {
            if let Some(ext) = input.extension().and_then(|e| e.to_str()) {
                if valid_exts.contains(&ext.to_lowercase().as_str()) {
                    images.push(input.clone());
                }
            }
        } else if input.is_dir() {
            let mut dir_files = Vec::new();
            for entry in WalkDir::new(input).max_depth(1).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path().to_path_buf();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if valid_exts.contains(&ext.to_lowercase().as_str()) {
                            dir_files.push(path);
                        }
                    }
                }
            }
            dir_files.sort_by(|a, b| {
                let m_a = fs::metadata(a).and_then(|m| m.modified()).ok();
                let m_b = fs::metadata(b).and_then(|m| m.modified()).ok();
                m_b.cmp(&m_a) // Newest first
            });

            images.extend(dir_files);
        }
    }

    Ok(images)
}

fn dirs_desktop() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Desktop"))
}
