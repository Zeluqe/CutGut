import sys
import os
import ctypes
from typing import Optional

def apply_windows_theme(widget, is_dark: bool = True):
    """
    Applies Windows 11 DWM immersive dark title bar and rounded corner attributes safely.
    """
    if os.name != 'nt':
        return
    try:
        hwnd = int(widget.winId())
        dwmapi = ctypes.windll.dwmapi
        # DWMWA_USE_IMMERSIVE_DARK_MODE = 20 (Windows 11 / Windows 10 20H1+) or 19 (older Win10)
        value = ctypes.c_int(1 if is_dark else 0)
        for attr in [20, 19]:
            res = dwmapi.DwmSetWindowAttribute(hwnd, attr, ctypes.byref(value), ctypes.sizeof(value))
            if res == 0:
                break
        
        # DWMWA_WINDOW_CORNER_PREFERENCE = 33 (1 = normal, 2 = round, 3 = round small)
        corner_pref = ctypes.c_int(2)
        dwmapi.DwmSetWindowAttribute(hwnd, 33, ctypes.byref(corner_pref), ctypes.sizeof(corner_pref))
    except Exception:
        pass

def get_fluent_stylesheet(is_dark: bool = True) -> str:
    """
    Generates a modern, clean Windows 11 Fluent UI stylesheet with dark graphite cards,
    refined typography, 8px grid spacing, and subtle 1px borders.
    """
    if is_dark:
        bg_window = "#141416"
        bg_card = "#1c1c1f"
        bg_card_hover = "#242429"
        bg_input = "#222226"
        bg_input_hover = "#28282e"
        border_subtle = "rgba(255, 255, 255, 0.08)"
        border_strong = "rgba(255, 255, 255, 0.16)"
        text_primary = "#f4f4f6"
        text_secondary = "#a1a1aa"
        text_disabled = "#52525b"
        accent_blue = "#0078d4"
        accent_hover = "#1a88dc"
        accent_pressed = "#0067b8"
        table_alt = "#1e1e22"
    else:
        bg_window = "#f4f4f7"
        bg_card = "#ffffff"
        bg_card_hover = "#f9f9fb"
        bg_input = "#f0f0f4"
        bg_input_hover = "#e4e4e9"
        border_subtle = "rgba(0, 0, 0, 0.08)"
        border_strong = "rgba(0, 0, 0, 0.18)"
        text_primary = "#18181b"
        text_secondary = "#71717a"
        text_disabled = "#a1a1aa"
        accent_blue = "#0067c0"
        accent_hover = "#107cd8"
        accent_pressed = "#00569e"
        table_alt = "#fafafa"

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

    QFrame.FluentCard {{
        background-color: {bg_card};
        border: 1px solid {border_subtle};
        border-radius: 10px;
    }}

    QFrame.FluentCard:hover {{
        border: 1px solid {border_strong};
    }}

    QLabel {{
        color: {text_primary};
    }}

    QLabel.Secondary {{
        color: {text_secondary};
        font-size: 12px;
    }}

    QLabel.HeaderTitle {{
        font-size: 17px;
        font-weight: 700;
        color: {text_primary};
    }}

    QLabel.HeaderSubtitle {{
        font-size: 12px;
        color: {text_secondary};
    }}

    /* Przyciski standardowe */
    QPushButton {{
        background-color: {bg_input};
        color: {text_primary};
        border: 1px solid {border_subtle};
        border-radius: 6px;
        padding: 7px 14px;
        font-weight: 600;
        font-size: 13px;
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

    /* Główny przycisk akcji (Primary) */
    QPushButton.Primary {{
        background-color: {accent_blue};
        color: #ffffff;
        border: 1px solid {accent_hover};
        border-radius: 7px;
        padding: 9px 20px;
        font-size: 14px;
        font-weight: 700;
    }}

    QPushButton.Primary:hover {{
        background-color: {accent_hover};
    }}

    QPushButton.Primary:pressed {{
        background-color: {accent_pressed};
    }}

    QPushButton.Primary:disabled {{
        background-color: {bg_input};
        color: {text_disabled};
        border: 1px solid {border_subtle};
    }}

    /* Przycisk Danger / Anuluj */
    QPushButton.Danger {{
        background-color: #dc2626;
        color: #ffffff;
        border: 1px solid #ef4444;
        border-radius: 6px;
        font-weight: 700;
    }}

    QPushButton.Danger:hover {{
        background-color: #b91c1c;
    }}

    /* Pola wyboru (ComboBox) */
    QComboBox {{
        background-color: {bg_input};
        color: {text_primary};
        border: 1px solid {border_subtle};
        border-radius: 6px;
        padding: 6px 12px;
        font-weight: 600;
        font-size: 13px;
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
        background-color: {bg_card};
        color: {text_primary};
        selection-background-color: {accent_blue};
        selection-color: #ffffff;
        border: 1px solid {border_strong};
        border-radius: 6px;
        padding: 4px;
    }}

    /* Suwak (Slider) */
    QSlider::groove:horizontal {{
        border: none;
        height: 4px;
        background: {bg_input_hover};
        border-radius: 2px;
    }}

    QSlider::sub-page:horizontal {{
        background: {accent_blue};
        border-radius: 2px;
    }}

    QSlider::handle:horizontal {{
        background: {accent_blue};
        border: 3px solid {bg_card};
        width: 16px;
        height: 16px;
        margin: -6px 0;
        border-radius: 8px;
    }}

    QSlider::handle:horizontal:hover {{
        background: {accent_hover};
        border: 2px solid #ffffff;
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
        border-radius: 4px;
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
        height: 12px;
    }}

    QProgressBar::chunk {{
        background-color: {accent_blue};
        border-radius: 4px;
    }}

    /* Tabela kolejki */
    QTableWidget {{
        background-color: {bg_card};
        border: 1px solid {border_subtle};
        border-radius: 8px;
        color: {text_primary};
        gridline-color: {border_subtle};
        selection-background-color: {bg_input_hover};
    }}

    QHeaderView::section {{
        background-color: {bg_input};
        color: {text_secondary};
        font-weight: 600;
        border: none;
        border-bottom: 1px solid {border_subtle};
        padding: 6px;
    }}

    /* ScrollBar */
    QScrollBar:vertical {{
        border: none;
        background: transparent;
        width: 8px;
        margin: 0;
    }}

    QScrollBar::handle:vertical {{
        background: {border_strong};
        min-height: 24px;
        border-radius: 4px;
    }}

    QScrollBar::handle:vertical:hover {{
        background: {text_disabled};
    }}

    QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{
        height: 0px;
    }}
    """
