# ✂️ CutGut

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/Python-3.10%2B-brightgreen.svg)](https://www.python.org/)
[![FFmpeg](https://img.shields.io/badge/FFmpeg-Supported-red.svg)](https://ffmpeg.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows-lightgrey.svg)]()

> **Smart, high-precision video trimming and 2-pass compression tool designed to hit exact file size limits (under 10 MB and 20 MB) for Discord, Messenger, Email, and social platforms.**

CutGut automatically calculates exact 2-pass video and audio bitrates using **H.264** or **H.265 (HEVC)** codecs, ensuring maximum visual clarity while keeping files safely below strict upload caps (e.g. **~9.95 MB** or **~19.95 MB** as reported by Windows File Explorer).

---

## 🌟 Key Features

- 🎯 **Target File Size Modes**:
  - **< 10 MB Limit (Default)**: Accurately targets **~9.95 MB** (perfect for Discord free tier and messaging apps).
  - **< 20 MB Limit**: Accurately targets **~19.95 MB** for longer clips and higher quality preservation.
- 🚀 **Three Encoding Presets**:
  - **Fast (H.264 Fast)**: High-speed export while respecting file limits.
  - **Balanced (H.264 Slow)**: Enhanced sharpness and motion stability.
  - **Cinematic (H.265 Ultra)**: Maximum compression efficiency with HEVC for high-motion gameplay clips.
- 🎬 **Interactive Video Timeline**:
  - Live video preview with playback controls and scrub bar.
  - Quick **Set Start** and **Set End** boundary markers.
  - Full **Drag & Drop** support (drop any video file directly into the app window).
- 🧹 **Automatic Workspace Cleanup**:
  - Auto-removes temporary multi-pass statistics and log files.
  - Optional prompt to free up storage by deleting the heavy raw input file after export.
  - Auto-opens the outputs/ folder upon completion.

---

## 🖥️ Available App Editions

| Edition | File | Description |
| :--- | :--- | :--- |
| **Desktop Pro** | [gui.py](gui.py) | Full-featured PyQt6 desktop application with embedded media player. |
| **Desktop Lite** | [gui_mini.py](gui_mini.py) | Ultra-lightweight edition (<12 MB) with auto-downloader for FFmpeg. |
| **Web Panel** | [pp.py](app.py) | Modern browser-based UI powered by Gradio. |

---

## 🚀 Quick Start & Installation

### Prerequisites
- **Python 3.10+**
- **[FFmpeg](https://ffmpeg.org/)** (installed and added to system PATH or placed in the application directory).

### 1. Clone the repository
`ash
git clone https://github.com/Zeluqe/CutGut.git
cd CutGut
`

### 2. Set up virtual environment & install dependencies
`ash
python -m venv venv
venv\Scripts\activate   # On Windows
pip install -r requirements.txt
`

### 3. Run the application
- **Desktop GUI (PyQt6)**:
  `ash
  python gui.py
  `
  *(Or double-click Uruchom_CutGut.bat)*

- **Lightweight Desktop GUI**:
  `ash
  python gui_mini.py
  `

- **Web Browser UI**:
  `ash
  python app.py
  `

---

## 📦 Building a Portable Standalone Executable

To generate a standalone .exe that does not require Python:

`ash
# Build Desktop Pro (PyQt6)
pyinstaller --noconfirm --onedir --windowed --name CutGut gui.py

# Build Ultra-Light Edition
pyinstaller --noconfirm --onefile --windowed --name CutGut_Lite gui_mini.py
`

---

## 📄 License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
