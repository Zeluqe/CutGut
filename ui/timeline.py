import sys
from typing import Optional
from PyQt6.QtWidgets import QWidget
from PyQt6.QtCore import Qt, QRectF, QPointF, pyqtSignal, QPoint
from PyQt6.QtGui import QPainter, QColor, QPen, QBrush, QMouseEvent, QFont, QPaintEvent

class FluentTimelineWidget(QWidget):
    position_changed = pyqtSignal(int)      # emits ms
    range_changed = pyqtSignal(int, int)    # emits (start_ms, end_ms)
    marker_set = pyqtSignal(str, int)       # emits ('in'/'out', ms)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFixedHeight(36)
        self.setMouseTracking(True)
        self.setCursor(Qt.CursorShape.PointingHandCursor)

        self.duration_ms = 0
        self.position_ms = 0
        self.start_ms = 0
        self.end_ms = 0

        self.hover_ms: Optional[int] = None
        self.is_dragging_playhead = False
        self.is_dragging_in = False
        self.is_dragging_out = False

    def set_duration(self, dur_ms: int):
        self.duration_ms = max(dur_ms, 0)
        self.start_ms = 0
        self.end_ms = self.duration_ms
        self.position_ms = 0
        self.update()

    def set_position(self, pos_ms: int):
        self.position_ms = max(0, min(pos_ms, self.duration_ms))
        self.update()

    def set_range(self, start_ms: int, end_ms: int):
        self.start_ms = max(0, min(start_ms, self.duration_ms))
        self.end_ms = max(self.start_ms, min(end_ms, self.duration_ms))
        self.update()

    def set_in_point(self, pos_ms: int):
        self.start_ms = max(0, min(pos_ms, self.duration_ms))
        if self.end_ms <= self.start_ms:
            self.end_ms = min(self.start_ms + 15000, self.duration_ms)
        self.range_changed.emit(self.start_ms, self.end_ms)
        self.marker_set.emit('in', self.start_ms)
        self.update()

    def set_out_point(self, pos_ms: int):
        self.end_ms = max(0, min(pos_ms, self.duration_ms))
        if self.end_ms <= self.start_ms:
            self.start_ms = max(self.end_ms - 15000, 0)
        self.range_changed.emit(self.start_ms, self.end_ms)
        self.marker_set.emit('out', self.end_ms)
        self.update()

    def format_time(self, ms: int) -> str:
        s = ms // 1000
        m, s = divmod(s, 60)
        hundredths = (ms % 1000) // 10
        return f"{m:02d}:{s:02d}.{hundredths:02d}"

    def ms_to_x(self, ms: int) -> float:
        if self.duration_ms <= 0:
            return 8.0
        track_w = self.width() - 16.0
        pct = max(0.0, min(ms / float(self.duration_ms), 1.0))
        return 8.0 + pct * track_w

    def x_to_ms(self, x: float) -> int:
        if self.duration_ms <= 0:
            return 0
        track_w = self.width() - 16.0
        pct = max(0.0, min((x - 8.0) / float(track_w), 1.0))
        return int(pct * self.duration_ms)

    def paintEvent(self, event: QPaintEvent):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        track_h = 10.0
        track_y = (self.height() - track_h) / 2.0
        track_w = max(self.width() - 16.0, 1.0)
        track_rect = QRectF(8.0, track_y, track_w, track_h)

        # 1. Tło ścieżki czasu (Graphite track)
        painter.setPen(QPen(QColor(255, 255, 255, 18), 1))
        painter.setBrush(QBrush(QColor(36, 36, 42)))
        painter.drawRoundedRect(track_rect, 5.0, 5.0)

        if self.duration_ms > 0:
            # 2. Zaznaczony fragment IN -> OUT (Półprzezroczysty Fluent Blue)
            x_in = self.ms_to_x(self.start_ms)
            x_out = self.ms_to_x(self.end_ms)
            sel_rect = QRectF(x_in, track_y, max(x_out - x_in, 2.0), track_h)
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(QBrush(QColor(0, 120, 212, 120)))
            painter.drawRoundedRect(sel_rect, 4.0, 4.0)

            # 3. Marker IN (Emerald green flag)
            painter.setPen(QPen(QColor('#10b981'), 2.5))
            painter.drawLine(QPointF(x_in, track_y - 4.0), QPointF(x_in, track_y + track_h + 4.0))
            # Główka markera IN
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(QBrush(QColor('#10b981')))
            in_tag = [QPointF(x_in, track_y - 6.0), QPointF(x_in + 6.0, track_y - 6.0), QPointF(x_in, track_y)]
            painter.drawPolygon(in_tag)

            # 4. Marker OUT (Ruby red flag)
            painter.setPen(QPen(QColor('#ef4444'), 2.5))
            painter.drawLine(QPointF(x_out, track_y - 4.0), QPointF(x_out, track_y + track_h + 4.0))
            # Główka markera OUT
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(QBrush(QColor('#ef4444')))
            out_tag = [QPointF(x_out, track_y - 6.0), QPointF(x_out - 6.0, track_y - 6.0), QPointF(x_out, track_y)]
            painter.drawPolygon(out_tag)

            # 5. Linia i uchwyt Playheada (Cyan / Windows Blue)
            x_play = self.ms_to_x(self.position_ms)
            painter.setPen(QPen(QColor('#38bdf8'), 2.0))
            painter.drawLine(QPointF(x_play, 4.0), QPointF(x_play, self.height() - 4.0))

            # Uchwyt Playheada (Pill cursor)
            painter.setPen(QPen(QColor('#0f172a'), 2.0))
            painter.setBrush(QBrush(QColor('#60cdff')))
            painter.drawEllipse(QPointF(x_play, self.height() / 2.0), 5.5, 5.5)

            # 6. Hover timestamp tooltip
            if self.hover_ms is not None and not self.is_dragging_playhead:
                x_h = self.ms_to_x(self.hover_ms)
                # Dyskretna linia podglądu
                painter.setPen(QPen(QColor(255, 255, 255, 60), 1, Qt.PenStyle.DashLine))
                painter.drawLine(QPointF(x_h, track_y - 2), QPointF(x_h, track_y + track_h + 2))

                # Plakietka czasu tooltipa
                tt_text = self.format_time(self.hover_ms)
                tt_w = 68.0
                tt_h = 18.0
                tt_x = max(2.0, min(x_h - tt_w / 2.0, self.width() - tt_w - 2.0))
                tt_y = 0.0

                painter.setPen(Qt.PenStyle.NoPen)
                painter.setBrush(QBrush(QColor(24, 24, 28, 230)))
                painter.drawRoundedRect(QRectF(tt_x, tt_y, tt_w, tt_h), 4.0, 4.0)

                painter.setPen(QPen(QColor('#38bdf8'), 1))
                painter.setBrush(Qt.BrushStyle.NoBrush)
                painter.drawRoundedRect(QRectF(tt_x, tt_y, tt_w, tt_h), 4.0, 4.0)

                painter.setPen(QColor('#f4f4f6'))
                font = QFont("Segoe UI", 8, QFont.Weight.Bold)
                painter.setFont(font)
                painter.drawText(QRectF(tt_x, tt_y, tt_w, tt_h), Qt.AlignmentFlag.AlignCenter, tt_text)

    def mousePressEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton and self.duration_ms > 0:
            click_x = event.position().x()
            x_in = self.ms_to_x(self.start_ms)
            x_out = self.ms_to_x(self.end_ms)

            # Sprawdzenie czy kliknięto marker IN (tolerancja 6px)
            if abs(click_x - x_in) <= 6.0:
                self.is_dragging_in = True
                event.accept()
                return
            elif abs(click_x - x_out) <= 6.0:
                self.is_dragging_out = True
                event.accept()
                return
            else:
                self.is_dragging_playhead = True
                target_ms = self.x_to_ms(click_x)
                self.position_ms = target_ms
                self.position_changed.emit(target_ms)
                self.update()
                event.accept()
                return
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event: QMouseEvent):
        if self.duration_ms > 0:
            cur_x = event.position().x()
            cur_ms = self.x_to_ms(cur_x)
            self.hover_ms = cur_ms

            if self.is_dragging_playhead:
                self.position_ms = cur_ms
                self.position_changed.emit(cur_ms)
            elif self.is_dragging_in:
                self.start_ms = min(cur_ms, max(self.end_ms - 500, 0))
                self.range_changed.emit(self.start_ms, self.end_ms)
            elif self.is_dragging_out:
                self.end_ms = max(cur_ms, min(self.start_ms + 500, self.duration_ms))
                self.range_changed.emit(self.start_ms, self.end_ms)

            self.update()
            event.accept()
            return
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            self.is_dragging_playhead = False
            self.is_dragging_in = False
            self.is_dragging_out = False
            self.update()
            event.accept()
            return
        super().mouseReleaseEvent(event)

    def leaveEvent(self, event):
        self.hover_ms = None
        self.update()
        super().leaveEvent(event)
