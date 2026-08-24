import sys
import time
from typing import Optional
from PyQt6.QtWidgets import (
    QWidget, QFrame, QLabel, QHBoxLayout, QVBoxLayout,
    QPushButton, QDialog
)
from PyQt6.QtCore import Qt, QTimer, QRectF, QPointF, QPoint
from PyQt6.QtGui import QPainter, QColor, QBrush, QPen, QFont, QIcon
import ui.icons

class FluentCard(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setProperty('class', 'FluentCard')
        self.setObjectName('FluentCard')

class ToastNotification(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFixedHeight(38)
        self.setStyleSheet("""
            QFrame {
                background-color: #1e1e24;
                border: 1px solid rgba(255, 255, 255, 0.14);
                border-radius: 8px;
            }
            QLabel {
                color: #f4f4f6;
                font-size: 12px;
                font-weight: 600;
                background: transparent;
            }
        """)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(12, 0, 12, 0)
        layout.setSpacing(8)

        self.btnIcon = QPushButton()
        self.btnIcon.setStyleSheet("background: transparent; border: none; padding: 0;")
        self.btnIcon.setFixedSize(20, 20)
        layout.addWidget(self.btnIcon)

        self.lblText = QLabel("")
        layout.addWidget(self.lblText)

        self.timer = QTimer(self)
        self.timer.setSingleShot(True)
        self.timer.timeout.connect(self.hide)

        self.hide()

    def show_toast(self, text: str, icon_name: str = "info", duration_ms: int = 3500):
        color = '#38bdf8'
        if icon_name == 'check':
            color = '#10b981'
        elif icon_name in ('cancel', 'cross'):
            color = '#ef4444'
            icon_name = 'cancel'

        self.btnIcon.setIcon(ui.icons.get_icon(icon_name, color, 18))
        self.lblText.setText(text)
        self.adjustSize()

        if self.parentWidget():
            parent_w = self.parentWidget().width()
            self.move(parent_w - self.width() - 25, 20)

        self.show()
        self.raise_()
        self.timer.start(duration_ms)

    def hide_toast(self):
        self.hide()

class QualityBadge(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFixedHeight(28)
        self.setFixedWidth(160)
        self.label_text = "No File Loaded"
        self.dot_color = QColor("#71717a")

    def set_assessment(self, label: str, color_hex: str):
        self.label_text = label
        self.dot_color = QColor(color_hex)
        self.update()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        rect = QRectF(0.5, 0.5, self.width() - 1, self.height() - 1)

        # 1. Background Pill
        painter.setPen(QPen(QColor(255, 255, 255, 18), 1))
        painter.setBrush(QBrush(QColor(30, 30, 36)))
        painter.drawRoundedRect(rect, 14, 14)

        # 2. Glowing Status Dot
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(self.dot_color))
        painter.drawEllipse(QPointF(14.0, self.height() / 2.0), 4.5, 4.5)

        # 3. Assessment Text
        painter.setPen(QColor("#f4f4f6"))
        font = QFont("Segoe UI Variable", 9, QFont.Weight.Bold)
        painter.setFont(font)
        text_rect = QRectF(24.0, 0, self.width() - 28.0, self.height())
        painter.drawText(text_rect, Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft, self.label_text)

class HelpShortcutsDialog(QDialog):
    def __init__(self, parent=None, lang='pl'):
        super().__init__(parent)
        self.setWindowTitle("Skróty klawiszowe i pomoc" if lang == 'pl' else "Keyboard Shortcuts & Help")
        self.setFixedWidth(460)
        self.setStyleSheet("""
            QDialog { background-color: #141416; }
            QLabel { color: #f4f4f6; font-size: 13px; }
            QFrame.ShortcutRow {
                background-color: #1c1c1f;
                border: 1px solid rgba(255, 255, 255, 0.08);
                border-radius: 6px;
                padding: 6px 12px;
            }
            QLabel.KeyBadge {
                background-color: #2e2e34;
                border: 1px solid rgba(255, 255, 255, 0.16);
                border-radius: 4px;
                padding: 2px 8px;
                font-weight: bold;
                font-family: Consolas, monospace;
            }
            QPushButton {
                background-color: #0078d4;
                color: white;
                border-radius: 6px;
                padding: 8px 18px;
                font-weight: bold;
            }
            QPushButton:hover { background-color: #1084d8; }
        """)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(20, 20, 20, 20)
        layout.setSpacing(10)

        title = QLabel(f"<b>{'Skróty klawiszowe CutGut' if lang == 'pl' else 'CutGut Shortcuts'}</b>")
        title.setStyleSheet("font-size: 16px; color: #60cdff; margin-bottom: 6px;")
        layout.addWidget(title)

        shortcuts = [
            ("I", "Ustawienie punktu początkowego (IN)" if lang == 'pl' else "Set starting point (IN)"),
            ("O", "Ustawienie punktu końcowego (OUT)" if lang == 'pl' else "Set ending point (OUT)"),
            ("Spacja / Klik", "Odtwarzanie / Pauza podglądu" if lang == 'pl' else "Play / Pause playback"),
            ("← / →", "Przewijanie o ±1 sekundę" if lang == 'pl' else "Seek ±1 second"),
            ("Shift + ← / →", "Precyzyjny skok o ±1 klatkę" if lang == 'pl' else "Frame-accurate ±1 frame seek"),
            ("Myszka (Drag)", "Przeciąganie ramki kadru 9:16 / 1:1" if lang == 'pl' else "Drag framing box on video"),
            ("Esc", "Anulowanie trwającej kompresji" if lang == 'pl' else "Cancel ongoing encode")
        ]

        for key, desc in shortcuts:
            row = QFrame()
            row.setProperty('class', 'ShortcutRow')
            row_l = QHBoxLayout(row)
            row_l.setContentsMargins(10, 5, 10, 5)

            k_lbl = QLabel(key)
            k_lbl.setProperty('class', 'KeyBadge')
            row_l.addWidget(k_lbl)

            d_lbl = QLabel(desc)
            d_lbl.setStyleSheet("color: #a1a1aa; margin-left: 8px;")
            row_l.addWidget(d_lbl, 1)

            layout.addWidget(row)

        layout.addSpacing(10)
        btn_close = QPushButton("Zamknij" if lang == 'pl' else "Close")
        btn_close.clicked.connect(self.accept)
        layout.addWidget(btn_close, 0, Qt.AlignmentFlag.AlignRight)
