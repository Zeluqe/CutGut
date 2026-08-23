# ✂️ CutGut

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/Python-3.10%2B-brightgreen.svg)](https://www.python.org/)
[![FFmpeg](https://img.shields.io/badge/FFmpeg-Supported-red.svg)](https://ffmpeg.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows-lightgrey.svg)]()

> **High-precision video trimming and smart compression tool tailored for Discord (20 MB free tier, 50 MB / 500 MB Nitro), Messenger, and social uploads.**

CutGut provides frame-accurate video clipping, hardware-accelerated **NVIDIA NVENC** & **AMD AMF** GPU encoding, and 2-pass rate control to ensure your clips fit strictly under upload size caps without visual degradation.

---

## 🌟 Key Features

- 🎯 **Target File Size Presets**:
  - **Discord Free (20 MB - Default)**: Targets ~19.60 MB for guaranteed upload safety across decimal and binary limits.
  - **Legacy / Small (10 MB)**: Targets ~9.80 MB for quick sharing and strict email/app limits.
  - **Nitro Basic (50 MB)** & **Nitro (500 MB)**: High-bitrate options for longer gaming highlights.
  - **Custom Size (MB)**: Specify any arbitrary target size.
- ⚡ **GPU Hardware Acceleration (NVIDIA NVENC & AMD AMF)**:
  - Blazing-fast GPU encoding for NVIDIA GeForce / RTX cards (`h264_nvenc`) and AMD Radeon cards (`h264_amf`).
  - Automatic hardware detection with graceful fallback hierarchy: **NVENC** ➔ **AMF** ➔ **CPU**.
  - CPU fallbacks: Balanced H.264 2-pass (`libx264 slow`), Fast H.264 (`libx264 veryfast`), and Cinematic H.265 (`libx265`).
- 🎬 **Interactive Video Timeline & Player**:
  - Frame-accurate clipping (`Set Start` / `Set End`).
  - **Loop Preview**: Seamlessly loop playback between start and end markers to inspect cuts.
  - Full **Drag & Drop** support.
- 📉 **Intelligent Downscaling & FPS Adaptation**:
  - Automatically downscales to 720p (<900 kbps) or 480p/30fps (<450 kbps) for long clips to prevent compression artifacts.
- 🔄 **Automated Verification & Retry Protection**:
  - Automatically verifies final file size; applies an instant 1-pass correction if video exceeds target limits.
- 💻 **Dual Surface (GUI & CLI)**:
  - Rich **PyQt6 Desktop App** (`gui.py`).
  - Fast, scriptable **Command Line Interface** (`cli.py`).

---

## 🚀 Quick Start & Installation

### Prerequisites
- **Python 3.10+**
- **[FFmpeg](https://ffmpeg.org/)** (installed and added to PATH, or placed in the application folder).

### 1. Clone the repository
```bash
git clone https://github.com/Zeluqe/CutGut.git
cd CutGut
```

### 2. Set up virtual environment & install dependencies
```bash
python -m venv venv
venv\Scripts\activate   # On Windows
pip install -r requirements.txt
```

### 3. Run Desktop GUI
```bash
python gui.py
```
*(Or double-click `Uruchom_CutGut.bat`)*

---

## 💻 CLI Usage

CutGut includes a standalone CLI tool for automated batch processing:

```bash
# Basic trim to Discord 20MB limit using NVIDIA GPU
python cli.py input.mp4 --start 10.0 --end 35.0 --target 20mb

# Specify custom size, CPU encoder, and output path
python cli.py gameplay.mp4 -s 0 -e 60 --target 10mb --encoder cpu --output clip.mp4
```

### Options:
- `-s, --start`: Start timestamp in seconds (default: `0.0`).
- `-e, --end`: End timestamp in seconds (default: full length).
- `-t, --target`: Target limit (`20mb`, `10mb`, `50mb`, `500mb`, or number in MB).
- `--encoder`: `nvenc` (default if GPU available), `nvenc_fast`, `cpu`, `cpu_fast`, `hevc`.
- `-o, --output`: Destination file path.

---

## 🧪 Running Unit Tests

To verify algorithmic calculations and downscaling rules:

```bash
python test_encoder.py
```

---

## 📄 License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
