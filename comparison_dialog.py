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

class ClickableVideoWidget(QVideoWidget):
    clicked_signal = pyqtSignal()

    def mouseReleaseEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked_signal.emit()
        super().mouseReleaseEvent(event)

class QualityComparisonDialog(QDialog):
    closed_signal = pyqtSignal(object) # emits ExportResult

    def __init__(self, parent, result: encoder.ExportResult, lang: str = 'pl'):
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
        self.audioSource.setMuted(True) # Dźwięk tylko z prawego (targetu)

        self.playerTarget.setVideoOutput(self.videoTarget)
        self.playerTarget.setAudioOutput(self.audioTarget)
        self.audioTarget.setVolume(0.7)

        # Timer do synchronizacji driftu
        self.syncTimer = QTimer(self)
        self.syncTimer.setInterval(250)
        self.syncTimer.timeout.connect(self.sync_drift)

        self.init_ui()
        self.load_media()

    def init_ui(self):
        title_text = "Porównanie jakości (A/B)" if self.lang == 'pl' else "Quality Comparison (A/B)"
        if self.result.is_sample:
            title_text += " — " + ("Próbka jakości" if self.lang == 'pl' else "Quality Sample")
        else:
            title_text += " — " + ("Wynik eksportu" if self.lang == 'pl' else "Export Result")
        self.setWindowTitle(title_text)
        self.setMinimumSize(1080, 680)

        self.setStyleSheet('''
            QDialog { background-color: #0b0f19; }
            QLabel { color: #f8fafc; font-family: 'Segoe UI', system-ui; font-size: 13px; }
            QPushButton { 
                border-radius: 6px; padding: 7px 14px; color: white; 
                font-weight: bold; background-color: #1e293b; border: 1px solid #334155; 
            }
            QPushButton:hover { background-color: #3b82f6; border: 1px solid #60a5fa; }
            QSlider::groove:horizontal {
                border: 1px solid #334155; height: 10px; background: #1e293b;
                margin: 2px 0; border-radius: 5px;
            }
            QSlider::handle:horizontal {
                background: #3b82f6; border: 2px solid #60a5fa;
                width: 18px; height: 18px; margin: -5px 0; border-radius: 9px;
            }
            QCheckBox { color: #cbd5e1; font-weight: bold; }
            QCheckBox::indicator { width: 16px; height: 16px; border-radius: 4px; border: 1px solid #475569; background: #1e293b; }
            QCheckBox::indicator:checked { background: #3b82f6; border: 1px solid #60a5fa; }
        ''')

        layout = QVBoxLayout(self)
        layout.setContentsMargins(14, 12, 14, 12)
        layout.setSpacing(8)

        # 1. Nagłówki A / B
        hdr_layout = QHBoxLayout()

        # Lewy nagłówek (Oryginał)
        lbl_left_title = QLabel("📹 <b>ORYGINAŁ</b> (Wycinek źródłowy)" if self.lang == 'pl' else "📹 <b>ORIGINAL</b> (Source Clip)")
        lbl_left_title.setStyleSheet('color: #60a5fa; font-size: 13px;')
        hdr_layout.addWidget(lbl_left_title, 1)

        # Prawy nagłówek (Wynik)
        right_title_str = "✨ <b>SKOMPRESOWANY WYNIK</b>" if not self.result.is_sample else "🔬 <b>PRÓBKA JAKOŚCI</b>"
        if self.lang != 'pl':
            right_title_str = "✨ <b>COMPRESSED RESULT</b>" if not self.result.is_sample else "🔬 <b>QUALITY SAMPLE</b>"
        lbl_right_title = QLabel(right_title_str)
        lbl_right_title.setStyleSheet('color: #34d399; font-size: 13px;')
        hdr_layout.addWidget(lbl_right_title, 1)
        layout.addLayout(hdr_layout)

        # 2. Widok wideo side-by-side
        video_box = QHBoxLayout()
        self.videoSource.setStyleSheet('background-color: black; border-radius: 6px; border: 2px solid #1e293b;')
        self.videoTarget.setStyleSheet('background-color: black; border-radius: 6px; border: 2px solid #1e293b;')
        self.videoSource.setMinimumHeight(350)
        self.videoTarget.setMinimumHeight(350)

        self.videoSource.clicked_signal.connect(self.play_pause)
        self.videoTarget.clicked_signal.connect(self.play_pause)

        video_box.addWidget(self.videoSource, 1)
        video_box.addWidget(self.videoTarget, 1)
        layout.addLayout(video_box, 1)

        # 3. Kontrolki i wspólny pasek czasu
        ctrl_card = QFrame()
        ctrl_card.setStyleSheet('background-color: #131d2e; border-radius: 8px; border: 1px solid #1e293b;')
        ctrl_layout = QVBoxLayout(ctrl_card)
        ctrl_layout.setContentsMargins(10, 6, 10, 6)
        ctrl_layout.setSpacing(6)

        self.timelineSlider = QSlider(Qt.Orientation.Horizontal)
        self.timelineSlider.setRange(0, self.total_duration_ms)
        self.timelineSlider.sliderMoved.connect(self.seek_position)
        ctrl_layout.addWidget(self.timelineSlider)

        btn_row = QHBoxLayout()
        self.playBtn = QPushButton()
        self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPlay))
        self.playBtn.setFixedWidth(46)
        self.playBtn.setStyleSheet('background-color: #3b82f6;')
        self.playBtn.clicked.connect(self.play_pause)
        btn_row.addWidget(self.playBtn)

        # Klatka po klatce
        btn_back_1s = QPushButton('◀ -1s')
        btn_back_1s.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_back_1s.clicked.connect(lambda: self.step_ms(-1000))
        btn_row.addWidget(btn_back_1s)

        btn_back_frame = QPushButton('◀ -1 fr')
        btn_back_frame.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_back_frame.clicked.connect(lambda: self.step_ms(-16))
        btn_row.addWidget(btn_back_frame)

        btn_fwd_frame = QPushButton('+1 fr ▶')
        btn_fwd_frame.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_fwd_frame.clicked.connect(lambda: self.step_ms(16))
        btn_row.addWidget(btn_fwd_frame)

        btn_fwd_1s = QPushButton('+1s ▶')
        btn_fwd_1s.setStyleSheet('font-size: 11px; padding: 5px 8px;')
        btn_fwd_1s.clicked.connect(lambda: self.step_ms(1000))
        btn_row.addWidget(btn_fwd_1s)

        self.timeLabel = QLabel(f"00:00.00 / {self.format_time(self.total_duration_ms)}")
        self.timeLabel.setStyleSheet('font-weight: bold; margin-left: 10px;')
        btn_row.addWidget(self.timeLabel)

        self.chkLoop = QCheckBox("🔁 Zapętlij" if self.lang == 'pl' else "🔁 Loop")
        self.chkLoop.setChecked(True)
        btn_row.addWidget(self.chkLoop)

        btn_row.addStretch()

        # Głośność
        lbl_vol = QLabel("🔊")
        btn_row.addWidget(lbl_vol)
        vol_slider = QSlider(Qt.Orientation.Horizontal)
        vol_slider.setRange(0, 100)
        vol_slider.setValue(70)
        vol_slider.setFixedWidth(70)
        vol_slider.valueChanged.connect(lambda v: self.audioTarget.setVolume(v / 100.0))
        btn_row.addWidget(vol_slider)

        ctrl_layout.addLayout(btn_row)
        layout.addWidget(ctrl_card)

        # 4. Karta podsumowania jakości i akcji
        info_card = QFrame()
        info_card.setStyleSheet('background-color: #1e293b; border-radius: 8px; border: 1px solid #334155;')
        info_layout = QHBoxLayout(info_card)
        info_layout.setContentsMargins(14, 8, 14, 8)

        # Dane techniczne wyniku
        actual_mb = self.result.actual_size_bytes / 1000000.0
        plan = self.result.plan or {}
        q = plan.get('quality')

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

        # Przyciski
        btn_open_file = QPushButton("🎬 Otwórz plik" if self.lang == 'pl' else "🎬 Open File")
        btn_open_file.clicked.connect(self.open_output_file)
        info_layout.addWidget(btn_open_file)

        btn_open_folder = QPushButton("📁 Otwórz folder" if self.lang == 'pl' else "📁 Open Folder")
        btn_open_folder.clicked.connect(self.open_output_folder)
        info_layout.addWidget(btn_open_folder)

        btn_close = QPushButton("✕ Zamknij" if self.lang == 'pl' else "✕ Close")
        btn_close.setStyleSheet('background-color: #2563eb; padding: 7px 16px;')
        btn_close.clicked.connect(self.accept)
        info_layout.addWidget(btn_close)

        layout.addWidget(info_card)

        # Sygnały playera targetu
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
        self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPause))

    def pause(self):
        self.syncTimer.stop()
        self.playerSource.pause()
        self.playerTarget.pause()
        self.playBtn.setIcon(self.style().standardIcon(QStyle.StandardPixmap.SP_MediaPlay))

    def seek_position(self, pos_target_ms: int):
        self.playerTarget.setPosition(pos_target_ms)
        self.playerSource.setPosition(self.start_offset_ms + pos_target_ms)

    def step_ms(self, delta_ms: int):
        new_pos = max(0, min(self.playerTarget.position() + delta_ms, self.total_duration_ms))
        self.seek_position(new_pos)

    def sync_drift(self):
        pos_t = self.playerTarget.position()
        pos_s = self.playerSource.position()
        expected_s = self.start_offset_ms + pos_t
        drift = abs(pos_s - expected_s)

        if drift > 80: # Drift powyżej 80ms korygowany
            self.playerSource.setPosition(expected_s)

        # Sprawdzenie końca klipu
        if pos_t >= self.total_duration_ms - 50:
            if self.chkLoop.isChecked():
                self.seek_position(0)
                self.play()
            else:
                self.pause()

    def on_target_position_changed(self, pos: int):
        if not self.timelineSlider.isSliderDown():
            self.timelineSlider.setValue(pos)
        self.timeLabel.setText(f"{self.format_time(pos)} / {self.format_time(self.total_duration_ms)}")

    def on_target_duration_changed(self, dur: int):
        if dur > 0:
            self.total_duration_ms = dur
            self.timelineSlider.setRange(0, dur)

    def format_time(self, ms: int) -> str:
        s = ms // 1000
        ms_rem = int((ms % 1000) / 10)
        m, s = divmod(s, 60)
        return f"{m:02d}:{s:02d}.{ms_rem:02d}"

    def keyPressEvent(self, event):
        key = event.key()
        modifiers = event.modifiers()
        if key == Qt.Key.Key_Space:
            self.play_pause()
            event.accept()
        elif key == Qt.Key.Key_Left:
            delta = -16 if (modifiers & Qt.KeyboardModifier.ShiftModifier) else -1000
            self.step_ms(delta)
            event.accept()
        elif key == Qt.Key.Key_Right:
            delta = 16 if (modifiers & Qt.KeyboardModifier.ShiftModifier) else 1000
            self.step_ms(delta)
            event.accept()
        elif key == Qt.Key.Key_Escape:
            self.accept()
            event.accept()
        else:
            super().keyPressEvent(event)

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
        self.playerSource.setSource(QUrl())
        self.playerTarget.setSource(QUrl())
        self.closed_signal.emit(self.result)
        event.accept()
