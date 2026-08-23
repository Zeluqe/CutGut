import sys
import os
from typing import Optional
from PyQt6.QtWidgets import QWidget
from PyQt6.QtMultimedia import QVideoSink, QVideoFrame
from PyQt6.QtCore import Qt, QRect, QRectF, QPoint, QPointF, pyqtSignal
from PyQt6.QtGui import QPainter, QColor, QPen, QBrush, QMouseEvent, QPaintEvent, QCursor, QFont, QImage

import encoder

class VideoCanvasWidget(QWidget):
    clicked_signal = pyqtSignal()
    crop_changed_signal = pyqtSignal(object)  # emits CropBox

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent, True)
        self.setMouseTracking(True)
        self.setCursor(Qt.CursorShape.PointingHandCursor)

        self.sink = QVideoSink(self)
        self.sink.videoFrameChanged.connect(self.on_video_frame_changed)

        self.current_frame: Optional[QImage] = None
        self.video_w = 1920
        self.video_h = 1080
        self.ratio_type = "original"  # "original", "16:9", "9:16", "1:1"
        self.crop_x_pct = 0.0
        self.crop_w_pct = 1.0
        self.crop_y_pct = 0.0
        self.crop_h_pct = 1.0

        self.is_dragging = False
        self.drag_start_pos = QPoint()
        self.drag_start_x_pct = 0.0
        self.drag_start_y_pct = 0.0

    def on_video_frame_changed(self, frame: QVideoFrame):
        if frame.isValid():
            img = frame.toImage()
            if not img.isNull():
                self.current_frame = img
                if self.video_w != frame.width() or self.video_h != frame.height():
                    self.video_w = max(frame.width(), 1)
                    self.video_h = max(frame.height(), 1)
                    self.recalculate_crop_pct()
                self.update()

    def clear_video(self):
        self.current_frame = None
        self.ratio_type = "original"
        self.crop_x_pct = 0.0
        self.crop_y_pct = 0.0
        self.crop_w_pct = 1.0
        self.crop_h_pct = 1.0
        self.update()

    def set_video_dimensions(self, w: int, h: int):
        self.video_w = max(w, 1)
        self.video_h = max(h, 1)
        self.recalculate_crop_pct()
        self.update()

    def set_ratio_type(self, ratio: str, alignment: str = "center"):
        self.ratio_type = ratio
        if ratio == "original":
            self.crop_x_pct = 0.0
            self.crop_y_pct = 0.0
            self.crop_w_pct = 1.0
            self.crop_h_pct = 1.0
            self.setCursor(Qt.CursorShape.PointingHandCursor)
        else:
            cb = encoder.calculate_default_crop(self.video_w, self.video_h, ratio, alignment)
            self.crop_w_pct = max(min(cb.w / float(self.video_w), 1.0), 0.05)
            self.crop_h_pct = max(min(cb.h / float(self.video_h), 1.0), 0.05)
            self.crop_x_pct = max(min(cb.x / float(self.video_w), 1.0 - self.crop_w_pct), 0.0)
            self.crop_y_pct = max(min(cb.y / float(self.video_h), 1.0 - self.crop_h_pct), 0.0)

        self.emit_crop_box()
        self.update()

    def set_alignment(self, alignment: str):
        if self.ratio_type == "original":
            return
        if alignment == "left":
            self.crop_x_pct = 0.0
        elif alignment == "right":
            self.crop_x_pct = max(1.0 - self.crop_w_pct, 0.0)
        else:  # center
            self.crop_x_pct = max((1.0 - self.crop_w_pct) / 2.0, 0.0)

        self.emit_crop_box()
        self.update()

    def recalculate_crop_pct(self):
        if self.ratio_type != "original":
            cb = encoder.calculate_default_crop(self.video_w, self.video_h, self.ratio_type, "center")
            self.crop_w_pct = max(min(cb.w / float(self.video_w), 1.0), 0.05)
            self.crop_h_pct = max(min(cb.h / float(self.video_h), 1.0), 0.05)
            self.crop_x_pct = max(min((1.0 - self.crop_w_pct) / 2.0, 1.0 - self.crop_w_pct), 0.0)
            self.crop_y_pct = max(min((1.0 - self.crop_h_pct) / 2.0, 1.0 - self.crop_h_pct), 0.0)

    def get_crop_box(self) -> encoder.CropBox:
        if self.ratio_type == "original":
            return encoder.CropBox(0, 0, self.video_w, self.video_h, "original")

        x = int(self.crop_x_pct * self.video_w)
        y = int(self.crop_y_pct * self.video_h)
        w = int(self.crop_w_pct * self.video_w)
        h = int(self.crop_h_pct * self.video_h)

        w = w if w % 2 == 0 else w - 1
        h = h if h % 2 == 0 else h - 1
        w = max(2, w)
        h = max(2, h)
        x = max(0, min(x, self.video_w - w))
        y = max(0, min(y, self.video_h - h))
        return encoder.CropBox(x=x, y=y, w=w, h=h, ratio_type=self.ratio_type)

    def emit_crop_box(self):
        self.crop_changed_signal.emit(self.get_crop_box())

    def get_rendered_video_rect(self) -> QRectF:
        widget_w = self.width()
        widget_h = self.height()
        if widget_w <= 0 or widget_h <= 0 or self.video_w <= 0 or self.video_h <= 0:
            return QRectF(0, 0, max(widget_w, 1), max(widget_h, 1))

        video_aspect = self.video_w / float(self.video_h)
        widget_aspect = widget_w / float(widget_h)

        if widget_aspect > video_aspect:
            h = widget_h
            w = h * video_aspect
            x = (widget_w - w) / 2.0
            y = 0
        else:
            w = widget_w
            h = w / video_aspect
            x = 0
            y = (widget_h - h) / 2.0

        return QRectF(x, y, w, h)

    def get_crop_screen_rect(self) -> QRectF:
        v_rect = self.get_rendered_video_rect()
        crop_screen_x = v_rect.x() + self.crop_x_pct * v_rect.width()
        crop_screen_y = v_rect.y() + self.crop_y_pct * v_rect.height()
        crop_screen_w = self.crop_w_pct * v_rect.width()
        crop_screen_h = self.crop_h_pct * v_rect.height()
        return QRectF(crop_screen_x, crop_screen_y, crop_screen_w, crop_screen_h)

    def paintEvent(self, event: QPaintEvent):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        # 1. Czarny obszar bazowy (letterbox / pillarbox)
        painter.fillRect(self.rect(), QColor(0, 0, 0))

        v_rect = self.get_rendered_video_rect()

        # 2. Rysowanie klatki wideo
        if self.current_frame and not self.current_frame.isNull():
            painter.drawImage(v_rect, self.current_frame)
        else:
            painter.setPen(QColor(100, 116, 139))
            font = QFont("Segoe UI", 12)
            painter.setFont(font)
            painter.drawText(self.rect(), Qt.AlignmentFlag.AlignCenter, "Wybierz film lub przeciągnij plik tutaj...")

        # 3. Jeśli kadr to nie "original", narysuj ciemnoszarą winietę i ramkę 9:16 / 1:1
        if self.ratio_type != "original":
            crop_rect = self.get_crop_screen_rect()

            # Półprzezroczysta ciemna maska (zaciemnienie obszaru poza kadrem)
            mask_color = QColor(10, 15, 29, 210)  # głęboki grafitowy półprzezroczysty
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(QBrush(mask_color))

            # Lewo
            if crop_rect.left() > 0:
                painter.drawRect(QRectF(0, 0, crop_rect.left(), self.height()))
            # Prawo
            if crop_rect.right() < self.width():
                painter.drawRect(QRectF(crop_rect.right(), 0, self.width() - crop_rect.right(), self.height()))
            # Góra
            if crop_rect.top() > 0:
                painter.drawRect(QRectF(crop_rect.left(), 0, crop_rect.width(), crop_rect.top()))
            # Dół
            if crop_rect.bottom() < self.height():
                painter.drawRect(QRectF(crop_rect.left(), crop_rect.bottom(), crop_rect.width(), self.height() - crop_rect.bottom()))

            # Jasna błękitna ramka kadru (#38bdf8)
            border_pen = QPen(QColor('#38bdf8'), 2.5)
            painter.setPen(border_pen)
            painter.setBrush(Qt.BrushStyle.NoBrush)
            painter.drawRect(crop_rect)

            # Siatka trójpodziału (Rule of Thirds)
            grid_pen = QPen(QColor(255, 255, 255, 65), 1, Qt.PenStyle.DashLine)
            painter.setPen(grid_pen)
            third_w = crop_rect.width() / 3.0
            third_h = crop_rect.height() / 3.0

            painter.drawLine(QPointF(crop_rect.left() + third_w, crop_rect.top()), QPointF(crop_rect.left() + third_w, crop_rect.bottom()))
            painter.drawLine(QPointF(crop_rect.left() + 2 * third_w, crop_rect.top()), QPointF(crop_rect.left() + 2 * third_w, crop_rect.bottom()))
            painter.drawLine(QPointF(crop_rect.left(), crop_rect.top() + third_h), QPointF(crop_rect.right(), crop_rect.top() + third_h))
            painter.drawLine(QPointF(crop_rect.left(), crop_rect.top() + 2 * third_h), QPointF(crop_rect.right(), crop_rect.top() + 2 * third_h))

            # Szmaragdowe narożniki (#34d399)
            handle_pen = QPen(QColor('#34d399'), 4.0, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap)
            painter.setPen(handle_pen)
            hl = min(18.0, crop_rect.width() / 4.0, crop_rect.height() / 4.0)

            painter.drawLine(QPointF(crop_rect.left(), crop_rect.top()), QPointF(crop_rect.left() + hl, crop_rect.top()))
            painter.drawLine(QPointF(crop_rect.left(), crop_rect.top()), QPointF(crop_rect.left(), crop_rect.top() + hl))
            painter.drawLine(QPointF(crop_rect.right(), crop_rect.top()), QPointF(crop_rect.right() - hl, crop_rect.top()))
            painter.drawLine(QPointF(crop_rect.right(), crop_rect.top()), QPointF(crop_rect.right(), crop_rect.top() + hl))
            painter.drawLine(QPointF(crop_rect.left(), crop_rect.bottom()), QPointF(crop_rect.left() + hl, crop_rect.bottom()))
            painter.drawLine(QPointF(crop_rect.left(), crop_rect.bottom()), QPointF(crop_rect.left(), crop_rect.bottom() - hl))
            painter.drawLine(QPointF(crop_rect.right(), crop_rect.bottom()), QPointF(crop_rect.right() - hl, crop_rect.bottom()))
            painter.drawLine(QPointF(crop_rect.right(), crop_rect.bottom()), QPointF(crop_rect.right(), crop_rect.bottom() - hl))

            # Plakietka wymiarów kadru na górze ramki
            cb = self.get_crop_box()
            tag_text = f"📱 {cb.ratio_type} ({cb.w}×{cb.h})" if cb.ratio_type == "9:16" else f"📐 {cb.ratio_type} ({cb.w}×{cb.h})"
            badge_w = 144
            badge_h = 24
            badge_x = crop_rect.center().x() - badge_w / 2.0
            badge_y = max(crop_rect.top() + 6, v_rect.top() + 6)

            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(QBrush(QColor(15, 23, 42, 235)))
            painter.drawRoundedRect(QRectF(badge_x, badge_y, badge_w, badge_h), 5, 5)

            painter.setPen(QPen(QColor('#38bdf8'), 1))
            painter.setBrush(Qt.BrushStyle.NoBrush)
            painter.drawRoundedRect(QRectF(badge_x, badge_y, badge_w, badge_h), 5, 5)

            painter.setPen(QColor('#f8fafc'))
            font = QFont("Segoe UI", 9, QFont.Weight.Bold)
            painter.setFont(font)
            painter.drawText(QRectF(badge_x, badge_y, badge_w, badge_h), Qt.AlignmentFlag.AlignCenter, tag_text)

    def mousePressEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            crop_rect = self.get_crop_screen_rect()

            if self.ratio_type != "original" and crop_rect.contains(event.position()):
                self.is_dragging = True
                self.drag_start_pos = event.position().toPoint()
                self.drag_start_x_pct = self.crop_x_pct
                self.drag_start_y_pct = self.crop_y_pct
                self.setCursor(Qt.CursorShape.ClosedHandCursor)
                event.accept()
                return
            elif self.ratio_type != "original":
                v_rect = self.get_rendered_video_rect()
                if v_rect.width() > 0:
                    click_x_in_v = event.position().x() - v_rect.x()
                    target_x_pct = (click_x_in_v / v_rect.width()) - (self.crop_w_pct / 2.0)
                    self.crop_x_pct = max(0.0, min(target_x_pct, 1.0 - self.crop_w_pct))
                    self.update()
                    self.emit_crop_box()
                    event.accept()
                    return

        super().mousePressEvent(event)

    def mouseMoveEvent(self, event: QMouseEvent):
        if self.ratio_type != "original":
            crop_rect = self.get_crop_screen_rect()
            if self.is_dragging:
                v_rect = self.get_rendered_video_rect()
                if v_rect.width() > 0 and v_rect.height() > 0:
                    delta = event.position().toPoint() - self.drag_start_pos
                    delta_x_pct = delta.x() / float(v_rect.width())
                    delta_y_pct = delta.y() / float(v_rect.height())

                    new_x_pct = self.drag_start_x_pct + delta_x_pct
                    new_x_pct = max(0.0, min(new_x_pct, 1.0 - self.crop_w_pct))

                    new_y_pct = self.drag_start_y_pct + delta_y_pct
                    new_y_pct = max(0.0, min(new_y_pct, 1.0 - self.crop_h_pct))

                    self.crop_x_pct = new_x_pct
                    self.crop_y_pct = new_y_pct
                    self.update()
                    self.emit_crop_box()
                    event.accept()
                    return
            else:
                if crop_rect.contains(event.position()):
                    self.setCursor(Qt.CursorShape.OpenHandCursor)
                else:
                    self.setCursor(Qt.CursorShape.PointingHandCursor)
        else:
            self.setCursor(Qt.CursorShape.PointingHandCursor)

        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            if self.is_dragging:
                self.is_dragging = False
                if self.ratio_type != "original":
                    self.setCursor(Qt.CursorShape.OpenHandCursor)
                else:
                    self.setCursor(Qt.CursorShape.PointingHandCursor)
                event.accept()
                return
            else:
                self.clicked_signal.emit()
        super().mouseReleaseEvent(event)

# Kompatybilny wrapper CropVideoContainer
class CropVideoContainer(VideoCanvasWidget):
    @property
    def overlay(self):
        return self

