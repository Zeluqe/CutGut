import os
from typing import Optional
from PyQt6.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QLabel, QPushButton,
    QRadioButton, QButtonGroup, QCheckBox, QFileDialog, QMessageBox,
    QFrame
)
from PyQt6.QtCore import Qt
import encoder
import update_service

class FluentSettingsDialog(QDialog):
    def __init__(
        self,
        parent,
        current_policy: str,
        current_lang: str,
        output_dir: str,
        auto_open: bool,
        auto_ab: bool,
        auto_check_updates: bool,
        include_prerelease: bool,
        current_theme: str = "dark",
        reduce_animations: bool = False,
        version_str: str = "202608240-6-0"
    ):
        super().__init__(parent)
        self.current_lang = current_lang
        self.selected_policy = current_policy
        self.output_dir = output_dir
        self.auto_open = auto_open
        self.auto_ab = auto_ab
        self.auto_check_updates = auto_check_updates
        self.include_prerelease = include_prerelease
        self.current_theme = current_theme
        self.reduce_animations = reduce_animations
        self.version_str = version_str
        self.check_worker = None

        self.init_ui()

    def t(self, pl: str, en: str) -> str:
        return pl if self.current_lang == 'pl' else en

    def init_ui(self):
        self.setWindowTitle(self.t("Ustawienia CutGut", "CutGut Settings"))
        self.setFixedWidth(540)
        self.setStyleSheet("""
            QDialog { background-color: #141416; }
            QLabel { color: #f4f4f6; font-size: 13px; font-family: 'Segoe UI Variable', 'Segoe UI', system-ui; }
            QRadioButton, QCheckBox { color: #e4e4e7; font-size: 13px; padding: 3px; font-weight: 500; }
            QRadioButton:hover, QCheckBox:hover { color: #60cdff; }
            QPushButton { 
                border-radius: 6px; padding: 7px 14px; color: #f4f4f6; 
                font-weight: 600; background-color: #222226; border: 1px solid rgba(255, 255, 255, 0.1); 
            }
            QPushButton:hover { background-color: #2c2c33; border: 1px solid rgba(255, 255, 255, 0.2); }
            QFrame.SectionCard {
                background-color: #1c1c1f;
                border: 1px solid rgba(255, 255, 255, 0.08);
                border-radius: 8px;
                padding: 10px;
            }
        """)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(20, 18, 20, 18)
        layout.setSpacing(12)

        # 1. FOLDER WYNIKOWY
        lbl_out_hdr = QLabel(f"<b>{self.t('📁 FOLDER WYNIKOWY', '📁 OUTPUT DIRECTORY')}</b>")
        lbl_out_hdr.setStyleSheet("color: #60cdff; font-size: 13px;")
        layout.addWidget(lbl_out_hdr)

        dir_card = QFrame()
        dir_card.setProperty('class', 'SectionCard')
        dir_layout = QVBoxLayout(dir_card)
        dir_layout.setContentsMargins(10, 8, 10, 8)
        dir_layout.setSpacing(6)

        dir_row = QHBoxLayout()
        display_path = self.output_dir if self.output_dir else encoder.get_default_output_dir()
        self.lblDirPath = QLabel(display_path)
        self.lblDirPath.setStyleSheet("background-color: #141416; border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 6px; padding: 6px 10px; color: #cbd5e1;")
        dir_row.addWidget(self.lblDirPath, 1)

        self.btnBrowseDir = QPushButton(self.t("Wybierz...", "Choose..."))
        self.btnBrowseDir.clicked.connect(self.browse_output_dir)
        dir_row.addWidget(self.btnBrowseDir)
        dir_layout.addLayout(dir_row)

        sub_row = QHBoxLayout()
        lbl_def_info = QLabel(self.t("Domyślnie: folder „outputs” obok programu.", "Default: „outputs” folder next to the app."))
        lbl_def_info.setStyleSheet("color: #71717a; font-size: 11px;")
        sub_row.addWidget(lbl_def_info, 1)

        self.btnResetDir = QPushButton(self.t("Domyślny", "Reset"))
        self.btnResetDir.setStyleSheet("font-size: 11px; padding: 3px 8px;")
        self.btnResetDir.clicked.connect(self.reset_output_dir)
        sub_row.addWidget(self.btnResetDir)
        dir_layout.addLayout(sub_row)

        layout.addWidget(dir_card)

        # 2. PO UDANYM EKSPORCIE
        lbl_clean_hdr = QLabel(f"<b>{self.t('🛡️ PO UDANYM EKSPORCIE', '🛡️ AFTER SUCCESSFUL EXPORT')}</b>")
        lbl_clean_hdr.setStyleSheet("color: #60cdff; font-size: 13px;")
        layout.addWidget(lbl_clean_hdr)

        clean_card = QFrame()
        clean_card.setProperty('class', 'SectionCard')
        clean_layout = QVBoxLayout(clean_card)
        clean_layout.setContentsMargins(10, 6, 10, 6)
        clean_layout.setSpacing(4)

        self.btn_group = QButtonGroup(self)
        self.rb_never = QRadioButton(self.t("Zachowaj oryginał (Zalecane)", "Keep original file (Recommended)"))
        self.rb_ask = QRadioButton(self.t("Zapytaj, co zrobić z oryginałem", "Ask what to do with original"))
        self.rb_trash = QRadioButton(self.t("Przenieś oryginał do Kosza automatycznie", "Move original to Recycle Bin automatically"))
        self.rb_delete = QRadioButton(self.t("Usuń oryginał na stałe automatycznie (⚠ Nieodwracalne)", "Delete original permanently automatically (⚠ Irreversible)"))

        self.btn_group.addButton(self.rb_never, 0)
        self.btn_group.addButton(self.rb_ask, 1)
        self.btn_group.addButton(self.rb_trash, 2)
        self.btn_group.addButton(self.rb_delete, 3)

        clean_layout.addWidget(self.rb_never)
        clean_layout.addWidget(self.rb_ask)
        clean_layout.addWidget(self.rb_trash)
        clean_layout.addWidget(self.rb_delete)

        if self.selected_policy == encoder.SourceCleanupPolicy.ASK.value: self.rb_ask.setChecked(True)
        elif self.selected_policy == encoder.SourceCleanupPolicy.TRASH.value: self.rb_trash.setChecked(True)
        elif self.selected_policy == encoder.SourceCleanupPolicy.DELETE_PERMANENTLY.value: self.rb_delete.setChecked(True)
        else: self.rb_never.setChecked(True)

        layout.addWidget(clean_card)

        # 3. DODATKOWE & WYGLĄD
        lbl_extra_hdr = QLabel(f"<b>{self.t('⚙️ DODATKOWE & WYGLĄD', '⚙️ OPTIONS & THEME')}</b>")
        lbl_extra_hdr.setStyleSheet("color: #60cdff; font-size: 13px;")
        layout.addWidget(lbl_extra_hdr)

        extra_card = QFrame()
        extra_card.setProperty('class', 'SectionCard')
        extra_layout = QVBoxLayout(extra_card)
        extra_layout.setContentsMargins(10, 6, 10, 6)
        extra_layout.setSpacing(4)

        self.chkAutoOpen = QCheckBox(self.t("Otwórz folder wynikowy po eksporcie", "Open output folder after export"))
        self.chkAutoOpen.setChecked(self.auto_open)
        extra_layout.addWidget(self.chkAutoOpen)

        self.chkAutoAB = QCheckBox(self.t("Automatycznie otwórz okno porównania A/B", "Automatically open A/B comparison window"))
        self.chkAutoAB.setChecked(self.auto_ab)
        extra_layout.addWidget(self.chkAutoAB)

        self.chkAutoCheckUpdates = QCheckBox(self.t("Sprawdzaj aktualizacje przy uruchamianiu", "Check for updates on startup"))
        self.chkAutoCheckUpdates.setChecked(self.auto_check_updates)
        extra_layout.addWidget(self.chkAutoCheckUpdates)

        layout.addWidget(extra_card)

        # Przyciski
        layout.addSpacing(6)
        btn_box = QHBoxLayout()
        btn_box.addStretch()

        self.btnCancel = QPushButton(self.t("Anuluj", "Cancel"))
        self.btnCancel.clicked.connect(self.reject)
        btn_box.addWidget(self.btnCancel)

        self.btnSave = QPushButton(self.t("Zapisz ustawienia", "Save Settings"))
        self.btnSave.setStyleSheet("background-color: #0078d4; color: white; padding: 8px 20px; font-weight: bold;")
        self.btnSave.clicked.connect(self.on_save)
        btn_box.addWidget(self.btnSave)

        layout.addLayout(btn_box)

    def browse_output_dir(self):
        cur = self.output_dir if self.output_dir else encoder.get_default_output_dir()
        d = QFileDialog.getExistingDirectory(self, self.t("Wybierz folder", "Choose folder"), cur)
        if d:
            ok, msg = encoder.validate_output_directory(d)
            if ok:
                self.output_dir = os.path.abspath(d)
                self.lblDirPath.setText(self.output_dir)
            else:
                QMessageBox.warning(self, "Błąd folderu", msg)

    def reset_output_dir(self):
        self.output_dir = ''
        self.lblDirPath.setText(encoder.get_default_output_dir())

    def on_save(self):
        checked_id = self.btn_group.checkedId()
        if checked_id == 1:
            pol = encoder.SourceCleanupPolicy.ASK.value
        elif checked_id == 2:
            pol = encoder.SourceCleanupPolicy.TRASH.value
        elif checked_id == 3:
            res = QMessageBox.warning(
                self, self.t("Ostrzeżenie", "Warning"),
                self.t("Uwaga: Ta opcja będzie trwale usuwać pliki z dysku.\\nCzy na pewno włączyć?", "Warning: This permanently deletes files.\\nAre you sure?"),
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if res != QMessageBox.StandardButton.Yes:
                return
            pol = encoder.SourceCleanupPolicy.DELETE_PERMANENTLY.value
        else:
            pol = encoder.SourceCleanupPolicy.NEVER.value

        self.selected_policy = pol
        self.auto_open = self.chkAutoOpen.isChecked()
        self.auto_ab = self.chkAutoAB.isChecked()
        self.auto_check_updates = self.chkAutoCheckUpdates.isChecked()
        self.accept()
