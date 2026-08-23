import sys
import os
import time
import subprocess
from typing import Optional, Callable
from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout,
    QHBoxLayout, QPushButton, QLabel, QSlider,
    QFileDialog, QProgressBar, QStyle, QComboBox, QMessageBox,
    QCheckBox, QInputDialog, QDialog, QRadioButton, QButtonGroup,
    QTableWidget, QTableWidgetItem, QHeaderView, QFrame, QTextEdit
)
from PyQt6.QtMultimedia import QMediaPlayer, QAudioOutput
from PyQt6.QtCore import Qt, QUrl, pyqtSignal, QThread, QSettings, QEvent
from PyQt6.QtGui import QFont, QColor

import encoder
import update_service
from comparison_dialog import QualityComparisonDialog
from crop_overlay import CropVideoContainer

class EncodingWorker(QThread):
    progress_signal = pyqtSignal(object)
    finished_signal = pyqtSignal(str, int)
    error_signal = pyqtSignal(str)

    def __init__(self, input_path: str, output_path: str, start_s: float, end_s: float, target_mb: float, mode: str, crop_box: Optional[encoder.CropBox] = None):
        super().__init__()
        self.input_path = input_path
        self.output_path = output_path
        self.start_s = start_s
        self.end_s = end_s
        self.target_mb = target_mb
        self.mode = mode
        self.crop_box = crop_box
        self.cancel_token = encoder.CancellationToken()

    def run(self):
        try:
            res = encoder.encode_video(
                input_path=self.input_path,
                output_path=self.output_path,
                start_s=self.start_s,
                end_s=self.end_s,
                target_mb=self.target_mb,
                preset_mode=self.mode,
                progress_callback=lambda p: self.progress_signal.emit(p),
                cancel_token=self.cancel_token,
                crop_box=self.crop_box
            )
            final_size = os.path.getsize(res) if os.path.exists(res) else 0
            self.finished_signal.emit(res, final_size)
        except Exception as e:
            self.error_signal.emit(str(e))

    def cancel(self):
        self.cancel_token.cancel()

class SamplePreviewWorker(QThread):
    finished_signal = pyqtSignal(str)
    error_signal = pyqtSignal(str)

    def __init__(self, input_path: str, center_s: float, plan: dict, preset_mode: str, crop_box: Optional[encoder.CropBox] = None):
        super().__init__()
        self.input_path = input_path
        self.center_s = center_s
        self.plan = plan
        self.preset_mode = preset_mode
        self.crop_box = crop_box
        self.cancel_token = encoder.CancellationToken()

    def run(self):
        try:
            sample_path = encoder.create_quality_preview(
                input_path=self.input_path,
                center_s=self.center_s,
                plan=self.plan,
                preset_mode=self.preset_mode,
                sample_dur_s=6.0,
                cancel_token=self.cancel_token
            )
            self.finished_signal.emit(sample_path)
        except Exception as e:
            self.error_signal.emit(str(e))

    def cancel(self):
        self.cancel_token.cancel()

__version__ = "202608230-5-3"

TRANSLATIONS = {
    'pl': {
        'title': 'CutGut v{version} - Format & Social Export | Przycinanie i inteligentna kompresja wideo',
        'btn_start': '📍 START (I)',
        'btn_end': '📍 KONIEC (O)',
        'btn_set_in': 'USTAW I',
        'btn_set_out': 'USTAW O',
        'range_initial': 'Wybierz film lub przeciągnij go do okna...',
        'duration_label': '⏱️ DŁUGOŚĆ: {dur:.2f}s',
        'chk_loop': '🔁 Zapętlij (Loop)',
        'lbl_volume': '🔊 Głośność:',
        'btn_select': '📁 Wybierz film',
        'btn_change_file': '📁 Zmień film',
        'btn_compress': '✂ PRZYTNIJ I KOMPRESUJ',
        'btn_add_queue': '＋ DO KOLEJKI',
        'btn_cancel': '✕ ANULUJ',
        'btn_settings': '⚙ Ustawienia',
        'btn_screenshot': '📸 Klatka PNG',
        'lbl_source': 'Źródło: {name}',
        'lbl_no_file': 'Brak wybranego filmu (przeciągnij plik tutaj)',
        'lbl_queue_header': '📋 Kolejka zadań ({count}):',
        'video_tooltip': 'Kliknij w podgląd lub naciśnij Spację, aby włączyć/zatrzymać odtwarzanie. Przeciągaj ramkę kadru myszką.',
        'plan_box_title': 'Plan eksportu & Jakość',
        'btn_test_sample': '🔬 Sprawdź jakość w tym momencie',
        'sample_remux_msg': '⚡ Brak kompresji (Remux) — film zachowa 100% oryginalnej jakości.',
        'sample_generating': '⏳ Generowanie próbki jakości (6s wokół kursora)...',
        'sample_done': '✅ Próbka gotowa! Otwarto okno porównania jakości A/B.',
        'screenshot_saved': '📸 Zapisano klatkę PNG: {path}',
        'social_presets': [
            '🎯 Discord Clip (20 MB)',
            '💎 Discord Nitro (50 MB)',
            '📱 Shorts / TikTok HQ (50 MB)',
            '📱 Shorts Small (20 MB)',
            '🎬 TikTok / Reels (50 MB)',
            '⬛ Square Meme (20 MB)',
            '⚙️ Własny format...'
        ],
        'aspect_ratios': [
            '🖥️ Oryginalny (Cały obraz)',
            '📱 Pionowy 9:16 (Shorts / TikTok)',
            '⬛ Kwadrat 1:1 (Post / Memy)',
            '🎬 Poziomy 16:9 (Gameplay)'
        ],
        'btn_align_left': '◀ Lewo',
        'btn_align_center': '🎯 Środek',
        'btn_align_right': 'Prawo ▶',
        'presets': [
            '🎯 Discord Free (20 MB)',
            '📦 Legacy / Small (10 MB)',
            '💎 Nitro Basic (50 MB)',
            '🚀 Nitro (500 MB)',
            '⚙️ Własny rozmiar...'
        ],
        'custom_preset_label': '⚙️ Własny ({val:.1f} MB)',
        'custom_input_title': 'Własny limit rozmiaru',
        'custom_input_msg': 'Podaj maksymalny rozmiar w MB:',
        'encoders_nvenc': [
            '⚡ NVIDIA NVENC (GPU HQ)',
            '🚀 NVIDIA NVENC (GPU Szybki)',
            '⚖️ CPU H.264 (Zbalansowany)',
            '💨 CPU H.264 (Szybki)',
            '💎 CPU H.265 (Kinowy)'
        ],
        'encoders_amf': [
            '🔴 AMD AMF (GPU HQ)',
            '🚀 AMD AMF (GPU Szybki)',
            '⚖️ CPU H.264 (Zbalansowany)',
            '💨 CPU H.264 (Szybki)',
            '💎 CPU H.265 (Kinowy)'
        ],
        'encoders_cpu': [
            '⚖️ CPU H.264 (Zbalansowany)',
            '💨 CPU H.264 (Szybki)',
            '💎 CPU H.265 (Kinowy)'
        ],
        'ready_status': 'Gotowy. Skróty: [I] Start, [O] Koniec, [Spacja] Play/Pause, [←/→] ±1s, [Shift+←/→] ±1 klatka',
        'loaded_status': 'Wczytano: {filename}',
        'init_status': 'Inicjalizacja kompresji...',
        'cancel_status': 'Anulowanie zadania...',
        'progress_status': '[{stage}] {percent:.1f}% | Prędkość: {speed}{eta}',
        'done_status': '✅ Gotowe! Zapisano: {filename} ({mb:.2f} MB / {mib:.2f} MiB)',
        'queue_done_status': '✅ Ukończono wszystkie zadania z kolejki ({count})!',
        'error_status': '❌ Błąd: {msg}',
        'invalid_range_title': 'Nieprawidłowy zakres',
        'invalid_range_msg': 'Czas końca musi być większy niż czas początku!',
        'ffmpeg_missing_status': '⚠️ Uwaga: Brak FFmpeg. Zainstaluj FFmpeg lub umieść ffmpeg.exe w folderze programu.',
        'dialog_open_title': 'Wybierz film',
        'dialog_open_filter': 'Pliki Wideo (*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts)',
        'error_dialog_title': 'Błąd kompresji',
        'settings_title': '⚙ Ustawienia CutGut',
        'settings_out_dir_header': 'FOLDER WYNIKOWY',
        'settings_out_dir_default_info': 'ⓘ Domyślnie: folder „outputs” obok programu.',
        'btn_choose_dir': 'Wybierz folder...',
        'btn_reset_dir': 'Przywróć domyślny',
        'settings_cleanup_header': 'PO UDANYM EKSPORCIE',
        'opt_never': '◉ Zachowaj oryginał (Zalecane)',
        'opt_ask': '○ Zapytaj, co zrobić z oryginałem',
        'opt_trash': '○ Przenieś oryginał do Kosza automatycznie',
        'opt_delete': '○ Usuń oryginał na stałe automatycznie (⚠ Nieodwracalne)',
        'settings_extra_header': 'DODATKOWE',
        'chk_auto_open': 'Otwórz folder wynikowy po eksporcie',
        'chk_auto_ab': 'Automatycznie otwórz okno porównania A/B',
        'settings_updates_header': 'AKTUALIZACJE',
        'lbl_current_ver': 'Wersja: {version}',
        'btn_check_updates': '🔄 Sprawdź aktualizacje',
        'chk_auto_check_updates': 'Sprawdzaj aktualizacje przy uruchamianiu',
        'chk_include_prerelease': 'Pokazuj wersje testowe (prerelease)',
        'btn_save_settings': 'Zapisz ustawienia',
        'btn_cancel_settings': 'Anuluj',
        'ask_cleanup_title': 'Usunięcie oryginału',
        'ask_cleanup_msg': 'Eksport zakończony sukcesem!\nCzy chcesz przenieść oryginalny plik do Kosza?\n\n{path}',
        'perm_delete_warn_title': 'Ostrzeżenie o trwałym usuwaniu',
        'perm_delete_warn_msg': 'Uwaga: Ta opcja będzie bezpowrotnie usuwać oryginalne pliki wideo z dysku po każdym eksporcie.\n\nCzy na pewno chcesz ją włączyć?',
        'no_update_title': 'Aktualizacje CutGut',
        'no_update_msg': 'Posiadasz najnowszą wersję programu ({version})!'
    },
    'en': {
        'title': 'CutGut v{version} - Format & Social Export | Video Trimming & Smart Compression',
        'btn_start': '📍 START (I)',
        'btn_end': '📍 END (O)',
        'btn_set_in': 'SET I',
        'btn_set_out': 'SET O',
        'range_initial': 'Select a video from disk or drag & drop here...',
        'duration_label': '⏱️ DURATION: {dur:.2f}s',
        'chk_loop': '🔁 Loop preview',
        'lbl_volume': '🔊 Volume:',
        'btn_select': '📁 Select video',
        'btn_change_file': '📁 Change video',
        'btn_compress': '✂ TRIM & COMPRESS',
        'btn_add_queue': '＋ TO QUEUE',
        'btn_cancel': '✕ CANCEL',
        'btn_settings': '⚙ Settings',
        'btn_screenshot': '📸 Frame PNG',
        'lbl_source': 'Source: {name}',
        'lbl_no_file': 'No video selected (drag & drop file here)',
        'lbl_queue_header': '📋 Task Queue ({count}):',
        'video_tooltip': 'Click preview or press Space to play/pause. Drag framing box with mouse.',
        'plan_box_title': 'Export Plan & Quality',
        'btn_test_sample': '🔬 Test Quality Sample at this point',
        'sample_remux_msg': '⚡ Direct Stream Copy (Remux) — 100% original quality preserved.',
        'sample_generating': '⏳ Generating quality sample (6s around playhead)...',
        'sample_done': '✅ Sample ready! Opened A/B quality comparison window.',
        'screenshot_saved': '📸 Saved PNG frame: {path}',
        'social_presets': [
            '🎯 Discord Clip (20 MB)',
            '💎 Discord Nitro (50 MB)',
            '📱 Shorts / TikTok HQ (50 MB)',
            '📱 Shorts Small (20 MB)',
            '🎬 TikTok / Reels (50 MB)',
            '⬛ Square Meme (20 MB)',
            '⚙️ Custom format...'
        ],
        'aspect_ratios': [
            '🖥️ Original (Full frame)',
            '📱 Vertical 9:16 (Shorts / TikTok)',
            '⬛ Square 1:1 (Post / Memes)',
            '🎬 Horizontal 16:9 (Gameplay)'
        ],
        'btn_align_left': '◀ Left',
        'btn_align_center': '🎯 Center',
        'btn_align_right': 'Right ▶',
        'presets': [
            '🎯 Discord Free (20 MB)',
            '📦 Legacy / Small (10 MB)',
            '💎 Nitro Basic (50 MB)',
            '🚀 Nitro (500 MB)',
            '⚙️ Custom size...'
        ],
        'custom_preset_label': '⚙️ Custom ({val:.1f} MB)',
        'custom_input_title': 'Custom Size Limit',
        'custom_input_msg': 'Enter maximum file size in MB:',
        'encoders_nvenc': [
            '⚡ NVIDIA NVENC (GPU HQ)',
            '🚀 NVIDIA NVENC (GPU Fast)',
            '⚖️ CPU H.264 (Balanced)',
            '💨 CPU H.264 (Fast)',
            '💎 CPU H.265 (Cinematic)'
        ],
        'encoders_amf': [
            '🔴 AMD AMF (GPU HQ)',
            '🚀 AMD AMF (GPU Fast)',
            '⚖️ CPU H.264 (Balanced)',
            '💨 CPU H.264 (Fast)',
            '💎 CPU H.265 (Cinematic)'
        ],
        'encoders_cpu': [
            '⚖️ CPU H.264 (Balanced)',
            '💨 CPU H.264 (Fast)',
            '💎 CPU H.265 (Cinematic)'
        ],
        'ready_status': 'Ready. Shortcuts: [I] Start, [O] End, [Space] Play/Pause, [←/→] ±1s, [Shift+←/→] ±1 frame',
        'loaded_status': 'Loaded: {filename}',
        'init_status': 'Initializing compression...',
        'cancel_status': 'Cancelling task...',
        'progress_status': '[{stage}] {percent:.1f}% | Speed: {speed}{eta}',
        'done_status': '✅ Done! Saved: {filename} ({mb:.2f} MB / {mib:.2f} MiB)',
        'queue_done_status': '✅ All queue tasks completed ({count} jobs)!',
        'error_status': '❌ Error: {msg}',
        'invalid_range_title': 'Invalid Range',
        'invalid_range_msg': 'End time must be greater than start time!',
        'ffmpeg_missing_status': '⚠️ Note: FFmpeg missing. Install FFmpeg or place ffmpeg.exe in app folder.',
        'dialog_open_title': 'Select video',
        'dialog_open_filter': 'Video Files (*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts)',
        'error_dialog_title': 'Compression Error',
        'settings_title': '⚙ CutGut Settings',
        'settings_out_dir_header': 'OUTPUT DIRECTORY',
        'settings_out_dir_default_info': 'ⓘ Default: „outputs” folder next to the application.',
        'btn_choose_dir': 'Choose folder...',
        'btn_reset_dir': 'Reset to default',
        'settings_cleanup_header': 'AFTER SUCCESSFUL EXPORT',
        'opt_never': '◉ Keep original file (Recommended)',
        'opt_ask': '○ Ask what to do with original',
        'opt_trash': '○ Move original to Recycle Bin automatically',
        'opt_delete': '○ Delete original permanently automatically (⚠ Irreversible)',
        'settings_extra_header': 'ADDITIONAL OPTIONS',
        'chk_auto_open': 'Open output folder after export',
        'chk_auto_ab': 'Automatically open A/B comparison window',
        'settings_updates_header': 'UPDATES',
        'lbl_current_ver': 'Version: {version}',
        'btn_check_updates': '🔄 Check for updates',
        'chk_auto_check_updates': 'Check for updates on startup',
        'chk_include_prerelease': 'Show test/prerelease builds',
        'btn_save_settings': 'Save settings',
        'btn_cancel_settings': 'Cancel',
        'ask_cleanup_title': 'Original File Cleanup',
        'ask_cleanup_msg': 'Export completed successfully!\nDo you want to move the original source video to Recycle Bin?\n\n{path}',
        'perm_delete_warn_title': 'Permanent Deletion Warning',
        'perm_delete_warn_msg': 'Warning: This option will permanently delete original videos after each export.\n\nAre you sure you want to enable this?',
        'no_update_title': 'CutGut Updates',
        'no_update_msg': 'You have the latest version ({version})!'
    }
}

class UpdateAvailableDialog(QDialog):
    def __init__(self, parent, release: update_service.ReleaseInfo, current_version: str, lang: str = 'pl'):
        super().__init__(parent)
        self.release = release
        self.current_version = current_version
        self.lang = lang
        self.download_worker = None
        self.init_ui()

    def init_ui(self):
        self.setWindowTitle("Dostępna aktualizacja" if self.lang == 'pl' else "Update Available")
        self.setFixedWidth(480)
        self.setStyleSheet('''
            QDialog { background-color: #0f172a; }
            QLabel { color: #f8fafc; font-size: 13px; font-family: 'Segoe UI', system-ui; }
            QTextEdit { background-color: #131d2e; color: #cbd5e1; border: 1px solid #334155; border-radius: 6px; font-size: 12px; }
            QPushButton { 
                border-radius: 6px; padding: 7px 14px; color: white; 
                font-weight: bold; background-color: #1e293b; border: 1px solid #334155; 
            }
            QPushButton:hover { background-color: #3b82f6; border: 1px solid #60a5fa; }
            QProgressBar { 
                border: 1px solid #334155; border-radius: 6px; text-align: center; 
                color: white; font-weight: bold; background-color: #1e293b; height: 16px;
            }
            QProgressBar::chunk { background-color: #22c55e; border-radius: 5px; }
        ''')

        layout = QVBoxLayout(self)
        layout.setContentsMargins(20, 18, 20, 18)
        layout.setSpacing(10)

        hdr = QLabel(f"🚀 <b>{'Nowa wersja CutGut jest dostępna!' if self.lang == 'pl' else 'A new CutGut version is available!'}</b>")
        hdr.setStyleSheet('color: #60a5fa; font-size: 14px;')
        layout.addWidget(hdr)

        info_lbl = QLabel(f"<b>{'Twoja wersja:' if self.lang == 'pl' else 'Your version:'}</b> {self.current_version} &nbsp;&nbsp;➔&nbsp;&nbsp; <b>{'Nowa wersja:' if self.lang == 'pl' else 'New version:'}</b> <span style='color: #34d399;'>{self.release.tag_name}</span>")
        layout.addWidget(info_lbl)

        lbl_notes = QLabel(f"<b>{'Co nowego:' if self.lang == 'pl' else 'Changelog:'}</b>")
        layout.addWidget(lbl_notes)

        self.txtNotes = QTextEdit()
        self.txtNotes.setReadOnly(True)
        self.txtNotes.setPlainText(self.release.body if self.release.body else "Brak opisu zmian.")
        self.txtNotes.setFixedHeight(130)
        layout.addWidget(self.txtNotes)

        self.progBar = QProgressBar()
        self.progBar.setVisible(False)
        layout.addWidget(self.progBar)

        self.lblStatus = QLabel('')
        self.lblStatus.setStyleSheet('color: #94a3b8; font-size: 11px;')
        self.lblStatus.setVisible(False)
        layout.addWidget(self.lblStatus)

        btn_box = QHBoxLayout()
        btn_box.addStretch()

        self.btnLater = QPushButton("Później" if self.lang == 'pl' else "Later")
        self.btnLater.clicked.connect(self.reject)
        btn_box.addWidget(self.btnLater)

        self.btnUpdate = QPushButton("Pobierz i zaktualizuj" if self.lang == 'pl' else "Download & Update")
        self.btnUpdate.setStyleSheet('background-color: #2563eb; padding: 8px 18px;')
        self.btnUpdate.clicked.connect(self.start_download)
        btn_box.addWidget(self.btnUpdate)

        layout.addLayout(btn_box)

    def start_download(self):
        if not getattr(sys, 'frozen', False):
            QMessageBox.information(
                self, "CutGut",
                "Auto-update z poziomu programu jest przeznaczony dla pliku CutGut.exe. W wersji deweloperskiej zaktualizuj kod przez Git."
            )
            return

        self.btnUpdate.setEnabled(False)
        self.btnLater.setEnabled(False)
        self.progBar.setVisible(True)
        self.lblStatus.setVisible(True)
        self.lblStatus.setText("Pobieranie nowego pliku wykonywalnego..." if self.lang == 'pl' else "Downloading executable...")

        temp_dir = encoder.get_temp_dir()
        target_path = os.path.join(temp_dir, "CutGut.exe.new")

        self.download_worker = update_service.DownloadUpdateWorker(self.release.asset_url, target_path)
        self.download_worker.progress_signal.connect(self.on_download_progress)
        self.download_worker.finished_signal.connect(self.on_download_finished)
        self.download_worker.error_signal.connect(self.on_download_error)
        self.download_worker.start()

    def on_download_progress(self, downloaded: int, total: int):
        if total > 0:
            pct = int((downloaded / total) * 100)
            self.progBar.setValue(pct)
            self.lblStatus.setText(f"{downloaded / 1000000:.1f} MB / {total / 1000000:.1f} MB ({pct}%)")

    def on_download_finished(self, downloaded_path: str):
        self.lblStatus.setText("Instalowanie aktualizacji i ponowne uruchamianie..." if self.lang == 'pl' else "Installing update & restarting...")
        QApplication.processEvents()
        time.sleep(0.5)
        update_service.apply_update_and_restart(downloaded_path)

    def on_download_error(self, err_msg: str):
        self.btnUpdate.setEnabled(True)
        self.btnLater.setEnabled(True)
        self.progBar.setVisible(False)
        self.lblStatus.setText(f"❌ {err_msg}")
        QMessageBox.critical(self, "Błąd aktualizacji", err_msg)

class SettingsDialog(QDialog):
    def __init__(self, parent, current_policy: str, current_lang: str, output_dir: str, auto_open: bool, auto_ab: bool, auto_check_updates: bool, include_prerelease: bool):
        super().__init__(parent)
        self.parent_app = parent
        self.current_lang = current_lang
        self.selected_policy = current_policy
        self.output_dir = output_dir
        self.auto_open = auto_open
        self.auto_ab = auto_ab
        self.auto_check_updates = auto_check_updates
        self.include_prerelease = include_prerelease
        self.check_worker = None
        self.init_ui()

    def t(self, key: str) -> str:
        lang_dict = TRANSLATIONS.get(self.current_lang, TRANSLATIONS['pl'])
        return lang_dict.get(key, TRANSLATIONS['en'].get(key, ''))

    def init_ui(self):
        self.setWindowTitle(self.t('settings_title'))
        self.setFixedWidth(520)
        self.setStyleSheet('''
            QDialog { background-color: #0f172a; }
            QLabel { color: #f8fafc; font-size: 13px; font-family: 'Segoe UI', system-ui; }
            QRadioButton { color: #e2e8f0; font-size: 13px; padding: 4px; }
            QRadioButton:hover { color: #60a5fa; }
            QCheckBox { color: #e2e8f0; font-size: 13px; padding: 4px; }
            QPushButton { 
                border-radius: 6px; padding: 7px 14px; color: white; 
                font-weight: bold; background-color: #1e293b; border: 1px solid #334155; 
            }
            QPushButton:hover { background-color: #3b82f6; border: 1px solid #60a5fa; }
        ''')

        layout = QVBoxLayout(self)
        layout.setContentsMargins(22, 18, 22, 18)
        layout.setSpacing(12)

        # 1. FOLDER WYNIKOWY
        lbl_out_hdr = QLabel(f"📁 <b>{self.t('settings_out_dir_header')}</b>")
        lbl_out_hdr.setStyleSheet('color: #60a5fa; font-size: 13px;')
        layout.addWidget(lbl_out_hdr)

        dir_box = QHBoxLayout()
        display_path = self.output_dir if self.output_dir else encoder.get_default_output_dir()
        self.lblDirPath = QLabel(display_path)
        self.lblDirPath.setStyleSheet('background-color: #131d2e; border: 1px solid #334155; border-radius: 6px; padding: 7px; color: #cbd5e1;')
        dir_box.addWidget(self.lblDirPath, 1)

        self.btnBrowseDir = QPushButton(self.t('btn_choose_dir'))
        self.btnBrowseDir.clicked.connect(self.browse_output_dir)
        dir_box.addWidget(self.btnBrowseDir)
        layout.addLayout(dir_box)

        reset_box = QHBoxLayout()
        lbl_def_info = QLabel(self.t('settings_out_dir_default_info'))
        lbl_def_info.setStyleSheet('color: #64748b; font-size: 11px;')
        reset_box.addWidget(lbl_def_info, 1)

        self.btnResetDir = QPushButton(self.t('btn_reset_dir'))
        self.btnResetDir.setStyleSheet('font-size: 11px; padding: 4px 8px;')
        self.btnResetDir.clicked.connect(self.reset_output_dir)
        reset_box.addWidget(self.btnResetDir)
        layout.addLayout(reset_box)

        line1 = QFrame()
        line1.setFrameShape(QFrame.Shape.HLine)
        line1.setStyleSheet('color: #1e293b;')
        layout.addWidget(line1)

        # 2. PO UDANYM EKSPORCIE
        lbl_clean_hdr = QLabel(f"🛡️ <b>{self.t('settings_cleanup_header')}</b>")
        lbl_clean_hdr.setStyleSheet('color: #60a5fa; font-size: 13px;')
        layout.addWidget(lbl_clean_hdr)

        self.btn_group = QButtonGroup(self)
        self.rb_never = QRadioButton(self.t('opt_never'))
        self.rb_ask = QRadioButton(self.t('opt_ask'))
        self.rb_trash = QRadioButton(self.t('opt_trash'))
        self.rb_delete = QRadioButton(self.t('opt_delete'))

        self.btn_group.addButton(self.rb_never, 0)
        self.btn_group.addButton(self.rb_ask, 1)
        self.btn_group.addButton(self.rb_trash, 2)
        self.btn_group.addButton(self.rb_delete, 3)

        layout.addWidget(self.rb_never)
        layout.addWidget(self.rb_ask)
        layout.addWidget(self.rb_trash)
        layout.addWidget(self.rb_delete)

        if self.selected_policy == encoder.SourceCleanupPolicy.ASK.value:
            self.rb_ask.setChecked(True)
        elif self.selected_policy == encoder.SourceCleanupPolicy.TRASH.value:
            self.rb_trash.setChecked(True)
        elif self.selected_policy == encoder.SourceCleanupPolicy.DELETE_PERMANENTLY.value:
            self.rb_delete.setChecked(True)
        else:
            self.rb_never.setChecked(True)

        line2 = QFrame()
        line2.setFrameShape(QFrame.Shape.HLine)
        line2.setStyleSheet('color: #1e293b;')
        layout.addWidget(line2)

        # 3. DODATKOWE OPCJE
        lbl_extra_hdr = QLabel(f"⚙️ <b>{self.t('settings_extra_header')}</b>")
        lbl_extra_hdr.setStyleSheet('color: #60a5fa; font-size: 13px;')
        layout.addWidget(lbl_extra_hdr)

        self.chkAutoOpen = QCheckBox(self.t('chk_auto_open'))
        self.chkAutoOpen.setChecked(self.auto_open)
        layout.addWidget(self.chkAutoOpen)

        self.chkAutoAB = QCheckBox(self.t('chk_auto_ab'))
        self.chkAutoAB.setChecked(self.auto_ab)
        layout.addWidget(self.chkAutoAB)

        line3 = QFrame()
        line3.setFrameShape(QFrame.Shape.HLine)
        line3.setStyleSheet('color: #1e293b;')
        layout.addWidget(line3)

        # 4. AKTUALIZACJE
        lbl_upd_hdr = QLabel(f"🚀 <b>{self.t('settings_updates_header')}</b>")
        lbl_upd_hdr.setStyleSheet('color: #60a5fa; font-size: 13px;')
        layout.addWidget(lbl_upd_hdr)

        upd_row = QHBoxLayout()
        lbl_v = QLabel(self.t('lbl_current_ver').format(version=__version__))
        lbl_v.setStyleSheet('color: #94a3b8;')
        upd_row.addWidget(lbl_v, 1)

        self.btnCheckUpdates = QPushButton(self.t('btn_check_updates'))
        self.btnCheckUpdates.clicked.connect(self.check_updates_clicked)
        upd_row.addWidget(self.btnCheckUpdates)
        layout.addLayout(upd_row)

        self.chkAutoCheckUpdates = QCheckBox(self.t('chk_auto_check_updates'))
        self.chkAutoCheckUpdates.setChecked(self.auto_check_updates)
        layout.addWidget(self.chkAutoCheckUpdates)

        self.chkIncludePrerelease = QCheckBox(self.t('chk_include_prerelease'))
        self.chkIncludePrerelease.setChecked(self.include_prerelease)
        layout.addWidget(self.chkIncludePrerelease)

        layout.addSpacing(8)
        btn_box = QHBoxLayout()
        btn_box.addStretch()

        self.btnCancel = QPushButton(self.t('btn_cancel_settings'))
        self.btnCancel.clicked.connect(self.reject)
        btn_box.addWidget(self.btnCancel)

        self.btnSave = QPushButton(self.t('btn_save_settings'))
        self.btnSave.setStyleSheet('background-color: #2563eb; padding: 8px 18px;')
        self.btnSave.clicked.connect(self.on_save)
        btn_box.addWidget(self.btnSave)

        layout.addLayout(btn_box)

    def browse_output_dir(self):
        cur = self.output_dir if self.output_dir else encoder.get_default_output_dir()
        d = QFileDialog.getExistingDirectory(self, self.t('btn_choose_dir'), cur)
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

    def check_updates_clicked(self):
        self.btnCheckUpdates.setEnabled(False)
        self.check_worker = update_service.CheckUpdateWorker(__version__, self.chkIncludePrerelease.isChecked())
        self.check_worker.finished_signal.connect(self.on_update_check_result)
        self.check_worker.start()

    def on_update_check_result(self, rel: Optional[update_service.ReleaseInfo]):
        self.btnCheckUpdates.setEnabled(True)
        if rel:
            dlg = UpdateAvailableDialog(self, rel, __version__, self.current_lang)
            dlg.exec()
        else:
            QMessageBox.information(
                self, self.t('no_update_title'),
                self.t('no_update_msg').format(version=__version__)
            )

    def on_save(self):
        checked_id = self.btn_group.checkedId()
        if checked_id == 1:
            pol = encoder.SourceCleanupPolicy.ASK.value
        elif checked_id == 2:
            pol = encoder.SourceCleanupPolicy.TRASH.value
        elif checked_id == 3:
            res = QMessageBox.warning(
                self, self.t('perm_delete_warn_title'), self.t('perm_delete_warn_msg'),
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
        self.include_prerelease = self.chkIncludePrerelease.isChecked()
        self.accept()

class CutGutApp(QMainWindow):
    def __init__(self):
        super().__init__()

        self.settings = QSettings('Zeluqe', 'CutGut')
        self.current_lang = self.settings.value('language', 'pl')
        if self.current_lang not in ('pl', 'en'):
            self.current_lang = 'pl'

        self.cleanup_policy = self.settings.value('cleanup_policy', encoder.SourceCleanupPolicy.NEVER.value)
        self.custom_output_dir = self.settings.value('output_directory', '')
        self.auto_open_folder = (self.settings.value('auto_open_folder', 'true') == 'true')
        self.auto_ab_compare = (self.settings.value('auto_ab_compare', 'true') == 'true')
        self.auto_check_updates = (self.settings.value('auto_check_updates', 'true') == 'true')
        self.include_prerelease = (self.settings.value('include_prerelease', 'false') == 'true')

        self.setMinimumSize(1020, 910)
        self.setAcceptDrops(True)

        self.setStyleSheet('''
            QMainWindow { background-color: #0b0f19; }
            QLabel { color: #f8fafc; font-family: 'Segoe UI', system-ui; font-size: 13px; }
            QPushButton { 
                border-radius: 6px; padding: 8px 14px; color: white; 
                font-weight: bold; background-color: #1e293b; border: 1px solid #334155; 
            }
            QPushButton:hover { background-color: #3b82f6; border: 1px solid #60a5fa; }
            QPushButton:disabled { background-color: #131d2e; color: #475569; border: 1px solid #1e293b; }
            QSlider::groove:horizontal {
                border: 1px solid #334155; height: 10px; background: #1e293b;
                margin: 2px 0; border-radius: 5px;
            }
            QSlider::handle:horizontal {
                background: #3b82f6; border: 2px solid #60a5fa;
                width: 18px; height: 18px; margin: -5px 0; border-radius: 9px;
            }
            QComboBox { 
                background-color: #1e293b; color: white; border-radius: 6px; 
                padding: 7px 10px; border: 1px solid #334155; font-weight: bold;
            }
            QComboBox:hover { border: 1px solid #3b82f6; }
            QComboBox QAbstractItemView {
                background-color: #1e293b; color: white; selection-background-color: #3b82f6;
                border: 1px solid #334155;
            }
            QProgressBar { 
                border: 1px solid #334155; border-radius: 6px; text-align: center; 
                color: white; font-weight: bold; background-color: #1e293b;
            }
            QProgressBar::chunk { background-color: #3b82f6; border-radius: 5px; }
            QCheckBox { color: #cbd5e1; font-weight: bold; }
            QCheckBox::indicator { width: 16px; height: 16px; border-radius: 4px; border: 1px solid #475569; background: #1e293b; }
            QCheckBox::indicator:checked { background: #3b82f6; border: 1px solid #60a5fa; }
            QTableWidget {
                background-color: #131d2e; border: 1px solid #334155; border-radius: 6px;
                color: #e2e8f0; gridline-color: #1e293b;
            }
            QHeaderView::section {
                background-color: #1e293b; color: #94a3b8; font-weight: bold;
                border: 1px solid #334155; padding: 4px;
            }
        ''')

        self.input_file = ''
        self.video_info = None
        self.video_fps = 60.0
        self.start_ms = 0
        self.end_ms = 0
        self.custom_mb = 20.0
        self.current_crop: Optional[encoder.CropBox] = None
        
        self.worker = None
        self.sample_worker = None
        self.bg_update_worker = None
        self.comparison_dialog = None
        self.queue: list[encoder.EncodeJob] = []
        self.active_job: Optional[encoder.EncodeJob] = None

        # Media Player & Custom Crop Container
        self.mediaPlayer = QMediaPlayer()
        self.cropContainer = CropVideoContainer()
        self.cropContainer.clicked_signal.connect(self.play_pause)
        self.cropContainer.crop_changed_signal.connect(self.on_crop_changed)
        
        self.audioOutput = QAudioOutput()
        self.mediaPlayer.setVideoSink(self.cropContainer.sink)
        self.mediaPlayer.setAudioOutput(self.audioOutput)
        self.audioOutput.setVolume(0.7)

        self.init_ui()
        self.retranslate_ui()
        self.check_ffmpeg_startup()
        self.startup_maintenance()

    def t(self, key: str) -> any:
        lang_dict = TRANSLATIONS.get(self.current_lang, TRANSLATIONS['pl'])
        return lang_dict.get(key, TRANSLATIONS['en'].get(key, ''))

    def init_ui(self):
        main_layout = QVBoxLayout()
        main_layout.setContentsMargins(16, 10, 16, 10)
        main_layout.setSpacing(7)

        # 1. Ekran wideo z nakładką CropOverlay
        self.cropContainer.setStyleSheet('background-color: black; border-radius: 8px; border: 2px solid #1e293b;')
        self.cropContainer.setMinimumHeight(350)
        self.cropContainer.setToolTip(self.t('video_tooltip'))
        main_layout.addWidget(self.cropContainer, 1)

        # 2. Pasek czasu i kontrolki odtwarzacza + Przycisk PNG Frame
        player_panel = QWidget()
        player_layout = QVBoxLayout(player_panel)
        player_layout.setContentsMargins(0, 0, 0, 0)
        player_layout.setSpacing(4)

        self.positionSlider = QSlider(Qt.Orientation.Horizontal)
        self.positionSlider.setCursor(Qt.CursorShape.PointingHandCursor)
        player_layout.addWidget(self.positionSlider)

        controls_row = QHBoxLayout()
        self.playBtn = QPushButton()
        self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPlay))
        self.playBtn.setFixedWidth(44)
        self.playBtn.setStyleSheet('background-color: #3b82f6;')
        self.playBtn.clicked.connect(self.play_pause)
        controls_row.addWidget(self.playBtn)

        self.timeLabel = QLabel('00:00 / 00:00')
        self.timeLabel.setStyleSheet('font-weight: bold; margin-left: 6px; font-size: 13px;')
        controls_row.addWidget(self.timeLabel)

        self.chkLoop = QCheckBox()
        self.chkLoop.setStyleSheet('margin-left: 8px;')
        controls_row.addWidget(self.chkLoop)

        # Przycisk zapisu klatki jako PNG (📸)
        self.btnScreenshot = QPushButton(self.t('btn_screenshot'))
        self.btnScreenshot.setStyleSheet('background-color: #0f766e; font-size: 11px; padding: 5px 10px;')
        self.btnScreenshot.setEnabled(False)
        self.btnScreenshot.clicked.connect(self.capture_png_frame)
        controls_row.addWidget(self.btnScreenshot)

        controls_row.addStretch()

        # Głośność
        self.lblVol = QLabel()
        controls_row.addWidget(self.lblVol)
        self.volSlider = QSlider(Qt.Orientation.Horizontal)
        self.volSlider.setRange(0, 100)
        self.volSlider.setValue(70)
        self.volSlider.setFixedWidth(70)
        self.volSlider.valueChanged.connect(lambda v: self.audioOutput.setVolume(v / 100.0))
        controls_row.addWidget(self.volSlider)

        # Przełącznik języka (PL / EN)
        self.langCombo = QComboBox()
        self.langCombo.addItems(['🇵🇱 PL', '🇬🇧 EN'])
        self.langCombo.setFixedWidth(85)
        self.langCombo.setCurrentIndex(0 if self.current_lang == 'pl' else 1)
        self.langCombo.currentIndexChanged.connect(self.on_lang_changed)
        controls_row.addWidget(self.langCombo)

        # Przycisk Ustawień (⚙)
        self.btnSettings = QPushButton('⚙')
        self.btnSettings.setFixedWidth(38)
        self.btnSettings.setStyleSheet('background-color: #1e293b; font-size: 14px;')
        self.btnSettings.clicked.connect(self.open_settings)
        controls_row.addWidget(self.btnSettings)

        player_layout.addLayout(controls_row)
        main_layout.addWidget(player_panel)

        # 3. Karta Zakresu: IN / DŁUGOŚĆ / OUT
        trim_card = QFrame()
        trim_card.setStyleSheet('background-color: #131d2e; border-radius: 8px; border: 1px solid #1e293b;')
        trim_layout = QHBoxLayout(trim_card)
        trim_layout.setContentsMargins(10, 4, 10, 4)
        trim_layout.setSpacing(10)

        in_box = QHBoxLayout()
        self.lblInTime = QLabel('IN: 00:00.00')
        self.lblInTime.setStyleSheet('font-weight: bold; color: #34d399; font-size: 13px;')
        self.btnStart = QPushButton()
        self.btnStart.setStyleSheet('background-color: #059669; padding: 4px 10px;')
        self.btnStart.clicked.connect(self.set_start)
        in_box.addWidget(self.lblInTime)
        in_box.addWidget(self.btnStart)
        trim_layout.addLayout(in_box)

        self.lblDuration = QLabel('⏱️ DŁUGOŚĆ: 00:00.00')
        self.lblDuration.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.lblDuration.setStyleSheet('font-size: 13px; font-weight: bold; color: #60a5fa;')
        trim_layout.addWidget(self.lblDuration, 1)

        out_box = QHBoxLayout()
        self.lblOutTime = QLabel('OUT: 00:00.00')
        self.lblOutTime.setStyleSheet('font-weight: bold; color: #f87171; font-size: 13px;')
        self.btnEnd = QPushButton()
        self.btnEnd.setStyleSheet('background-color: #dc2626; padding: 4px 10px;')
        self.btnEnd.clicked.connect(self.set_end)
        out_box.addWidget(self.lblOutTime)
        out_box.addWidget(self.btnEnd)
        trim_layout.addLayout(out_box)

        main_layout.addWidget(trim_card)

        # 4. Centrum Format & Social Presets + Plan Jakości
        format_card = QFrame()
        format_card.setStyleSheet('background-color: #1e293b; border-radius: 8px; border: 1px solid #334155;')
        fmt_layout = QVBoxLayout(format_card)
        fmt_layout.setContentsMargins(12, 6, 12, 6)
        fmt_layout.setSpacing(4)

        row_fmt = QHBoxLayout()
        
        # Social Profile Preset
        self.lblProf = QLabel('📱 <b>Preset:</b>')
        self.lblProf.setStyleSheet('color: #60a5fa; font-size: 12px;')
        row_fmt.addWidget(self.lblProf)

        self.socialCombo = QComboBox()
        self.socialCombo.setFixedWidth(205)
        self.socialCombo.currentIndexChanged.connect(self.on_social_preset_changed)
        row_fmt.addWidget(self.socialCombo)

        # Aspect Ratio / Kadrowanie
        self.lblCrop = QLabel('📐 <b>Kadr:</b>')
        self.lblCrop.setStyleSheet('color: #60a5fa; font-size: 12px; margin-left: 6px;')
        row_fmt.addWidget(self.lblCrop)

        self.ratioCombo = QComboBox()
        self.ratioCombo.setFixedWidth(190)
        self.ratioCombo.currentIndexChanged.connect(self.on_ratio_changed)
        row_fmt.addWidget(self.ratioCombo)

        # Przyciski wyrównania kadru (Lewo / Środek / Prawo)
        self.alignBox = QWidget()
        align_layout = QHBoxLayout(self.alignBox)
        align_layout.setContentsMargins(0, 0, 0, 0)
        align_layout.setSpacing(3)

        self.btnAlignLeft = QPushButton(self.t('btn_align_left'))
        self.btnAlignLeft.setStyleSheet('font-size: 10px; padding: 4px 7px;')
        self.btnAlignLeft.clicked.connect(lambda: self.cropContainer.overlay.set_alignment('left'))
        align_layout.addWidget(self.btnAlignLeft)

        self.btnAlignCenter = QPushButton(self.t('btn_align_center'))
        self.btnAlignCenter.setStyleSheet('font-size: 10px; padding: 4px 7px; background-color: #2563eb;')
        self.btnAlignCenter.clicked.connect(lambda: self.cropContainer.overlay.set_alignment('center'))
        align_layout.addWidget(self.btnAlignCenter)

        self.btnAlignRight = QPushButton(self.t('btn_align_right'))
        self.btnAlignRight.setStyleSheet('font-size: 10px; padding: 4px 7px;')
        self.btnAlignRight.clicked.connect(lambda: self.cropContainer.overlay.set_alignment('right'))
        align_layout.addWidget(self.btnAlignRight)

        self.alignBox.setVisible(False)
        row_fmt.addWidget(self.alignBox)

        row_fmt.addStretch()

        self.btnTestSample = QPushButton(self.t('btn_test_sample'))
        self.btnTestSample.setStyleSheet('background-color: #0e7490; font-size: 11px; padding: 4px 10px;')
        self.btnTestSample.setEnabled(False)
        self.btnTestSample.clicked.connect(self.create_sample_preview)
        row_fmt.addWidget(self.btnTestSample)

        self.lblQualityBadge = QLabel('● Oczekiwanie na film')
        self.lblQualityBadge.setStyleSheet('font-weight: bold; font-size: 13px; color: #94a3b8; margin-left: 6px;')
        row_fmt.addWidget(self.lblQualityBadge)

        fmt_layout.addLayout(row_fmt)

        # Linia 2: Parametry techniczne i Wskazówki
        self.lblPlanDetails = QLabel('Wybierz wideo, aby zobaczyć planowane parametry kodowania.')
        self.lblPlanDetails.setStyleSheet('font-size: 12px; font-weight: 600; color: #f8fafc;')
        fmt_layout.addWidget(self.lblPlanDetails)

        self.lblPlanTip = QLabel('')
        self.lblPlanTip.setStyleSheet('font-size: 11px; color: #38bdf8;')
        self.lblPlanTip.setWordWrap(True)
        fmt_layout.addWidget(self.lblPlanTip)

        main_layout.addWidget(format_card)

        # 5. Dół: Konfiguracja (Źródło, Limit, Enkoder) i Akcje
        ctrl_card = QFrame()
        ctrl_card.setStyleSheet('background-color: #131d2e; border-radius: 8px; border: 1px solid #1e293b;')
        ctrl_layout = QVBoxLayout(ctrl_card)
        ctrl_layout.setContentsMargins(12, 7, 12, 7)
        ctrl_layout.setSpacing(6)

        row_cfg = QHBoxLayout()
        self.btnSelect = QPushButton()
        self.btnSelect.setStyleSheet('background-color: #2563eb; padding: 6px 12px;')
        self.btnSelect.clicked.connect(self.open_file)
        row_cfg.addWidget(self.btnSelect)

        self.lblSourceFile = QLabel(self.t('lbl_no_file'))
        self.lblSourceFile.setStyleSheet('color: #cbd5e1; font-weight: 500;')
        row_cfg.addWidget(self.lblSourceFile, 1)

        self.limitCombo = QComboBox()
        self.limitCombo.setFixedWidth(190)
        self.limitCombo.currentIndexChanged.connect(self.on_limit_changed)
        row_cfg.addWidget(self.limitCombo)

        self.modeCombo = QComboBox()
        self.modeCombo.setFixedWidth(205)
        self.modeCombo.currentIndexChanged.connect(self.update_live_estimate)
        row_cfg.addWidget(self.modeCombo)
        ctrl_layout.addLayout(row_cfg)

        row_act = QHBoxLayout()
        self.btnCompress = QPushButton()
        self.btnCompress.setStyleSheet('background-color: #2563eb; font-size: 13px; padding: 8px 18px; font-weight: bold;')
        self.btnCompress.setEnabled(False)
        self.btnCompress.clicked.connect(self.start_or_run_queue)
        row_act.addWidget(self.btnCompress, 2)

        self.btnAddToQueue = QPushButton()
        self.btnAddToQueue.setStyleSheet('background-color: #0891b2; font-size: 12px; padding: 8px 12px;')
        self.btnAddToQueue.setEnabled(False)
        self.btnAddToQueue.clicked.connect(self.add_to_queue)
        row_act.addWidget(self.btnAddToQueue, 1)

        self.btnCancel = QPushButton()
        self.btnCancel.setStyleSheet('background-color: #1e293b; color: #64748b; min-width: 80px; font-size: 12px;')
        self.btnCancel.setEnabled(False)
        self.btnCancel.clicked.connect(self.cancel_compression)
        row_act.addWidget(self.btnCancel)
        ctrl_layout.addLayout(row_act)

        main_layout.addWidget(ctrl_card)

        # 6. Panel Kolejki
        self.queueContainer = QWidget()
        self.queueLayout = QVBoxLayout(self.queueContainer)
        self.queueLayout.setContentsMargins(0, 0, 0, 0)
        self.queueLayout.setSpacing(4)

        self.lblQueueHeader = QLabel(self.t('lbl_queue_header').format(count=0))
        self.lblQueueHeader.setStyleSheet('font-weight: bold; color: #60a5fa; font-size: 12px;')
        self.queueLayout.addWidget(self.lblQueueHeader)

        self.queueTable = QTableWidget()
        self.queueTable.setColumnCount(7)
        self.queueTable.setHorizontalHeaderLabels(['Plik', 'Kadr', 'Zakres', 'Limit', 'Enkoder', 'Status', 'Akcja'])
        self.queueTable.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self.queueTable.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(4, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(5, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(6, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.setMaximumHeight(90)
        self.queueTable.cellClicked.connect(self.on_queue_table_clicked)
        self.queueLayout.addWidget(self.queueTable)
        self.queueContainer.setVisible(False)
        main_layout.addWidget(self.queueContainer)

        # 7. Pasek postępu i status
        self.progBar = QProgressBar()
        self.progBar.setFixedHeight(16)
        main_layout.addWidget(self.progBar)

        self.statusLabel = QLabel()
        self.statusLabel.setAlignment(Qt.AlignmentFlag.AlignCenter)
        main_layout.addWidget(self.statusLabel)

        container = QWidget()
        container.setLayout(main_layout)
        self.setCentralWidget(container)

        self.mediaPlayer.positionChanged.connect(self.position_changed)
        self.mediaPlayer.durationChanged.connect(self.duration_changed)
        self.positionSlider.sliderMoved.connect(self.set_position)

    def startup_maintenance(self):
        base_dir = encoder.get_base_dir()
        for f in ['CutGut.exe.bak', 'CutGut.exe.new', 'cutgut_updater.bat']:
            p = os.path.join(base_dir, f)
            if os.path.exists(p):
                try: os.remove(p)
                except Exception: pass

        if self.auto_check_updates:
            self.bg_update_worker = update_service.CheckUpdateWorker(__version__, self.include_prerelease)
            self.bg_update_worker.finished_signal.connect(self.on_background_update_found)
            self.bg_update_worker.start()

    def on_background_update_found(self, rel: Optional[update_service.ReleaseInfo]):
        if rel:
            dlg = UpdateAvailableDialog(self, rel, __version__, self.current_lang)
            dlg.exec()

    def on_crop_changed(self, cb: encoder.CropBox):
        self.current_crop = cb
        self.update_live_estimate()

    def on_ratio_changed(self, idx: int):
        ratios = ['original', '9:16', '1:1', '16:9']
        r = ratios[idx] if idx < len(ratios) else 'original'
        self.cropContainer.overlay.set_ratio_type(r)
        self.alignBox.setVisible(r != 'original')
        self.update_live_estimate()

    def on_social_preset_changed(self, idx: int):
        # ['Discord Clip (20 MB)', 'Discord Nitro (50 MB)', 'Shorts / TikTok HQ (50 MB)', 'Shorts Small (20 MB)', 'TikTok / Reels (50 MB)', 'Square Meme (20 MB)', 'Custom']
        self.ratioCombo.blockSignals(True)
        self.limitCombo.blockSignals(True)

        if idx == 0: # Discord Clip (20 MB, 16:9)
            self.ratioCombo.setCurrentIndex(0)
            self.limitCombo.setCurrentIndex(0) # 20 MB
            self.cropContainer.overlay.set_ratio_type('original')
            self.alignBox.setVisible(False)
        elif idx == 1: # Discord Nitro (50 MB, 16:9)
            self.ratioCombo.setCurrentIndex(0)
            self.limitCombo.setCurrentIndex(2) # 50 MB
            self.cropContainer.overlay.set_ratio_type('original')
            self.alignBox.setVisible(False)
        elif idx == 2: # Shorts HQ (9:16, 50 MB)
            self.ratioCombo.setCurrentIndex(1) # 9:16
            self.limitCombo.setCurrentIndex(2) # 50 MB
            self.cropContainer.overlay.set_ratio_type('9:16')
            self.alignBox.setVisible(True)
        elif idx == 3: # Shorts Small (9:16, 20 MB)
            self.ratioCombo.setCurrentIndex(1) # 9:16
            self.limitCombo.setCurrentIndex(0) # 20 MB
            self.cropContainer.overlay.set_ratio_type('9:16')
            self.alignBox.setVisible(True)
        elif idx == 4: # TikTok / Reels (9:16, 50 MB)
            self.ratioCombo.setCurrentIndex(1) # 9:16
            self.limitCombo.setCurrentIndex(2) # 50 MB
            self.cropContainer.overlay.set_ratio_type('9:16')
            self.alignBox.setVisible(True)
        elif idx == 5: # Square Meme (1:1, 20 MB)
            self.ratioCombo.setCurrentIndex(2) # 1:1
            self.limitCombo.setCurrentIndex(0) # 20 MB
            self.cropContainer.overlay.set_ratio_type('1:1')
            self.alignBox.setVisible(True)

        self.ratioCombo.blockSignals(False)
        self.limitCombo.blockSignals(False)
        self.update_live_estimate()

    def capture_png_frame(self):
        if not self.input_file or not os.path.exists(self.input_file):
            return

        cur_time_s = self.mediaPlayer.position() / 1000.0
        out_dir = self.custom_output_dir if self.custom_output_dir else encoder.get_default_output_dir()
        base_name = os.path.splitext(os.path.basename(self.input_file))[0]
        ts_str = time.strftime('%Y%m%d_%H%M%S')
        png_path = os.path.join(out_dir, f"CutGut_{base_name}_{ts_str}_{int(cur_time_s*1000)}ms.png")

        try:
            crop = self.cropContainer.overlay.get_crop_box()
            saved = encoder.extract_frame_png(self.input_file, cur_time_s, png_path, crop)
            self.statusLabel.setText(self.t('screenshot_saved').format(path=os.path.basename(saved)))
            if self.auto_open_folder:
                os.startfile(out_dir)
        except Exception as e:
            QMessageBox.critical(self, "Błąd zrzutu klatki", str(e))

    def keyPressEvent(self, event):
        key = event.key()
        modifiers = event.modifiers()
        
        if key == Qt.Key.Key_I:
            self.set_start()
            event.accept()
        elif key == Qt.Key.Key_O:
            self.set_end()
            event.accept()
        elif key == Qt.Key.Key_Space:
            self.play_pause()
            event.accept()
        elif key == Qt.Key.Key_Left:
            step_ms = int(1000.0 / max(self.video_fps, 1.0)) if (modifiers & Qt.KeyboardModifier.ShiftModifier) else 1000
            self.mediaPlayer.setPosition(max(self.mediaPlayer.position() - step_ms, 0))
            event.accept()
        elif key == Qt.Key.Key_Right:
            step_ms = int(1000.0 / max(self.video_fps, 1.0)) if (modifiers & Qt.KeyboardModifier.ShiftModifier) else 1000
            self.mediaPlayer.setPosition(min(self.mediaPlayer.position() + step_ms, self.mediaPlayer.duration()))
            event.accept()
        elif key == Qt.Key.Key_Escape:
            if self.worker and self.worker.isRunning():
                self.cancel_compression()
            elif self.sample_worker and self.sample_worker.isRunning():
                self.sample_worker.cancel()
            event.accept()
        else:
            super().keyPressEvent(event)

    def on_lang_changed(self, idx: int):
        self.current_lang = 'pl' if idx == 0 else 'en'
        self.settings.setValue('language', self.current_lang)
        self.retranslate_ui()

    def open_settings(self):
        dlg = SettingsDialog(
            self,
            self.cleanup_policy,
            self.current_lang,
            self.custom_output_dir,
            self.auto_open_folder,
            self.auto_ab_compare,
            self.auto_check_updates,
            self.include_prerelease
        )
        if dlg.exec() == QDialog.DialogCode.Accepted:
            self.cleanup_policy = dlg.selected_policy
            self.custom_output_dir = dlg.output_dir
            self.auto_open_folder = dlg.auto_open
            self.auto_ab_compare = dlg.auto_ab
            self.auto_check_updates = dlg.auto_check_updates
            self.include_prerelease = dlg.include_prerelease

            self.settings.setValue('cleanup_policy', self.cleanup_policy)
            self.settings.setValue('output_directory', self.custom_output_dir)
            self.settings.setValue('auto_open_folder', 'true' if self.auto_open_folder else 'false')
            self.settings.setValue('auto_ab_compare', 'true' if self.auto_ab_compare else 'false')
            self.settings.setValue('auto_check_updates', 'true' if self.auto_check_updates else 'false')
            self.settings.setValue('include_prerelease', 'true' if self.include_prerelease else 'false')

    def retranslate_ui(self):
        self.setWindowTitle(self.t('title').format(version=__version__))

        self.btnStart.setText(self.t('btn_set_in'))
        self.btnEnd.setText(self.t('btn_set_out'))
        self.chkLoop.setText(self.t('chk_loop'))
        self.lblVol.setText(self.t('lbl_volume'))
        self.btnSelect.setText(self.t('btn_change_file') if self.input_file else self.t('btn_select'))
        self.btnAddToQueue.setText(self.t('btn_add_queue'))
        self.btnCompress.setText(self.t('btn_compress'))
        self.btnCancel.setText(self.t('btn_cancel'))
        self.btnScreenshot.setText(self.t('btn_screenshot'))
        self.btnAlignLeft.setText(self.t('btn_align_left'))
        self.btnAlignCenter.setText(self.t('btn_align_center'))
        self.btnAlignRight.setText(self.t('btn_align_right'))
        self.lblProf.setText(f"📱 <b>{'Preset:' if self.current_lang == 'pl' else 'Preset:'}</b>")
        self.lblCrop.setText(f"📐 <b>{'Kadr:' if self.current_lang == 'pl' else 'Crop:'}</b>")
        self.btnTestSample.setText(self.t('btn_test_sample'))
        self.cropContainer.setToolTip(self.t('video_tooltip'))

        self.update_range_text()

        if not self.input_file:
            self.statusLabel.setText(self.t('ready_status'))
            self.lblSourceFile.setText(self.t('lbl_no_file'))

        # Social Presets
        cur_soc_idx = max(self.socialCombo.currentIndex(), 0)
        self.socialCombo.blockSignals(True)
        self.socialCombo.clear()
        self.socialCombo.addItems(self.t('social_presets'))
        self.socialCombo.setCurrentIndex(cur_soc_idx)
        self.socialCombo.blockSignals(False)

        # Aspect Ratios
        cur_rat_idx = max(self.ratioCombo.currentIndex(), 0)
        self.ratioCombo.blockSignals(True)
        self.ratioCombo.clear()
        self.ratioCombo.addItems(self.t('aspect_ratios'))
        self.ratioCombo.setCurrentIndex(cur_rat_idx)
        self.ratioCombo.blockSignals(False)

        # Presets MB
        cur_limit_idx = max(self.limitCombo.currentIndex(), 0)
        self.limitCombo.blockSignals(True)
        self.limitCombo.clear()
        presets = list(self.t('presets'))
        if self.custom_mb != 20.0 and len(presets) > 4:
            presets[4] = self.t('custom_preset_label').format(val=self.custom_mb)
        self.limitCombo.addItems(presets)
        self.limitCombo.setCurrentIndex(cur_limit_idx)
        self.limitCombo.blockSignals(False)

        # Enkodery
        cur_mode_idx = max(self.modeCombo.currentIndex(), 0)
        self.modeCombo.blockSignals(True)
        self.modeCombo.clear()
        if encoder.check_nvenc_support():
            self.modeCombo.addItems(self.t('encoders_nvenc'))
        elif encoder.check_amf_support():
            self.modeCombo.addItems(self.t('encoders_amf'))
        else:
            self.modeCombo.addItems(self.t('encoders_cpu'))
        if cur_mode_idx < self.modeCombo.count():
            self.modeCombo.setCurrentIndex(cur_mode_idx)
        self.modeCombo.blockSignals(False)

        self.update_live_estimate()
        self.update_queue_table()

    def check_ffmpeg_startup(self):
        if not encoder.ensure_ffmpeg():
            self.statusLabel.setText(self.t('ffmpeg_missing_status'))

    def dragEnterEvent(self, event):
        if event.mimeData().hasUrls():
            for url in event.mimeData().urls():
                fp = url.toLocalFile()
                if fp.lower().endswith(('.mp4', '.mkv', '.avi', '.mov', '.webm', '.flv', '.ts')):
                    event.acceptProposedAction()
                    return
        event.ignore()

    def dropEvent(self, event):
        for url in event.mimeData().urls():
            fp = url.toLocalFile()
            if fp.lower().endswith(('.mp4', '.mkv', '.avi', '.mov', '.webm', '.flv', '.ts')):
                self.load_video(fp)
                break

    def get_default_video_dir(self) -> str:
        saved = self.settings.value('last_video_dir', '')
        if saved and os.path.exists(saved):
            return saved

        user_profile = os.environ.get('USERPROFILE', os.path.expanduser('~'))
        nvidia_path = os.path.join(user_profile, 'Videos', 'NVIDIA')
        if os.path.exists(nvidia_path):
            return nvidia_path

        videos_path = os.path.join(user_profile, 'Videos')
        if os.path.exists(videos_path):
            return videos_path
        return user_profile

    def open_file(self):
        default_dir = self.get_default_video_dir()
        fp, _ = QFileDialog.getOpenFileName(
            self, self.t('dialog_open_title'), default_dir,
            self.t('dialog_open_filter')
        )
        if fp:
            self.settings.setValue('last_video_dir', os.path.dirname(fp))
            self.load_video(fp)

    def load_video(self, file_path: str):
        if file_path and os.path.exists(file_path):
            self.input_file = file_path
            try:
                self.video_info = encoder.probe_video(file_path)
                self.video_fps = self.video_info.fps
                self.cropContainer.overlay.set_video_dimensions(self.video_info.width, self.video_info.height)
            except Exception:
                self.video_info = None
                self.video_fps = 60.0

            self.mediaPlayer.setSource(QUrl.fromLocalFile(file_path))
            self.btnCompress.setEnabled(True)
            self.btnAddToQueue.setEnabled(True)
            self.btnTestSample.setEnabled(True)
            self.btnScreenshot.setEnabled(True)
            self.btnSelect.setText(self.t('btn_change_file'))
            self.lblSourceFile.setText(f"{os.path.basename(file_path)} ({self.video_info.width}x{self.video_info.height} @ {self.video_fps:.0f}fps)" if self.video_info else os.path.basename(file_path))
            self.statusLabel.setText(self.t('loaded_status').format(filename=os.path.basename(file_path)))
            self.mediaPlayer.play()
            self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPause))
            self.update_live_estimate()

    def play_pause(self):
        if self.mediaPlayer.playbackState() == QMediaPlayer.PlaybackState.PlayingState:
            self.mediaPlayer.pause()
            self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPlay))
        else:
            self.mediaPlayer.play()
            self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPause))

    def set_position(self, pos):
        self.mediaPlayer.setPosition(pos)

    def position_changed(self, pos):
        if not self.positionSlider.isSliderDown():
            self.positionSlider.setValue(pos)
        self.timeLabel.setText(f'{self.format_time(pos)} / {self.format_time(self.mediaPlayer.duration())}')

        if self.chkLoop.isChecked() and self.end_ms > self.start_ms:
            if pos >= self.end_ms or pos < self.start_ms:
                self.mediaPlayer.setPosition(self.start_ms)

    def duration_changed(self, dur):
        self.positionSlider.setRange(0, dur)
        self.start_ms = 0
        self.end_ms = dur
        self.update_range_text()

    def format_time(self, ms: int) -> str:
        s = ms // 1000
        m, s = divmod(s, 60)
        return f'{m:02d}:{s:02d}'

    def set_start(self):
        self.start_ms = self.mediaPlayer.position()
        if self.end_ms <= self.start_ms:
            self.end_ms = min(self.start_ms + 15000, self.mediaPlayer.duration())
        self.update_range_text()

    def set_end(self):
        self.end_ms = self.mediaPlayer.position()
        if self.end_ms <= self.start_ms:
            self.start_ms = max(self.end_ms - 15000, 0)
        self.update_range_text()

    def update_range_text(self):
        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        dur_s = max(end_s - start_s, 0.0)

        self.lblInTime.setText(f"IN: {self.format_time(self.start_ms)}.{int((self.start_ms % 1000)/10):02d}")
        self.lblOutTime.setText(f"OUT: {self.format_time(self.end_ms)}.{int((self.end_ms % 1000)/10):02d}")
        self.lblDuration.setText(self.t('duration_label').format(dur=dur_s))
        self.update_live_estimate()

    def update_live_estimate(self):
        if not self.input_file or not self.video_info:
            self.lblQualityBadge.setText('● ' + ('Brak pliku' if self.current_lang == 'pl' else 'No file'))
            self.lblQualityBadge.setStyleSheet('font-weight: bold; color: #94a3b8;')
            self.lblPlanDetails.setText('Wybierz film, aby zobaczyć plan i przewidywaną jakość.' if self.current_lang == 'pl' else 'Select a video to see export plan and quality assessment.')
            self.lblPlanTip.setText('')
            return

        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        dur_s = max(end_s - start_s, 0.1)
        target_mb = self.get_selected_target_mb()
        mode = self.get_selected_encoder_mode()
        is_hevc = (mode == 'CPU_HEVC')
        crop = self.cropContainer.overlay.get_crop_box()

        plan = encoder.calculate_plan(
            self.video_info, start_s, end_s, target_mb, is_hevc, self.input_file, self.current_lang, crop_box=crop
        )
        q: encoder.QualityAssessment = plan['quality']

        self.lblQualityBadge.setText(f"● {q.label}")
        self.lblQualityBadge.setStyleSheet(f"font-weight: bold; font-size: 13px; color: {q.color};")

        crop_info = f" [Kadr: {crop.ratio_type}]" if crop.ratio_type != "original" else ""

        if plan['is_remux']:
            self.lblPlanDetails.setText(
                f"{self.video_info.width}×{self.video_info.height} · {self.video_fps:.0f} FPS · Błyskawiczny Remux (Direct Stream Copy) · Limit: {target_mb} MB"
            )
        else:
            enc_name = self.modeCombo.currentText().replace('⚡ ', '').replace('🔴 ', '').replace('🚀 ', '').replace('⚖️ ', '').replace('💨 ', '').replace('💎 ', '')
            self.lblPlanDetails.setText(
                f"{plan['out_width']}×{plan['out_height']}{crop_info} · {plan['out_fps']:.0f} FPS · {enc_name} · ~{plan['video_kbps']} kbps · Cel: ~{plan['target_bytes']/1000000:.1f} MB (Limit: {target_mb:.1f} MB)"
            )

        tip_text = f"<b>{q.description}</b>"
        if q.tip:
            tip_text += f"<br><span style='color: #38bdf8;'>💡 <b>Wskazówka:</b> {q.tip}</span>"
        self.lblPlanTip.setText(tip_text)

    def create_sample_preview(self):
        if not self.input_file or not self.video_info:
            return

        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        target_mb = self.get_selected_target_mb()
        mode = self.get_selected_encoder_mode()
        is_hevc = (mode == 'CPU_HEVC')
        crop = self.cropContainer.overlay.get_crop_box()

        plan = encoder.calculate_plan(
            self.video_info, start_s, end_s, target_mb, is_hevc, self.input_file, self.current_lang, crop_box=crop
        )

        if plan['is_remux']:
            QMessageBox.information(self, "CutGut", self.t('sample_remux_msg'))
            return

        cur_center_s = self.mediaPlayer.position() / 1000.0
        self.statusLabel.setText(self.t('sample_generating'))
        self.btnTestSample.setEnabled(False)

        self.sample_worker = SamplePreviewWorker(
            input_path=self.input_file,
            center_s=cur_center_s,
            plan=plan,
            preset_mode=mode,
            crop_box=crop
        )
        self.sample_worker.finished_signal.connect(lambda sp: self.on_sample_preview_finished(sp, cur_center_s, plan, mode, crop))
        self.sample_worker.error_signal.connect(self.on_sample_preview_error)
        self.sample_worker.start()

    def on_sample_preview_finished(self, sample_path: str, center_s: float, plan: dict, mode: str, crop: Optional[encoder.CropBox]):
        self.btnTestSample.setEnabled(True)
        self.statusLabel.setText(self.t('sample_done'))

        half_dur = 3.0
        start_s = max(center_s - half_dur, 0.0)
        dur_s = 6.0
        actual_size = os.path.getsize(sample_path) if os.path.exists(sample_path) else 0

        res = encoder.ExportResult(
            input_path=self.input_file,
            output_path=sample_path,
            start_s=start_s,
            end_s=start_s + dur_s,
            duration_s=dur_s,
            target_mb=self.get_selected_target_mb(),
            actual_size_bytes=actual_size,
            is_remux=False,
            is_sample=True,
            plan=plan,
            preset_mode=mode,
            cleanup_policy="never",
            crop_box=crop
        )
        self.open_ab_comparison(res)

    def on_sample_preview_error(self, err_msg: str):
        self.btnTestSample.setEnabled(True)
        self.statusLabel.setText(f"❌ {err_msg}")
        QMessageBox.warning(self, "Błąd próbki jakości", err_msg)

    def open_ab_comparison(self, result: encoder.ExportResult):
        self.comparison_dialog = QualityComparisonDialog(self, result, self.current_lang)
        self.comparison_dialog.closed_signal.connect(self.on_comparison_closed)
        self.comparison_dialog.show()

    def on_comparison_closed(self, result: encoder.ExportResult):
        if not result.is_sample and result.cleanup_policy != "never":
            self.execute_cleanup_policy(result.input_path, result.output_path, result.cleanup_policy)

    def execute_cleanup_policy(self, input_path: str, output_path: str, policy: str):
        if not os.path.exists(output_path) or os.path.getsize(output_path) <= 0:
            return

        if policy == encoder.SourceCleanupPolicy.ASK.value:
            res = QMessageBox.question(
                self, self.t('ask_cleanup_title'),
                self.t('ask_cleanup_msg').format(path=os.path.basename(input_path)),
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if res == QMessageBox.StandardButton.Yes:
                encoder.cleanup_source_file(input_path, output_path, encoder.SourceCleanupPolicy.TRASH)
        elif policy in (encoder.SourceCleanupPolicy.TRASH.value, encoder.SourceCleanupPolicy.DELETE_PERMANENTLY.value):
            encoder.cleanup_source_file(input_path, output_path, encoder.SourceCleanupPolicy(policy))

    def on_limit_changed(self, idx: int):
        if idx == 4:
            val, ok = QInputDialog.getDouble(
                self, self.t('custom_input_title'), self.t('custom_input_msg'),
                self.custom_mb, 1.0, 2000.0, 1
            )
            if ok:
                self.custom_mb = val
                self.limitCombo.setItemText(4, self.t('custom_preset_label').format(val=val))
            else:
                self.limitCombo.setCurrentIndex(0)
        self.update_live_estimate()

    def get_selected_target_mb(self) -> float:
        idx = self.limitCombo.currentIndex()
        if idx == 0: return 20.0
        elif idx == 1: return 10.0
        elif idx == 2: return 50.0
        elif idx == 3: return 500.0
        else: return self.custom_mb

    def get_selected_encoder_mode(self) -> str:
        idx = self.modeCombo.currentIndex()
        if encoder.check_nvenc_support():
            if idx == 0: return 'NVENC_HQ'
            elif idx == 1: return 'NVENC_FAST'
            elif idx == 2: return 'CPU_BALANCED'
            elif idx == 3: return 'CPU_FAST'
            elif idx == 4: return 'CPU_HEVC'
        elif encoder.check_amf_support():
            if idx == 0: return 'AMF_HQ'
            elif idx == 1: return 'AMF_FAST'
            elif idx == 2: return 'CPU_BALANCED'
            elif idx == 3: return 'CPU_FAST'
            elif idx == 4: return 'CPU_HEVC'
        else:
            if idx == 0: return 'CPU_BALANCED'
            elif idx == 1: return 'CPU_FAST'
            elif idx == 2: return 'CPU_HEVC'
        return 'CPU_BALANCED'

    def add_to_queue(self):
        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        if (end_s - start_s) <= 0.1:
            QMessageBox.warning(self, self.t('invalid_range_title'), self.t('invalid_range_msg'))
            return

        out_dir = self.custom_output_dir if self.custom_output_dir else encoder.get_default_output_dir()
        base_name = os.path.splitext(os.path.basename(self.input_file))[0]
        crop = self.cropContainer.overlay.get_crop_box()
        crop_tag = f"_{crop.ratio_type.replace(':', '_')}" if crop.ratio_type != "original" else ""
        output_file = encoder.generate_output_filepath(output_dir=out_dir, base_name=f"{base_name}{crop_tag}")

        job = encoder.EncodeJob(
            job_id=str(int(time.time() * 1000)),
            input_path=self.input_file,
            output_path=output_file,
            start_s=start_s,
            end_s=end_s,
            target_mb=self.get_selected_target_mb(),
            preset_mode=self.get_selected_encoder_mode(),
            cleanup_policy=self.cleanup_policy,
            crop_box=crop
        )
        self.queue.append(job)
        self.update_queue_table()
        self.statusLabel.setText(f"Dodano do kolejki: {os.path.basename(self.input_file)} ({len(self.queue)} w kolejce)")

    def update_queue_table(self):
        if not self.queue:
            self.queueContainer.setVisible(False)
            return

        self.queueContainer.setVisible(True)
        self.lblQueueHeader.setText(self.t('lbl_queue_header').format(count=len(self.queue)))
        self.queueTable.setRowCount(len(self.queue))

        for row, job in enumerate(self.queue):
            self.queueTable.setItem(row, 0, QTableWidgetItem(os.path.basename(job.input_path)))
            crop_txt = job.crop_box.ratio_type if (job.crop_box and job.crop_box.ratio_type != "original") else "16:9"
            self.queueTable.setItem(row, 1, QTableWidgetItem(crop_txt))
            self.queueTable.setItem(row, 2, QTableWidgetItem(f"{job.start_s:.1f}s - {job.end_s:.1f}s"))
            self.queueTable.setItem(row, 3, QTableWidgetItem(f"{job.target_mb:.0f} MB"))
            self.queueTable.setItem(row, 4, QTableWidgetItem(job.preset_mode))
            
            st_text = job.status
            if job.status == 'pending': st_text = '⏳ Oczekuje' if self.current_lang == 'pl' else '⏳ Pending'
            elif job.status == 'running': st_text = f'⚡ Kodowanie ({int(job.progress_pct)}%)'
            elif job.status == 'finished': st_text = '✅ Gotowe' if self.current_lang == 'pl' else '✅ Done'
            elif job.status == 'error': st_text = '❌ Błąd' if self.current_lang == 'pl' else '❌ Error'
            elif job.status == 'cancelled': st_text = '🛑 Anulowano' if self.current_lang == 'pl' else '🛑 Cancelled'
            self.queueTable.setItem(row, 5, QTableWidgetItem(st_text))

            act_text = 'A/B Podgląd' if job.status == 'finished' else 'Usuń'
            item_act = QTableWidgetItem(act_text)
            item_act.setForeground(QColor('#60a5fa' if job.status == 'finished' else '#f87171'))
            self.queueTable.setItem(row, 6, item_act)

    def on_queue_table_clicked(self, row: int, col: int):
        if row < 0 or row >= len(self.queue):
            return
        job = self.queue[row]

        if col == 6:
            if job.status == 'finished' and os.path.exists(job.output_path):
                dur_s = max(job.end_s - job.start_s, 0.1)
                try: info = encoder.probe_video(job.input_path)
                except Exception: info = None
                plan = encoder.calculate_plan(info, job.start_s, job.end_s, job.target_mb, (job.preset_mode == 'CPU_HEVC'), job.input_path, self.current_lang, crop_box=job.crop_box) if info else None

                res = encoder.ExportResult(
                    input_path=job.input_path,
                    output_path=job.output_path,
                    start_s=job.start_s,
                    end_s=job.end_s,
                    duration_s=dur_s,
                    target_mb=job.target_mb,
                    actual_size_bytes=job.result_size,
                    is_remux=plan['is_remux'] if plan else False,
                    is_sample=False,
                    plan=plan,
                    preset_mode=job.preset_mode,
                    cleanup_policy="never",
                    crop_box=job.crop_box
                )
                self.open_ab_comparison(res)
            elif job.status in ('pending', 'cancelled', 'error'):
                self.queue.pop(row)
                self.update_queue_table()

    def start_or_run_queue(self):
        if not self.queue:
            self.add_to_queue()

        if self.queue and (self.worker is None or not self.worker.isRunning()):
            self.process_next_job()

    def process_next_job(self):
        pending_jobs = [j for j in self.queue if j.status == 'pending']
        if not pending_jobs:
            self.btnCompress.setEnabled(True)
            self.btnSelect.setEnabled(True)
            self.btnAddToQueue.setEnabled(True)
            self.btnCancel.setEnabled(False)
            self.btnCancel.setStyleSheet('background-color: #1e293b; color: #64748b; min-width: 80px;')
            self.active_job = None
            self.statusLabel.setText(self.t('queue_done_status').format(count=len(self.queue)))
            self.update_queue_table()
            return

        job = pending_jobs[0]
        job.status = 'running'
        self.active_job = job
        self.update_queue_table()

        self.btnCompress.setEnabled(False)
        self.btnSelect.setEnabled(False)
        self.btnAddToQueue.setEnabled(False)
        self.btnCancel.setEnabled(True)
        self.btnCancel.setStyleSheet('background-color: #dc2626; color: white; min-width: 80px; font-weight: bold;')
        self.progBar.setValue(0)
        self.statusLabel.setText(self.t('init_status'))

        self.worker = EncodingWorker(
            input_path=job.input_path,
            output_path=job.output_path,
            start_s=job.start_s,
            end_s=job.end_s,
            target_mb=job.target_mb,
            mode=job.preset_mode,
            crop_box=job.crop_box
        )
        self.worker.progress_signal.connect(self.on_worker_progress)
        self.worker.finished_signal.connect(self.on_worker_finished)
        self.worker.error_signal.connect(self.on_worker_error)
        self.worker.start()

    def cancel_compression(self):
        if self.worker and self.worker.isRunning():
            self.statusLabel.setText(self.t('cancel_status'))
            if self.active_job:
                self.active_job.status = 'cancelled'
            self.update_queue_table()
            self.worker.cancel()

    def on_worker_progress(self, p: encoder.ProgressUpdate):
        self.progBar.setValue(int(p.percent))
        if self.active_job:
            self.active_job.progress_pct = p.percent
        eta_txt = f' | ETA: {p.eta_s:.1f}s' if p.eta_s > 0 else ''
        self.statusLabel.setText(self.t('progress_status').format(
            stage=p.stage, percent=p.percent, speed=p.speed, eta=eta_txt
        ))

    def on_worker_finished(self, out_path: str, size_bytes: int):
        cur_job = self.active_job
        if cur_job:
            cur_job.status = 'finished'
            cur_job.result_size = size_bytes

        self.progBar.setValue(100)
        mb_val = size_bytes / 1000000.0
        mib_val = size_bytes / (1024.0 * 1024.0)
        self.statusLabel.setText(self.t('done_status').format(
            filename=os.path.basename(out_path), mb=mb_val, mib=mib_val
        ))
        self.update_queue_table()

        if self.auto_open_folder:
            out_dir = os.path.dirname(out_path)
            if os.path.exists(out_dir):
                os.startfile(out_dir)

        if cur_job:
            dur_s = max(cur_job.end_s - cur_job.start_s, 0.1)
            try: info = encoder.probe_video(cur_job.input_path)
            except Exception: info = None
            plan = encoder.calculate_plan(info, cur_job.start_s, cur_job.end_s, cur_job.target_mb, (cur_job.preset_mode == 'CPU_HEVC'), cur_job.input_path, self.current_lang, crop_box=cur_job.crop_box) if info else None

            res = encoder.ExportResult(
                input_path=cur_job.input_path,
                output_path=out_path,
                start_s=cur_job.start_s,
                end_s=cur_job.end_s,
                duration_s=dur_s,
                target_mb=cur_job.target_mb,
                actual_size_bytes=size_bytes,
                is_remux=plan['is_remux'] if plan else False,
                is_sample=False,
                plan=plan,
                preset_mode=cur_job.preset_mode,
                cleanup_policy=cur_job.cleanup_policy,
                crop_box=cur_job.crop_box
            )

            if self.auto_ab_compare:
                self.open_ab_comparison(res)
            elif cur_job.cleanup_policy != "never":
                self.execute_cleanup_policy(cur_job.input_path, out_path, cur_job.cleanup_policy)

        self.process_next_job()

    def on_worker_error(self, err_msg: str):
        if self.active_job:
            self.active_job.status = 'error'
            self.active_job.error_message = err_msg

        self.progBar.setValue(0)
        self.statusLabel.setText(self.t('error_status').format(msg=err_msg))
        self.update_queue_table()
        QMessageBox.critical(self, self.t('error_dialog_title'), err_msg)
        self.process_next_job()

if __name__ == '__main__':
    app = QApplication(sys.argv)
    window = CutGutApp()
    window.show()
    sys.exit(app.exec())
