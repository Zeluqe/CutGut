## ✂️ CutGut 202608240-6-3 — Self-Updater Security Fix & Robust Windows 11 Fluent Launch

### 🌟 What's New & Fixed in this Release:
- **🛡️ Clean Self-Updater & Process Detachment Fix**:
  - Replaced the batch script updater with an isolated PowerShell & desktop shell launcher.
  - Fixes the Windows security error: `Security validation failure: failed to obtain executable path for parent proces!` during in-app auto-updates.
- **🛡️ Global Exception Catcher**:
  - Added a global `sys.excepthook` exception handler that captures any unexpected runtime issues and displays a readable modal dialog rather than exiting silently.
- **🎨 Windows 11 Fluent UI**:
  - Modern dark graphite theme (`#141416` / `#1c1c1f`), rounded cards, distinct IN/OUT interactive timeline with range highlighting and hover tooltips.

### 📦 Download
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608240-6-3/CutGut.exe)** (Standalone Windows x64 Executable)
