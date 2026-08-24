## ✂️ CutGut 202608240-6-7 — Audio Leak Fix, Smooth Scrollable Layout & Windows 11 Acrylic Blur

### 🌟 What's New & Fixed in this Release:
- **🔇 Fixed Audio Leak on Sample Comparison Close**:
  - `QualityComparisonDialog` now completely unloads and terminates all background audio players and sync timers across all close, accept, reject (Esc), and hide paths (`cleanup_and_stop`), ensuring sound stops immediately when the window closes.
- **📜 Smooth Scrollable Layout & Large Video Preview**:
  - Restored full prominent video canvas height (380px+).
  - Encapsulated content in a smooth, borderless `QScrollArea`. When the Job Queue is populated, it expands downward naturally below the export cards rather than compressing or shrinking the video player.
- **✨ Windows 11 Acrylic & Mica Blur Backdrop**:
  - Implemented Windows 11 DWM backdrop effects (Acrylic Blur / Mica) and modern translucent frosted glass card styling (`rgba(28, 28, 34, 0.78)`).

### 📦 Download
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608240-6-7/CutGut.exe)** (Standalone Windows x64 Executable)
