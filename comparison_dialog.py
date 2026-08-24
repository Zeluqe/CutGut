import sys
import os
import subprocess
from typing import Optional
from PyQt6.QtWidgets import (
    QDialog, QWidget, QVBoxLayout, QHBoxLayout,
    QPushButton, QLabel, QSlider, QStyle, QFrame,
    QCheckBox, QMessageBox
)
from PyQt6.QtMultimedia import QMediaPlayer, QAudioOutput
from PyQt6.QtMultimediaWidgets import QVideoWidget
from PyQt6.QtCore import Qt, QUrl, pyqtSignal, QTimer
from PyQt6.QtGui import QColor, QFont
import encoder
import ui.icons

class ClickableVideoWidget(QVideoWidget):
    clicked_signal = pyqtSignal()

    def mouseReleaseEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked_signal.emit()
        super().mouseReleaseEvent(event)

class QualityComparisonDialog(QDialog):
    closed_signal = pyqtSignal(object)  # emits ExportResult

    def __init__(self, parent, result: encoder.ExportResult, lang: str = 'en'):
        super().__init__(parent)
        self.result = result
        self.lang = lang
        self.start_offset_ms = int(result.start_s * 1000)
        self.total_duration_ms = max(int(result.duration_s * 1000), 100)

        # Media Players
        self.playerSource = QMediaPlayer()
        self.playerTarget = QMediaPlayer()

        self.videoSource = ClickableVideoWidget()
        self.videoTarget = ClickableVideoWidget()

        self.audioSource = QAudioOutput()
        self.audioTarget = QAudioOutput()

        self.playerSource.setVideoOutput(self.videoSource)
        self.playerSource.setAudioOutput(self.audioSource)
        self.audioSource.setMuted(True)

        self.playerTarget.setVideoOutput(self.videoTarget)
        self.playerTarget.setAudioOutput(self.audioTarget)
        self.audioTarget.setVolume(0.7)

        # Sync Timer
        self.syncTimer = QTimer(self)
        self.syncTimer.setInterval(250)
        self.syncTimer.timeout.connect(self.sync_drift)

        self.init_ui()
        self.load_media()

    def init_ui(self):
        title_text = "Quality Comparison (A/B)" if self.lang == 'en' else "Porównanie jakości (A/B)"
        if self.result.is_sample:
            title_text += " — " + ("Quality Sample" if self.lang == 'en' else "Próbka jakości")
        else:
            title_text += " — " + ("Export Result" if self.lang == 'en' else "Wynik eksportu")
        self.setWindowTitle(title_text)
        self.setMinimumSize(1080, 680)

        self.setStyleSheet('''
            QDialog { background-color: #141416; }
            QLabel { color: #f4f4f6; font-family: 'Segoe UI Variable', 'Segoe UI', system-ui; font-size: 13px; }
            QPushButton { 
                border-radius: 6px; padding: 7px 14px; color: #f4f4f6; 
                font-weight: 600; background-color: #222226; border: 1px solid rgba(255, 255, 255, 0.1); 
            }
            QPushButton:hover { background-color: #2c2c33; border: 1px solid #0078d4; }
            QSlider::groove:horizontal {
                border: none; height: 6px; background: #222226; border-radius: 3px;
            }
            QSlider::handle:horizontal {
                background: #0078d4; border: 2px solid #ffffff;
                width: 16px; height: 16px; margin: -5px 0; border-radius: 8px;
            }
            QCheckBox { color: #a1a1aa; font-weight: 500; }
            QCheckBox::indicator { width: 16px; height: 16px; border-radius: 4px; border: 1px solid rgba(255, 255, 255, 0.16); background: #222226; }
            QCheckBox::indicator:checked { background: #0078d4; border: 1px solid #0078d4; }
        ''')

        layout = QVBoxLayout(self)
        layout.setContentsMargins(14, 12, 14, 12)
        layout.setSpacing(8)

        # 1. Headers A / B
        hdr_layout = QHBoxLayout()

        lbl_left_title = QLabel("<b>ORIGINAL</b> (Source Clip)" if self.lang == 'en' else "<b>ORYGINAŁ</b> (Wycinek źródłowy)")
        lbl_left_title.setStyleSheet('color: #60cdff; font-size: 13px; font-weight: 700;')
        hdr_layout.addWidget(lbl_left_title, 1)

        right_title_str = "<b>COMPRESSED RESULT</b>" if not self.result.is_sample else "<b>QUALITY SAMPLE</b>"
        if self.lang == 'pl':
            right_title_str = "<b>SKOMPRESOWANY WYNIK</b>" if not self.result.is_sample else "<b>PRÓBKA JAKOŚCI</b>"
        lbl_right_title = QLabel(right_title_str)
        lbl_right_title.setStyleSheet('color: #34d399; font-size: 13px; font-weight: 700;')
        hdr_layout.addWidget(lbl_right_title, 1)
        layout.addLayout(hdr_layout)

        # 2. Side-by-side Video
        video_box = QHBoxLayout()
        self.videoSource.setStyleSheet('background-color: black; border-radius: 6px; border: 1px solid rgba(255, 255, 255, 0.1);')
        self.videoTarget.setStyleSheet('background-color: black; border-radius: 6px; border: 1px solid rgba(255, 255, 255, 0.1);')
        self.videoSource.setMinimumHeight(350)
        self.videoTarget.setMinimumHeight(350)

        self.videoSource.clicked_signal.connect(self.play_pause)
        self.videoTarget.clicked_signal.connect(self.play_pause)

        video_box.addWidget(self.videoSource, 1)
        video_box.addWidget(self.videoTarget, 1)
        layout.addLayout(video_box, 1)

        # 3. Controls Card
        ctrl_card = QFrame()
        ctrl_card.setStyleSheet('background-color: #1c1c1f; border-radius: 8px; border: 1px solid rgba(255, 255, 255, 0.08);')
        ctrl_layout = QVBoxLayout(ctrl_card)
        ctrl_layout.setContentsMargins(10, 6, 10, 6)
        ctrl_layout.setSpacing(6)

        self.timelineSlider = QSlider(Qt.Orientation.Horizontal)
        self.timelineSlider.setRange(0, self.total_duration_ms)
        self.timelineSlider.sliderMoved.connect(self.seek_position)
        ctrl_layout.addWidget(self.timelineSlider)

        btn_row = QHBoxLayout()
        self.playBtn = QPushButton()
        self.playBtn.setIcon(ui.icons.get_icon('play', '#ffffff', 18))
        self.playBtn.setFixedSize(40, 32)
        self.playBtn.setStyleSheet('background-color: #0078d4;')
        self.playBtn.clicked.connect(self.play_pause)
        btn_row.addWidget(self.playBtn)

        # Step buttons
        btn_back_1s = QPushButton('-1s')
        btn_back_1s.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_back_1s.clicked.connect(lambda: self.step_ms(-1000))
        btn_row.addWidget(btn_back_1s)

        btn_back_frame = QPushButton('-1 fr')
        btn_back_frame.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_back_frame.clicked.connect(lambda: self.step_ms(-16))
        btn_row.addWidget(btn_back_frame)

        btn_fwd_frame = QPushButton('+1 fr')
        btn_fwd_frame.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_fwd_frame.clicked.connect(lambda: self.step_ms(16))
        btn_row.addWidget(btn_fwd_frame)

        btn_fwd_1s = QPushButton('+1s')
        btn_fwd_1s.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_fwd_1s.clicked.connect(lambda: self.step_ms(1000))
        btn_row.addWidget(btn_fwd_1s)

        self.timeLabel = QLabel(f"00:00.00 / {self.format_time(self.total_duration_ms)}")
        self.timeLabel.setStyleSheet('font-weight: bold; margin-left: 10px;')
        btn_row.addWidget(self.timeLabel)

        self.chkLoop = QCheckBox("Loop" if self.lang == 'en' else "Zapętlij")
        self.chkLoop.setChecked(True)
        btn_row.addWidget(self.chkLoop)

        btn_row.addStretch()

        # Volume
        btn_vol = QPushButton()
        btn_vol.setIcon(ui.icons.get_icon('volume', '#a1a1aa', 16))
        btn_vol.setStyleSheet("background: transparent; border: none; padding: 0;")
        btn_vol.setFixedSize(22, 22)
        btn_row.addWidget(btn_vol)

        vol_slider = QSlider(Qt.Orientation.Horizontal)
        vol_slider.setRange(0, 100)
        vol_slider.setValue(70)
        vol_slider.setFixedWidth(70)
        vol_slider.valueChanged.connect(lambda v: self.audioTarget.setVolume(v / 100.0))
        btn_row.addWidget(vol_slider)

        ctrl_layout.addLayout(btn_row)
        layout.addWidget(ctrl_card)

        # 4. Info Card
        info_card = QFrame()
        info_card.setStyleSheet('background-color: #1c1c1f; border-radius: 8px; border: 1px solid rgba(255, 255, 255, 0.08);')
        info_layout = QHBoxLayout(info_card)
        info_layout.setContentsMargins(14, 8, 14, 8)

        actual_mb = self.result.actual_size_bytes / 1000000.0
        plan = self.result.plan or {}
        q = plan.get('quality')

        details_text = f"<b>Result:</b> {actual_mb:.2f} MB / limit {self.result.target_mb:.0f} MB"
        if self.lang == 'pl':
            details_text = f"<b>Wynik:</b> {actual_mb:.2f} MB / limit {self.result.target_mb:.0f} MB"
        if plan:
            details_text += f" | {plan.get('out_width', 1920)}×{plan.get('out_height', 1080)} · {plan.get('out_fps', 60.0):.0f} FPS · {self.result.preset_mode}"
            if plan.get('video_kbps'):
                details_text += f" · ~{plan['video_kbps']} kbps"

        lbl_details = QLabel(details_text)
        info_layout.addWidget(lbl_details, 1)

        if q:
            lbl_q = QLabel(f"● {q.label}")
            lbl_q.setStyleSheet(f"font-weight: bold; color: {q.color}; margin-right: 12px;")
            info_layout.addWidget(lbl_q)

        # Actions
        btn_open_file = QPushButton("Open File" if self.lang == 'en' else "Otwórz plik")
        btn_open_file.clicked.connect(self.open_output_file)
        info_layout.addWidget(btn_open_file)

        btn_open_folder = QPushButton("Open Folder" if self.lang == 'en' else "Otwórz folder")
        btn_open_folder.setIcon(ui.icons.get_icon('folder', '#f4f4f6', 14))
        btn_open_folder.clicked.connect(self.open_output_folder)
        info_layout.addWidget(btn_open_folder)

        btn_close = QPushButton("Close" if self.lang == 'en' else "Zamknij")
        btn_close.setStyleSheet('background-color: #0078d4; padding: 7px 16px;')
        btn_close.clicked.connect(self.accept)
        info_layout.addWidget(btn_close)

        layout.addWidget(info_card)

        self.playerTarget.positionChanged.connect(self.on_target_position_changed)
        self.playerTarget.durationChanged.connect(self.on_target_duration_changed)

    def load_media(self):
        if os.path.exists(self.result.input_path):
            self.playerSource.setSource(QUrl.fromLocalFile(self.result.input_path))
        if os.path.exists(self.result.output_path):
            self.playerTarget.setSource(QUrl.fromLocalFile(self.result.output_path))

        self.seek_position(0)
        self.play()

    def play_pause(self):
        if self.playerTarget.playbackState() == QMediaPlayer.PlaybackState.PlayingState:
            self.pause()
        else:
            self.play()

    def play(self):
        self.playerSource.play()
        self.playerTarget.play()
        self.syncTimer.start()
        self.playBtn.setIcon(ui.icons.get_icon('pause', '#ffffff', 18))

    def pause(self):
        self.syncTimer.stop()
        self.playerSource.pause()
        self.playerTarget.pause()
        self.playBtn.setIcon(ui.icons.get_icon('play', '#ffffff', 18))

    def step_ms(self, delta_ms: int):
        self.pause()
        cur = self.playerTarget.position()
        target = max(0, min(cur + delta_ms, self.total_duration_ms))
        self.seek_position(target)

    def seek_position(self, pos_ms: int):
        self.playerTarget.setPosition(pos_ms)
        self.playerSource.setPosition(self.start_offset_ms + pos_ms)
        self.timelineSlider.setValue(pos_ms)
        self.timeLabel.setText(f"{self.format_time(pos_ms)} / {self.format_time(self.total_duration_ms)}")

    def on_target_position_changed(self, pos_ms: int):
        self.timelineSlider.setValue(pos_ms)
        self.timeLabel.setText(f"{self.format_time(pos_ms)} / {self.format_time(self.total_duration_ms)}")

        if self.chkLoop.isChecked() and pos_ms >= self.total_duration_ms - 50:
            self.seek_position(0)

    def on_target_duration_changed(self, dur_ms: int):
        if dur_ms > 0:
            self.total_duration_ms = dur_ms
            self.timelineSlider.setRange(0, dur_ms)
            self.timeLabel.setText(f"00:00.00 / {self.format_time(dur_ms)}")

    def sync_drift(self):
        target_pos = self.playerTarget.position()
        source_pos = self.playerSource.position() - self.start_offset_ms
        drift = abs(target_pos - source_pos)
        if drift > 80:
            self.playerSource.setPosition(self.start_offset_ms + target_pos)

    def format_time(self, ms: int) -> str:
        s = ms // 1000
        m, s = divmod(s, 60)
        hundredths = (ms % 1000) // 10
        return f"{m:02d}:{s:02d}.{hundredths:02d}"

    def open_output_file(self):
        if os.path.exists(self.result.output_path):
            os.startfile(self.result.output_path)

    def open_output_folder(self):
        out_dir = os.path.dirname(self.result.output_path)
        if os.path.exists(out_dir):
            os.startfile(out_dir)

    def closeEvent(self, event):
        self.syncTimer.stop()
        self.playerSource.stop()
        self.playerTarget.stop()
        self.closed_signal.emit(self.result)
        super().closeEvent(event)
