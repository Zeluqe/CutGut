import sys
import os
import time
from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout,
    QHBoxLayout, QPushButton, QLabel, QSlider,
    QFileDialog, QProgressBar, QStyle, QComboBox, QMessageBox,
    QCheckBox, QInputDialog
)
from PyQt6.QtMultimedia import QMediaPlayer, QAudioOutput
from PyQt6.QtMultimediaWidgets import QVideoWidget
from PyQt6.QtCore import Qt, QUrl, pyqtSignal, QThread, QSettings

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

__version__ = "202608230-0-0"

TRANSLATIONS = {
    'pl': {
        'title': 'CutGut v{version} - Przycinanie i inteligentna kompresja wideo',
        'btn_start': '📍 USTAW START',
        'btn_end': '📍 USTAW KONIEC',
        'range_initial': 'Wybierz fragment filmu do przycięcia...',
        'range_text': 'ZAKRES: {start:.2f}s - {end:.2f}s  (Długość: {dur:.2f}s)',
        'chk_loop': '🔁 Zapętlij podgląd (Loop)',
        'lbl_volume': '🔊 Głośność:',
        'btn_select': '📁 WYBIERZ FILM',
        'btn_compress': '🚀 PRZYTNIJ I KOMPRESUJ',
        'btn_cancel': '🛑 ANULUJ',
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
        'ready_status': 'Gotowy. Wybierz film z dysku lub przeciągnij go do okna.',
        'loaded_status': 'Wczytano: {filename}',
        'init_status': 'Inicjalizacja kompresji...',
        'cancel_status': 'Anulowanie zadania...',
        'progress_status': '[{stage}] {percent:.1f}% | Prędkość: {speed}{eta}',
        'done_status': '✅ Gotowe! Zapisano: {filename} ({mb:.2f} MB / {mib:.2f} MiB)',
        'error_status': '❌ Błąd: {msg}',
        'invalid_range_title': 'Nieprawidłowy zakres',
        'invalid_range_msg': 'Czas końca musi być większy niż czas początku!',
        'ffmpeg_missing_status': '⚠️ Uwaga: Brak FFmpeg. Zainstaluj FFmpeg lub umieść ffmpeg.exe w folderze programu.',
        'dialog_open_title': 'Wybierz film',
        'dialog_open_filter': 'Pliki Wideo (*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts)',
        'error_dialog_title': 'Błąd kompresji'
    },
    'en': {
        'title': 'CutGut v{version} - Video Trimming & Smart Compression',
        'btn_start': '📍 SET START',
        'btn_end': '📍 SET END',
        'range_initial': 'Select a video segment to trim...',
        'range_text': 'RANGE: {start:.2f}s - {end:.2f}s  (Duration: {dur:.2f}s)',
        'chk_loop': '🔁 Loop preview',
        'lbl_volume': '🔊 Volume:',
        'btn_select': '📁 SELECT VIDEO',
        'btn_compress': '🚀 TRIM & COMPRESS',
        'btn_cancel': '🛑 CANCEL',
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
        'ready_status': 'Ready. Select a video from disk or drag & drop here.',
        'loaded_status': 'Loaded: {filename}',
        'init_status': 'Initializing compression...',
        'cancel_status': 'Cancelling task...',
        'progress_status': '[{stage}] {percent:.1f}% | Speed: {speed}{eta}',
        'done_status': '✅ Done! Saved: {filename} ({mb:.2f} MB / {mib:.2f} MiB)',
        'error_status': '❌ Error: {msg}',
        'invalid_range_title': 'Invalid Range',
        'invalid_range_msg': 'End time must be greater than start time!',
        'ffmpeg_missing_status': '⚠️ Note: FFmpeg missing. Install FFmpeg or place ffmpeg.exe in app folder.',
        'dialog_open_title': 'Select video',
        'dialog_open_filter': 'Video Files (*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts)',
        'error_dialog_title': 'Compression Error'
    }
}

class CutGutApp(QMainWindow):
    def __init__(self):
        super().__init__()

        self.settings = QSettings('Zeluqe', 'CutGut')
        self.current_lang = self.settings.value('language', 'pl')
        if self.current_lang not in ('pl', 'en'):
            self.current_lang = 'pl'

        self.setMinimumSize(980, 800)
        self.setAcceptDrops(True)

        self.setStyleSheet('''
            QMainWindow { background-color: #0f172a; }
            QLabel { color: #f8fafc; font-family: 'Segoe UI'; font-size: 13px; }
            QPushButton { 
                border-radius: 6px; padding: 10px 14px; color: white; 
                font-weight: bold; background-color: #1e293b; border: 1px solid #334155; 
            }
            QPushButton:hover { background-color: #3b82f6; border: 1px solid #60a5fa; }
            QPushButton:disabled { background-color: #1e293b; color: #64748b; border: 1px solid #334155; }
            
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
        ''')

        self.input_file = ''
        self.start_ms = 0
        self.end_ms = 0
        self.custom_mb = 20.0
        self.worker = None

        # Media Player
        self.mediaPlayer = QMediaPlayer()
        self.videoWidget = QVideoWidget()
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
        main_layout.setContentsMargins(20, 20, 20, 20)
        main_layout.setSpacing(12)

        # 1. Ekran wideo
        self.videoWidget.setStyleSheet('background-color: black; border-radius: 10px; border: 2px solid #1e293b;')
        self.videoWidget.setMinimumHeight(420)
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
        self.playBtn.setFixedWidth(50)
        self.playBtn.setStyleSheet('background-color: #3b82f6;')
        self.playBtn.clicked.connect(self.play_pause)
        controls_row.addWidget(self.playBtn)

        self.timeLabel = QLabel('00:00 / 00:00')
        self.timeLabel.setStyleSheet('font-weight: bold; margin-left: 8px;')
        controls_row.addWidget(self.timeLabel)

        self.chkLoop = QCheckBox()
        self.chkLoop.setStyleSheet('margin-left: 15px;')
        controls_row.addWidget(self.chkLoop)

        controls_row.addStretch()

        # Głośność
        self.lblVol = QLabel()
        controls_row.addWidget(self.lblVol)
        self.volSlider = QSlider(Qt.Orientation.Horizontal)
        self.volSlider.setRange(0, 100)
        self.volSlider.setValue(70)
        self.volSlider.setFixedWidth(80)
        self.volSlider.valueChanged.connect(lambda v: self.audioOutput.setVolume(v / 100.0))
        controls_row.addWidget(self.volSlider)

        # Przełącznik języka (PL / EN)
        self.langCombo = QComboBox()
        self.langCombo.addItems(['🇵🇱 Polski', '🇬🇧 English'])
        self.langCombo.setFixedWidth(120)
        self.langCombo.setCurrentIndex(0 if self.current_lang == 'pl' else 1)
        self.langCombo.currentIndexChanged.connect(self.on_lang_changed)
        controls_row.addWidget(self.langCombo)

        player_layout.addLayout(controls_row)
        main_layout.addWidget(player_panel)

        # 3. Panel zakresu przycinania
        trim_panel = QWidget()
        trim_panel.setStyleSheet('background-color: #1e293b; border-radius: 10px; border: 1px solid #334155;')
        trim_layout = QHBoxLayout(trim_panel)
        trim_layout.setContentsMargins(12, 10, 12, 10)

        self.btnStart = QPushButton()
        self.btnStart.setStyleSheet('background-color: #059669; min-width: 140px;')
        self.btnStart.clicked.connect(self.set_start)
        trim_layout.addWidget(self.btnStart)

        self.lblRange = QLabel()
        self.lblRange.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.lblRange.setStyleSheet('font-size: 14px; font-weight: bold; color: #60a5fa;')
        trim_layout.addWidget(self.lblRange)

        self.btnEnd = QPushButton()
        self.btnEnd.setStyleSheet('background-color: #dc2626; min-width: 140px;')
        self.btnEnd.clicked.connect(self.set_end)
        trim_layout.addWidget(self.btnEnd)
        main_layout.addWidget(trim_panel)

        # 4. Wybór limitu, enkodera i przyciski akcji
        bottom_row = QHBoxLayout()
        bottom_row.setSpacing(10)

        # Limit preset
        self.limitCombo = QComboBox()
        self.limitCombo.setFixedWidth(200)
        self.limitCombo.currentIndexChanged.connect(self.on_limit_changed)
        bottom_row.addWidget(self.limitCombo)

        # Enkoder
        self.modeCombo = QComboBox()
        self.modeCombo.setFixedWidth(220)
        bottom_row.addWidget(self.modeCombo)

        self.btnSelect = QPushButton()
        self.btnSelect.setStyleSheet('background-color: #3b82f6;')
        self.btnSelect.clicked.connect(self.open_file)
        bottom_row.addWidget(self.btnSelect)

        self.btnCompress = QPushButton()
        self.btnCompress.setStyleSheet('background-color: #2563eb; min-width: 200px;')
        self.btnCompress.setEnabled(False)
        self.btnCompress.clicked.connect(self.start_compression)
        bottom_row.addWidget(self.btnCompress)

        self.btnCancel = QPushButton()
        self.btnCancel.setStyleSheet('background-color: #991b1b; min-width: 90px;')
        self.btnCancel.setEnabled(False)
        self.btnCancel.clicked.connect(self.cancel_compression)
        bottom_row.addWidget(self.btnCancel)

        main_layout.addLayout(bottom_row)

        # 5. Pasek postępu i status
        self.progBar = QProgressBar()
        self.progBar.setFixedHeight(22)
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

    def on_lang_changed(self, idx: int):
        self.current_lang = 'pl' if idx == 0 else 'en'
        self.settings.setValue('language', self.current_lang)
        self.retranslate_ui()

    def retranslate_ui(self):
        # Tytuł okna
        self.setWindowTitle(self.t('title').format(version=__version__))

        # Przyciski i etykiety
        self.btnStart.setText(self.t('btn_start'))
        self.btnEnd.setText(self.t('btn_end'))
        self.chkLoop.setText(self.t('chk_loop'))
        self.lblVol.setText(self.t('lbl_volume'))
        self.btnSelect.setText(self.t('btn_select'))
        self.btnCompress.setText(self.t('btn_compress'))
        self.btnCancel.setText(self.t('btn_cancel'))

        # Zakres czasu
        if not self.input_file:
            self.lblRange.setText(self.t('range_initial'))
        else:
            self.update_range_text()

        # Status początkowy
        if not self.input_file:
            self.statusLabel.setText(self.t('ready_status'))

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
            self.mediaPlayer.setSource(QUrl.fromLocalFile(file_path))
            self.btnCompress.setEnabled(True)
            self.statusLabel.setText(self.t('loaded_status').format(filename=os.path.basename(file_path)))
            self.mediaPlayer.play()
            self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPause))

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
        dur_s = max((self.end_ms - self.start_ms) / 1000.0, 0.0)
        self.lblRange.setText(
            self.t('range_text').format(start=self.start_ms/1000.0, end=self.end_ms/1000.0, dur=dur_s)
        )

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

    def start_compression(self):
        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        dur_s = end_s - start_s

        if dur_s <= 0.1:
            QMessageBox.warning(self, self.t('invalid_range_title'), self.t('invalid_range_msg'))
            return

        target_mb = self.get_selected_target_mb()
        mode = self.get_selected_encoder_mode()

        out_dir = os.path.join(encoder.get_base_dir(), 'outputs')
        os.makedirs(out_dir, exist_ok=True)
        unique_id = int(time.time())
        output_file = os.path.join(out_dir, f'CutGut_{unique_id}.mp4')

        self.btnCompress.setEnabled(False)
        self.btnSelect.setEnabled(False)
        self.btnCancel.setEnabled(True)
        self.progBar.setValue(0)
        self.statusLabel.setText(self.t('init_status'))

        self.worker = EncodingWorker(
            input_path=self.input_file,
            output_path=output_file,
            start_s=start_s,
            end_s=end_s,
            target_mb=target_mb,
            mode=mode
        )
        self.worker.progress_signal.connect(self.on_worker_progress)
        self.worker.finished_signal.connect(self.on_worker_finished)
        self.worker.error_signal.connect(self.on_worker_error)
        self.worker.start()

    def cancel_compression(self):
        if self.worker and self.worker.isRunning():
            self.statusLabel.setText(self.t('cancel_status'))
            self.worker.cancel()

    def on_worker_progress(self, p: encoder.ProgressUpdate):
        self.progBar.setValue(int(p.percent))
        eta_txt = f' | ETA: {p.eta_s:.1f}s' if p.eta_s > 0 else ''
        self.statusLabel.setText(self.t('progress_status').format(
            stage=p.stage, percent=p.percent, speed=p.speed, eta=eta_txt
        ))

    def on_worker_finished(self, out_path: str, size_bytes: int):
        self.btnCompress.setEnabled(True)
        self.btnSelect.setEnabled(True)
        self.btnCancel.setEnabled(False)
        self.progBar.setValue(100)

        mb_val = size_bytes / 1000000.0
        mib_val = size_bytes / (1024.0 * 1024.0)
        self.statusLabel.setText(self.t('done_status').format(
            filename=os.path.basename(out_path), mb=mb_val, mib=mib_val
        ))

        out_dir = os.path.dirname(out_path)
        if os.path.exists(out_dir):
            os.startfile(out_dir)

    def on_worker_error(self, err_msg: str):
        self.btnCompress.setEnabled(True)
        self.btnSelect.setEnabled(True)
        self.btnCancel.setEnabled(False)
        self.progBar.setValue(0)
        self.statusLabel.setText(self.t('error_status').format(msg=err_msg))
        QMessageBox.critical(self, self.t('error_dialog_title'), err_msg)

if __name__ == '__main__':
    app = QApplication(sys.argv)
    window = CutGutApp()
    window.show()
    sys.exit(app.exec())
