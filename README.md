# ✂️ CutGut

[![Version](https://img.shields.io/badge/version-202608240--6--3-blue.svg)](https://github.com/Zeluqe/CutGut/releases/tag/202608240-6-3)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/Python-3.10%2B-brightgreen.svg)](https://www.python.org/)
[![FFmpeg](https://img.shields.io/badge/FFmpeg-Supported-red.svg)](https://ffmpeg.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows-lightgrey.svg)]()

[![Download CutGut.exe](https://img.shields.io/badge/Download-CutGut.exe%20(Build%20202608240--6--3)-2563eb?logo=windows&style=for-the-badge)](https://github.com/Zeluqe/CutGut/releases/download/202608240-6-3/CutGut.exe)

> **Modern Windows 11 Fluent UI video trimming, interactive social framing/cropping, and smart compression tool tailored for Discord (20 MB free tier, 50 MB / 500 MB Nitro), YouTube Shorts, TikTok, Reels, and social uploads.**

CutGut provides frame-accurate video clipping, Windows 11 Fluent UI aesthetic, rich interactive timeline with IN/OUT markers and range highlighting, hardware-accelerated video canvas with dimmed 9:16/1:1 crop overlays, 1-click lossless PNG frame capture, social export presets, real-time quality grading (bppf), dedicated side-by-side A/B video player comparison, 1-click in-app auto-updates, custom output directory management, safe source recycling policies, hardware-accelerated **NVIDIA NVENC** & **AMD AMF** GPU encoding, task queue batch processing, and 2-pass rate control.

---

## 🌟 Key Features

- 🎨 **Windows 11 / Fluent UI Redesign (Wave 6 - `202608240-6-0`)**:
  - Dark graphite card architecture (`#141416` / `#1c1c1f`) with subtle 1px borders and 8–10px rounded corners.
  - Native Windows 11 DWM immersive dark title bar and rounded window corner integration.
  - Clear visual hierarchy: **Export Configuration** and **Quality Assessment** split into balanced responsive columns.
  - Primary button accenting with distinct Windows Blue (`#0078d4`) styling.
- ⏱️ **Rich Interactive Fluent Timeline (`FluentTimelineWidget`)**:
  - Distinct time track with translucent Fluent blue selection range.
  - Visual green (IN) and red (OUT) markers with drag & drop adjustments.
  - Floating timestamp tooltip (`MM:SS.ms`) on hover and scrubbing.
  - Subtle floating toast notifications for instant user feedback.
- 📱 **Interactive Framing & Crop Presets (`CropOverlay` / `CropBox`)**:
  - **Vertical 9:16**: Instant conversion of gameplay/widescreen videos into YouTube Shorts, TikTok, and Instagram Reels with dark dimmed vignette outside the active frame.
  - **Square 1:1**: Crop to square for Discord, social posts, and memes.
  - **Horizontal 16:9 & Original**: Full widescreen preservation.
  - **Interactive Drag & Align**: Move the framing box directly with your mouse over the live video, or use 1-click alignment buttons (`[ ◀ Lewo ]`, `[ 🎯 Środek ]`, `[ Prawo ▶ ]`).
- 📸 **Lossless PNG Frame Capture (`extract_frame_png`)**:
  - Click `[ 📸 Klatka PNG ]` to instantly extract and save the current video frame as a full-resolution PNG to the output directory (respecting any active crop).
- 🎯 **Social Export Profiles**:
  - **Discord Clip (20 MB)**, **Discord Nitro (50 MB)**, **Shorts / TikTok HQ (50 MB)**, **Shorts Small (20 MB)**, **TikTok / Reels (50 MB)**, **Square Meme (20 MB)**, and **Custom**.
- 🚀 **1-Click In-App Auto-Updates (`update_service.py`)**:
  - Automatically queries official GitHub Releases in the background.
  - One-click seamless update and restart replacing `CutGut.exe` with backup (`.bak`) safety.
- 👥 **Dedicated Side-by-Side A/B Quality Comparison Window (`QualityComparisonDialog`)**:
  - Non-blocking separate comparison window displaying the trimmed original vs. the compressed output / quality sample.
  - Single synchronized timeline and play/pause controls with automated drift correction (every 250ms).
  - Frame-by-frame stepping controls (`◀ -1s`, `◀ -1 fr`, `+1 fr ▶`, `+1s ▶`).
  - **Deferred Source File Cleanup**: Source cleanup policies only execute after the user closes the A/B comparison window.
- 🔬 **Quality Preview Sample (`create_quality_preview`)**:
  - Test the actual encoded video quality on dynamic scenes (smoke, teamfights, motion) before running the full export with `[ 🔬 Sprawdź jakość w tym momencie ]`.
  - Encodes a rapid 6-second sample with exact resolution, bitrate, filters, and encoder presets into `temp/`.
- 📁 **Custom Output Directory & Collision-Free Naming**:
  - Choose your preferred save destination in Settings (⚙) with instant 1-click reset to default `outputs/`.
  - Automatic collision resolution (`CutGut_YYYYMMDD_HHMMSS.mp4`, `_2`, `_3`).
- 🧠 **Real-Time Quality Assessment & Actionable Tips (`QualityAssessment`)**:
  - Live calculation of bits-per-pixel-frame (`bppf`) evaluating visual fidelity.
  - Clear color-coded badges (**Great** 🟢 / **Good** 🟩 / **OK for Sharing** 🟨 / **Low** 🟧 / **Very Low** 🟥) accompanied by tailored advice.
- ⚙️ **Safe Source File Management (`SourceCleanupPolicy`)**:
  - Configurable source cleanup preferences in Settings: **Keep original** (default), **Ask every time**, **Recycle Bin (auto)**, or **Permanent Delete**.
- 🎬 **Intuitive Click-to-Play Video Interface**:
  - Click directly on the video viewport or press `Space` to Play/Pause.
  - Frame-accurate keyboard stepping (`←`/`→` for ±1s, `Shift + ←`/`→` for exact ±1 frame).
- ⚡ **Instant Lossless Remuxing (`can_stream_copy`)**:
  - If a clip or file already fits under the target limit, CutGut automatically copies video/audio streams directly (`-c copy`) in ~0.2 seconds without re-encoding!
- 📋 **Batch Processing & Job Queue (`EncodeJob`)**:
  - Interactive queue panel with per-task progress, status tracking, and cancel/delete controls.
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
