# 📷 Photo Grid Print

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/GUI-egui%20%2F%20eframe-blue.svg" alt="egui">
  <img src="https://img.shields.io/badge/Resolution-300%20DPI%20Print-brightgreen.svg" alt="300 DPI">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-informational.svg" alt="Platforms">
  <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="MIT License">
</p>

A blazing-fast, standalone desktop application and CLI built with **Rust** that arranges photos into multi-page print-ready grid sheets in **300 DPI PDF format** with **Multi-Sheet Live Preview**, **Interactive Photo Cell Editing**, **Page Margins & Gaps**, **Rotation**, and **Direct Printing**.

---

## ⚡ Highlights & Key Features

* **🖥️ Standalone GUI (Pure Windows Subsystem)**:
  * Opens instantly without background CMD or console windows.
* **📄 Multi-Sheet Live Preview**:
  * **Continuous Vertical Scroll**: View all pages in your print job (`Sheet #1 of N`, `Sheet #2 of N`, etc.) with mouse wheel scrolling.
  * **Single Page Mode**: Flip through pages with `◀ Prev` and `Next ▶` buttons.
* **🖱️ Interactive & Clickable Photo Cells**:
  * Hover over any photo across all sheets to inspect its details.
  * Click any photo to open the quick-edit popup: adjust copy counts (`[-]`, `[+]`, `1x`–`16x`), **Rotate 90°**, or remove it.
* **🔢 Grid Presets & 6-Photo Grids**:
  * Quick presets: **`6 (3x2)`**, **`6 (2x3)`**, **`16 (4x4)`**, **`9 (3x3)`**, **`8 (4x2)`**, **`4 (2x2)`**, plus custom sliders ($1$ to $8$).
* **📐 Margins, Gaps & Trimmer Cut Guides**:
  * Outer page margin slider + `0px (No Margin)` button.
  * Inner photo gap slider + `0px (Touching)` button.
  * Optional **Trimmer / Cutting Corner Guides** for scissors or paper guillotine alignment.
* **⚙️ Persistent AppData Configuration**:
  * Automatically saves user settings to `%APPDATA%\PhotoGridPrint\config.json`.
* **🖨️ Direct Print & Multi-Page PDF Export**:
  * **Print Now**: Direct printer routing via native Windows print runner.
  * **Save & Open PDF**: Generates sharp 300 DPI PDF files ready for high-grade photo printing.
* **📐 Multiple Paper Sizes**:
  * **A4**, **US Letter**, **US Legal**, **4 × 6 in Photo**, **5 × 7 in Photo**, **A3**, **A5**.

---

## 🚀 Quick Start

### 1. Download & Run
Download the latest `photo_grid_print.exe` from the [Releases](https://github.com/your-username/photo_grid_print/releases) page and double-click to launch!

### 2. Build from Source

```bash
# Clone repository
git clone https://github.com/your-username/photo_grid_print.git
cd photo_grid_print

# Build optimized release binary
cargo build --release

# Run
./target/release/photo_grid_print.exe
```

---

## 💻 CLI Usage

You can also run Photo Grid Print directly from the terminal or in scripts:

```bash
# Generate a 4x4 sheet from a folder of photos
photo_grid_print --input "C:\Photos\Album" --cols 4 --rows 4 --paper a4

# Generate 6-photo (3x2) passport/id batch with 2 copies each
photo_grid_print --input "C:\Photos\ID.jpg" --cols 3 --rows 2 --copies 2 --print
```

### CLI Arguments

| Argument | Description | Default |
|---|---|---|
| `-i, --input <PATHS>` | Input images or directory | Interactive / GUI |
| `-c, --cols <N>` | Number of columns | `4` |
| `-r, --rows <N>` | Number of rows | `4` |
| `-n, --copies <N>` | Copies per photo | `1` |
| `--paper <NAME>` | `a4`, `letter`, `legal`, `4x6`, `5x7`, `a3`, `a5` | `a4` |
| `-g, --gap <PX>` | Spacing gap between photos | `24` |
| `--borderless` | Full-bleed edge-to-edge layout | `false` |
| `-p, --portrait` | Portrait orientation | `false` (Landscape) |
| `--print` | Send directly to printer | `false` |

---

## 🛠️ Configuration File

Settings are saved automatically in:
* **Windows**: `%APPDATA%\PhotoGridPrint\config.json`
* **Linux / macOS**: `~/.config/PhotoGridPrint/config.json`

---

## 📜 License

Licensed under the [MIT License](LICENSE).
