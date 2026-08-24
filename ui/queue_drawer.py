from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton,
    QTableWidget, QTableWidgetItem, QHeaderView, QFrame
)
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtGui import QColor
import ui.icons

class QueueDrawerWidget(QFrame):
    job_action_clicked = pyqtSignal(int, str)  # emits (row, action_name)

    def __init__(self, parent=None, lang='pl'):
        super().__init__(parent)
        self.lang = lang
        self.setProperty('class', 'FluentCard')
        self.setStyleSheet("""
            QFrame.FluentCard {
                background-color: #1c1c1f;
                border: 1px solid rgba(255, 255, 255, 0.08);
                border-radius: 10px;
            }
        """)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(12, 10, 12, 10)
        layout.setSpacing(8)

        hdr_row = QHBoxLayout()
        self.lblHeader = QLabel("Kolejka zadań (0)" if lang == 'pl' else "Job Queue (0)")
        self.lblHeader.setStyleSheet("font-weight: 700; font-size: 13px; color: #60cdff;")
        hdr_row.addWidget(self.lblHeader)

        hdr_row.addStretch()

        self.btnClearFinished = QPushButton("Wyczyść zakończone" if lang == 'pl' else "Clear Finished")
        self.btnClearFinished.setStyleSheet("font-size: 11px; padding: 4px 10px;")
        hdr_row.addWidget(self.btnClearFinished)
        layout.addLayout(hdr_row)

        self.table = QTableWidget()
        self.table.setColumnCount(7)
        headers = ['Plik', 'Kadr', 'Zakres', 'Limit', 'Enkoder', 'Status', 'Akcja'] if lang == 'pl' else ['File', 'Crop', 'Range', 'Limit', 'Encoder', 'Status', 'Action']
        self.table.setHorizontalHeaderLabels(headers)
        self.table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        for i in range(1, 7):
            self.table.horizontalHeader().setSectionResizeMode(i, QHeaderView.ResizeMode.ResizeToContents)
        self.table.setFixedHeight(110)
        self.table.cellClicked.connect(self.on_cell_clicked)
        layout.addWidget(self.table)

    def on_cell_clicked(self, row: int, col: int):
        if col == 6:
            item = self.table.item(row, col)
            if item:
                self.job_action_clicked.emit(row, item.text())
