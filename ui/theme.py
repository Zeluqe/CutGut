import sys
import os
import ctypes
from ctypes import wintypes
from typing import Optional

def apply_windows_theme(widget, is_dark: bool = True):
    """
    Applies Windows 11 DWM immersive dark title bar, Mica / Acrylic blur backdrop,
    and rounded corner attributes safely.
    """
    if os.name != 'nt':
        return
    try:
        hwnd = wintypes.HWND(int(widget.winId()))
        dwmapi = ctypes.windll.dwmapi

        # 1. DWMWA_USE_IMMERSIVE_DARK_MODE (20 on Win11/Win10 20H1+, 19 on older)
        val_dark = wintypes.BOOL(is_dark)
        for attr in [20, 19]:
            res = dwmapi.DwmSetWindowAttribute(hwnd, attr, ctypes.byref(val_dark), ctypes.sizeof(val_dark))
            if res == 0:
                break

        # 2. DWMWA_WINDOW_CORNER_PREFERENCE = 33 (2 = DWMWCP_ROUND)
        corner_pref = wintypes.DWORD(2)
        dwmapi.DwmSetWindowAttribute(hwnd, 33, ctypes.byref(corner_pref), ctypes.sizeof(corner_pref))

        # 3. DWMWA_SYSTEMBACKDROP_TYPE = 38 (2 = Mica, 3 = Acrylic blur, 4 = Mica Alt)
        # We try Acrylic (3) first for smooth blur transparency, fallback to Mica (2)
        backdrop_type = wintypes.DWORD(3)  # DWMSBT_TRANSIENTWINDOW (Acrylic Blur)
        res_backdrop = dwmapi.DwmSetWindowAttribute(hwnd, 38, ctypes.byref(backdrop_type), ctypes.sizeof(backdrop_type))
        if res_backdrop != 0:
            backdrop_type = wintypes.DWORD(2)  # DWMSBT_MAINWINDOW (Mica)
            dwmapi.DwmSetWindowAttribute(hwnd, 38, ctypes.byref(backdrop_type), ctypes.sizeof(backdrop_type))

        # 4. Legacy Mica on Build 22000
        val_mica = wintypes.BOOL(True)
        dwmapi.DwmSetWindowAttribute(hwnd, 1029, ctypes.byref(val_mica), ctypes.sizeof(val_mica))

    except Exception:
        pass

def get_fluent_stylesheet(is_dark: bool = True) -> str:
    """
    Generates a modern, clean Windows 11 Fluent UI stylesheet with translucent acrylic cards,
    refined typography, 8px grid spacing, and subtle frosted glass borders.
    """
    if is_dark:
        bg_window = "#141416"
        bg_card = "rgba(28, 28, 34, 0.78)"
        bg_card_hover = "rgba(36, 36, 44, 0.88)"
        bg_input = "rgba(34, 34, 42, 0.70)"
        bg_input_hover = "rgba(42, 42, 52, 0.85)"
        border_subtle = "rgba(255, 255, 255, 0.08)"
        border_strong = "rgba(255, 255, 255, 0.16)"
        border_accent = "rgba(96, 205, 255, 0.45)"
        text_primary = "#f4f4f6"
        text_secondary = "#a1a1aa"
        text_disabled = "#52525b"
        accent_blue = "#0078d4"
        accent_hover = "#1a88dc"
        accent_pressed = "#0067b8"
    else:
        bg_window = "#f4f4f7"
        bg_card = "rgba(255, 255, 255, 0.85)"
        bg_card_hover = "rgba(249, 249, 251, 0.95)"
        bg_input = "rgba(240, 240, 244, 0.85)"
        bg_input_hover = "rgba(228, 228, 233, 0.95)"
        border_subtle = "rgba(0, 0, 0, 0.08)"
        border_strong = "rgba(0, 0, 0, 0.18)"
        border_accent = "rgba(0, 120, 212, 0.45)"
        text_primary = "#18181b"
        text_secondary = "#71717a"
        text_disabled = "#a1a1aa"
        accent_blue = "#0067c0"
        accent_hover = "#107cd8"
        accent_pressed = "#00569e"

    return f"""
    * {{
        font-family: 'Segoe UI Variable', 'Segoe UI', system-ui, -apple-system, sans-serif;
        outline: none;
    }}

    QMainWindow, QDialog {{
        background-color: {bg_window};
        color: {text_primary};
    }}

    QWidget {{
        color: {text_primary};
        font-size: 13px;
    }}

    QScrollArea {{
        background: transparent;
        border: none;
    }}

    QScrollArea > QWidget > QWidget {{
        background: transparent;
    }}

    QFrame.FluentCard {{
        background-color: {bg_card};
        border: 1px solid {border_subtle};
        border-radius: 12px;
    }}

    QFrame.FluentCard:hover {{
        border: 1px solid {border_accent};
        background-color: {bg_card_hover};
    }}

    QLabel {{
        color: {text_primary};
    }}

    QLabel.Secondary {{
        color: {text_secondary};
        font-size: 12px;
    }}

    /* Standard Buttons */
    QPushButton {{
        background-color: {bg_input};
        color: {text_primary};
        border: 1px solid {border_subtle};
        border-radius: 7px;
        padding: 7px 14px;
        font-weight: 600;
        font-size: 12px;
    }}

    QPushButton:hover {{
        background-color: {bg_input_hover};
        border: 1px solid {border_strong};
    }}

    QPushButton:pressed {{
        background-color: {bg_card};
    }}

    QPushButton:disabled {{
        background-color: transparent;
        color: {text_disabled};
        border: 1px solid {border_subtle};
    }}

    /* Primary Action Button */
    QPushButton.Primary {{
        background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #0078d4, stop:1 #0063b1);
        color: #ffffff;
        border: 1px solid rgba(255, 255, 255, 0.20);
        border-radius: 8px;
        padding: 8px 18px;
        font-size: 13px;
        font-weight: 700;
    }}

    QPushButton.Primary:hover {{
        background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #1a88dc, stop:1 #0078d4);
        border: 1px solid {border_accent};
    }}

    QPushButton.Primary:pressed {{
        background: #00569e;
    }}

    QPushButton.Primary:disabled {{
        background: {bg_input};
        color: {text_disabled};
        border: 1px solid {border_subtle};
    }}

    /* Danger Button */
    QPushButton.Danger {{
        background-color: rgba(220, 38, 38, 0.85);
        color: #ffffff;
        border: 1px solid #ef4444;
        border-radius: 7px;
        font-weight: 700;
    }}

    QPushButton.Danger:hover {{
        background-color: #dc2626;
        border: 1px solid #f87171;
    }}

    /* ComboBox */
    QComboBox {{
        background-color: {bg_input};
        color: {text_primary};
        border: 1px solid {border_subtle};
        border-radius: 7px;
        padding: 6px 12px;
        font-weight: 600;
        font-size: 12px;
    }}

    QComboBox:hover {{
        background-color: {bg_input_hover};
        border: 1px solid {accent_blue};
    }}

    QComboBox::drop-down {{
        subcontrol-origin: padding;
        subcontrol-position: top right;
        width: 24px;
        border-left: none;
    }}

    QComboBox QAbstractItemView {{
        background-color: #1e1e24;
        color: {text_primary};
        selection-background-color: {accent_blue};
        selection-color: #ffffff;
        border: 1px solid {border_strong};
        border-radius: 8px;
        padding: 4px;
    }}

    /* Horizontal Slider */
    QSlider::groove:horizontal {{
        border: none;
        height: 5px;
        background: {bg_input_hover};
        border-radius: 2.5px;
    }}

    QSlider::sub-page:horizontal {{
        background: {accent_blue};
        border-radius: 2.5px;
    }}

    QSlider::handle:horizontal {{
        background: #ffffff;
        border: 3px solid {accent_blue};
        width: 16px;
        height: 16px;
        margin: -5.5px 0;
        border-radius: 8px;
    }}

    QSlider::handle:horizontal:hover {{
        background: #ffffff;
        border: 3px solid {accent_hover};
    }}

    /* CheckBox */
    QCheckBox {{
        color: {text_primary};
        font-weight: 500;
        spacing: 8px;
    }}

    QCheckBox::indicator {{
        width: 18px;
        height: 18px;
        border-radius: 5px;
        border: 1px solid {border_strong};
        background: {bg_input};
    }}

    QCheckBox::indicator:hover {{
        border: 1px solid {accent_blue};
    }}

    QCheckBox::indicator:checked {{
        background: {accent_blue};
        border: 1px solid {accent_blue};
    }}

    /* ProgressBar */
    QProgressBar {{
        border: 1px solid {border_subtle};
        border-radius: 5px;
        text-align: center;
        color: #ffffff;
        font-weight: 600;
        background-color: {bg_input};
        height: 8px;
    }}

    QProgressBar::chunk {{
        background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #0078d4, stop:1 #38bdf8);
        border-radius: 4px;
    }}

    /* Queue Table */
    QTableWidget {{
        background-color: rgba(24, 24, 28, 0.85);
        border: 1px solid {border_subtle};
        border-radius: 8px;
        color: {text_primary};
        gridline-color: rgba(255, 255, 255, 0.05);
        selection-background-color: {bg_input_hover};
    }}

    QHeaderView::section {{
        background-color: rgba(34, 34, 42, 0.90);
        color: {text_secondary};
        font-weight: 600;
        border: none;
        border-bottom: 1px solid {border_subtle};
        padding: 5px 8px;
    }}

    /* Modern Smooth Scrollbar */
    QScrollBar:vertical {{
        border: none;
        background: transparent;
        width: 8px;
        margin: 2px 0 2px 0;
    }}

    QScrollBar::handle:vertical {{
        background: rgba(255, 255, 255, 0.16);
        min-height: 28px;
        border-radius: 4px;
    }}

    QScrollBar::handle:vertical:hover {{
        background: rgba(255, 255, 255, 0.32);
    }}

    QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{
        height: 0px;
    }}

    QScrollBar:horizontal {{
        border: none;
        background: transparent;
        height: 8px;
        margin: 0 2px 0 2px;
    }}

    QScrollBar::handle:horizontal {{
        background: rgba(255, 255, 255, 0.16);
        min-width: 28px;
        border-radius: 4px;
    }}

    QScrollBar::handle:horizontal:hover {{
        background: rgba(255, 255, 255, 0.32);
    }}

    QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {{
        width: 0px;
    }}
    """
