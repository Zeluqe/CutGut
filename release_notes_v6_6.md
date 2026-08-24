## ✂️ CutGut 202608240-6-6 — Responsive Layout, Queue Table Fix & Full Bi-directional Translation

### 🌟 What's Fixed & Improved in this Release:
- **📐 Responsive Adaptive Layout (No Button Cutoffs)**:
  - Fixed the rigid height constraints on the video container and export boxes.
  - When the Job Queue is added or opened, the video canvas scales fluidly and proportionally (`minimumHeight: 180px`), keeping every action button, card, and slider 100% visible on any display or laptop resolution.
  - Removed accidental underscore accelerator in the export button (`Trim and Compress`).
- **🐛 Fixed Queue Table NameError**:
  - Imported `QTableWidgetItem` in `gui.py`, fixing the crash when clicking `Add to Queue`.
- **🌐 100% Complete Polish & English Translations**:
  - Every single label, card title, queue table column, status text, quality tip, and tooltip now switches dynamically between Polish and English without any missing translations.
- **🎨 Official Vector Icon Library (Lucide & Fluent)**:
  - All icons rendered via sharp vector SVGs (`QSvgRenderer`) with zero emoji dependencies.

### 📦 Download
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608240-6-6/CutGut.exe)** (Standalone Windows x64 Executable)
