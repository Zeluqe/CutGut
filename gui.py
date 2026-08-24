import sys
import os
import time
import subprocess
from typing import Optional

from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout,
    QHBoxLayout, QPushButton, QLabel, QSlider,
    QFileDialog, QProgressBar, QStyle, QComboBox, QMessageBox,
    QCheckBox, QInputDialog, QDialog, QFrame, QTableWidgetItem
)
from PyQt6.QtMultimedia import QMediaPlayer, QAudioOutput
from PyQt6.QtCore import Qt, QUrl, pyqtSignal, QThread, QSettings, QPoint
from PyQt6.QtGui import QFont, QColor, QIcon

import encoder
import update_service
from comparison_dialog import QualityComparisonDialog
from crop_overlay import CropVideoContainer
from ui.theme import apply_windows_theme, get_fluent_stylesheet
from ui.timeline import FluentTimelineWidget
from ui.widgets import FluentCard, ToastNotification, QualityBadge, HelpShortcutsDialog
from ui.settings_dialog import FluentSettingsDialog
from ui.queue_drawer import QueueDrawerWidget
import ui.icons

__version__ = "202608240-6-6"

TRANSLATIONS = {
    'pl': {
        'title': 'CutGut v{version} — Inteligentne przycinanie i kompresja wideo',
        'btn_open_file': 'Otwórz film',
        'btn_change_file': 'Zmień film',
        'no_video_selected': 'Nie wybrano filmu',
        'tooltip_help': 'Skróty klawiszowe i pomoc',
        'tooltip_settings': 'Ustawienia programu',
        'btn_set_in': 'USTAW IN',
        'btn_set_out': 'USTAW OUT',
        'lbl_duration': 'Długość: {dur:.2f}s',
        'chk_loop': 'Zapętlij',
        'btn_screenshot': 'Klatka PNG',
        'lbl_export_card': 'Konfiguracja eksportu',
        'lbl_plan_card': 'Ocena jakości',
        'lbl_preset': 'Profil:',
        'lbl_crop': 'Kadr:',
        'lbl_limit': 'Limit:',
        'lbl_encoder': 'Enkoder:',
        'btn_align_left': 'Lewo',
        'btn_align_center': 'Środek',
        'btn_align_right': 'Prawo',
        'btn_add_queue': 'Dodaj do kolejki',
        'btn_compress': 'Przytnij i kompresuj',
        'btn_cancel': 'Anuluj',
        'btn_test_sample': 'Sprawdź próbkę jakości',
        'plan_no_video': 'Wybierz film, aby zobaczyć plan eksportu i przewidywaną jakość.',
        'sample_generating': 'Generowanie próbki jakości (6s wokół kursora)...',
        'sample_done': 'Próbka gotowa! Otwarto porównanie A/B.',
        'screenshot_saved': 'Zapisano klatkę PNG: {name}',
        'toast_in_set': 'Początek (IN): {time}',
        'toast_out_set': 'Koniec (OUT): {time}',
        'toast_job_added': 'Dodano zadanie do kolejki ({count})',
        'toast_export_done': 'Eksport zakończony: {size} MB',
        'queue_header': 'Kolejka zadań ({count})',
        'queue_clear_btn': 'Wyczyść zakończone',
        'queue_status_pending': 'Oczekuje',
        'queue_status_running': 'Kompresja ({pct}%)',
        'queue_status_done': 'Gotowe',
        'queue_status_error': 'Błąd',
        'queue_status_cancelled': 'Anulowano',
        'queue_action_ab': 'Widok A/B',
        'queue_action_delete': 'Usuń',
        'social_presets': [
            'Discord Clip (20 MB)',
            'Discord Nitro (50 MB)',
            'Shorts / TikTok HQ (50 MB)',
            'Shorts Small (20 MB)',
            'TikTok / Reels (50 MB)',
            'Square Meme (20 MB)',
            'Własny format...'
        ],
        'aspect_ratios': [
            'Oryginalny (Cały obraz)',
            'Pionowy 9:16 (Shorts / TikTok)',
            'Kwadrat 1:1 (Post / Memy)',
            'Poziomy 16:9 (Gameplay)'
        ],
        'presets': [
            'Discord Free (20 MB)',
            'Legacy / Small (10 MB)',
            'Nitro Basic (50 MB)',
            'Nitro (500 MB)',
            'Własny rozmiar...'
        ],
        'custom_preset_label': 'Własny ({val:.1f} MB)',
        'custom_input_title': 'Własny limit rozmiaru',
        'custom_input_msg': 'Podaj maksymalny rozmiar w MB:',
        'ready_status': 'Gotowy do pracy. Skróty: [I] Początek, [O] Koniec, [Spacja] Play/Pauza, [←/→] ±1s, [Shift+←/→] ±1 klatka',
        'loaded_status': 'Wczytano: {filename} ({w}×{h} @ {fps:.0f} FPS)',
        'init_status': 'Inicjalizacja kompresji...',
        'cancel_status': 'Anulowanie zadania...',
        'progress_status': '[{stage}] {percent:.1f}% | Prędkość: {speed}{eta}',
        'done_status': 'Zapisano: {filename} ({mb:.2f} MB / {mib:.2f} MiB)',
        'queue_done_status': 'Ukończono wszystkie zadania z kolejki ({count})!',
        'error_status': 'Błąd: {msg}',
        'dialog_open_title': 'Wybierz plik wideo',
        'dialog_open_filter': 'Pliki Wideo (*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts)',
        'invalid_range_title': 'Nieprawidłowy zakres',
        'invalid_range_msg': 'Czas końca musi być większy niż czas początku!',
        'ffmpeg_missing_status': 'Uwaga: Brak FFmpeg w systemie.',
        'ask_cleanup_title': 'Usunięcie oryginału',
        'ask_cleanup_msg': 'Eksport zakończony sukcesem!\nCzy przenieść oryginalny plik do Kosza?\n\n{path}'
    },
    'en': {
        'title': 'CutGut v{version} — Smart Video Trimming & Compression',
        'btn_open_file': 'Open Video',
        'btn_change_file': 'Change Video',
        'no_video_selected': 'No video selected',
        'tooltip_help': 'Keyboard Shortcuts & Help',
        'tooltip_settings': 'Application Settings',
        'btn_set_in': 'SET IN',
        'btn_set_out': 'SET OUT',
        'lbl_duration': 'Duration: {dur:.2f}s',
        'chk_loop': 'Loop',
        'btn_screenshot': 'PNG Frame',
        'lbl_export_card': 'Export Setup',
        'lbl_plan_card': 'Quality Assessment',
        'lbl_preset': 'Profile:',
        'lbl_crop': 'Crop:',
        'lbl_limit': 'Limit:',
        'lbl_encoder': 'Encoder:',
        'btn_align_left': 'Left',
        'btn_align_center': 'Center',
        'btn_align_right': 'Right',
        'btn_add_queue': 'Add to Queue',
        'btn_compress': 'Trim and Compress',
        'btn_cancel': 'Cancel',
        'btn_test_sample': 'Test Quality Sample',
        'plan_no_video': 'Select a video to see export plan and quality assessment.',
        'sample_generating': 'Generating quality sample (6s around playhead)...',
        'sample_done': 'Sample ready! Opened A/B comparison window.',
        'screenshot_saved': 'Saved PNG frame: {name}',
        'toast_in_set': 'Start (IN): {time}',
        'toast_out_set': 'End (OUT): {time}',
        'toast_job_added': 'Job added to queue ({count})',
        'toast_export_done': 'Export finished: {size} MB',
        'queue_header': 'Job Queue ({count})',
        'queue_clear_btn': 'Clear Finished',
        'queue_status_pending': 'Pending',
        'queue_status_running': 'Encoding ({pct}%)',
        'queue_status_done': 'Done',
        'queue_status_error': 'Error',
        'queue_status_cancelled': 'Cancelled',
        'queue_action_ab': 'A/B View',
        'queue_action_delete': 'Delete',
        'social_presets': [
            'Discord Clip (20 MB)',
            'Discord Nitro (50 MB)',
            'Shorts / TikTok HQ (50 MB)',
            'Shorts Small (20 MB)',
            'TikTok / Reels (50 MB)',
            'Square Meme (20 MB)',
            'Custom format...'
        ],
        'aspect_ratios': [
            'Original (Full frame)',
            'Vertical 9:16 (Shorts / TikTok)',
            'Square 1:1 (Post / Memes)',
            'Horizontal 16:9 (Gameplay)'
        ],
        'presets': [
            'Discord Free (20 MB)',
            'Legacy / Small (10 MB)',
            'Nitro Basic (50 MB)',
            'Nitro (500 MB)',
            'Custom size...'
        ],
        'custom_preset_label': 'Custom ({val:.1f} MB)',
        'custom_input_title': 'Custom Size Limit',
        'custom_input_msg': 'Enter maximum file size in MB:',
        'ready_status': 'Ready. Shortcuts: [I] Start, [O] End, [Space] Play/Pause, [←/→] ±1s, [Shift+←/→] ±1 frame',
        'loaded_status': 'Loaded: {filename} ({w}×{h} @ {fps:.0f} FPS)',
        'init_status': 'Initializing compression...',
        'cancel_status': 'Cancelling task...',
        'progress_status': '[{stage}] {percent:.1f}% | Speed: {speed}{eta}',
        'done_status': 'Saved: {filename} ({mb:.2f} MB / {mib:.2f} MiB)',
        'queue_done_status': 'Completed all queued tasks ({count} jobs)!',
        'error_status': 'Error: {msg}',
        'dialog_open_title': 'Select video file',
        'dialog_open_filter': 'Video Files (*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts)',
        'invalid_range_title': 'Invalid Range',
        'invalid_range_msg': 'End time must be greater than start time!',
        'ffmpeg_missing_status': 'Note: FFmpeg not found on this system.',
        'ask_cleanup_title': 'Original Cleanup',
        'ask_cleanup_msg': 'Export completed successfully!\nDo you want to move the original source video to Recycle Bin?\n\n{path}'
    }
}

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

class CutGutApp(QMainWindow):
    def __init__(self):
        super().__init__()

        self.settings = QSettings('Zeluqe', 'CutGut')
        self.current_lang = self.settings.value('language', 'en')
        if self.current_lang not in ('pl', 'en'):
            self.current_lang = 'en'

        self.cleanup_policy = self.settings.value('cleanup_policy', encoder.SourceCleanupPolicy.NEVER.value)
        self.custom_output_dir = self.settings.value('output_directory', '')
        self.auto_open_folder = (self.settings.value('auto_open_folder', 'true') == 'true')
        self.auto_ab_compare = (self.settings.value('auto_ab_compare', 'true') == 'true')
        self.auto_check_updates = (self.settings.value('auto_check_updates', 'true') == 'true')
        self.include_prerelease = (self.settings.value('include_prerelease', 'false') == 'true')

        self.setMinimumSize(960, 680)
        self.resize(1120, 820)
        self.setAcceptDrops(True)

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

        # Media Player & QVideoSink Canvas
        self.mediaPlayer = QMediaPlayer()
        self.cropCanvas = CropVideoContainer()
        self.cropCanvas.clicked_signal.connect(self.play_pause)
        self.cropCanvas.crop_changed_signal.connect(self.on_crop_changed)

        self.audioOutput = QAudioOutput()
        self.mediaPlayer.setVideoSink(self.cropCanvas.sink)
        self.mediaPlayer.setAudioOutput(self.audioOutput)
        self.audioOutput.setVolume(0.7)

        # Apply Fluent Stylesheet
        self.setStyleSheet(get_fluent_stylesheet(is_dark=True))

        self.init_ui()
        self.retranslate_ui()
        self.check_ffmpeg_startup()
        self.startup_maintenance()

    def t(self, key: str) -> any:
        lang_dict = TRANSLATIONS.get(self.current_lang, TRANSLATIONS['en'])
        return lang_dict.get(key, TRANSLATIONS['en'].get(key, ''))

    def init_ui(self):
        main_widget = QWidget()
        main_layout = QVBoxLayout(main_widget)
        main_layout.setContentsMargins(14, 10, 14, 10)
        main_layout.setSpacing(8)

        # 1. Sleek Top Action Bar (Integrated Controls)
        top_bar = QHBoxLayout()
        top_bar.setContentsMargins(2, 0, 2, 0)
        top_bar.setSpacing(8)

        self.btnHeaderOpen = QPushButton(self.t('btn_open_file'))
        self.btnHeaderOpen.setIcon(ui.icons.get_icon('folder', '#f4f4f6', 18))
        self.btnHeaderOpen.setStyleSheet("padding: 6px 14px; font-weight: 700; background-color: #222226;")
        self.btnHeaderOpen.clicked.connect(self.open_file)
        top_bar.addWidget(self.btnHeaderOpen)

        self.lblCurrentFile = QLabel(self.t('no_video_selected'))
        self.lblCurrentFile.setStyleSheet("color: #a1a1aa; font-weight: 600; font-size: 12px; margin-left: 6px;")
        top_bar.addWidget(self.lblCurrentFile, 1)

        self.langCombo = QComboBox()
        self.langCombo.addItems(['PL', 'EN'])
        self.langCombo.setFixedWidth(66)
        self.langCombo.setCurrentIndex(0 if self.current_lang == 'pl' else 1)
        self.langCombo.currentIndexChanged.connect(self.on_lang_changed)
        top_bar.addWidget(self.langCombo)

        self.btnHelp = QPushButton()
        self.btnHelp.setIcon(ui.icons.get_icon('help', '#a1a1aa', 18))
        self.btnHelp.setFixedSize(36, 30)
        self.btnHelp.setToolTip(self.t('tooltip_help'))
        self.btnHelp.clicked.connect(self.open_help)
        top_bar.addWidget(self.btnHelp)

        self.btnSettings = QPushButton()
        self.btnSettings.setIcon(ui.icons.get_icon('settings', '#a1a1aa', 18))
        self.btnSettings.setFixedSize(36, 30)
        self.btnSettings.setToolTip(self.t('tooltip_settings'))
        self.btnSettings.clicked.connect(self.open_settings)
        top_bar.addWidget(self.btnSettings)

        main_layout.addLayout(top_bar)

        # 2. Video Player Canvas & Floating Toast (Adaptive Height)
        player_container = QWidget()
        player_container_layout = QVBoxLayout(player_container)
        player_container_layout.setContentsMargins(0, 0, 0, 0)
        player_container_layout.setSpacing(0)

        self.cropCanvas.setMinimumHeight(180)
        player_container_layout.addWidget(self.cropCanvas, 1)

        self.toast = ToastNotification(self)

        main_layout.addWidget(player_container, 3)

        # 3. Timeline & Media Controls Card
        timeline_card = FluentCard()
        timeline_layout = QVBoxLayout(timeline_card)
        timeline_layout.setContentsMargins(12, 8, 12, 8)
        timeline_layout.setSpacing(6)

        # Rich Fluent Timeline
        self.timeline = FluentTimelineWidget()
        self.timeline.position_changed.connect(self.set_position)
        self.timeline.range_changed.connect(self.on_timeline_range_changed)
        self.timeline.marker_set.connect(self.on_marker_set)
        timeline_layout.addWidget(self.timeline)

        # Player Controls Row
        ctrl_row = QHBoxLayout()
        ctrl_row.setSpacing(8)

        self.playBtn = QPushButton()
        self.playBtn.setIcon(ui.icons.get_icon('play', '#ffffff', 18))
        self.playBtn.setFixedSize(38, 30)
        self.playBtn.setStyleSheet("background-color: #0078d4; border-radius: 6px;")
        self.playBtn.clicked.connect(self.play_pause)
        ctrl_row.addWidget(self.playBtn)

        self.timeLabel = QLabel("00:00.00 / 00:00.00")
        self.timeLabel.setStyleSheet("font-weight: 700; font-size: 13px; color: #f4f4f6; margin-left: 4px;")
        ctrl_row.addWidget(self.timeLabel)

        self.btnSetIn = QPushButton(self.t('btn_set_in'))
        self.btnSetIn.setStyleSheet("background-color: #059669; color: white; padding: 4px 10px; font-size: 11px; font-weight: bold;")
        self.btnSetIn.clicked.connect(self.set_start)
        ctrl_row.addWidget(self.btnSetIn)

        self.lblInTime = QLabel("IN: 00:00.00")
        self.lblInTime.setStyleSheet("color: #10b981; font-weight: 700; font-size: 12px;")
        ctrl_row.addWidget(self.lblInTime)

        self.lblDuration = QLabel(self.t('lbl_duration').format(dur=0.0))
        self.lblDuration.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.lblDuration.setStyleSheet("color: #60cdff; font-weight: 700; font-size: 13px;")
        ctrl_row.addWidget(self.lblDuration, 1)

        self.lblOutTime = QLabel("OUT: 00:00.00")
        self.lblOutTime.setStyleSheet("color: #ef4444; font-weight: 700; font-size: 12px;")
        ctrl_row.addWidget(self.lblOutTime)

        self.btnSetOut = QPushButton(self.t('btn_set_out'))
        self.btnSetOut.setStyleSheet("background-color: #dc2626; color: white; padding: 4px 10px; font-size: 11px; font-weight: bold;")
        self.btnSetOut.clicked.connect(self.set_end)
        ctrl_row.addWidget(self.btnSetOut)

        self.chkLoop = QCheckBox(self.t('chk_loop'))
        ctrl_row.addWidget(self.chkLoop)

        self.btnScreenshot = QPushButton(self.t('btn_screenshot'))
        self.btnScreenshot.setIcon(ui.icons.get_icon('camera', '#ffffff', 14))
        self.btnScreenshot.setStyleSheet("background-color: #0f766e; color: white; font-size: 11px; padding: 5px 10px;")
        self.btnScreenshot.setEnabled(False)
        self.btnScreenshot.clicked.connect(self.capture_png_frame)
        ctrl_row.addWidget(self.btnScreenshot)

        # Volume Controls
        btn_vol = QPushButton()
        btn_vol.setIcon(ui.icons.get_icon('volume', '#a1a1aa', 16))
        btn_vol.setStyleSheet("background: transparent; border: none; padding: 0;")
        btn_vol.setFixedSize(22, 22)
        ctrl_row.addWidget(btn_vol)

        self.volSlider = QSlider(Qt.Orientation.Horizontal)
        self.volSlider.setRange(0, 100)
        self.volSlider.setValue(70)
        self.volSlider.setFixedWidth(65)
        self.volSlider.valueChanged.connect(lambda v: self.audioOutput.setVolume(v / 100.0))
        ctrl_row.addWidget(self.volSlider)

        timeline_layout.addLayout(ctrl_row)
        main_layout.addWidget(timeline_card)

        # 4. Two-Column Fluent Row: Export Setup (Left) & Quality Plan (Right)
        bottom_row = QHBoxLayout()
        bottom_row.setSpacing(10)

        # LEFT CARD: Export Setup
        self.exportCard = FluentCard()
        export_layout = QVBoxLayout(self.exportCard)
        export_layout.setContentsMargins(12, 10, 12, 10)
        export_layout.setSpacing(6)

        self.lblExportTitle = QLabel(self.t('lbl_export_card'))
        self.lblExportTitle.setStyleSheet("font-size: 13px; font-weight: 700; color: #60cdff;")
        export_layout.addWidget(self.lblExportTitle)

        # Row 1: Profile & Crop
        r1 = QHBoxLayout()
        r1.setSpacing(8)

        r1_sub1 = QVBoxLayout()
        self.lblProf = QLabel(self.t('lbl_preset'))
        self.lblProf.setProperty('class', 'Secondary')
        self.socialCombo = QComboBox()
        self.socialCombo.currentIndexChanged.connect(self.on_social_preset_changed)
        r1_sub1.addWidget(self.lblProf)
        r1_sub1.addWidget(self.socialCombo)
        r1.addLayout(r1_sub1, 1)

        r1_sub2 = QVBoxLayout()
        self.lblCrop = QLabel(self.t('lbl_crop'))
        self.lblCrop.setProperty('class', 'Secondary')
        self.ratioCombo = QComboBox()
        self.ratioCombo.currentIndexChanged.connect(self.on_ratio_changed)
        r1_sub2.addWidget(self.lblCrop)
        r1_sub2.addWidget(self.ratioCombo)
        r1.addLayout(r1_sub2, 1)

        export_layout.addLayout(r1)

        # Alignment Buttons (for 9:16 / 1:1)
        self.alignBox = QWidget()
        align_layout = QHBoxLayout(self.alignBox)
        align_layout.setContentsMargins(0, 0, 0, 0)
        align_layout.setSpacing(4)

        self.btnAlignLeft = QPushButton(self.t('btn_align_left'))
        self.btnAlignLeft.setIcon(ui.icons.get_icon('align_left', '#f4f4f6', 14))
        self.btnAlignLeft.setStyleSheet("font-size: 11px; padding: 4px 8px;")
        self.btnAlignLeft.clicked.connect(lambda: self.cropCanvas.set_alignment('left'))
        align_layout.addWidget(self.btnAlignLeft)

        self.btnAlignCenter = QPushButton(self.t('btn_align_center'))
        self.btnAlignCenter.setIcon(ui.icons.get_icon('align_center', '#ffffff', 14))
        self.btnAlignCenter.setStyleSheet("font-size: 11px; padding: 4px 8px; background-color: #0078d4;")
        self.btnAlignCenter.clicked.connect(lambda: self.cropCanvas.set_alignment('center'))
        align_layout.addWidget(self.btnAlignCenter)

        self.btnAlignRight = QPushButton(self.t('btn_align_right'))
        self.btnAlignRight.setIcon(ui.icons.get_icon('align_right', '#f4f4f6', 14))
        self.btnAlignRight.setStyleSheet("font-size: 11px; padding: 4px 8px;")
        self.btnAlignRight.clicked.connect(lambda: self.cropCanvas.set_alignment('right'))
        align_layout.addWidget(self.btnAlignRight)

        align_layout.addStretch()
        self.alignBox.setVisible(False)
        export_layout.addWidget(self.alignBox)

        # Row 2: Limit & Encoder
        r2 = QHBoxLayout()
        r2.setSpacing(8)

        r2_sub1 = QVBoxLayout()
        self.lblLimit = QLabel(self.t('lbl_limit'))
        self.lblLimit.setProperty('class', 'Secondary')
        self.limitCombo = QComboBox()
        self.limitCombo.currentIndexChanged.connect(self.on_limit_changed)
        r2_sub1.addWidget(self.lblLimit)
        r2_sub1.addWidget(self.limitCombo)
        r2.addLayout(r2_sub1, 1)

        r2_sub2 = QVBoxLayout()
        self.lblEnc = QLabel(self.t('lbl_encoder'))
        self.lblEnc.setProperty('class', 'Secondary')
        self.modeCombo = QComboBox()
        self.modeCombo.currentIndexChanged.connect(self.update_live_estimate)
        r2_sub2.addWidget(self.lblEnc)
        r2_sub2.addWidget(self.modeCombo)
        r2.addLayout(r2_sub2, 1)

        export_layout.addLayout(r2)

        # Action Buttons Row
        r_act = QHBoxLayout()
        r_act.setSpacing(6)

        self.btnCompress = QPushButton(self.t('btn_compress'))
        self.btnCompress.setIcon(ui.icons.get_icon('scissors', '#ffffff', 18))
        self.btnCompress.setProperty('class', 'Primary')
        self.btnCompress.setEnabled(False)
        self.btnCompress.clicked.connect(self.start_or_run_queue)
        r_act.addWidget(self.btnCompress, 2)

        self.btnAddToQueue = QPushButton(self.t('btn_add_queue'))
        self.btnAddToQueue.setIcon(ui.icons.get_icon('plus', '#f4f4f6', 16))
        self.btnAddToQueue.setEnabled(False)
        self.btnAddToQueue.clicked.connect(self.add_to_queue)
        r_act.addWidget(self.btnAddToQueue, 1)

        self.btnCancel = QPushButton(self.t('btn_cancel'))
        self.btnCancel.setIcon(ui.icons.get_icon('cancel', '#ffffff', 14))
        self.btnCancel.setProperty('class', 'Danger')
        self.btnCancel.setEnabled(False)
        self.btnCancel.clicked.connect(self.cancel_compression)
        r_act.addWidget(self.btnCancel)

        export_layout.addLayout(r_act)
        bottom_row.addWidget(self.exportCard, 1)

        # RIGHT CARD: Quality Plan
        self.planCard = FluentCard()
        plan_layout = QVBoxLayout(self.planCard)
        plan_layout.setContentsMargins(12, 10, 12, 10)
        plan_layout.setSpacing(6)

        plan_hdr = QHBoxLayout()
        self.lblPlanTitle = QLabel(self.t('lbl_plan_card'))
        self.lblPlanTitle.setStyleSheet("font-size: 13px; font-weight: 700; color: #60cdff;")
        plan_hdr.addWidget(self.lblPlanTitle)
        plan_hdr.addStretch()

        self.qualityBadge = QualityBadge()
        plan_hdr.addWidget(self.qualityBadge)
        plan_layout.addLayout(plan_hdr)

        self.lblPlanDetails = QLabel(self.t('plan_no_video'))
        self.lblPlanDetails.setStyleSheet("font-size: 12px; font-weight: 600; color: #f4f4f6; line-height: 1.3;")
        self.lblPlanDetails.setWordWrap(True)
        plan_layout.addWidget(self.lblPlanDetails)

        self.lblPlanTip = QLabel("")
        self.lblPlanTip.setStyleSheet("font-size: 11px; color: #38bdf8;")
        self.lblPlanTip.setWordWrap(True)
        plan_layout.addWidget(self.lblPlanTip, 1)

        self.btnTestSample = QPushButton(self.t('btn_test_sample'))
        self.btnTestSample.setIcon(ui.icons.get_icon('flask', '#ffffff', 16))
        self.btnTestSample.setStyleSheet("background-color: #0e7490; color: white; padding: 7px 12px;")
        self.btnTestSample.setEnabled(False)
        self.btnTestSample.clicked.connect(self.create_sample_preview)
        plan_layout.addWidget(self.btnTestSample)

        bottom_row.addWidget(self.planCard, 1)
        main_layout.addLayout(bottom_row)

        # 5. Queue Drawer Widget
        self.queueDrawer = QueueDrawerWidget(self, self.current_lang)
        self.queueDrawer.job_action_clicked.connect(self.on_queue_action)
        self.queueDrawer.btnClearFinished.clicked.connect(self.clear_finished_jobs)
        self.queueDrawer.setVisible(False)
        main_layout.addWidget(self.queueDrawer)

        # 6. Progress Bar & Status Line
        self.progBar = QProgressBar()
        self.progBar.setFixedHeight(8)
        main_layout.addWidget(self.progBar)

        self.statusLabel = QLabel(self.t('ready_status'))
        self.statusLabel.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.statusLabel.setProperty('class', 'Secondary')
        main_layout.addWidget(self.statusLabel)

        self.setCentralWidget(main_widget)

        self.mediaPlayer.positionChanged.connect(self.position_changed)
        self.mediaPlayer.durationChanged.connect(self.duration_changed)

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
            self.toast.show_toast(f"Update available: {rel.tag_name}", "info", 6000)

    def open_help(self):
        dlg = HelpShortcutsDialog(self, self.current_lang)
        dlg.exec()

    def on_marker_set(self, kind: str, pos_ms: int):
        t_str = self.timeline.format_time(pos_ms)
        if kind == 'in':
            self.toast.show_toast(self.t('toast_in_set').format(time=t_str), "check", 2500)
            self.start_ms = pos_ms
        else:
            self.toast.show_toast(self.t('toast_out_set').format(time=t_str), "check", 2500)
            self.end_ms = pos_ms
        self.update_range_text()

    def on_timeline_range_changed(self, start_ms: int, end_ms: int):
        self.start_ms = start_ms
        self.end_ms = end_ms
        self.update_range_text()

    def on_crop_changed(self, cb: encoder.CropBox):
        self.current_crop = cb
        self.update_live_estimate()

    def on_ratio_changed(self, idx: int):
        ratios = ['original', '9:16', '1:1', '16:9']
        r = ratios[idx] if idx < len(ratios) else 'original'
        self.cropCanvas.set_ratio_type(r)
        self.alignBox.setVisible(r != 'original')
        self.update_live_estimate()

    def on_social_preset_changed(self, idx: int):
        self.ratioCombo.blockSignals(True)
        self.limitCombo.blockSignals(True)

        if idx == 0:  # Discord Clip (20 MB, 16:9)
            self.ratioCombo.setCurrentIndex(0)
            self.limitCombo.setCurrentIndex(0)
            self.cropCanvas.set_ratio_type('original')
            self.alignBox.setVisible(False)
        elif idx == 1:  # Discord Nitro (50 MB, 16:9)
            self.ratioCombo.setCurrentIndex(0)
            self.limitCombo.setCurrentIndex(2)
            self.cropCanvas.set_ratio_type('original')
            self.alignBox.setVisible(False)
        elif idx == 2:  # Shorts HQ (9:16, 50 MB)
            self.ratioCombo.setCurrentIndex(1)
            self.limitCombo.setCurrentIndex(2)
            self.cropCanvas.set_ratio_type('9:16')
            self.alignBox.setVisible(True)
        elif idx == 3:  # Shorts Small (9:16, 20 MB)
            self.ratioCombo.setCurrentIndex(1)
            self.limitCombo.setCurrentIndex(0)
            self.cropCanvas.set_ratio_type('9:16')
            self.alignBox.setVisible(True)
        elif idx == 4:  # TikTok / Reels (9:16, 50 MB)
            self.ratioCombo.setCurrentIndex(1)
            self.limitCombo.setCurrentIndex(2)
            self.cropCanvas.set_ratio_type('9:16')
            self.alignBox.setVisible(True)
        elif idx == 5:  # Square Meme (1:1, 20 MB)
            self.ratioCombo.setCurrentIndex(2)
            self.limitCombo.setCurrentIndex(0)
            self.cropCanvas.set_ratio_type('1:1')
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
            crop = self.cropCanvas.get_crop_box()
            saved = encoder.extract_frame_png(self.input_file, cur_time_s, png_path, crop)
            self.toast.show_toast(self.t('screenshot_saved').format(name=os.path.basename(saved)), "camera", 3500)
            if self.auto_open_folder:
                os.startfile(out_dir)
        except Exception as e:
            QMessageBox.critical(self, "Screenshot Error", str(e))

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
        dlg = FluentSettingsDialog(
            self,
            self.cleanup_policy,
            self.current_lang,
            self.custom_output_dir,
            self.auto_open_folder,
            self.auto_ab_compare,
            self.auto_check_updates,
            self.include_prerelease,
            version_str=__version__
        )
        if dlg.exec() == QDialog.DialogCode.Accepted:
            self.cleanup_policy = dlg.selected_policy
            self.custom_output_dir = dlg.output_dir
            self.auto_open_folder = dlg.auto_open
            self.auto_ab_compare = dlg.auto_ab
            self.auto_check_updates = dlg.auto_check_updates

            self.settings.setValue('cleanup_policy', self.cleanup_policy)
            self.settings.setValue('output_directory', self.custom_output_dir)
            self.settings.setValue('auto_open_folder', 'true' if self.auto_open_folder else 'false')
            self.settings.setValue('auto_ab_compare', 'true' if self.auto_ab_compare else 'false')
            self.settings.setValue('auto_check_updates', 'true' if self.auto_check_updates else 'false')

    def retranslate_ui(self):
        self.setWindowTitle(self.t('title').format(version=__version__))
        self.btnHeaderOpen.setText(self.t('btn_change_file') if self.input_file else self.t('btn_open_file'))
        
        if not self.input_file:
            self.lblCurrentFile.setText(self.t('no_video_selected'))
        else:
            w = self.video_info.width if self.video_info else 1920
            h = self.video_info.height if self.video_info else 1080
            self.lblCurrentFile.setText(f"{os.path.basename(self.input_file)} ({w}×{h} @ {self.video_fps:.0f} FPS)")

        self.btnHelp.setToolTip(self.t('tooltip_help'))
        self.btnSettings.setToolTip(self.t('tooltip_settings'))

        self.btnSetIn.setText(self.t('btn_set_in'))
        self.btnSetOut.setText(self.t('btn_set_out'))
        self.chkLoop.setText(self.t('chk_loop'))
        self.btnScreenshot.setText(self.t('btn_screenshot'))

        self.lblExportTitle.setText(self.t('lbl_export_card'))
        self.lblPlanTitle.setText(self.t('lbl_plan_card'))

        self.lblProf.setText(self.t('lbl_preset'))
        self.lblCrop.setText(self.t('lbl_crop'))
        self.lblLimit.setText(self.t('lbl_limit'))
        self.lblEnc.setText(self.t('lbl_encoder'))

        self.btnAlignLeft.setText(self.t('btn_align_left'))
        self.btnAlignCenter.setText(self.t('btn_align_center'))
        self.btnAlignRight.setText(self.t('btn_align_right'))

        self.btnCompress.setText(self.t('btn_compress'))
        self.btnAddToQueue.setText(self.t('btn_add_queue'))
        self.btnCancel.setText(self.t('btn_cancel'))
        self.btnTestSample.setText(self.t('btn_test_sample'))

        self.queueDrawer.set_language(self.current_lang)

        self.update_range_text()

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

        # Encoders
        cur_mode_idx = max(self.modeCombo.currentIndex(), 0)
        self.modeCombo.blockSignals(True)
        self.modeCombo.clear()
        if encoder.check_nvenc_support():
            self.modeCombo.addItems(['NVIDIA NVENC (GPU HQ)', 'NVIDIA NVENC (GPU Fast)', 'CPU H.264 (Balanced)', 'CPU H.264 (Fast)', 'CPU H.265 (Cinematic)'])
        elif encoder.check_amf_support():
            self.modeCombo.addItems(['AMD AMF (GPU HQ)', 'AMD AMF (GPU Fast)', 'CPU H.264 (Balanced)', 'CPU H.264 (Fast)', 'CPU H.265 (Cinematic)'])
        else:
            self.modeCombo.addItems(['CPU H.264 (Balanced)', 'CPU H.264 (Fast)', 'CPU H.265 (Cinematic)'])
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
                self.cropCanvas.set_video_dimensions(self.video_info.width, self.video_info.height)
            except Exception:
                self.video_info = None
                self.video_fps = 60.0

            self.mediaPlayer.setSource(QUrl.fromLocalFile(file_path))
            self.btnCompress.setEnabled(True)
            self.btnAddToQueue.setEnabled(True)
            self.btnTestSample.setEnabled(True)
            self.btnScreenshot.setEnabled(True)
            self.btnHeaderOpen.setText(self.t('btn_change_file'))

            w = self.video_info.width if self.video_info else 1920
            h = self.video_info.height if self.video_info else 1080
            self.lblCurrentFile.setText(f"{os.path.basename(file_path)} ({w}×{h} @ {self.video_fps:.0f} FPS)")
            self.lblCurrentFile.setStyleSheet("color: #60cdff; font-weight: 700; font-size: 12px; margin-left: 6px;")

            self.statusLabel.setText(self.t('loaded_status').format(
                filename=os.path.basename(file_path), w=w, h=h, fps=self.video_fps
            ))
            self.mediaPlayer.play()
            self.playBtn.setIcon(ui.icons.get_icon('pause', '#ffffff', 18))
            self.update_live_estimate()

    def play_pause(self):
        if self.mediaPlayer.playbackState() == QMediaPlayer.PlaybackState.PlayingState:
            self.mediaPlayer.pause()
            self.playBtn.setIcon(ui.icons.get_icon('play', '#ffffff', 18))
        else:
            self.mediaPlayer.play()
            self.playBtn.setIcon(ui.icons.get_icon('pause', '#ffffff', 18))

    def set_position(self, pos):
        self.mediaPlayer.setPosition(pos)

    def position_changed(self, pos):
        self.timeline.set_position(pos)
        dur = self.mediaPlayer.duration()
        self.timeLabel.setText(f'{self.timeline.format_time(pos)} / {self.timeline.format_time(dur)}')

        if self.chkLoop.isChecked() and self.end_ms > self.start_ms:
            if pos >= self.end_ms or pos < self.start_ms:
                self.mediaPlayer.setPosition(self.start_ms)

    def duration_changed(self, dur):
        self.timeline.set_duration(dur)
        self.start_ms = 0
        self.end_ms = dur
        self.update_range_text()

    def set_start(self):
        self.timeline.set_in_point(self.mediaPlayer.position())

    def set_end(self):
        self.timeline.set_out_point(self.mediaPlayer.position())

    def update_range_text(self):
        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        dur_s = max(end_s - start_s, 0.0)

        self.lblInTime.setText(f"IN: {self.timeline.format_time(self.start_ms)}")
        self.lblOutTime.setText(f"OUT: {self.timeline.format_time(self.end_ms)}")
        self.lblDuration.setText(self.t('lbl_duration').format(dur=dur_s))
        self.update_live_estimate()

    def update_live_estimate(self):
        if not self.input_file or not self.video_info:
            self.qualityBadge.set_assessment("No file loaded" if self.current_lang == 'en' else "Brak pliku", "#71717a")
            self.lblPlanDetails.setText(self.t('plan_no_video'))
            self.lblPlanTip.setText("")
            return

        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        dur_s = max(end_s - start_s, 0.1)
        target_mb = self.get_selected_target_mb()
        mode = self.get_selected_encoder_mode()
        is_hevc = (mode == 'CPU_HEVC')
        crop = self.cropCanvas.get_crop_box()

        plan = encoder.calculate_plan(
            self.video_info, start_s, end_s, target_mb, is_hevc, self.input_file, self.current_lang, crop_box=crop
        )
        q: encoder.QualityAssessment = plan['quality']

        self.qualityBadge.set_assessment(q.label, q.color)

        crop_info = f" [Crop: {crop.ratio_type}]" if crop.ratio_type != "original" else ""

        if plan['is_remux']:
            self.lblPlanDetails.setText(
                f"{self.video_info.width}×{self.video_info.height} · {self.video_fps:.0f} FPS · Remux (Direct Stream Copy) · Limit: {target_mb} MB"
            )
        else:
            enc_name = self.modeCombo.currentText()
            self.lblPlanDetails.setText(
                f"{plan['out_width']}×{plan['out_height']}{crop_info} · {plan['out_fps']:.0f} FPS · {enc_name} · ~{plan['video_kbps']} kbps · Target: ~{plan['target_bytes']/1000000:.1f} MB (Limit: {target_mb:.1f} MB)"
            )

        tip_text = f"<b>{q.description}</b>"
        if q.tip:
            tip_text += f"<br><span style='color: #60cdff;'><b>{'Wskazówka:' if self.current_lang == 'pl' else 'Tip:'}</b> {q.tip}</span>"
        self.lblPlanTip.setText(tip_text)

    def create_sample_preview(self):
        if not self.input_file or not self.video_info:
            return

        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        target_mb = self.get_selected_target_mb()
        mode = self.get_selected_encoder_mode()
        is_hevc = (mode == 'CPU_HEVC')
        crop = self.cropCanvas.get_crop_box()

        plan = encoder.calculate_plan(
            self.video_info, start_s, end_s, target_mb, is_hevc, self.input_file, self.current_lang, crop_box=crop
        )

        if plan['is_remux']:
            QMessageBox.information(self, "CutGut", "Lossless remux preserves 100% of original quality.")
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
        self.toast.show_toast(self.t('sample_done'), "flask", 3000)

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
        self.statusLabel.setText(f"Error: {err_msg}")
        QMessageBox.warning(self, "Quality Sample Error", err_msg)

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
        crop = self.cropCanvas.get_crop_box()
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
        self.toast.show_toast(self.t('toast_job_added').format(count=len(self.queue)), "plus", 3000)

    def update_queue_table(self):
        if not self.queue:
            self.queueDrawer.setVisible(False)
            return

        self.queueDrawer.setVisible(True)
        self.queueDrawer.lblHeader.setText(self.t('queue_header').format(count=len(self.queue)))
        self.queueDrawer.table.setRowCount(len(self.queue))

        for row, job in enumerate(self.queue):
            self.queueDrawer.table.setItem(row, 0, QTableWidgetItem(os.path.basename(job.input_path)))
            crop_txt = job.crop_box.ratio_type if (job.crop_box and job.crop_box.ratio_type != "original") else "16:9"
            self.queueDrawer.table.setItem(row, 1, QTableWidgetItem(crop_txt))
            self.queueDrawer.table.setItem(row, 2, QTableWidgetItem(f"{job.start_s:.1f}s - {job.end_s:.1f}s"))
            self.queueDrawer.table.setItem(row, 3, QTableWidgetItem(f"{job.target_mb:.0f} MB"))
            self.queueDrawer.table.setItem(row, 4, QTableWidgetItem(job.preset_mode))

            st_text = job.status
            if job.status == 'pending': st_text = self.t('queue_status_pending')
            elif job.status == 'running': st_text = self.t('queue_status_running').format(pct=int(job.progress_pct))
            elif job.status == 'finished': st_text = self.t('queue_status_done')
            elif job.status == 'error': st_text = self.t('queue_status_error')
            elif job.status == 'cancelled': st_text = self.t('queue_status_cancelled')
            self.queueDrawer.table.setItem(row, 5, QTableWidgetItem(st_text))

            act_text = self.t('queue_action_ab') if job.status == 'finished' else self.t('queue_action_delete')
            item_act = QTableWidgetItem(act_text)
            item_act.setForeground(QColor('#60cdff' if job.status == 'finished' else '#ef4444'))
            self.queueDrawer.table.setItem(row, 6, item_act)

    def on_queue_action(self, row: int, action_text: str):
        if row < 0 or row >= len(self.queue):
            return
        job = self.queue[row]

        if ('A/B' in action_text or 'Widok' in action_text) and job.status == 'finished' and os.path.exists(job.output_path):
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
        elif 'Delete' in action_text or 'Usuń' in action_text:
            if job.status in ('pending', 'cancelled', 'error', 'finished'):
                self.queue.pop(row)
                self.update_queue_table()

    def clear_finished_jobs(self):
        self.queue = [j for j in self.queue if j.status not in ('finished', 'cancelled')]
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
            self.btnHeaderOpen.setEnabled(True)
            self.btnAddToQueue.setEnabled(True)
            self.btnCancel.setEnabled(False)
            self.active_job = None
            self.statusLabel.setText(self.t('queue_done_status').format(count=len(self.queue)))
            self.update_queue_table()
            return

        job = pending_jobs[0]
        job.status = 'running'
        self.active_job = job
        self.update_queue_table()

        self.btnCompress.setEnabled(False)
        self.btnHeaderOpen.setEnabled(False)
        self.btnAddToQueue.setEnabled(False)
        self.btnCancel.setEnabled(True)
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
        self.toast.show_toast(self.t('toast_export_done').format(size=f"{mb_val:.2f}"), "check", 4000)
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
        self.toast.show_toast(f"Error: {err_msg}", "cancel", 5000)
        self.update_queue_table()
        QMessageBox.critical(self, "Compression Error", err_msg)
        self.process_next_job()

    def showEvent(self, event):
        super().showEvent(event)
        try:
            apply_windows_theme(self, is_dark=True)
        except Exception:
            pass

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if hasattr(self, 'toast') and self.toast.isVisible():
            self.toast.move(self.width() - self.toast.width() - 30, 25)

def global_exception_handler(exctype, value, tb):
    import traceback
    err_str = "".join(traceback.format_exception(exctype, value, tb))
    print(err_str, file=sys.stderr)
    try:
        QMessageBox.critical(None, "CutGut Error", f"An unexpected error occurred:\n\n{err_str}")
    except Exception:
        pass
    sys.__excepthook__(exctype, value, tb)

if __name__ == '__main__':
    sys.excepthook = global_exception_handler
    app = QApplication(sys.argv)
    window = CutGutApp()
    window.show()
    sys.exit(app.exec())
