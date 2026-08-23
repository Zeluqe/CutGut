import sys
import os
import time
from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout,
    QHBoxLayout, QPushButton, QLabel, QSlider,
    QFileDialog, QProgressBar, QStyle, QComboBox, QMessageBox,
    QCheckBox, QInputDialog, QDialog, QRadioButton, QButtonGroup,
    QTableWidget, QTableWidgetItem, QHeaderView, QFrame
)
from PyQt6.QtMultimedia import QMediaPlayer, QAudioOutput
from PyQt6.QtMultimediaWidgets import QVideoWidget
from PyQt6.QtCore import Qt, QUrl, pyqtSignal, QThread, QSettings, QEvent
from PyQt6.QtGui import QFont, QColor

import encoder

class EncodingWorker(QThread):
    progress_signal = pyqtSignal(object)
    finished_signal = pyqtSignal(str, int)
    error_signal = pyqtSignal(str)

    def __init__(self, input_path: str, output_path: str, start_s: float, end_s: float, target_mb: float, mode: str):
        super().__init__()
        self.input_path = input_path
        self.output_path = output_path
        self.start_s = start_s
        self.end_s = end_s
        self.target_mb = target_mb
        self.mode = mode
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
                cancel_token=self.cancel_token
            )
            final_size = os.path.getsize(res) if os.path.exists(res) else 0
            self.finished_signal.emit(res, final_size)
        except Exception as e:
            self.error_signal.emit(str(e))

    def cancel(self):
        self.cancel_token.cancel()

__version__ = "202608230-2-0"

TRANSLATIONS = {
    'pl': {
        'title': 'CutGut v{version} - Przycinanie i inteligentna kompresja wideo',
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
        'lbl_source': 'Źródło: {name}',
        'lbl_no_file': 'Brak wybranego filmu (przeciągnij plik tutaj)',
        'lbl_queue_header': '📋 Kolejka zadań ({count}):',
        'video_tooltip': 'Kliknij w podgląd lub naciśnij Spację, aby włączyć/zatrzymać odtwarzanie',
        'plan_box_title': 'Plan eksportu & Jakość',
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
        'settings_cleanup_header': 'Po udanym eksporcie wideo:',
        'opt_never': '🛡️ Nie usuwaj oryginału (Zalecane domyślnie)',
        'opt_ask': '❓ Pytaj za każdym razem po eksporcie',
        'opt_trash': '🗑️ Przenieś oryginał do Kosza automatycznie',
        'opt_delete': '⚠️ Usuń oryginał na stałe automatycznie (bez Kosza)',
        'btn_save': 'Zapisz',
        'btn_close': 'Zamknij',
        'ask_cleanup_title': 'Usunięcie oryginału',
        'ask_cleanup_msg': 'Eksport zakończony sukcesem!\nCzy chcesz przenieść oryginalny plik do Kosza?\n\n{path}',
        'perm_delete_warn_title': 'Ostrzeżenie o trwałym usuwaniu',
        'perm_delete_warn_msg': 'Uwaga: Wybranie tej opcji będzie bezpowrotnie usuwać oryginalne pliki wideo z dysku po każdym udanym eksporcie.\n\nCzy na pewno chcesz włączyć to ustawienie?'
    },
    'en': {
        'title': 'CutGut v{version} - Video Trimming & Smart Compression',
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
        'lbl_source': 'Source: {name}',
        'lbl_no_file': 'No video selected (drag & drop file here)',
        'lbl_queue_header': '📋 Task Queue ({count}):',
        'video_tooltip': 'Click preview or press Space to play/pause',
        'plan_box_title': 'Export Plan & Quality',
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
        'settings_cleanup_header': 'After successful video export:',
        'opt_never': '🛡️ Do not delete original file (Recommended default)',
        'opt_ask': '❓ Ask every time after export',
        'opt_trash': '🗑️ Move original to Recycle Bin automatically',
        'opt_delete': '⚠️ Delete original permanently automatically',
        'btn_save': 'Save',
        'btn_close': 'Close',
        'ask_cleanup_title': 'Original File Cleanup',
        'ask_cleanup_msg': 'Export completed successfully!\nDo you want to move the original source video to Recycle Bin?\n\n{path}',
        'perm_delete_warn_title': 'Permanent Deletion Warning',
        'perm_delete_warn_msg': 'Warning: This option will permanently delete original videos from disk after each export.\n\nAre you sure you want to enable this?'
    }
}

class SettingsDialog(QDialog):
    def __init__(self, parent, current_policy: str, current_lang: str):
        super().__init__(parent)
        self.parent_app = parent
        self.current_lang = current_lang
        self.selected_policy = current_policy
        self.init_ui()

    def t(self, key: str) -> str:
        lang_dict = TRANSLATIONS.get(self.current_lang, TRANSLATIONS['pl'])
        return lang_dict.get(key, TRANSLATIONS['en'].get(key, ''))

    def init_ui(self):
        self.setWindowTitle(self.t('settings_title'))
        self.setFixedWidth(440)
        self.setStyleSheet('''
            QDialog { background-color: #0f172a; }
            QLabel { color: #f8fafc; font-size: 13px; font-weight: bold; }
            QRadioButton { color: #e2e8f0; font-size: 13px; padding: 6px; }
            QRadioButton:hover { color: #60a5fa; }
            QPushButton { 
                border-radius: 6px; padding: 8px 16px; color: white; 
                font-weight: bold; background-color: #1e293b; border: 1px solid #334155; 
            }
            QPushButton:hover { background-color: #3b82f6; border: 1px solid #60a5fa; }
        ''')

        layout = QVBoxLayout(self)
        layout.setContentsMargins(20, 20, 20, 20)
        layout.setSpacing(12)

        lbl_hdr = QLabel(self.t('settings_cleanup_header'))
        layout.addWidget(lbl_hdr)

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

        layout.addSpacing(10)
        btn_box = QHBoxLayout()
        btn_box.addStretch()

        self.btnSave = QPushButton(self.t('btn_save'))
        self.btnSave.setStyleSheet('background-color: #2563eb;')
        self.btnSave.clicked.connect(self.on_save)
        btn_box.addWidget(self.btnSave)

        self.btnClose = QPushButton(self.t('btn_close'))
        self.btnClose.clicked.connect(self.reject)
        btn_box.addWidget(self.btnClose)

        layout.addLayout(btn_box)

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
        self.accept()

class ClickableVideoWidget(QVideoWidget):
    clicked_signal = pyqtSignal()

    def mouseReleaseEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked_signal.emit()
        super().mouseReleaseEvent(event)

class CutGutApp(QMainWindow):
    def __init__(self):
        super().__init__()

        self.settings = QSettings('Zeluqe', 'CutGut')
        self.current_lang = self.settings.value('language', 'pl')
        if self.current_lang not in ('pl', 'en'):
            self.current_lang = 'pl'

        self.cleanup_policy = self.settings.value('cleanup_policy', encoder.SourceCleanupPolicy.NEVER.value)

        self.setMinimumSize(1000, 860)
        self.setAcceptDrops(True)

        self.setStyleSheet('''
            QMainWindow { background-color: #0b0f19; }
            QLabel { color: #f8fafc; font-family: 'Segoe UI', system-ui; font-size: 13px; }
            
            QPushButton { 
                border-radius: 6px; padding: 9px 14px; color: white; 
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
        self.worker = None
        self.queue: list[encoder.EncodeJob] = []
        self.active_job: Optional[encoder.EncodeJob] = None

        # Media Player
        self.mediaPlayer = QMediaPlayer()
        self.videoWidget = ClickableVideoWidget()
        self.videoWidget.clicked_signal.connect(self.play_pause)
        self.audioOutput = QAudioOutput()
        self.mediaPlayer.setVideoOutput(self.videoWidget)
        self.mediaPlayer.setAudioOutput(self.audioOutput)
        self.audioOutput.setVolume(0.7)

        self.init_ui()
        self.retranslate_ui()
        self.check_ffmpeg_startup()

    def t(self, key: str) -> any:
        lang_dict = TRANSLATIONS.get(self.current_lang, TRANSLATIONS['pl'])
        return lang_dict.get(key, TRANSLATIONS['en'].get(key, ''))

    def init_ui(self):
        main_layout = QVBoxLayout()
        main_layout.setContentsMargins(16, 14, 16, 14)
        main_layout.setSpacing(10)

        # 1. Ekran wideo z tooltipem
        self.videoWidget.setStyleSheet('background-color: black; border-radius: 8px; border: 2px solid #1e293b;')
        self.videoWidget.setMinimumHeight(380)
        self.videoWidget.setToolTip(self.t('video_tooltip'))
        main_layout.addWidget(self.videoWidget)

        # 2. Pasek czasu i kontrolki odtwarzacza
        player_panel = QWidget()
        player_layout = QVBoxLayout(player_panel)
        player_layout.setContentsMargins(0, 0, 0, 0)
        player_layout.setSpacing(6)

        self.positionSlider = QSlider(Qt.Orientation.Horizontal)
        self.positionSlider.setCursor(Qt.CursorShape.PointingHandCursor)
        player_layout.addWidget(self.positionSlider)

        controls_row = QHBoxLayout()
        self.playBtn = QPushButton()
        self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPlay))
        self.playBtn.setFixedWidth(46)
        self.playBtn.setStyleSheet('background-color: #3b82f6;')
        self.playBtn.clicked.connect(self.play_pause)
        controls_row.addWidget(self.playBtn)

        self.timeLabel = QLabel('00:00 / 00:00')
        self.timeLabel.setStyleSheet('font-weight: bold; margin-left: 6px; font-size: 13px;')
        controls_row.addWidget(self.timeLabel)

        self.chkLoop = QCheckBox()
        self.chkLoop.setStyleSheet('margin-left: 12px;')
        controls_row.addWidget(self.chkLoop)

        controls_row.addStretch()

        # Głośność
        self.lblVol = QLabel()
        controls_row.addWidget(self.lblVol)
        self.volSlider = QSlider(Qt.Orientation.Horizontal)
        self.volSlider.setRange(0, 100)
        self.volSlider.setValue(70)
        self.volSlider.setFixedWidth(75)
        self.volSlider.valueChanged.connect(lambda v: self.audioOutput.setVolume(v / 100.0))
        controls_row.addWidget(self.volSlider)

        # Przełącznik języka (PL / EN)
        self.langCombo = QComboBox()
        self.langCombo.addItems(['🇵🇱 PL', '🇬🇧 EN'])
        self.langCombo.setFixedWidth(90)
        self.langCombo.setCurrentIndex(0 if self.current_lang == 'pl' else 1)
        self.langCombo.currentIndexChanged.connect(self.on_lang_changed)
        controls_row.addWidget(self.langCombo)

        # Przycisk Ustawień (⚙)
        self.btnSettings = QPushButton('⚙')
        self.btnSettings.setFixedWidth(40)
        self.btnSettings.setStyleSheet('background-color: #1e293b; font-size: 14px;')
        self.btnSettings.clicked.connect(self.open_settings)
        controls_row.addWidget(self.btnSettings)

        player_layout.addLayout(controls_row)
        main_layout.addWidget(player_panel)

        # 3. Karta Zakresu: IN / DŁUGOŚĆ / OUT
        trim_card = QFrame()
        trim_card.setStyleSheet('background-color: #131d2e; border-radius: 8px; border: 1px solid #1e293b;')
        trim_layout = QHBoxLayout(trim_card)
        trim_layout.setContentsMargins(10, 6, 10, 6)
        trim_layout.setSpacing(10)

        # Start (IN)
        in_box = QHBoxLayout()
        self.lblInTime = QLabel('IN: 00:00.00')
        self.lblInTime.setStyleSheet('font-weight: bold; color: #34d399; font-size: 13px;')
        self.btnStart = QPushButton()
        self.btnStart.setStyleSheet('background-color: #059669; padding: 6px 12px;')
        self.btnStart.clicked.connect(self.set_start)
        in_box.addWidget(self.lblInTime)
        in_box.addWidget(self.btnStart)
        trim_layout.addLayout(in_box)

        # Długość (Środek)
        self.lblDuration = QLabel('⏱️ DŁUGOŚĆ: 00:00.00')
        self.lblDuration.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.lblDuration.setStyleSheet('font-size: 13px; font-weight: bold; color: #60a5fa;')
        trim_layout.addWidget(self.lblDuration, 1)

        # Koniec (OUT)
        out_box = QHBoxLayout()
        self.lblOutTime = QLabel('OUT: 00:00.00')
        self.lblOutTime.setStyleSheet('font-weight: bold; color: #f87171; font-size: 13px;')
        self.btnEnd = QPushButton()
        self.btnEnd.setStyleSheet('background-color: #dc2626; padding: 6px 12px;')
        self.btnEnd.clicked.connect(self.set_end)
        out_box.addWidget(self.lblOutTime)
        out_box.addWidget(self.btnEnd)
        trim_layout.addLayout(out_box)

        main_layout.addWidget(trim_card)

        # 4. Centrum decyzji usera: Panel Planu i Oceny Jakości („Co wyjdzie?”)
        self.planCard = QFrame()
        self.planCard.setStyleSheet('background-color: #1e293b; border-radius: 8px; border: 1px solid #334155;')
        plan_layout = QVBoxLayout(self.planCard)
        plan_layout.setContentsMargins(14, 10, 14, 10)
        plan_layout.setSpacing(4)

        # Linia 1: Badge Jakości + Tytuł
        q_row = QHBoxLayout()
        self.lblPlanTitle = QLabel('📊 Plan eksportu')
        self.lblPlanTitle.setStyleSheet('font-weight: bold; color: #94a3b8; font-size: 12px;')
        q_row.addWidget(self.lblPlanTitle)
        q_row.addStretch()

        self.lblQualityBadge = QLabel('● Oczekiwanie na film')
        self.lblQualityBadge.setStyleSheet('font-weight: bold; font-size: 13px; color: #94a3b8;')
        q_row.addWidget(self.lblQualityBadge)
        plan_layout.addLayout(q_row)

        # Linia 2: Parametry techniczne
        self.lblPlanDetails = QLabel('Wybierz wideo, aby zobaczyć planowane parametry kodowania.')
        self.lblPlanDetails.setStyleSheet('font-size: 13px; font-weight: 600; color: #f8fafc;')
        plan_layout.addWidget(self.lblPlanDetails)

        # Linia 3: Opis jakości i Wskazówka
        self.lblPlanTip = QLabel('')
        self.lblPlanTip.setStyleSheet('font-size: 12px; color: #38bdf8;')
        self.lblPlanTip.setWordWrap(True)
        plan_layout.addWidget(self.lblPlanTip)

        main_layout.addWidget(self.planCard)

        # 5. Dół: Konfiguracja (Źródło, Limit, Enkoder) i Akcje
        ctrl_card = QFrame()
        ctrl_card.setStyleSheet('background-color: #131d2e; border-radius: 8px; border: 1px solid #1e293b;')
        ctrl_layout = QVBoxLayout(ctrl_card)
        ctrl_layout.setContentsMargins(12, 10, 12, 10)
        ctrl_layout.setSpacing(8)

        row_cfg = QHBoxLayout()
        self.btnSelect = QPushButton()
        self.btnSelect.setStyleSheet('background-color: #2563eb; padding: 8px 14px;')
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
        self.modeCombo.setFixedWidth(210)
        self.modeCombo.currentIndexChanged.connect(self.update_live_estimate)
        row_cfg.addWidget(self.modeCombo)
        ctrl_layout.addLayout(row_cfg)

        # Rząd akcji głównych
        row_act = QHBoxLayout()
        self.btnCompress = QPushButton()
        self.btnCompress.setStyleSheet('background-color: #2563eb; font-size: 14px; padding: 10px 20px; font-weight: bold;')
        self.btnCompress.setEnabled(False)
        self.btnCompress.clicked.connect(self.start_or_run_queue)
        row_act.addWidget(self.btnCompress, 2)

        self.btnAddToQueue = QPushButton()
        self.btnAddToQueue.setStyleSheet('background-color: #0891b2; font-size: 13px; padding: 10px 14px;')
        self.btnAddToQueue.setEnabled(False)
        self.btnAddToQueue.clicked.connect(self.add_to_queue)
        row_act.addWidget(self.btnAddToQueue, 1)

        self.btnCancel = QPushButton()
        self.btnCancel.setStyleSheet('background-color: #1e293b; color: #64748b; min-width: 80px; font-size: 13px;')
        self.btnCancel.setEnabled(False)
        self.btnCancel.clicked.connect(self.cancel_compression)
        row_act.addWidget(self.btnCancel)
        ctrl_layout.addLayout(row_act)

        main_layout.addWidget(ctrl_card)

        # 6. Panel Kolejki (ukryty domyślnie, widoczny gdy zadania stoją w kolejce)
        self.queueContainer = QWidget()
        self.queueLayout = QVBoxLayout(self.queueContainer)
        self.queueLayout.setContentsMargins(0, 0, 0, 0)
        self.queueLayout.setSpacing(4)

        self.lblQueueHeader = QLabel(self.t('lbl_queue_header').format(count=0))
        self.lblQueueHeader.setStyleSheet('font-weight: bold; color: #60a5fa; font-size: 12px;')
        self.queueLayout.addWidget(self.lblQueueHeader)

        self.queueTable = QTableWidget()
        self.queueTable.setColumnCount(6)
        self.queueTable.setHorizontalHeaderLabels(['Plik', 'Zakres', 'Limit', 'Jakość', 'Status', 'Akcja'])
        self.queueTable.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self.queueTable.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(4, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.horizontalHeader().setSectionResizeMode(5, QHeaderView.ResizeMode.ResizeToContents)
        self.queueTable.setMaximumHeight(110)
        self.queueLayout.addWidget(self.queueTable)
        self.queueContainer.setVisible(False)
        main_layout.addWidget(self.queueContainer)

        # 7. Pasek postępu i status
        self.progBar = QProgressBar()
        self.progBar.setFixedHeight(18)
        main_layout.addWidget(self.progBar)

        self.statusLabel = QLabel()
        self.statusLabel.setAlignment(Qt.AlignmentFlag.AlignCenter)
        main_layout.addWidget(self.statusLabel)

        container = QWidget()
        container.setLayout(main_layout)
        self.setCentralWidget(container)

        # Sygnały odtwarzacza
        self.mediaPlayer.positionChanged.connect(self.position_changed)
        self.mediaPlayer.durationChanged.connect(self.duration_changed)
        self.positionSlider.sliderMoved.connect(self.set_position)

    def keyPressEvent(self, event):
        key = event.key()
        modifiers = event.modifiers()
        
        # [I] = Start z playhead
        if key == Qt.Key.Key_I:
            self.set_start()
            event.accept()
        # [O] = Koniec z playhead
        elif key == Qt.Key.Key_O:
            self.set_end()
            event.accept()
        # [Spacja] = Play / Pauza
        elif key == Qt.Key.Key_Space:
            self.play_pause()
            event.accept()
        # [←] = -1s lub -1 klatka (z Shiftem)
        elif key == Qt.Key.Key_Left:
            step_ms = int(1000.0 / max(self.video_fps, 1.0)) if (modifiers & Qt.KeyboardModifier.ShiftModifier) else 1000
            self.mediaPlayer.setPosition(max(self.mediaPlayer.position() - step_ms, 0))
            event.accept()
        # [→] = +1s lub +1 klatka (z Shiftem)
        elif key == Qt.Key.Key_Right:
            step_ms = int(1000.0 / max(self.video_fps, 1.0)) if (modifiers & Qt.KeyboardModifier.ShiftModifier) else 1000
            self.mediaPlayer.setPosition(min(self.mediaPlayer.position() + step_ms, self.mediaPlayer.duration()))
            event.accept()
        # [Escape] = Anuluj
        elif key == Qt.Key.Key_Escape:
            if self.worker and self.worker.isRunning():
                self.cancel_compression()
            event.accept()
        else:
            super().keyPressEvent(event)

    def on_lang_changed(self, idx: int):
        self.current_lang = 'pl' if idx == 0 else 'en'
        self.settings.setValue('language', self.current_lang)
        self.retranslate_ui()

    def open_settings(self):
        dlg = SettingsDialog(self, self.cleanup_policy, self.current_lang)
        if dlg.exec() == QDialog.DialogCode.Accepted:
            self.cleanup_policy = dlg.selected_policy
            self.settings.setValue('cleanup_policy', self.cleanup_policy)

    def retranslate_ui(self):
        self.setWindowTitle(self.t('title').format(version=__version__))

        # Przyciski
        self.btnStart.setText(self.t('btn_set_in'))
        self.btnEnd.setText(self.t('btn_set_out'))
        self.chkLoop.setText(self.t('chk_loop'))
        self.lblVol.setText(self.t('lbl_volume'))
        self.btnSelect.setText(self.t('btn_change_file') if self.input_file else self.t('btn_select'))
        self.btnAddToQueue.setText(self.t('btn_add_queue'))
        self.btnCompress.setText(self.t('btn_compress'))
        self.btnCancel.setText(self.t('btn_cancel'))
        self.lblPlanTitle.setText(f"📊 {self.t('plan_box_title')}")
        self.videoWidget.setToolTip(self.t('video_tooltip'))

        # Zakres czasu
        self.update_range_text()

        # Status początkowy
        if not self.input_file:
            self.statusLabel.setText(self.t('ready_status'))
            self.lblSourceFile.setText(self.t('lbl_no_file'))

        # Presety rozmiaru
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
            except Exception:
                self.video_info = None
                self.video_fps = 60.0

            self.mediaPlayer.setSource(QUrl.fromLocalFile(file_path))
            self.btnCompress.setEnabled(True)
            self.btnAddToQueue.setEnabled(True)
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

        plan = encoder.calculate_plan(
            self.video_info, start_s, end_s, target_mb, is_hevc, self.input_file, self.current_lang
        )
        q: encoder.QualityAssessment = plan['quality']

        # 1. Badge jakości
        self.lblQualityBadge.setText(f"● {q.label}")
        self.lblQualityBadge.setStyleSheet(f"font-weight: bold; font-size: 13px; color: {q.color};")

        # 2. Szczegóły techniczne
        if plan['is_remux']:
            self.lblPlanDetails.setText(
                f"{self.video_info.width}×{self.video_info.height} · {self.video_fps:.0f} FPS · Błyskawiczny Remux (Direct Stream Copy) · Limit: {target_mb} MB"
            )
        else:
            enc_name = self.modeCombo.currentText().replace('⚡ ', '').replace('🔴 ', '').replace('🚀 ', '').replace('⚖️ ', '').replace('💨 ', '').replace('💎 ', '')
            self.lblPlanDetails.setText(
                f"{plan['out_width']}×{plan['out_height']} · {plan['out_fps']:.0f} FPS · {enc_name} · ~{plan['video_kbps']} kbps · Cel: ~{plan['target_bytes']/1000000:.1f} MB (Limit: {target_mb:.1f} MB)"
            )

        # 3. Opis i Wskazówka
        tip_text = f"<b>{q.description}</b>"
        if q.tip:
            tip_text += f"<br><span style='color: #38bdf8;'>💡 <b>Wskazówka:</b> {q.tip}</span>"
        self.lblPlanTip.setText(tip_text)

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

        out_dir = os.path.join(encoder.get_base_dir(), 'outputs')
        os.makedirs(out_dir, exist_ok=True)
        unique_id = int(time.time() * 1000)
        base_name = os.path.splitext(os.path.basename(self.input_file))[0]
        output_file = os.path.join(out_dir, f'CutGut_{base_name}_{unique_id}.mp4')

        job = encoder.EncodeJob(
            job_id=str(unique_id),
            input_path=self.input_file,
            output_path=output_file,
            start_s=start_s,
            end_s=end_s,
            target_mb=self.get_selected_target_mb(),
            preset_mode=self.get_selected_encoder_mode(),
            cleanup_policy=self.cleanup_policy
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
            self.queueTable.setItem(row, 1, QTableWidgetItem(f"{job.start_s:.1f}s - {job.end_s:.1f}s"))
            self.queueTable.setItem(row, 2, QTableWidgetItem(f"{job.target_mb:.0f} MB"))
            self.queueTable.setItem(row, 3, QTableWidgetItem(job.preset_mode))
            
            st_text = job.status
            if job.status == 'pending': st_text = '⏳ Oczekuje' if self.current_lang == 'pl' else '⏳ Pending'
            elif job.status == 'running': st_text = f'⚡ Kodowanie ({int(job.progress_pct)}%)'
            elif job.status == 'finished': st_text = '✅ Gotowe' if self.current_lang == 'pl' else '✅ Done'
            elif job.status == 'error': st_text = '❌ Błąd' if self.current_lang == 'pl' else '❌ Error'
            elif job.status == 'cancelled': st_text = '🛑 Anulowano' if self.current_lang == 'pl' else '🛑 Cancelled'
            self.queueTable.setItem(row, 4, QTableWidgetItem(st_text))

            self.queueTable.setItem(row, 5, QTableWidgetItem('Otwórz' if job.status == 'finished' else 'Usuń'))

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
            mode=job.preset_mode
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

        # Obsługa polityki kasowania pliku źródłowego (SourceCleanupPolicy)
        if cur_job and os.path.exists(out_path) and size_bytes > 0:
            pol = cur_job.cleanup_policy
            if pol == encoder.SourceCleanupPolicy.ASK.value:
                res = QMessageBox.question(
                    self, self.t('ask_cleanup_title'),
                    self.t('ask_cleanup_msg').format(path=os.path.basename(cur_job.input_path)),
                    QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                    QMessageBox.StandardButton.No
                )
                if res == QMessageBox.StandardButton.Yes:
                    encoder.cleanup_source_file(cur_job.input_path, out_path, encoder.SourceCleanupPolicy.TRASH)
            elif pol in (encoder.SourceCleanupPolicy.TRASH.value, encoder.SourceCleanupPolicy.DELETE_PERMANENTLY.value):
                encoder.cleanup_source_file(cur_job.input_path, out_path, encoder.SourceCleanupPolicy(pol))

        out_dir = os.path.dirname(out_path)
        if os.path.exists(out_dir):
            os.startfile(out_dir)

        # Kolejne zadanie z kolejki
        self.process_next_job()

    def on_worker_error(self, err_msg: str):
        if self.active_job:
            self.active_job.status = 'error'
            self.active_job.error_message = err_msg

        self.progBar.setValue(0)
        self.statusLabel.setText(self.t('error_status').format(msg=err_msg))
        self.update_queue_table()
        QMessageBox.critical(self, self.t('error_dialog_title'), err_msg)

        # Kontynuuj kolejne zadania
        self.process_next_job()

if __name__ == '__main__':
    app = QApplication(sys.argv)
    window = CutGutApp()
    window.show()
    sys.exit(app.exec())
