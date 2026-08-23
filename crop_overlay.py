import sys
import os
from typing import Optional
from PyQt6.QtWidgets import QWidget, QVBoxLayout
from PyQt6.QtMultimediaWidgets import QVideoWidget
from PyQt6.QtCore import Qt, QRect, QRectF, QPoint, pyqtSignal
from PyQt6.QtGui import QPainter, QColor, QPen, QBrush, QMouseEvent, QPaintEvent

import encoder

class CropOverlay(QWidget):
    crop_changed = pyqtSignal(object) # emits CropBox
    clicked = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, False)
        self.setAttribute(Qt.WidgetAttribute.WA_NoSystemBackground, True)
        
        self.video_w = 1920
        self.video_h = 1080
        self.ratio_type = "original" # "original", "16:9", "9:16", "1:1"
        self.crop_x_pct = 0.0 # 0.0 to 1.0 (relative to normalized video width)
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
        else:
            cb = encoder.calculate_default_crop(self.video_w, self.video_h, ratio, alignment)
            self.crop_w_pct = cb.w / float(self.video_w)
            self.crop_h_pct = cb.h / float(self.video_h)
            self.crop_x_pct = cb.x / float(self.video_w)
            self.crop_y_pct = cb.y / float(self.video_h)
            
        self.emit_crop_box()
        self.update()

    def set_alignment(self, alignment: str):
        if self.ratio_type == "original":
            return
        if alignment == "left":
            self.crop_x_pct = 0.0
        elif alignment == "right":
            self.crop_x_pct = max(1.0 - self.crop_w_pct, 0.0)
        else: # center
            self.crop_x_pct = max((1.0 - self.crop_w_pct) / 2.0, 0.0)
            
        self.emit_crop_box()
        self.update()

    def recalculate_crop_pct(self):
        if self.ratio_type != "original":
            cb = encoder.calculate_default_crop(self.video_w, self.video_h, self.ratio_type, "center")
            self.crop_w_pct = cb.w / float(self.video_w)
            self.crop_h_pct = cb.h / float(self.video_h)
            self.crop_x_pct = max((1.0 - self.crop_w_pct) / 2.0, 0.0)
            self.crop_y_pct = max((1.0 - self.crop_h_pct) / 2.0, 0.0)

    def get_crop_box(self) -> encoder.CropBox:
        if self.ratio_type == "original":
            return encoder.CropBox(0, 0, self.video_w, self.video_h, "original")
            
        x = int(self.crop_x_pct * self.video_w)
        y = int(self.crop_y_pct * self.video_h)
        w = int(self.crop_w_pct * self.video_w)
        h = int(self.crop_h_pct * self.video_h)
        
        w = w if w % 2 == 0 else w - 1
        h = h if h % 2 == 0 else h - 1
        x = max(0, min(x, self.video_w - w))
        y = max(0, min(y, self.video_h - h))
        return encoder.CropBox(x=x, y=y, w=w, h=h, ratio_type=self.ratio_type)

    def emit_crop_box(self):
        self.crop_changed.emit(self.get_crop_box())

    def get_rendered_video_rect(self) -> QRectF:
        widget_w = self.width()
        widget_h = self.height()
        if widget_w <= 0 or widget_h <= 0 or self.video_w <= 0 or self.video_h <= 0:
            return QRectF(0, 0, widget_w, widget_h)

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

    def paintEvent(self, event: QPaintEvent):
        if self.ratio_type == "original":
            return

        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        v_rect = self.get_rendered_video_rect()
        if v_rect.width() <= 0 or v_rect.height() <= 0:
            return

        crop_screen_x = v_rect.x() + self.crop_x_pct * v_rect.width()
        crop_screen_y = v_rect.y() + self.crop_y_pct * v_rect.height()
        crop_screen_w = self.crop_w_pct * v_rect.width()
        crop_screen_h = self.crop_h_pct * v_rect.height()
        crop_rect = QRectF(crop_screen_x, crop_screen_y, crop_screen_w, crop_screen_h)

        mask_brush = QBrush(QColor(0, 0, 0, 180))
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(mask_brush)

        if crop_rect.left() > v_rect.left():
            painter.drawRect(QRectF(v_rect.left(), v_rect.top(), crop_rect.left() - v_rect.left(), v_rect.height()))
        if crop_rect.right() < v_rect.right():
            painter.drawRect(QRectF(crop_rect.right(), v_rect.top(), v_rect.right() - crop_rect.right(), v_rect.height()))
        if crop_rect.top() > v_rect.top():
            painter.drawRect(QRectF(crop_rect.left(), v_rect.top(), crop_rect.width(), crop_rect.top() - v_rect.top()))
        if crop_rect.bottom() < v_rect.bottom():
            painter.drawRect(QRectF(crop_rect.left(), crop_rect.bottom(), crop_rect.width(), v_rect.bottom() - crop_rect.bottom()))

        border_pen = QPen(QColor('#38bdf8'), 2)
        painter.setPen(border_pen)
        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.drawRect(crop_rect)

        grid_pen = QPen(QColor(255, 255, 255, 60), 1, Qt.PenStyle.DashLine)
        painter.setPen(grid_pen)
        third_w = crop_rect.width() / 3.0
        third_h = crop_rect.height() / 3.0
        painter.drawLine(QPoint(int(crop_rect.left() + third_w), int(crop_rect.top())), QPoint(int(crop_rect.left() + third_w), int(crop_rect.bottom())))
        painter.drawLine(QPoint(int(crop_rect.left() + 2 * third_w), int(crop_rect.top())), QPoint(int(crop_rect.left() + 2 * third_w), int(crop_rect.bottom())))
        painter.drawLine(QPoint(int(crop_rect.left()), int(crop_rect.top() + third_h)), QPoint(int(crop_rect.right()), int(crop_rect.top() + third_h)))
        painter.drawLine(QPoint(int(crop_rect.left()), int(crop_rect.top() + 2 * third_h)), QPoint(int(crop_rect.right()), int(crop_rect.top() + 2 * third_h)))

        handle_pen = QPen(QColor('#34d399'), 3)
        painter.setPen(handle_pen)
        hl = 14
        painter.drawLine(int(crop_rect.left()), int(crop_rect.top()), int(crop_rect.left() + hl), int(crop_rect.top()))
        painter.drawLine(int(crop_rect.left()), int(crop_rect.top()), int(crop_rect.left()), int(crop_rect.top() + hl))
        painter.drawLine(int(crop_rect.right()), int(crop_rect.top()), int(crop_rect.right() - hl), int(crop_rect.top()))
        painter.drawLine(int(crop_rect.right()), int(crop_rect.top()), int(crop_rect.right()), int(crop_rect.top() + hl))
        painter.drawLine(int(crop_rect.left()), int(crop_rect.bottom()), int(crop_rect.left() + hl), int(crop_rect.bottom()))
        painter.drawLine(int(crop_rect.left()), int(crop_rect.bottom()), int(crop_rect.left()), int(crop_rect.bottom() - hl))
        painter.drawLine(int(crop_rect.right()), int(crop_rect.bottom()), int(crop_rect.right() - hl), int(crop_rect.bottom()))
        painter.drawLine(int(crop_rect.right()), int(crop_rect.bottom()), int(crop_rect.right()), int(crop_rect.bottom() - hl))

    def mousePressEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            v_rect = self.get_rendered_video_rect()
            crop_screen_x = v_rect.x() + self.crop_x_pct * v_rect.width()
            crop_screen_y = v_rect.y() + self.crop_y_pct * v_rect.height()
            crop_screen_w = self.crop_w_pct * v_rect.width()
            crop_screen_h = self.crop_h_pct * v_rect.height()
            crop_rect = QRectF(crop_screen_x, crop_screen_y, crop_screen_w, crop_screen_h)

            if self.ratio_type != "original" and crop_rect.contains(event.position()):
                self.is_dragging = True
                self.drag_start_pos = event.position().toPoint()
                self.drag_start_x_pct = self.crop_x_pct
                self.drag_start_y_pct = self.crop_y_pct
                event.accept()
                return

        super().mousePressEvent(event)

    def mouseMoveEvent(self, event: QMouseEvent):
        if self.is_dragging and self.ratio_type != "original":
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

        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            if self.is_dragging:
                self.is_dragging = False
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
        self.overlay = CropOverlay(self)

        self.overlay.clicked.connect(self.clicked_signal)
        self.overlay.crop_changed.connect(self.crop_changed_signal)

    def resizeEvent(self, event):
        super().resizeEvent(event)
        self.video_widget.setGeometry(0, 0, self.width(), self.height())
        self.overlay.setGeometry(0, 0, self.width(), self.height())
