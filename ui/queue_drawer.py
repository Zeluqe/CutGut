from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton,
    QTableWidget, QTableWidgetItem, QHeaderView, QFrame
)
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtGui import QColor
import ui.icons

class QueueDrawerWidget(QFrame):
    job_action_clicked = pyqtSignal(int, str)  # emits (row, action_name)

    def __init__(self, parent=None, lang='en'):
        super().__init__(parent)
        self.lang = lang
        self.setProperty('class', 'FluentCard')
        self.setStyleSheet("""
            QFrame.FluentCard {
                background-color: #1c1c1f;
                border: 1px solid rgba(255, 255, 255, 0.08);
                border-radius: 8px;
            }
        """)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(12, 8, 12, 8)
        layout.setSpacing(6)

        hdr_row = QHBoxLayout()
        self.lblHeader = QLabel("Job Queue (0)" if lang == 'en' else "Kolejka zadań (0)")
        self.lblHeader.setStyleSheet("font-weight: 700; font-size: 13px; color: #60cdff;")
        hdr_row.addWidget(self.lblHeader)

        hdr_row.addStretch()

        self.btnClearFinished = QPushButton("Clear Finished" if lang == 'en' else "Wyczyść zakończone")
        self.btnClearFinished.setIcon(ui.icons.get_icon('trash', '#a1a1aa', 13))
        self.btnClearFinished.setStyleSheet("font-size: 11px; padding: 3px 8px;")
        hdr_row.addWidget(self.btnClearFinished)
        layout.addLayout(hdr_row)

        self.table = QTableWidget()
        self.table.setColumnCount(7)
        self.retranslate_headers()
        self.table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        for i in range(1, 7):
            self.table.horizontalHeader().setSectionResizeMode(i, QHeaderView.ResizeMode.ResizeToContents)
        self.table.setFixedHeight(85)
        self.table.cellClicked.connect(self.on_cell_clicked)
        layout.addWidget(self.table)

    def set_language(self, lang: str):
        self.lang = lang
        self.retranslate_headers()
        self.btnClearFinished.setText("Clear Finished" if lang == 'en' else "Wyczyść zakończone")

    def retranslate_headers(self):
        headers = ['File', 'Crop', 'Range', 'Limit', 'Encoder', 'Status', 'Action'] if self.lang == 'en' else ['Plik', 'Kadr', 'Zakres', 'Limit', 'Enkoder', 'Status', 'Akcja']
        self.table.setHorizontalHeaderLabels(headers)

    def on_cell_clicked(self, row: int, col: int):
        if col == 6:
            item = self.table.item(row, col)
            if item:
                self.job_action_clicked.emit(row, item.text())
