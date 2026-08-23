import sys
import os
from typing import Optional
from PyQt6.QtWidgets import QWidget
from PyQt6.QtMultimediaWidgets import QVideoWidget
from PyQt6.QtCore import Qt, QRect, QRectF, QPoint, QPointF, pyqtSignal
from PyQt6.QtGui import QPainter, QColor, QPen, QBrush, QMouseEvent, QPaintEvent, QCursor, QFont

import encoder

class CropOverlay(QWidget):
    crop_changed = pyqtSignal(object)  # emits CropBox
    clicked = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)
        self.setAttribute(Qt.WidgetAttribute.WA_NoSystemBackground, True)
        self.setMouseTracking(True)

        self.video_w = 1920
        self.video_h = 1080
        self.ratio_type = "original"  # "original", "16:9", "9:16", "1:1"
        self.crop_x_pct = 0.0  # 0.0 to 1.0 (relative to normalized video width)
        self.crop_w_pct = 1.0
        self.crop_y_pct = 0.0
        self.crop_h_pct = 1.0

        self.is_dragging = False
        self.drag_start_pos = QPoint()
        self.drag_start_x_pct = 0.0
        self.drag_start_y_pct = 0.0

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
            self.unsetCursor()
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
        self.crop_changed.emit(self.get_crop_box())

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
        if self.ratio_type == "original":
            return

        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        v_rect = self.get_rendered_video_rect()
        if v_rect.width() <= 0 or v_rect.height() <= 0:
            return

        crop_rect = self.get_crop_screen_rect()

        # 1. Półprzezroczysta ciemna/szara maska wokół kadru (wewnątrz jest czysto i jasno!)
        mask_color = QColor(10, 15, 29, 200)  # głęboki półprzezroczysty szary/grafit
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(mask_color))

        # Lewy pas
        if crop_rect.left() > 0:
            painter.drawRect(QRectF(0, 0, crop_rect.left(), self.height()))
        # Prawy pas
        if crop_rect.right() < self.width():
            painter.drawRect(QRectF(crop_rect.right(), 0, self.width() - crop_rect.right(), self.height()))
        # Górny pas nad klatką
        if crop_rect.top() > 0:
            painter.drawRect(QRectF(crop_rect.left(), 0, crop_rect.width(), crop_rect.top()))
        # Dolny pas pod klatką
        if crop_rect.bottom() < self.height():
            painter.drawRect(QRectF(crop_rect.left(), crop_rect.bottom(), crop_rect.width(), self.height() - crop_rect.bottom()))

        # 2. Główna podświetlona ramka kadru (Błękit / Cyan #38bdf8)
        border_pen = QPen(QColor('#38bdf8'), 2.5)
        painter.setPen(border_pen)
        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.drawRect(crop_rect)

        # 3. Siatka trójpodziału (Rule of Thirds)
        grid_pen = QPen(QColor(255, 255, 255, 65), 1, Qt.PenStyle.DashLine)
        painter.setPen(grid_pen)
        third_w = crop_rect.width() / 3.0
        third_h = crop_rect.height() / 3.0

        p1_top = QPointF(crop_rect.left() + third_w, crop_rect.top())
        p1_bot = QPointF(crop_rect.left() + third_w, crop_rect.bottom())
        p2_top = QPointF(crop_rect.left() + 2 * third_w, crop_rect.top())
        p2_bot = QPointF(crop_rect.left() + 2 * third_w, crop_rect.bottom())

        p1_l = QPointF(crop_rect.left(), crop_rect.top() + third_h)
        p1_r = QPointF(crop_rect.right(), crop_rect.top() + third_h)
        p2_l = QPointF(crop_rect.left(), crop_rect.top() + 2 * third_h)
        p2_r = QPointF(crop_rect.right(), crop_rect.top() + 2 * third_h)

        painter.drawLine(p1_top, p1_bot)
        painter.drawLine(p2_top, p2_bot)
        painter.drawLine(p1_l, p1_r)
        painter.drawLine(p2_l, p2_r)

        # 4. Rogowe uchwyty (Emerald #34d399)
        handle_pen = QPen(QColor('#34d399'), 4.0, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap)
        painter.setPen(handle_pen)
        hl = min(18.0, crop_rect.width() / 4.0, crop_rect.height() / 4.0)

        # Lewy górny
        painter.drawLine(QPointF(crop_rect.left(), crop_rect.top()), QPointF(crop_rect.left() + hl, crop_rect.top()))
        painter.drawLine(QPointF(crop_rect.left(), crop_rect.top()), QPointF(crop_rect.left(), crop_rect.top() + hl))
        # Prawy górny
        painter.drawLine(QPointF(crop_rect.right(), crop_rect.top()), QPointF(crop_rect.right() - hl, crop_rect.top()))
        painter.drawLine(QPointF(crop_rect.right(), crop_rect.top()), QPointF(crop_rect.right(), crop_rect.top() + hl))
        # Lewy dolny
        painter.drawLine(QPointF(crop_rect.left(), crop_rect.bottom()), QPointF(crop_rect.left() + hl, crop_rect.bottom()))
        painter.drawLine(QPointF(crop_rect.left(), crop_rect.bottom()), QPointF(crop_rect.left(), crop_rect.bottom() - hl))
        # Prawy dolny
        painter.drawLine(QPointF(crop_rect.right(), crop_rect.bottom()), QPointF(crop_rect.right() - hl, crop_rect.bottom()))
        painter.drawLine(QPointF(crop_rect.right(), crop_rect.bottom()), QPointF(crop_rect.right(), crop_rect.bottom() - hl))

        # 5. Pływająca plakietka wymiarów kadru na górze ramki
        cb = self.get_crop_box()
        tag_text = f"📱 {cb.ratio_type} ({cb.w}×{cb.h})" if cb.ratio_type == "9:16" else f"📐 {cb.ratio_type} ({cb.w}×{cb.h})"
        badge_w = 140
        badge_h = 24
        badge_x = crop_rect.center().x() - badge_w / 2.0
        badge_y = max(crop_rect.top() + 6, 6)

        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(QColor(15, 23, 42, 225)))
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
                # Kliknięcie poza kadr przesuwa środek kadru w kliknięte miejsce w poziomie
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

        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            if self.is_dragging:
                self.is_dragging = False
                if self.ratio_type != "original":
                    self.setCursor(Qt.CursorShape.OpenHandCursor)
                else:
                    self.unsetCursor()
                event.accept()
                return
            else:
                self.clicked.emit()
        super().mouseReleaseEvent(event)

class CropVideoContainer(QWidget):
    clicked_signal = pyqtSignal()
    crop_changed_signal = pyqtSignal(object)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.video_widget = QVideoWidget(self)
        # Overlay jest dzieckiem video_widget, aby leżał bezpośrednio na nim!
        self.overlay = CropOverlay(self.video_widget)

        self.overlay.clicked.connect(self.clicked_signal)
        self.overlay.crop_changed.connect(self.crop_changed_signal)

    def resizeEvent(self, event):
        super().resizeEvent(event)
        self.video_widget.setGeometry(0, 0, self.width(), self.height())
        self.overlay.setGeometry(0, 0, self.video_widget.width(), self.video_widget.height())
        self.overlay.raise_()
        self.overlay.show()
