import sys
import os
import subprocess
import time
import threading

# Ścieżka dla PyInstaller (frozen) lub zwykłego uruchomienia
if getattr(sys, 'frozen', False):
    APP_DIR = sys._MEIPASS
    BASE_EXEC_DIR = os.path.dirname(sys.executable)
else:
    APP_DIR = os.path.dirname(os.path.abspath(__file__))
    BASE_EXEC_DIR = APP_DIR

os.environ["PATH"] = APP_DIR + os.pathsep + BASE_EXEC_DIR + os.pathsep + os.environ.get("PATH", "")

from PyQt6.QtWidgets import (QApplication, QMainWindow, QWidget, QVBoxLayout, 
                             QHBoxLayout, QPushButton, QLabel, QSlider, 
                             QFileDialog, QProgressBar, QStyle, QComboBox, QMessageBox)
from PyQt6.QtMultimedia import QMediaPlayer, QAudioOutput
from PyQt6.QtMultimediaWidgets import QVideoWidget
from PyQt6.QtCore import Qt, QUrl, pyqtSignal

CREATE_NO_WINDOW = 0x08000000

class CutGutPro(QMainWindow):
    status_requested = pyqtSignal(str, int)
    compression_finished = pyqtSignal()

    def __init__(self):
        super().__init__()

        self.setWindowTitle("CutGut Desktop")
        self.setMinimumSize(950, 750)
        
        # Aktywacja obsługi przeciągania plików (Drag & Drop)
        self.setAcceptDrops(True)
        
        # Dominujący kolor: NIEBIESKI + Slate Dark
        self.setStyleSheet("""
            QMainWindow { background-color: #0f172a; }
            QLabel { color: #f8fafc; font-family: 'Segoe UI'; font-size: 13px; }
            QPushButton { border-radius: 6px; padding: 10px; color: white; font-weight: bold; background-color: #1e293b; border: 1px solid #334155; }
            QPushButton:hover { background-color: #3b82f6; border: 1px solid #60a5fa; }
            
            /* STYL OSI CZASU (SLIDER) */
            QSlider::groove:horizontal {
                border: 1px solid #334155;
                height: 10px;
                background: #1e293b;
                margin: 2px 0;
                border-radius: 5px;
            }
            QSlider::handle:horizontal {
                background: #3b82f6;
                border: 2px solid #60a5fa;
                width: 18px;
                height: 18px;
                margin: -6px 0;
                border-radius: 9px;
            }
            
            QComboBox { background-color: #1e293b; color: white; border-radius: 5px; padding: 8px; border: 1px solid #3b82f6; }
            QProgressBar { border: 1px solid #334155; border-radius: 5px; text-align: center; color: white; font-weight: bold; }
            QProgressBar::chunk { background-color: #3b82f6; }
        """)

        self.input_file = ""
        self.start_ms = 0
        self.end_ms = 0

        self.mediaPlayer = QMediaPlayer()
        self.videoWidget = QVideoWidget()
        self.audioOutput = QAudioOutput()
        self.mediaPlayer.setVideoOutput(self.videoWidget)
        self.mediaPlayer.setAudioOutput(self.audioOutput)
        self.audioOutput.setVolume(70)

        main_layout = QVBoxLayout()
        main_layout.setContentsMargins(25, 25, 25, 25)
        main_layout.setSpacing(15)

        # 1. Ekran wideo (Centrum)
        self.videoWidget.setStyleSheet("background-color: black; border-radius: 12px; border: 2px solid #1e293b;")
        self.videoWidget.setMinimumHeight(450)
        main_layout.addWidget(self.videoWidget)

        # 2. Pasek Osi Czasu i Kontrolki (Timeline)
        player_panel = QWidget()
        player_layout = QVBoxLayout(player_panel)
        player_layout.setContentsMargins(0, 0, 0, 0)

        # Suwak (Timeline)
        self.positionSlider = QSlider(Qt.Orientation.Horizontal)
        self.positionSlider.setCursor(Qt.CursorShape.PointingHandCursor)
        player_layout.addWidget(self.positionSlider)

        # Przyciski pod Timeline
        controls_row = QHBoxLayout()
        self.playBtn = QPushButton()
        self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPlay))
        self.playBtn.setFixedWidth(60)
        self.playBtn.setStyleSheet("background-color: #3b82f6;")
        self.playBtn.clicked.connect(self.play_pause)
        controls_row.addWidget(self.playBtn)

        self.timeLabel = QLabel("00:00 / 00:00")
        self.timeLabel.setStyleSheet("font-weight: bold; margin-left: 10px;")
        controls_row.addWidget(self.timeLabel)
        controls_row.addStretch()
        
        player_layout.addLayout(controls_row)
        main_layout.addWidget(player_panel)

        # 3. Panel Przycinania (📍 START / 📍 KONIEC)
        trim_panel = QWidget()
        trim_panel.setStyleSheet("background-color: #1e293b; border-radius: 12px; border: 1px solid #334155;")
        trim_layout = QHBoxLayout(trim_panel)
        trim_layout.setContentsMargins(15, 15, 15, 15)
        
        self.btnStart = QPushButton("📍 USTAW START")
        self.btnStart.setStyleSheet("background-color: #059669; min-width: 140px;")
        self.btnStart.clicked.connect(self.set_start)
        trim_layout.addWidget(self.btnStart)

        self.lblRange = QLabel("Wybierz fragment filmu...")
        self.lblRange.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.lblRange.setStyleSheet("font-size: 15px; font-weight: bold; color: #60a5fa;")
        trim_layout.addWidget(self.lblRange)

        self.btnEnd = QPushButton("📍 USTAW KONIEC")
        self.btnEnd.setStyleSheet("background-color: #dc2626; min-width: 140px;")
        self.btnEnd.clicked.connect(self.set_end)
        trim_layout.addWidget(self.btnEnd)
        main_layout.addWidget(trim_panel)

        # 4. Wybór trybu, limitu i Eksport
        bottom_row = QHBoxLayout()
        
        self.modeCombo = QComboBox()
        self.modeCombo.addItems([
            "🚀 Szybki (H.264 Fast)", 
            "⚖️ Zbalansowany (H.264 Slow)", 
            "💎 Kinowy (H.265 Ultra)"
        ])
        self.modeCombo.setFixedWidth(210)
        bottom_row.addWidget(self.modeCombo)

        self.sizeCombo = QComboBox()
        self.sizeCombo.addItems([
            "🎯 Limit: Do 10 MB",
            "🚀 Limit: Do 20 MB"
        ])
        self.sizeCombo.setFixedWidth(160)
        bottom_row.addWidget(self.sizeCombo)

        self.btnSelect = QPushButton("📁 WYBIERZ FILM")
        self.btnSelect.setStyleSheet("background-color: #3b82f6;")
        self.btnSelect.clicked.connect(self.open_file)
        bottom_row.addWidget(self.btnSelect)

        self.btnCompress = QPushButton("🚀 PRZYTNIJ I KOMPRESUJ")
        self.btnCompress.setStyleSheet("background-color: #2563eb; min-width: 210px;")
        self.btnCompress.setEnabled(False)
        self.btnCompress.clicked.connect(self.start_compression)
        bottom_row.addWidget(self.btnCompress)
        
        main_layout.addLayout(bottom_row)

        # 5. Status i Progres
        self.progBar = QProgressBar()
        self.progBar.setFixedHeight(22)
        main_layout.addWidget(self.progBar)
        
        self.statusLabel = QLabel("Gotowy. Wybierz film z folderu NVIDIA lub przeciągnij go tutaj.")
        self.statusLabel.setAlignment(Qt.AlignmentFlag.AlignCenter)
        main_layout.addWidget(self.statusLabel)

        container = QWidget()
        container.setLayout(main_layout)
        self.setCentralWidget(container)

        # Sygnały PyQt
        self.mediaPlayer.positionChanged.connect(self.position_changed)
        self.mediaPlayer.durationChanged.connect(self.duration_changed)
        self.positionSlider.sliderMoved.connect(self.set_position)
        
        self.status_requested.connect(self.update_status_ui)
        self.compression_finished.connect(self.ask_to_delete_source)

    # --- Obsługa Drag and Drop ---
    def dragEnterEvent(self, event):
        if event.mimeData().hasUrls():
            for url in event.mimeData().urls():
                file_path = url.toLocalFile()
                if file_path.lower().endswith(('.mp4', '.mkv', '.avi', '.mov')):
                    event.acceptProposedAction()
                    return
        event.ignore()

    def dropEvent(self, event):
        for url in event.mimeData().urls():
            file_path = url.toLocalFile()
            if file_path.lower().endswith(('.mp4', '.mkv', '.avi', '.mov')):
                self.load_video(file_path)
                break

    def load_video(self, file_path):
        if file_path:
            self.input_file = file_path
            self.mediaPlayer.setSource(QUrl.fromLocalFile(file_path))
            self.btnCompress.setEnabled(True)
            self.statusLabel.setText(f"Wczytano: {os.path.basename(file_path)}")
            self.mediaPlayer.play()

    def open_file(self):
        default_path = r"C:\Users\malpi\Videos\NVIDIA"
        if not os.path.exists(default_path): default_path = ""
        file_path, _ = QFileDialog.getOpenFileName(self, "Wybierz film", default_path, "Wideo (*.mp4 *.mkv *.avi *.mov)")
        if file_path:
            self.load_video(file_path)

    def play_pause(self):
        if self.mediaPlayer.playbackState() == QMediaPlayer.PlaybackState.PlayingState:
            self.mediaPlayer.pause()
            self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPlay))
        else:
            self.mediaPlayer.play()
            self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPause))

    def set_position(self, position):
        self.mediaPlayer.setPosition(position)

    def position_changed(self, position):
        if not self.positionSlider.isSliderDown():
            self.positionSlider.setValue(position)
        self.timeLabel.setText(f"{self.format_time(position)} / {self.format_time(self.mediaPlayer.duration())}")

    def duration_changed(self, duration):
        self.positionSlider.setRange(0, duration)
        self.end_ms = duration
        self.update_range_text()

    def format_time(self, ms):
        s = ms // 1000
        m, s = divmod(s, 60)
        return f"{m:02d}:{s:02d}"

    def set_start(self):
        self.start_ms = self.mediaPlayer.position()
        self.update_range_text()

    def set_end(self):
        self.end_ms = self.mediaPlayer.position()
        self.update_range_text()

    def update_range_text(self):
        self.lblRange.setText(f"ZAKRES: {self.start_ms/1000:.2f}s - {self.end_ms/1000:.2f}s")

    def start_compression(self):
        self.btnCompress.setEnabled(False)
        threading.Thread(target=self.run_ffmpeg, daemon=True).start()

    def run_ffmpeg(self):
        try:
            start_s = self.start_ms / 1000
            dur_s = (self.end_ms - self.start_ms) / 1000
            if dur_s <= 0: return

            mode = self.modeCombo.currentIndex()
            size_mode = self.sizeCombo.currentIndex()
            out_dir = os.path.join(BASE_EXEC_DIR, "outputs")
            os.makedirs(out_dir, exist_ok=True)
            unique_id = int(time.time())
            
            # Docelowy rozmiar w bajtach: 9.95 MiB (~10 MB) lub 19.95 MiB (~20 MB)
            target_mb = 19.95 if size_mode == 1 else 9.95
            target_bytes = target_mb * 1024 * 1024
            a_br = 96000 if mode == 2 else 128000
            overhead_bytes = 30000 + (dur_s * 500)
            net_video_bytes = max(target_bytes - (a_br * dur_s / 8) - overhead_bytes, 1000)
            v_br = max(int((net_video_bytes * 8 / dur_s) / 1000), 50)
            
            if mode == 0: # Szybki
                codec, preset, stats = 'libx264', 'veryfast', f"stats_f_{unique_id}"
                p1 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '1']
                p2 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '2', '-c:a', 'aac', '-b:a', '128k']
            elif mode == 1: # Zbalansowany
                codec, preset, stats = 'libx264', 'slow', f"stats_b_{unique_id}"
                p1 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '1']
                p2 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '2', '-c:a', 'aac', '-b:a', '128k']
            else: # Kinowy H.265
                codec, preset, stats = 'libx265', 'slow', f"stats_h_{unique_id}"
                p1 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-x265-params', f'pass=1:stats={stats}']
                p2 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-x265-params', f'pass=2:stats={stats}', '-c:a', 'aac', '-b:a', '96k']

            out_file = os.path.join(out_dir, f"CutGut_{unique_id}.mp4")
            common = ['-ss', str(start_s), '-t', str(dur_s), '-i', self.input_file]

            self.status_requested.emit("Etap 1: Analiza wideo...", 30)
            subprocess.run(['ffmpeg', '-y'] + common + p1 + (['-passlogfile', stats] if mode < 2 else []) + ['-an', '-f', 'mp4', 'NUL'], creationflags=CREATE_NO_WINDOW)
            
            self.status_requested.emit("Etap 2: Finalna kompresja...", 75)
            subprocess.run(['ffmpeg', '-y'] + common + p2 + (['-passlogfile', stats] if mode < 2 else []) + [out_file], creationflags=CREATE_NO_WINDOW)
            
            self.status_requested.emit("Sukces! Plik gotowy.", 100)
            
            for f in os.listdir('.'):
                if stats in f:
                    try: os.remove(f)
                    except: pass
            
            os.startfile(out_dir)
            self.compression_finished.emit()

        except Exception as e:
            self.status_requested.emit(f"Błąd: {str(e)}", 0)
        finally:
            self.btnCompress.setEnabled(True)

    def update_status_ui(self, text, val):
        self.statusLabel.setText(text)
        self.progBar.setValue(val)

    def ask_to_delete_source(self):
        msg = QMessageBox(self)
        msg.setWindowTitle("Zwolnić miejsce?")
        msg.setText(f"Kompresja zakończona!\n\nCzy chcesz usunąć ORYGINALNY plik?\n({os.path.basename(self.input_file)})")
        msg.setStandardButtons(QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No)
        ret = msg.exec()
        if ret == QMessageBox.StandardButton.Yes:
            try:
                self.mediaPlayer.stop()
                self.mediaPlayer.setSource(QUrl(""))
                time.sleep(0.5)
                os.remove(self.input_file)
                self.statusLabel.setText("Oryginał usunięty.")
            except Exception as e:
                self.statusLabel.setText(f"Nie udało się usunąć: {e}")
        else:
            # Gdy użytkownik kliknie "Nie", po prostu przywracamy normalny stan wideo i status
            self.statusLabel.setText("Oryginał zachowany. Gotowy do kolejnej pracy.")

if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = CutGutPro()
    window.show()
    sys.exit(app.exec())
