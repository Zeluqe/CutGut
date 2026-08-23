# ✂️ CutGut

[![Version](https://img.shields.io/badge/version-202608230--1--0-blue.svg)](https://github.com/Zeluqe/CutGut/releases/tag/202608230-1-0)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/Python-3.10%2B-brightgreen.svg)](https://www.python.org/)
[![FFmpeg](https://img.shields.io/badge/FFmpeg-Supported-red.svg)](https://ffmpeg.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows-lightgrey.svg)]()

[![Download CutGut.exe](https://img.shields.io/badge/Download-CutGut.exe%20(Build%20202608230--1--0)-2563eb?logo=windows&style=for-the-badge)](https://github.com/Zeluqe/CutGut/releases/download/202608230-1-0/CutGut.exe)

> **High-precision video trimming and smart compression tool tailored for Discord (20 MB free tier, 50 MB / 500 MB Nitro), Messenger, and social uploads.**

CutGut provides frame-accurate video clipping, instant lossless stream copy (remuxing), hardware-accelerated **NVIDIA NVENC** & **AMD AMF** GPU encoding, task queue batch processing, and 2-pass rate control.

---

## 🌟 Key Features

- ⚡ **Instant Lossless Remuxing (`can_stream_copy`)**:
  - If a clip or file already fits under the target limit, CutGut automatically copies video/audio streams directly (`-c copy`) in ~0.2 seconds without re-encoding!
- 📊 **Real-Time Plan Estimation**:
  - Live bitrate, output resolution, FPS, and ETA calculations displayed dynamically before encoding.
- ⌨️ **Keyboard Shortcuts & Timeline Handles**:
  - `I`: Set Start (In point) from playhead.
  - `O`: Set End (Out point) from playhead.
  - `Space`: Play / Pause playback.
  - `←` / `→`: Seek ±1 second (or `Shift + ←/→` for precise ±1 frame seeking).
- 📋 **Batch Processing & Job Queue (`EncodeJob`)**:
  - Queue multiple clips/ranges with sequential execution and per-task cancellation in GUI and CLI.
- 🌐 **Multi-Language (Polski 🇵🇱 / English 🇬🇧)**:
  - Instant on-the-fly UI language switching persisted across sessions.
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
  - Frame-accurate clipping with **Loop Preview** and **Drag & Drop** support.
- 📉 **Intelligent Downscaling & FPS Adaptation**:
  - Automatically downscales to 720p (<900 kbps) or 480p/30fps (<450 kbps) for long clips to prevent compression artifacts.
- 🔄 **Automated Verification & Retry Protection**:
  - Automatically verifies final file size; applies an instant 1-pass correction if video exceeds target limits.
- 💻 **Dual Surface (GUI & CLI)**:
  - Rich **PyQt6 Desktop App** (`gui.py`).
  - Fast, scriptable batch **Command Line Interface** (`cli.py`).

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
