# 📷 Photo Grid Print

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust%20%7C%20Kotlin-orange.svg" alt="Rust & Kotlin">
  <img src="https://img.shields.io/badge/GUI-egui%20%2F%20Jetpack%20Compose-blue.svg" alt="egui & Jetpack Compose">
  <img src="https://img.shields.io/badge/Resolution-300%20DPI%20Print-brightgreen.svg" alt="300 DPI">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Android%20%7C%20Linux%20%7C%20macOS-informational.svg" alt="Platforms">
  <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="MIT License">
</p>

A blazing-fast, standalone desktop application and **native Android mobile app** that arranges photos into multi-page print-ready grid sheets in **300 DPI PDF format** with **Multi-Sheet Live Preview**, **Interactive Photo Cell Editing**, **Passport / ID Presets**, **Color Filters & B&W**, **Page Margins & Gaps**, **6 Color Themes**, and **Direct Printing**.

---

## ⚡ Highlights & Key Features

* **🖥️ Standalone GUI (Pure Windows Subsystem)**:
  * Opens cleanly and instantly without background CMD or console windows.
* **📄 Multi-Sheet Live Preview with Smooth Scrolling**:
  * **Scroll All Sheets**: View all pages in your print job (`Sheet #1 of N`, `Sheet #2 of N`, etc.) with mouse wheel scrolling.
  * **Single Page Mode**: Flip through pages with `◀ Prev` and `Next ▶` buttons.
* **🖱️ Interactive & Clickable Photo Cells**:
  * Hover over any photo across all sheets to inspect its index and details.
  * Click any photo to open the quick-edit popup:
    * Adjust copies (`[-]`, `[+]`, `1x`–`16x`)
    * **`↻ Rotate 90°`** (correct sideways camera shots)
    * **`⇄ Flip Mirror`** (mirror orientation)
    * **`▲ Move Up` / `▼ Move Down`** (reorder print sequence)
    * Remove photo from batch
* **🎨 Color Tone Filters & Enhancements**:
  * **`Color`**: Original full color
  * **`Grayscale (B&W)`**: Crisp monochrome for document & ID photocopies
  * **`High Contrast`**: Enhanced punch for photos
* **📐 Passport & ID Photo Presets**:
  * **US Passport $2 \times 2\text{ in}$** ($51 \times 51\text{ mm}$)
  * **Passport $35 \times 45\text{ mm}$** (Schengen, India, UK)
  * **Stamp / ID $30 \times 40\text{ mm}$**
  * Standard presets: `16 (4x4)`, `9 (3x3)`, `8 (4x2)`, `6 (3x2)`, `6 (2x3)`, `4 (2x2)`
* **📐 Margins, Gaps & Trimmer Cut Guides**:
  * Outer page margin slider + `0px (No Margin)` button.
  * Inner photo gap slider + `0px (Touching)` button.
  * Optional **Trimmer / Cutting Corner Guides** for guillotine or scissor cutting.
* **⚙️ Persistent AppData Configuration**:
  * Automatically saves all your settings to `%APPDATA%\PhotoGridPrint\config.json`.
* **🖨️ Direct Print & Multi-Page PDF Export**:
  * **Print Now**: Direct printer routing via native Windows print runner.
  * **Save & Open PDF**: Generates 300 DPI multi-page PDF files.
* **📐 Multiple Paper Sizes**:
  * **A4**, **US Letter**, **US Legal**, **4 × 6 in Photo**, **5 × 7 in Photo**, **A3**, **A5**.

---

## 🚀 Quick Start

### 1. Download & Run
* **Desktop (Windows / Linux / macOS)**: Download the latest binary from [GitHub Releases](https://github.com/sidhantjohnaind/photo_grid_print/releases).
* **Android (Phone / Tablet)**: Download **`PhotoGridPrint-Android.apk`** from [GitHub Releases](https://github.com/sidhantjohnaind/photo_grid_print/releases) and install directly on your Android device!

### 2. Build Desktop from Source

```bash
# Clone repository
git clone https://github.com/sidhantjohnaind/photo_grid_print.git
cd photo_grid_print

# Build optimized release binary
cargo build --release

# Run
./target/release/photo_grid_print.exe
```

### 3. Build Android APK from Source

```bash
cd android
./gradlew assembleRelease
# Output APK: android/app/build/outputs/apk/release/app-release.apk
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

---

## 🛠️ Configuration File

Settings are saved automatically in:
* **Windows**: `%APPDATA%\PhotoGridPrint\config.json`
* **Linux / macOS**: `~/.config/PhotoGridPrint/config.json`

---

## 📜 License

Licensed under the [MIT License](LICENSE).
