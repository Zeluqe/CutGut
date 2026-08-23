## ✂️ CutGut 202608240-6-4 — QualityBadge Paint Fix & Complete Fluent UI

### 🌟 What's Fixed in this Release:
- **🐛 Fixed QualityBadge Paint Crash**:
  - Added missing `QPointF` and `QPoint` imports in `ui/widgets.py`, resolving the `NameError: name 'QPointF' is not defined` error during widget paint events.
  - Verified that all UI elements (`QualityBadge`, `FluentTimelineWidget`, `ToastNotification`, `VideoCanvasWidget`, `QueueDrawerWidget`, `FluentSettingsDialog`, and `HelpShortcutsDialog`) paint with zero runtime errors.
- **🛡️ Process Self-Updater Fix**:
  - Verified clean process detachment for in-app auto-updates without parent process security validation errors.

### 📦 Download
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608240-6-4/CutGut.exe)** (Standalone Windows x64 Executable)
