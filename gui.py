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

class CutGutApp(QMainWindow):
    def __init__(self):
        super().__init__()

        self.setWindowTitle('CutGut - Video Trimming & Smart Compression')
        self.setMinimumSize(960, 780)
        self.setAcceptDrops(True)

        self.settings = QSettings('Zeluqe', 'CutGut')

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
        self.check_ffmpeg_startup()

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

        self.chkLoop = QCheckBox('Zapętlij podgląd (Loop)')
        self.chkLoop.setStyleSheet('margin-left: 15px;')
        controls_row.addWidget(self.chkLoop)

        controls_row.addStretch()

        # Volume slider
        lbl_vol = QLabel('Głośność:')
        controls_row.addWidget(lbl_vol)
        self.volSlider = QSlider(Qt.Orientation.Horizontal)
        self.volSlider.setRange(0, 100)
        self.volSlider.setValue(70)
        self.volSlider.setFixedWidth(80)
        self.volSlider.valueChanged.connect(lambda v: self.audioOutput.setVolume(v / 100.0))
        controls_row.addWidget(self.volSlider)

        player_layout.addLayout(controls_row)
        main_layout.addWidget(player_panel)

        # 3. Panel zakresu przycinania
        trim_panel = QWidget()
        trim_panel.setStyleSheet('background-color: #1e293b; border-radius: 10px; border: 1px solid #334155;')
        trim_layout = QHBoxLayout(trim_panel)
        trim_layout.setContentsMargins(12, 10, 12, 10)

        self.btnStart = QPushButton('USTAW START')
        self.btnStart.setStyleSheet('background-color: #059669; min-width: 130px;')
        self.btnStart.clicked.connect(self.set_start)
        trim_layout.addWidget(self.btnStart)

        self.lblRange = QLabel('Wybierz fragment filmu do przycięcia...')
        self.lblRange.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.lblRange.setStyleSheet('font-size: 14px; font-weight: bold; color: #60a5fa;')
        trim_layout.addWidget(self.lblRange)

        self.btnEnd = QPushButton('USTAW KONIEC')
        self.btnEnd.setStyleSheet('background-color: #dc2626; min-width: 130px;')
        self.btnEnd.clicked.connect(self.set_end)
        trim_layout.addWidget(self.btnEnd)
        main_layout.addWidget(trim_panel)

        # 4. Wybor limitu, enkodera i przyciski akcji
        bottom_row = QHBoxLayout()
        bottom_row.setSpacing(10)

        # Limit preset
        self.limitCombo = QComboBox()
        self.limitCombo.addItems([
            'Discord Free (20 MB)',
            'Legacy / Small (10 MB)',
            'Nitro Basic (50 MB)',
            'Nitro (500 MB)',
            'Własny rozmiar...'
        ])
        self.limitCombo.setFixedWidth(200)
        self.limitCombo.currentIndexChanged.connect(self.on_limit_changed)
        bottom_row.addWidget(self.limitCombo)

        # Enkoder
        self.modeCombo = QComboBox()
        nvenc_avail = encoder.check_nvenc_support()
        amf_avail = encoder.check_amf_support()

        if nvenc_avail:
            self.modeCombo.addItems([
                '⚡ NVIDIA NVENC (GPU HQ)',
                '🚀 NVIDIA NVENC (GPU Fast)',
                '⚖️ CPU H.264 (Zbalansowany)',
                '💨 CPU H.264 (Szybki)',
                '💎 CPU H.265 (Kinowy)'
            ])
        elif amf_avail:
            self.modeCombo.addItems([
                '🔴 AMD AMF (GPU HQ)',
                '🚀 AMD AMF (GPU Fast)',
                '⚖️ CPU H.264 (Zbalansowany)',
                '💨 CPU H.264 (Szybki)',
                '💎 CPU H.265 (Kinowy)'
            ])
        else:
            self.modeCombo.addItems([
                '⚖️ CPU H.264 (Zbalansowany)',
                '💨 CPU H.264 (Szybki)',
                '💎 CPU H.265 (Kinowy)'
            ])
        self.modeCombo.setFixedWidth(220)
        bottom_row.addWidget(self.modeCombo)

        self.btnSelect = QPushButton('WYBIERZ FILM')
        self.btnSelect.setStyleSheet('background-color: #3b82f6;')
        self.btnSelect.clicked.connect(self.open_file)
        bottom_row.addWidget(self.btnSelect)

        self.btnCompress = QPushButton('PRZYTNIJ I KOMPRESUJ')
        self.btnCompress.setStyleSheet('background-color: #2563eb; min-width: 200px;')
        self.btnCompress.setEnabled(False)
        self.btnCompress.clicked.connect(self.start_compression)
        bottom_row.addWidget(self.btnCompress)

        self.btnCancel = QPushButton('ANULUJ')
        self.btnCancel.setStyleSheet('background-color: #991b1b; min-width: 90px;')
        self.btnCancel.setEnabled(False)
        self.btnCancel.clicked.connect(self.cancel_compression)
        bottom_row.addWidget(self.btnCancel)

        main_layout.addLayout(bottom_row)

        # 5. Pasek postepu i status
        self.progBar = QProgressBar()
        self.progBar.setFixedHeight(22)
        main_layout.addWidget(self.progBar)

        self.statusLabel = QLabel('Gotowy. Wybierz film z dysku lub przeciągnij go do okna.')
        self.statusLabel.setAlignment(Qt.AlignmentFlag.AlignCenter)
        main_layout.addWidget(self.statusLabel)

        container = QWidget()
        container.setLayout(main_layout)
        self.setCentralWidget(container)

        # Sygnaly odtwarzacza
        self.mediaPlayer.positionChanged.connect(self.position_changed)
        self.mediaPlayer.durationChanged.connect(self.duration_changed)
        self.positionSlider.sliderMoved.connect(self.set_position)

    def check_ffmpeg_startup(self):
        if not encoder.ensure_ffmpeg():
            self.statusLabel.setText('Uwaga: Brak FFmpeg. Zainstaluj FFmpeg lub umieść ffmpeg.exe w folderze programu.')

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
            self, 'Wybierz film', default_dir,
            'Pliki Wideo (*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts)'
        )
        if fp:
            self.settings.setValue('last_video_dir', os.path.dirname(fp))
            self.load_video(fp)

    def load_video(self, file_path: str):
        if file_path and os.path.exists(file_path):
            self.input_file = file_path
            self.mediaPlayer.setSource(QUrl.fromLocalFile(file_path))
            self.btnCompress.setEnabled(True)
            self.statusLabel.setText(f'Wczytano: {os.path.basename(file_path)}')
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
            f'ZAKRES: {self.start_ms/1000:.2f}s - {self.end_ms/1000:.2f}s  (Długość: {dur_s:.2f}s)'
        )

    def on_limit_changed(self, idx: int):
        if idx == 4:
            val, ok = QInputDialog.getDouble(
                self, 'Własny limit rozmiaru', 'Podaj maksymalny rozmiar w MB:',
                self.custom_mb, 1.0, 2000.0, 1
            )
            if ok:
                self.custom_mb = val
                self.limitCombo.setItemText(4, f'Własny ({val:.1f} MB)')
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
        txt = self.modeCombo.currentText()
        if 'NVENC (GPU HQ)' in txt:
            return 'NVENC_HQ'
        elif 'NVENC (GPU Fast)' in txt:
            return 'NVENC_FAST'
        elif 'AMD AMF (GPU HQ)' in txt:
            return 'AMF_HQ'
        elif 'AMD AMF (GPU Fast)' in txt:
            return 'AMF_FAST'
        elif 'CPU H.264 (Szybki)' in txt:
            return 'CPU_FAST'
        elif 'CPU H.265' in txt:
            return 'CPU_HEVC'
        else:
            return 'CPU_BALANCED'

    def start_compression(self):
        start_s = self.start_ms / 1000.0
        end_s = self.end_ms / 1000.0
        dur_s = end_s - start_s

        if dur_s <= 0.1:
            QMessageBox.warning(self, 'Nieprawidłowy zakres', 'Czas końca musi być większy niż czas początku!')
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
        self.statusLabel.setText('Inicjalizacja kompresji...')

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
            self.statusLabel.setText('Anulowanie zadania...')
            self.worker.cancel()

    def on_worker_progress(self, p: encoder.ProgressUpdate):
        self.progBar.setValue(int(p.percent))
        eta_txt = f' | ETA: {p.eta_s:.1f}s' if p.eta_s > 0 else ''
        self.statusLabel.setText(f'[{p.stage}] {p.percent:.1f}% | Prędkość: {p.speed}{eta_txt}')

    def on_worker_finished(self, out_path: str, size_bytes: int):
        self.btnCompress.setEnabled(True)
        self.btnSelect.setEnabled(True)
        self.btnCancel.setEnabled(False)
        self.progBar.setValue(100)

        mb_val = size_bytes / 1000000.0
        mib_val = size_bytes / (1024.0 * 1024.0)
        self.statusLabel.setText(
            f'Gotowe! Zapisano: {os.path.basename(out_path)} ({mb_val:.2f} MB / {mib_val:.2f} MiB)'
        )

        out_dir = os.path.dirname(out_path)
        if os.path.exists(out_dir):
            os.startfile(out_dir)

    def on_worker_error(self, err_msg: str):
        self.btnCompress.setEnabled(True)
        self.btnSelect.setEnabled(True)
        self.btnCancel.setEnabled(False)
        self.progBar.setValue(0)
        self.statusLabel.setText(f'Błąd: {err_msg}')
        QMessageBox.critical(self, 'Błąd kompresji', err_msg)

if __name__ == '__main__':
    app = QApplication(sys.argv)
    window = CutGutApp()
    window.show()
    sys.exit(app.exec())
