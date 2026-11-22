use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, Response,
    RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetType,
};

use crate::i18n::{t, Lang};
use crate::icons::{self, Icon};
use crate::player::PlaybackRate;
use crate::theme::{self, Palette};

pub const TOOLBAR_HEIGHT: f32 = 34.0;
const TOOLBAR_TINT: f32 = 0.065;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BtnKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

pub fn heading(ui: &mut Ui, icon: Icon, label: &str) {
    let palette = Palette::get(ui);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(19.0), Sense::hover());
        icons::paint(ui.painter(), icon, rect, palette.accent, 1.55);
        ui.label(
            RichText::new(label)
                .family(egui::FontFamily::Name("Redaction".into()))
                .size(16.0),
        );
    });
}

/// Inset shadows inspired by adamgiebl's button on Uiverse.
pub fn inset_footer_button(ui: &mut Ui, label: &str) -> Response {
    let palette = Palette::get(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, response.enabled(), label));
    let active = ui.ctx().animate_bool_with_time(
        response.id.with("inset"),
        response.hovered() || response.has_focus(),
        0.2,
    );
    let pressed = response.is_pointer_button_down_on();
    let base = if palette.glass || palette.dark {
        palette.raised
    } else {
        Color32::from_gray(224)
    };
    let light = if palette.dark {
        Color32::from_rgb(72, 77, 82)
    } else {
        Color32::WHITE
    };
    let dark = if palette.dark {
        Color32::from_gray(10)
    } else {
        Color32::from_gray(151)
    };
    let painter = ui.painter();
    let radius = 18;
    for (offset, color) in [([-2, -2], light), ([2, 2], dark)] {
        if active > 0.0 {
            painter.add(
                egui::epaint::Shadow {
                    offset,
                    blur: 5,
                    spread: 0,
                    color: color.gamma_multiply(active * 0.55),
                }
                .as_shape(rect, radius),
            );
        }
    }
    let depth = if pressed {
        0.45
    } else {
        0.25 * (1.0 - active * 0.5)
    };
    icons::rounded_gradient(
        painter,
        rect,
        f32::from(radius),
        icons::lerp_color(base, dark, depth),
        icons::lerp_color(base, light, depth),
    );
    let top = Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y));
    let bottom = Rect::from_min_max(pos2(rect.left(), rect.center().y), rect.max);
    for (clip, tint) in [(top, dark), (bottom, light)] {
        let inset = painter.with_clip_rect(clip);
        for step in 0..4 {
            let strength = (0.52 - 0.11 * f32::from(step)) * (1.0 - 0.43 * active);
            inset.rect_stroke(
                rect.shrink(f32::from(step)),
                radius - step,
                Stroke::new(1.0_f32, tint.gamma_multiply(strength)),
                StrokeKind::Inside,
            );
        }
    }
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(
            1.0_f32,
            icons::lerp_color(palette.line, light, active * 0.25),
        ),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center() + vec2(0.0, if pressed { 1.0 } else { 0.0 }),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        palette.text,
    );
    finish(ui, response, rect, radius)
}

fn action_color(icon: Icon, palette: Palette) -> Color32 {
    let pair = match icon {
        Icon::Trash | Icon::Close => return palette.danger,
        Icon::Play | Icon::Pause | Icon::Refresh | Icon::Check => return palette.good,
        Icon::Scissors | Icon::Folder | Icon::Pencil => return palette.accent,
        Icon::Heart | Icon::HeartOutline if palette.dark => return theme::PINK,
        Icon::Heart | Icon::HeartOutline => return theme::PINK_LIGHT,
        Icon::Camera => ((189, 172, 252), (90, 58, 151)),
        Icon::Upload | Icon::Gallery => ((139, 198, 255), (32, 91, 153)),
        _ => return palette.muted,
    };
    let (r, g, b) = if palette.dark { pair.0 } else { pair.1 };
    Color32::from_rgb(r, g, b)
}

/// Size of the theme preview drawn in the background-theme picker.
const PREVIEW_WIDTH: f32 = 58.0;
const PREVIEW_HEIGHT: f32 = 24.0;

/// Small two-state switch between the bar and pie chart of the storage section.
pub fn chart_toggle(ui: &mut Ui, pie: &mut bool, lang: Lang) -> Response {
    let palette = Palette::get(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(52.0, 22.0), Sense::click());
    if response.clicked() {
        *pie = !*pie;
    }
    let slide = ui
        .ctx()
        .animate_bool_with_time(response.id.with("pie"), *pie, 0.18);
    let hovered = response.hovered();
    let painter = ui.painter();
    painter.rect_filled(rect, 7, palette.well);
    let knob = Rect::from_center_size(
        pos2(rect.left() + 13.0 + 26.0 * slide, rect.center().y),
        vec2(24.0, 18.0),
    );
    painter.rect_filled(
        knob,
        5,
        icons::lerp_color(palette.raised, palette.accent, 0.16),
    );
    for (index, icon) in [Icon::BarChart, Icon::PieChart].into_iter().enumerate() {
        let active = (index == 1) == *pie;
        let slot = Rect::from_center_size(
            pos2(rect.left() + 13.0 + 26.0 * index as f32, rect.center().y),
            Vec2::splat(13.0),
        );
        let color = if active {
            palette.text
        } else if hovered {
            palette.muted
        } else {
            palette.muted.gamma_multiply(0.7)
        };
        icons::paint(painter, icon, slot, color, 1.35);
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(t(lang, "storage"))
}

/// Warm, slightly orange highlight used by every dropdown row.
pub fn menu_hover(palette: Palette) -> Color32 {
    let warm = Color32::from_rgb(236, 162, 92);
    icons::lerp_color(palette.raised, warm, if palette.dark { 0.16 } else { 0.13 })
}

/// One row of a dropdown: optional icon, label and a check mark when selected.
pub fn menu_row(ui: &mut Ui, icon: Option<Icon>, label: &str, selected: bool) -> Response {
    menu_row_tinted(ui, icon, label, selected, None)
}

/// Same row, but the glyph and the label carry a colour of their own so that
/// destructive or positive actions read at a glance.
pub fn menu_row_tinted(
    ui: &mut Ui,
    icon: Option<Icon>,
    label: &str,
    selected: bool,
    tint: Option<Color32>,
) -> Response {
    menu_row_sized(ui, icon, label, selected, tint, 30.0, 9.0)
}

pub fn menu_row_sized(
    ui: &mut Ui,
    icon: Option<Icon>,
    label: &str,
    selected: bool,
    tint: Option<Color32>,
    height: f32,
    inset: f32,
) -> Response {
    let palette = Palette::get(ui);
    let text_color = tint.unwrap_or(palette.text);
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), palette.text);
    let icon_room = if icon.is_some() { 22.0 } else { 0.0 };
    let width = ui
        .available_width()
        .max(8.0 + icon_room + galley.size().x + 28.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            label,
        )
    });
    let hovered = response.hovered();
    if selected || hovered {
        ui.painter().rect_filled(
            rect,
            6,
            if hovered {
                menu_hover(palette)
            } else {
                palette.accent_tint
            },
        );
    }
    let mut x = rect.left() + inset;
    if let Some(icon) = icon {
        let icon_rect = Rect::from_center_size(pos2(x + 7.5, rect.center().y), Vec2::splat(15.0));
        let icon_hover = ui
            .ctx()
            .animate_bool(response.id.with("icon_hover"), response.hovered());
        icons::paint_animated(
            ui.painter(),
            icon,
            icon_rect,
            match (selected, tint) {
                (true, _) => palette.accent,
                (false, Some(tint)) => tint,
                (false, None) => palette.muted,
            },
            1.45,
            icon_hover,
        );
        x += icon_room;
    }
    ui.painter().galley(
        pos2(x, rect.center().y - galley.size().y * 0.5),
        galley,
        text_color,
    );
    if selected {
        let center = pos2(rect.right() - 15.0, rect.center().y);
        ui.painter().add(Shape::line(
            vec![
                center + vec2(-4.0, 0.0),
                center + vec2(-1.0, 3.0),
                center + vec2(4.0, -3.0),
            ],
            Stroke::new(1.5_f32, palette.accent),
        ));
    }
    finish(ui, response, rect, 6)
}

/// Row of the background-theme picker: a small preview of the theme, its name and
/// a check mark on the active one.
pub fn theme_row(
    ui: &mut Ui,
    thumb: Option<&egui::TextureHandle>,
    label: &str,
    selected: bool,
) -> Response {
    let palette = Palette::get(ui);
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), palette.text);
    // Wide enough for the longest theme name, so the popup grows instead of
    // clipping it.
    let width = ui
        .available_width()
        .max(6.0 + PREVIEW_WIDTH + 12.0 + galley.size().x + 26.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 38.0), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            label,
        )
    });
    let hovered = response.hovered();
    if selected || hovered {
        ui.painter().rect_filled(
            rect,
            6,
            if hovered {
                menu_hover(palette)
            } else {
                palette.accent_tint
            },
        );
    }
    let preview = Rect::from_min_size(
        pos2(rect.left() + 6.0, rect.center().y - PREVIEW_HEIGHT * 0.5),
        vec2(PREVIEW_WIDTH, PREVIEW_HEIGHT),
    );
    match thumb {
        Some(texture) => {
            egui::Image::new((texture.id(), preview.size()))
                .uv(theme::cover_uv(texture.size_vec2(), preview.size()))
                .corner_radius(4)
                .paint_at(ui, preview);
        }
        None => {
            ui.painter()
                .rect_filled(preview, 4, palette.raised.gamma_multiply(0.9));
            icons::paint(
                ui.painter(),
                Icon::Image,
                Rect::from_center_size(preview.center(), Vec2::splat(13.0)),
                palette.muted,
                1.5,
            );
        }
    }
    ui.painter().galley(
        pos2(
            preview.right() + 12.0,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        palette.text,
    );
    if selected {
        let center = pos2(rect.right() - 15.0, rect.center().y);
        ui.painter().add(Shape::line(
            vec![
                center + vec2(-4.0, 0.0),
                center + vec2(-1.0, 3.0),
                center + vec2(4.0, -3.0),
            ],
            Stroke::new(1.5_f32, palette.accent),
        ));
    }
    finish(ui, response, rect, 6)
}

pub fn animate_menu(ui: &mut Ui) {
    let frame = ui.ctx().cumulative_frame_nr();
    let now = ui.input(|input| input.time);
    let id = ui.id().with("menu_enter");
    let start = ui.data_mut(|data| {
        let start = data
            .get_temp::<(u64, f64)>(id)
            .filter(|(last, _)| frame.saturating_sub(*last) <= 1)
            .map_or(now, |(_, start)| start);
        data.insert_temp(id, (frame, start));
        start
    });
    let remaining = (1.0 - (now - start) as f32 / 0.14).clamp(0.0, 1.0);
    if remaining > 0.0 {
        ui.multiply_opacity(1.0 - 0.28 * remaining);
        ui.add_space(4.0 * remaining * remaining);
        ui.ctx().request_repaint();
    }
}

pub fn playback_speed(ui: &mut Ui, speed: &mut PlaybackRate, lang: Lang) -> bool {
    const WIDTH: f32 = 84.0;
    let palette = Palette::get(ui);
    let previous = *speed;
    let (rect, response) = ui.allocate_exact_size(vec2(WIDTH, 28.0), Sense::click());
    let label = t(lang, "playback_speed");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, response.enabled(), label));
    let hover = hover(ui, &response);
    let panel = opaque(palette.panel);
    // The speed pill sits on a dark control row, so it needs a surface that is
    // actually lighter than the row instead of blending into it.
    let fill = icons::lerp_color(
        opaque(palette.raised),
        opaque(palette.text),
        0.06 + hover * 0.06,
    );
    let radius = 7;
    ui.painter().rect_filled(rect, radius, fill);
    ui.painter().text(
        pos2(rect.left() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        speed.label(),
        FontId::proportional(13.0),
        palette.text,
    );
    let arrow = pos2(rect.right() - 13.0, rect.center().y);
    ui.painter().add(Shape::convex_polygon(
        vec![
            arrow + vec2(-4.0, -2.0),
            arrow + vec2(4.0, -2.0),
            arrow + vec2(0.0, 3.0),
        ],
        palette.text,
        Stroke::NONE,
    ));
    egui::Popup::menu(&response)
        .width(WIDTH)
        .gap(3.0)
        .frame(
            egui::Frame::popup(ui.style())
                .inner_margin(4)
                .corner_radius(radius)
                .fill(panel)
                .stroke(Stroke::NONE),
        )
        .show(|ui| {
            ui.set_width(WIDTH - 8.0);
            animate_menu(ui);
            for rate in PlaybackRate::OPTIONS {
                if choice(ui, rate.label(), *speed == rate).clicked() {
                    *speed = rate;
                    ui.close();
                }
            }
        });
    finish(ui, response, rect, radius).on_hover_text(label);
    *speed != previous
}

pub fn borderless_controls(ui: &mut Ui) {
    let visuals = &mut ui.style_mut().visuals.widgets;
    for visual in [
        &mut visuals.inactive,
        &mut visuals.hovered,
        &mut visuals.active,
        &mut visuals.open,
    ] {
        visual.bg_stroke = Stroke::NONE;
    }
}

fn opaque(color: Color32) -> Color32 {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    Color32::from_rgb(r, g, b)
}

fn button_border(ui: &Ui, stroke: Stroke) -> Stroke {
    if ui.visuals().widgets.inactive.bg_stroke == Stroke::NONE {
        Stroke::NONE
    } else {
        stroke
    }
}

pub fn playback_time(ui: &mut Ui, current: String, total: String, color: Color32) {
    let font = FontId::new(11.0, egui::FontFamily::Monospace);
    let widest = ui.fonts_mut(|fonts| {
        ('0'..='9')
            .max_by(|a, b| {
                fonts
                    .glyph_width(&font, *a)
                    .total_cmp(&fonts.glyph_width(&font, *b))
            })
            .unwrap_or('8')
    });
    let sample: String = total
        .chars()
        .map(|c| if c.is_ascii_digit() { widest } else { c })
        .collect();
    let width = ui
        .painter()
        .layout_no_wrap(format!("{sample} / {sample}"), font.clone(), color)
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(vec2(width + 3.0, 28.0), Sense::hover());
    let text = format!("{current} / {total}");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
    ui.painter()
        .text(rect.left_center(), Align2::LEFT_CENTER, text, font, color);
}

#[derive(Default)]
pub struct PreviewPress {
    held: bool,
    dragged: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PreviewAction {
    None,
    Click,
    Drag,
    Hold(PlaybackRate),
    Release,
}

impl PreviewPress {
    pub fn update(
        &mut self,
        response: &Response,
        image: Option<Rect>,
        enabled: bool,
    ) -> PreviewAction {
        const HOLD_DELAY: f64 = 0.22;
        let (press, origin, position, age, down) = response.ctx.input(|input| {
            let pointer = &input.pointer;
            (
                pointer.primary_pressed(),
                pointer.press_origin(),
                pointer.interact_pos(),
                input.time - pointer.press_start_time().unwrap_or(input.time),
                pointer.primary_down() && input.focused,
            )
        });
        if press {
            *self = Self::default();
        }
        if !down || !enabled {
            let previous = std::mem::take(self);
            return if previous.held {
                PreviewAction::Release
            } else if enabled && !previous.dragged && response.clicked() {
                PreviewAction::Click
            } else {
                PreviewAction::None
            };
        }
        if self.held || self.dragged || !response.is_pointer_button_down_on() {
            return PreviewAction::None;
        }
        let (Some(origin), Some(position)) = (origin, position) else {
            return PreviewAction::None;
        };
        if origin.distance(position) > 7.0 && response.dragged_by(egui::PointerButton::Primary) {
            self.dragged = true;
            return PreviewAction::Drag;
        }
        if let Some(image) = image.filter(|image| image.contains(origin)) {
            if age >= HOLD_DELAY {
                self.held = true;
                let rate = if origin.x < image.center().x {
                    PlaybackRate::Half
                } else {
                    PlaybackRate::Double
                };
                return PreviewAction::Hold(rate);
            }
            response
                .ctx
                .request_repaint_after(std::time::Duration::from_secs_f64(HOLD_DELAY - age));
        }
        PreviewAction::None
    }

    pub fn cancel(&mut self) -> bool {
        std::mem::take(self).held
    }
}

pub fn language_picker(ui: &mut Ui, lang: &mut Lang) -> bool {
    let palette = Palette::get(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(78.0, TOOLBAR_HEIGHT), Sense::click());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(
            WidgetType::ComboBox,
            response.enabled(),
            t(*lang, "language"),
        );
        info.current_text_value = Some(lang.native_name().into());
        info
    });
    let popup = egui::Popup::menu(&response).width(146.0);
    let open = popup.is_open();
    let fill = icons::lerp_color(
        palette.panel,
        palette.raised,
        if open { 1.0 } else { hover(ui, &response) },
    );
    let painter = ui.painter();
    // Flat surface: the hairline outline made these chips look boxed in.
    painter.rect_filled(rect, 8, fill);
    flag(painter, pos2(rect.left() + 17.0, rect.center().y), *lang);
    painter.text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        lang.code().to_ascii_uppercase(),
        FontId::proportional(12.0),
        palette.text,
    );
    let chevron = pos2(rect.right() - 12.0, rect.center().y);
    let direction = if open { -1.0 } else { 1.0 };
    painter.add(Shape::line(
        vec![
            chevron + vec2(-3.0, -1.5 * direction),
            chevron + vec2(0.0, 1.5 * direction),
            chevron + vec2(3.0, -1.5 * direction),
        ],
        Stroke::new(1.35_f32, palette.muted),
    ));
    let selected = popup
        .show(|ui| {
            animate_menu(ui);
            ui.set_min_width(136.0);
            let mut picked = None;
            for option in Lang::ALL {
                let (row, response) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                let current = option == *lang;
                response.widget_info(|| {
                    WidgetInfo::selected(
                        WidgetType::SelectableLabel,
                        response.enabled(),
                        current,
                        option.native_name(),
                    )
                });
                let painter = ui.painter();
                if current || response.hovered() {
                    painter.rect_filled(
                        row,
                        5,
                        if response.hovered() {
                            menu_hover(palette)
                        } else {
                            palette.accent_tint
                        },
                    );
                }
                flag(painter, pos2(row.left() + 17.0, row.center().y), option);
                painter.text(
                    pos2(row.left() + 34.0, row.center().y),
                    Align2::LEFT_CENTER,
                    option.native_name(),
                    FontId::proportional(13.0),
                    palette.text,
                );
                if current {
                    let center = pos2(row.right() - 13.0, row.center().y);
                    painter.add(Shape::line(
                        vec![
                            center + vec2(-4.0, 0.0),
                            center + vec2(-1.0, 3.0),
                            center + vec2(4.0, -3.0),
                        ],
                        Stroke::new(1.5_f32, palette.accent),
                    ));
                }
                if finish(ui, response, row, 5).clicked() {
                    picked = Some(option);
                }
            }
            picked
        })
        .and_then(|result| result.inner);
    finish(ui, response, rect, 8).on_hover_text(t(*lang, "language"));
    if let Some(option) = selected.filter(|option| option != lang) {
        *lang = option;
        true
    } else {
        false
    }
}

fn flag(painter: &egui::Painter, center: egui::Pos2, lang: Lang) {
    let rect = Rect::from_center_size(center, vec2(22.0, 14.0));
    match lang {
        Lang::Pl => {
            painter.rect_filled(rect, 2, Color32::from_rgb(250, 250, 250));
            let bottom = Rect::from_min_max(pos2(rect.left(), center.y), rect.max);
            painter.rect_filled(
                bottom,
                CornerRadius {
                    sw: 2,
                    se: 2,
                    ..CornerRadius::ZERO
                },
                Color32::from_rgb(220, 20, 60),
            );
        }
        Lang::En => {
            painter.rect_filled(rect, 2, Color32::from_rgb(1, 33, 105));
            let clipped = painter.with_clip_rect(rect);
            let diagonals = [
                [rect.left_top(), rect.right_bottom()],
                [rect.right_top(), rect.left_bottom()],
            ];
            for diagonal in diagonals {
                clipped.line_segment(diagonal, Stroke::new(3.1_f32, Color32::WHITE));
            }
            let red = Color32::from_rgb(200, 16, 46);
            for (index, diagonal) in diagonals.into_iter().enumerate() {
                let shift = if index == 0 { 0.6 } else { -0.6 };
                for (start, end, offset) in
                    [(diagonal[0], center, shift), (center, diagonal[1], -shift)]
                {
                    clipped.line_segment(
                        [start + vec2(0.0, offset), end + vec2(0.0, offset)],
                        Stroke::new(1.0_f32, red),
                    );
                }
            }
            for (width, color) in [(4.8_f32, Color32::WHITE), (2.8, red)] {
                clipped.hline(rect.x_range(), center.y, Stroke::new(width, color));
                clipped.vline(center.x, rect.y_range(), Stroke::new(width, color));
            }
        }
    }
    painter.rect_stroke(
        rect,
        2,
        Stroke::new(1.0_f32, Color32::from_black_alpha(60)),
        StrokeKind::Inside,
    );
}

pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    kind: BtnKind,
    min_width: f32,
) -> Response {
    let palette = Palette::get(ui);
    let font = FontId::proportional(13.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), palette.text)
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(
        vec2((text_width + 45.0).max(min_width), TOOLBAR_HEIGHT),
        Sense::click(),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, response.enabled(), label));
    let t = hover(ui, &response);
    let down = response.is_pointer_button_down_on();
    let (top, bottom, text, border) = match kind {
        BtnKind::Primary if response.enabled() => (
            icons::lerp_color(
                Color32::from_rgb(253, 222, 155),
                Color32::from_rgb(255, 234, 189),
                t,
            ),
            Color32::from_rgb(215, 154, 66),
            Color32::from_rgb(43, 28, 12),
            Color32::from_rgb(255, 223, 166),
        ),
        BtnKind::Danger => (
            icons::lerp_color(palette.panel, palette.danger, 0.10 + t * 0.10),
            icons::lerp_color(palette.panel, palette.danger, 0.06),
            palette.danger,
            icons::lerp_color(palette.line, palette.danger, 0.34),
        ),
        BtnKind::Ghost => (palette.panel, palette.panel, palette.muted, palette.line),
        _ => (palette.raised, palette.panel, palette.text, palette.line),
    };
    let painter = ui.painter();
    let radius = 8;
    let rect = if down {
        rect.translate(vec2(0.0, 1.0))
    } else {
        rect
    };
    if kind == BtnKind::Primary && response.enabled() {
        painter.add(
            egui::epaint::RectShape::filled(
                rect.translate(vec2(0.0, 3.0)),
                radius,
                theme::GOLD.gamma_multiply(0.17 + 0.13 * t),
            )
            .with_blur_width(11.0),
        );
    }

    let top = if kind == BtnKind::Primary || !response.enabled() {
        top
    } else {
        icons::lerp_color(top, palette.text, t * 0.065)
    };
    let (top, bottom, border) = if palette.glass && kind == BtnKind::Primary && response.enabled() {
        (
            top.gamma_multiply(0.82),
            bottom.gamma_multiply(0.82),
            border.gamma_multiply(0.68),
        )
    } else {
        (top, bottom, border)
    };
    if !matches!(
        kind,
        BtnKind::Primary | BtnKind::Secondary | BtnKind::Danger
    ) {
        painter.rect_filled(rect, radius, top);
    } else {
        icons::rounded_gradient(painter, rect, f32::from(radius), top, bottom);
    }
    // Only the filled kinds keep an outline; plain buttons read as flat surfaces
    // (the hairline border on every button made the UI look generated).
    if matches!(kind, BtnKind::Primary | BtnKind::Danger) {
        painter.rect_stroke(
            rect,
            radius,
            button_border(ui, Stroke::new(1.0_f32, border)),
            StrokeKind::Inside,
        );
    }
    let start = rect.center().x - (text_width + 24.0) * 0.5;
    let icon_rect = Rect::from_center_size(pos2(start + 8.0, rect.center().y), Vec2::splat(16.0));
    let icon_color = if kind != BtnKind::Ghost || !response.enabled() {
        if kind == BtnKind::Secondary {
            icons::lerp_color(action_color(icon, palette), palette.text, 0.55)
        } else {
            text
        }
    } else {
        action_color(icon, palette)
    };
    icons::paint_animated(painter, icon, icon_rect, icon_color, 1.6, t);
    painter.text(
        pos2(start + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        text,
    );
    finish(ui, response, rect, radius)
}

pub fn icon_square(ui: &mut Ui, icon: Icon, label: &str, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    square_at(ui, rect, response, icon, label)
}

fn toolbar_surface(palette: Palette, accent: Color32, tint: f32, hover: f32) -> (Color32, Color32) {
    // Icon buttons sit on a raised surface: on the flat panel colour they were
    // almost invisible until hovered.
    let base = icons::lerp_color(palette.panel, palette.raised, 0.75);
    (
        icons::lerp_color(base, accent, tint + hover * 0.12),
        icons::lerp_color(palette.line, accent, tint + hover * 0.22),
    )
}

fn square_at(ui: &mut Ui, rect: Rect, response: Response, icon: Icon, label: &str) -> Response {
    let palette = Palette::get(ui);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, response.enabled(), label));
    let t = hover(ui, &response);
    let accent = action_color(icon, palette);
    let color = icons::lerp_color(accent, palette.text, t * 0.12);
    let painter = ui.painter();
    let strength = if matches!(icon, Icon::Trash) {
        0.16
    } else {
        TOOLBAR_TINT
    };
    // A red cross keeps a neutral surface; only the glyph turns red.
    let surface_tint = if matches!(icon, Icon::Close) {
        palette.muted
    } else {
        accent
    };
    let (background, _border) = toolbar_surface(palette, surface_tint, strength, t);
    let background = if palette.glass && matches!(icon, Icon::Play | Icon::Pause) {
        icons::lerp_color(opaque(palette.panel), accent, 0.14 + t * 0.08).gamma_multiply(0.9)
    } else {
        background
    };
    let rect = if response.is_pointer_button_down_on() {
        rect.shrink(1.0)
    } else {
        rect
    };
    painter.rect_filled(rect, 8, background);
    let icon_size = if matches!(icon, Icon::Camera) {
        23.0
    } else {
        18.0
    };
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(icon_size));
    icons::paint_animated(painter, icon, icon_rect, color, 1.6, t);
    finish(ui, response, rect, 8).on_hover_text(label)
}

pub fn tab(ui: &mut Ui, label: &str, selected: bool, icon: Option<Icon>) -> Response {
    let palette = Palette::get(ui);
    let text = if selected {
        palette.accent
    } else {
        palette.muted
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), text);
    let offset = if icon.is_some() { 21.0 } else { 0.0 };
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 22.0 + offset, 34.0), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            label,
        )
    });
    let t = hover(ui, &response);
    let bg = if selected {
        palette.accent_tint
    } else {
        icons::lerp_color(palette.canvas, palette.raised, t)
    };
    ui.painter().rect_filled(rect, 8, bg);
    if let Some(icon) = icon {
        let icon_rect =
            Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), Vec2::splat(15.0));
        icons::paint(ui.painter(), icon, icon_rect, text, 1.5);
    }
    let baseline = pos2(
        rect.left() + 11.0 + offset,
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter().galley(baseline, galley, text);
    finish(ui, response, rect, 8)
}

pub fn filter_tab(
    ui: &mut Ui,
    label: &str,
    count: usize,
    digits: usize,
    selected: bool,
    paint_icon: Option<&dyn Fn(&egui::Painter, Rect)>,
) -> Response {
    let palette = Palette::get(ui);
    let color = if selected {
        palette.accent
    } else {
        palette.text
    };
    let font = FontId::proportional(12.0);
    let counter_font = FontId::monospace(10.5);
    let painter = ui.painter();
    let label = painter.layout_no_wrap(label.to_owned(), font, color);
    let counter = painter.layout_no_wrap(count.to_string(), counter_font.clone(), color);
    let counter_width = painter
        .layout_no_wrap("0".repeat(digits), counter_font, color)
        .size()
        .x
        + 8.0;
    let icon_width = if paint_icon.is_some() { 22.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(
        vec2(
            8.0 + icon_width + label.size().x + 5.0 + counter_width,
            29.0,
        ),
        Sense::click(),
    );
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            format!("{} ({count})", label.text()),
        )
    });
    let hover = hover(ui, &response);
    let selected_amount =
        ui.ctx()
            .animate_bool_with_time(response.id.with("selected"), selected, 0.15);
    let bg = icons::lerp_color(
        icons::lerp_color(palette.canvas, palette.raised, hover),
        palette.accent_tint,
        selected_amount,
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 7, bg);
    let left = rect.left() + 4.0;
    if let Some(paint_icon) = paint_icon {
        paint_icon(
            painter,
            Rect::from_center_size(pos2(left + 9.0, rect.center().y), Vec2::splat(18.0)),
        );
    }
    painter.galley(
        pos2(left + icon_width, rect.center().y - label.size().y * 0.5),
        label,
        color,
    );
    let count_rect = Rect::from_center_size(
        pos2(rect.right() - 4.0 - counter_width * 0.5, rect.center().y),
        vec2(counter_width, 17.0),
    );
    painter.rect_filled(count_rect, 5, icons::lerp_color(bg, color, 0.10));
    painter.galley(count_rect.center() - counter.size() * 0.5, counter, color);
    finish(ui, response, rect, 7)
}

pub fn icon_toggle(ui: &mut Ui, value: &mut bool, icon: Icon, label: &str, size: f32) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Checkbox, response.enabled(), *value, label)
    });
    let palette = Palette::get(ui);
    let enabled = ui.ctx().animate_bool(response.id.with("checked"), *value);
    let hover = hover(ui, &response);
    let fill = icons::lerp_color(
        icons::lerp_color(palette.panel, palette.raised, hover),
        palette.accent_tint,
        enabled,
    );
    let line = icons::lerp_color(palette.line, palette.accent, enabled);
    let color = icons::lerp_color(palette.muted, palette.accent, enabled);
    ui.painter().rect_filled(rect, 8, fill);
    ui.painter()
        .rect_stroke(rect, 8, Stroke::new(1.0_f32, line), StrokeKind::Inside);
    icons::paint(
        ui.painter(),
        icon,
        Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
        color,
        1.55,
    );
    finish(ui, response, rect, 8).on_hover_text(label)
}

pub fn trim_point(ui: &mut Ui, icon: Icon, label: &str, time: &str, hint: &str) -> Response {
    let palette = Palette::get(ui);
    let color = if matches!(icon, Icon::MarkIn) {
        palette.good
    } else {
        palette.danger
    };
    let (rect, response) = ui.allocate_exact_size(vec2(128.0, 28.0), Sense::click());
    let hover = hover(ui, &response);
    let painter = ui.painter();
    let base = if palette.glass {
        opaque(palette.well).gamma_multiply(0.88)
    } else {
        palette.well
    };
    painter.rect_filled(rect, 7, icons::lerp_color(base, color, 0.13 + 0.10 * hover));
    painter.rect_stroke(
        rect,
        7,
        button_border(
            ui,
            Stroke::new(
                1.0_f32,
                icons::lerp_color(palette.line, color, 0.25 + 0.35 * hover),
            ),
        ),
        StrokeKind::Inside,
    );
    let is_in = matches!(icon, Icon::MarkIn);
    let icon_x = if is_in {
        rect.right() - 14.0
    } else {
        rect.left() + 14.0
    };
    icons::paint(
        painter,
        icon,
        Rect::from_center_size(pos2(icon_x, rect.center().y), Vec2::splat(16.0)),
        color,
        1.55,
    );
    let label_x = if is_in { icon_x - 12.0 } else { icon_x + 12.0 };
    painter.text(
        pos2(label_x, rect.center().y),
        if is_in {
            Align2::RIGHT_CENTER
        } else {
            Align2::LEFT_CENTER
        },
        label,
        FontId::proportional(10.0),
        color,
    );
    painter.text(
        pos2(
            if is_in {
                rect.left() + 5.0
            } else {
                rect.right() - 5.0
            },
            rect.center().y,
        ),
        if is_in {
            Align2::LEFT_CENTER
        } else {
            Align2::RIGHT_CENTER
        },
        time,
        FontId::monospace(11.5),
        palette.text,
    );
    response.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::Button,
            response.enabled(),
            format!("{hint} · {time}"),
        )
    });
    finish(ui, response, rect, 7).on_hover_text(hint)
}

pub fn choice(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let palette = Palette::get(ui);
    let font = FontId::proportional(13.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, palette.text);
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().max(galley.size().x + 42.0), 29.0),
        Sense::click(),
    );
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            label,
        )
    });
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect,
            6,
            if selected {
                palette.accent_tint
            } else {
                palette.raised
            },
        );
    }
    let position = pos2(rect.left() + 9.0, rect.center().y - galley.size().y * 0.5);
    ui.painter().galley(position, galley, palette.text);
    let progress = ui.ctx().animate_bool(response.id.with("check"), selected);
    let x = rect.right() - 17.0;
    let y = rect.center().y;
    let start = pos2(x - 5.0, y);
    let middle = pos2(x - 1.5, y + 3.5);
    let end = pos2(x + 5.5, y - 3.5);
    if progress > 0.0 {
        let stroke = Stroke::new(1.6_f32, palette.accent);
        ui.painter().line_segment(
            [start, start.lerp(middle, (progress * 3.0).min(1.0))],
            stroke,
        );
        if progress > 0.33 {
            ui.painter()
                .line_segment([middle, middle.lerp(end, (progress - 0.33) / 0.67)], stroke);
        }
    }
    finish(ui, response, rect, 6)
}

/// Settings row: the label sits on the left and the switch is pushed to the right
/// edge, so every control in the dialog ends on the same vertical line.
pub fn switch_row(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            switch_impl(ui, value, label)
        })
        .inner
    })
    .inner
}

fn switch_impl(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let palette = Palette::get(ui);
    let (rect, mut response) = ui.allocate_exact_size(vec2(40.0, 28.0), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Checkbox, response.enabled(), *value, label)
    });
    let progress = ui
        .ctx()
        .animate_bool_with_time(response.id.with("switch"), *value, 0.3);
    let track = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(40.0, 20.0));
    let painter = ui.painter();
    let enabled = if palette.dark {
        Color32::from_rgb(235, 147, 67)
    } else {
        Color32::from_rgb(218, 124, 45)
    };
    painter.rect_filled(
        track,
        10,
        icons::lerp_color(palette.raised, enabled, progress),
    );
    let center = pos2(track.left() + 10.0 + 20.0 * progress, track.center().y);
    painter.circle_filled(center + vec2(0.7, 1.2), 9.0, Color32::from_black_alpha(95));
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, Color32::from_gray(174));
    for n in 0..=32 {
        let angle = n as f32 / 32.0 * std::f32::consts::TAU;
        let sheen = (angle * 2.0 - 0.9).cos() * 0.5 + 0.5;
        mesh.colored_vertex(
            center + 8.5 * vec2(angle.cos(), angle.sin()),
            icons::lerp_color(Color32::from_gray(102), Color32::from_gray(246), sheen),
        );
        if n > 0 {
            mesh.add_triangle(0, n, n + 1);
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.circle_stroke(
        center,
        8.0,
        Stroke::new(1.0_f32, Color32::from_white_alpha(92)),
    );
    finish(ui, response, rect, 7)
}

pub fn theme_switch(ui: &mut Ui, dark: bool, lang: Lang) -> Response {
    let palette = Palette::get(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(62.0, TOOLBAR_HEIGHT), Sense::click());
    let label = t(lang, if dark { "light_theme" } else { "dark_theme" });
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::Checkbox,
            response.enabled(),
            dark,
            t(lang, "dark_theme"),
        )
    });
    let progress = ui
        .ctx()
        .animate_bool_with_time(response.id.with("theme"), dark, 0.30);
    let painter = ui.painter();
    // A little sky: gradient panel, one sun/moon and a cloud that drifts out of
    // sight at night. Everything stays inside the rounded corners, so the chip no
    // longer needs a border to look finished.
    let day_top = Color32::from_rgb(126, 196, 236);
    let day_bottom = Color32::from_rgb(78, 152, 199);
    let night_top = Color32::from_rgb(38, 42, 74);
    let night_bottom = Color32::from_rgb(20, 23, 42);
    let top = icons::lerp_color(day_top, night_top, progress);
    let bottom = icons::lerp_color(day_bottom, night_bottom, progress);
    let (top, bottom) = if palette.glass {
        (top.gamma_multiply(0.72), bottom.gamma_multiply(0.72))
    } else {
        (top, bottom)
    };
    icons::rounded_gradient(painter, rect, 9.0, top, bottom);

    // Stars light up only once the night side takes over.
    for (x, y, size) in [
        (10.0, 10.0, 1.0),
        (22.0, 7.0, 1.2),
        (30.0, 15.0, 0.9),
        (40.0, 8.0, 1.3),
        (50.0, 13.0, 1.0),
        (56.0, 6.0, 0.8),
    ] {
        painter.circle_filled(
            rect.min + vec2(x, y),
            size,
            Color32::from_rgb(255, 249, 226).gamma_multiply(progress),
        );
    }

    // Sun by day, moon by night, sliding across the chip.
    let center = rect.min + vec2(16.0 + 30.0 * progress, rect.height() * 0.5);
    painter.circle_filled(
        center,
        13.5,
        Color32::from_white_alpha((26.0 * (1.0 - progress)) as u8),
    );
    painter.circle_filled(center + vec2(0.0, 1.0), 10.0, Color32::from_black_alpha(54));
    let sun = Color32::from_rgb(248, 202, 34);
    let moon = Color32::from_rgb(226, 224, 206);
    painter.circle_filled(center, 9.6, icons::lerp_color(sun, moon, progress));
    for (x, y, radius) in [(-3.1, 0.8, 2.8), (3.1, -3.1, 1.9), (3.5, 3.9, 1.3)] {
        painter.circle_filled(
            center + vec2(x, y),
            radius,
            Color32::from_rgb(160, 156, 140).gamma_multiply(progress),
        );
    }

    // Cloud: three puffs on a soft base, fully inside the chip.
    let cloud = ((1.0 - progress) * 255.0) as u8;
    if cloud > 4 {
        let lift = 6.0 * progress;
        let parts = [(40.0, 20.0, 6.0), (48.0, 17.5, 7.5), (55.0, 20.5, 5.5)];
        for (x, y, radius) in parts {
            painter.circle_filled(
                rect.min + vec2(x, y + lift),
                radius,
                Color32::from_rgb(247, 251, 255).gamma_multiply(cloud as f32 / 255.0),
            );
        }
        painter.rect_filled(
            Rect::from_min_max(
                rect.min + vec2(35.0, 20.0 + lift),
                rect.min + vec2(59.0, 26.0 + lift),
            ),
            egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 3,
                se: 3,
            },
            Color32::from_rgb(247, 251, 255).gamma_multiply(cloud as f32 / 255.0),
        );
    }
    // A faint white rim instead of a grey hairline: enough to lift it off the bar.
    painter.rect_stroke(
        rect,
        9,
        Stroke::new(1.0_f32, Color32::from_white_alpha(30)),
        StrokeKind::Inside,
    );
    finish(ui, response, rect, 9).on_hover_text(label)
}

pub fn favorite(
    ui: &mut Ui,
    rect: Rect,
    response: Response,
    active: bool,
    lang: Lang,
    on_image: bool,
) -> Response {
    let palette = Palette::get(ui);
    let label = t(
        lang,
        if active {
            "remove_favorite"
        } else {
            "add_favorite"
        },
    );
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Checkbox, response.enabled(), active, label)
    });
    let progress = ui.ctx().animate_bool(response.id.with("heart"), active);
    let t = hover(ui, &response);
    let pop = (std::f32::consts::PI * progress).sin() * 3.0;
    let painter = ui.painter();
    let surface = if on_image {
        Color32::from_rgba_unmultiplied(20, 23, 26, 218)
    } else {
        palette.raised
    };
    painter.rect_filled(rect, 9, surface);
    let inactive = if on_image {
        Color32::from_gray(234)
    } else {
        palette.muted
    };
    let accent = if palette.dark {
        theme::PINK
    } else {
        theme::PINK_LIGHT
    };
    let heart = Rect::from_center_size(rect.center(), Vec2::splat(18.0 + pop + t));
    icons::heart(
        painter,
        heart,
        icons::lerp_color(inactive, accent, progress),
        progress,
    );
    finish(ui, response, rect, 9).on_hover_text(label)
}

/// Chrome around the search text: margins, the icon and a little caret room.
const SEARCH_CHROME: f32 = 8.0 + 8.0 + 16.0 + 6.0 + 14.0;

/// Width the search field needs to keep `value` fully visible, clamped to the
/// given bounds.
pub fn search_width(ui: &Ui, value: &str, min: f32, max: f32) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(value.to_owned(), font, Color32::WHITE)
        .size()
        .x;
    (text + SEARCH_CHROME).clamp(min, max)
}

pub fn search(
    ui: &mut Ui,
    value: &mut String,
    hint: &str,
    min_width: f32,
    max_width: f32,
) -> Response {
    let palette = Palette::get(ui);
    let icon_size = Vec2::splat(16.0);
    let gap = 6.0;
    // The field grows leftwards as the query gets longer, so the whole text stays
    // visible until it would start pushing the rest of the toolbar.
    let width = search_width(ui, value, min_width, max_width);
    let padding = egui::Margin::symmetric(8, 0);
    let mut frame = egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, Color32::TRANSPARENT))
        .corner_radius(8)
        .inner_margin(padding)
        .begin(ui);
    frame.content_ui.set_width(width - padding.sum().x - 2.0);
    let response = frame
        .content_ui
        .with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let id = ui.make_persistent_id("gallery_search_edit");
            let has_focus = ui.memory(|memory| memory.has_focus(id));
            // The icon stays put so the field never changes shape while typing.
            let (icon, _) = ui.allocate_exact_size(icon_size, Sense::hover());
            let color = if has_focus {
                palette.accent
            } else {
                palette.muted
            };
            icons::paint(ui.painter(), Icon::Search, icon, color, 1.45);
            let available = ui.available_width();
            let request_focus = !has_focus
                && !ui.ctx().wants_keyboard_input()
                && !egui::Popup::is_any_open(ui.ctx())
                && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Slash));
            let response = ui.add_sized(
                vec2(available, TOOLBAR_HEIGHT - 2.0),
                egui::TextEdit::singleline(value)
                    .id(id)
                    .hint_text(RichText::new(hint).color(palette.muted))
                    .desired_width(available)
                    .margin(egui::Margin::ZERO)
                    .vertical_align(egui::Align::Center)
                    .frame(false),
            );
            if request_focus {
                response.request_focus();
            }
            response
        })
        .inner;
    let focus = response.has_focus();
    let hovered = ui.rect_contains_pointer(frame.frame.widget_rect(frame.content_ui.min_rect()));
    let amount =
        ui.ctx()
            .animate_bool_with_time(response.id.with("search_border"), focus || hovered, 0.14);
    let (fill, border) = toolbar_surface(palette, palette.accent, TOOLBAR_TINT, amount);
    frame.frame.fill = fill;
    frame.frame.stroke.color = if focus { palette.accent } else { border };
    frame.end(ui);
    response
}

pub fn upload_zone(ui: &mut Ui, label: &str, hint: &str, max_width: f32, height: f32) -> Response {
    let palette = Palette::get(ui);
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().min(max_width), height),
        Sense::click(),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, response.enabled(), label));
    let t = hover(ui, &response);
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        12,
        icons::lerp_color(palette.panel, palette.accent_tint, 0.12 + t * 0.5),
    );
    let border = rect.shrink(5.0);
    let stroke = Stroke::new(1.0_f32, icons::lerp_color(palette.muted, palette.accent, t));
    let corners = [
        border.left_top(),
        border.right_top(),
        border.right_bottom(),
        border.left_bottom(),
        border.left_top(),
    ];
    for edge in corners.windows(2) {
        painter.extend(Shape::dashed_line(edge, stroke, 4.0, 5.0));
    }
    let icon_center = rect.center() - vec2(0.0, 35.0 + t * 3.0);
    let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(38.0));
    painter.rect_filled(icon_rect, 10, palette.accent_tint);
    icons::paint(
        painter,
        Icon::Upload,
        icon_rect.shrink(8.0),
        palette.accent,
        1.7,
    );
    painter.text(
        rect.center() + vec2(0.0, 5.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(19.0),
        palette.text,
    );
    painter.text(
        rect.center() + vec2(0.0, 30.0),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(13.0),
        palette.muted,
    );
    finish(ui, response, rect, 12)
}

pub struct Volume {
    percent: f32,
    last_audible: f32,
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            percent: 100.0,
            last_audible: 100.0,
        }
    }
}

impl Volume {
    pub fn percent(&self) -> f32 {
        self.percent
    }

    /// Restores a persisted volume without touching the player.
    pub fn set_percent(&mut self, percent: f32) {
        self.percent = percent.clamp(0.0, 100.0);
        if self.percent > 0.0 {
            self.last_audible = self.percent;
        }
    }

    pub fn show(&mut self, ui: &mut Ui, lang: Lang, width: f32) -> Option<f32> {
        let palette = Palette::get(ui);
        let before = self.percent;
        egui::Frame::new()
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(4, 0))
            .fill(opaque(palette.panel).gamma_multiply(0.86))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    let muted = self.percent == 0.0;
                    let label = t(lang, if muted { "unmute" } else { "mute" });
                    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 28.0), Sense::click());
                    if response.clicked() {
                        self.percent = if muted { self.last_audible } else { 0.0 };
                    }
                    response.widget_info(|| {
                        WidgetInfo::selected(WidgetType::Checkbox, response.enabled(), muted, label)
                    });
                    let progress = ui
                        .ctx()
                        .animate_bool(response.id.with("mute"), self.percent == 0.0);
                    let hovered = hover(ui, &response);
                    ui.painter()
                        .rect_filled(rect, 8, palette.raised.gamma_multiply(hovered));
                    let speaker = Rect::from_center_size(rect.center(), Vec2::splat(19.0));
                    icons::speaker(ui.painter(), speaker, palette.text, 1.5, progress);
                    finish(ui, response, rect, 8).on_hover_text(label);

                    let label = t(lang, "volume");
                    ui.scope(|ui| {
                        ui.spacing_mut().slider_width = width;
                        ui.spacing_mut().slider_rail_height = 5.0;
                        ui.spacing_mut().interact_size.y = 14.0;
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Body, FontId::proportional(10.0));
                        let visuals = ui.visuals_mut();
                        visuals.selection.bg_fill = Color32::TRANSPARENT;
                        for widget in [
                            &mut visuals.widgets.inactive,
                            &mut visuals.widgets.hovered,
                            &mut visuals.widgets.active,
                        ] {
                            widget.fg_stroke = Stroke::NONE;
                            widget.bg_fill = Color32::TRANSPARENT;
                        }
                        let slider = ui.add(
                            egui::Slider::new(&mut self.percent, 0.0..=100.0)
                                .integer()
                                .trailing_fill(false)
                                .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.65 })
                                .show_value(false),
                        );
                        let rail = Rect::from_center_size(
                            slider.rect.center(),
                            vec2(slider.rect.width(), 4.0),
                        );
                        let half_width = slider.rect.height() / 2.5 * 0.65;
                        let marker = egui::lerp(
                            (rail.left() + half_width)..=(rail.right() - half_width),
                            self.percent / 100.0,
                        );
                        let painter = ui.painter();
                        let track = if palette.dark {
                            Color32::from_gray(82)
                        } else {
                            Color32::from_gray(179)
                        };
                        painter.rect_filled(rail, 2, track);
                        if self.percent > 0.0 {
                            painter.rect_filled(
                                rail.with_max_x(if self.percent >= 100.0 {
                                    rail.right()
                                } else {
                                    marker
                                }),
                                2,
                                palette.accent,
                            );
                        }
                        painter.rect_filled(
                            Rect::from_center_size(
                                pos2(marker, rail.center().y),
                                vec2(2.0 * half_width, 10.0),
                            ),
                            2,
                            palette.accent,
                        );
                        if slider.has_focus() {
                            painter.rect_stroke(
                                slider.rect,
                                3,
                                Stroke::new(1.0_f32, palette.accent),
                                StrokeKind::Inside,
                            );
                        }
                        slider.widget_info(|| {
                            WidgetInfo::slider(slider.enabled(), f64::from(self.percent), label)
                        });
                        slider.on_hover_text(label);
                    });
                    ui.add_sized(
                        [32.0, 22.0],
                        egui::Label::new(
                            theme::mono(format!("{:>3.0}%", self.percent))
                                .size(11.0)
                                .color(palette.muted),
                        ),
                    );
                })
            });
        if self.percent > 0.0 {
            self.last_audible = self.percent;
        }
        (before != self.percent).then_some(self.percent)
    }
}

fn hover(ui: &Ui, response: &Response) -> f32 {
    ui.ctx()
        .animate_bool(response.id.with("hover"), response.hovered())
}

fn finish(ui: &Ui, response: Response, rect: Rect, radius: u8) -> Response {
    if response.has_focus() {
        let palette = Palette::get(ui);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(radius),
            Stroke::new(1.5_f32, palette.accent),
            StrokeKind::Outside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_distinguishes_side_holds_from_clicks_and_drags() {
        use egui::{Event, Modifiers, PointerButton, RawInput};
        let ctx = egui::Context::default();
        let mut gesture = PreviewPress::default();
        let mut frame = |time, events| {
            let mut action = PreviewAction::None;
            let _ = ctx.run(
                RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(200.0, 100.0), Sense::click_and_drag());
                        action = gesture.update(&response, Some(rect), true);
                    });
                },
            );
            action
        };
        let button = |pos, pressed| Event::PointerButton {
            pos,
            pressed,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        let left = pos2(35.0, 50.0);
        let right = pos2(165.0, 50.0);
        frame(0.0, vec![Event::PointerMoved(left)]);
        frame(0.1, vec![button(left, true)]);
        assert_eq!(frame(0.35, vec![]), PreviewAction::Hold(PlaybackRate::Half));
        assert_eq!(
            frame(0.4, vec![button(left, false)]),
            PreviewAction::Release
        );
        frame(0.5, vec![Event::PointerMoved(right), button(right, true)]);
        assert_eq!(
            frame(0.75, vec![]),
            PreviewAction::Hold(PlaybackRate::Double)
        );
        assert_eq!(
            frame(0.8, vec![button(right, false)]),
            PreviewAction::Release
        );
        frame(0.9, vec![button(right, true)]);
        assert_eq!(frame(1.0, vec![button(right, false)]), PreviewAction::Click);
        frame(1.1, vec![button(right, true)]);
        assert_eq!(
            frame(1.15, vec![Event::PointerMoved(left)]),
            PreviewAction::Drag
        );
        assert_eq!(frame(1.4, vec![button(left, false)]), PreviewAction::None);
    }

    #[test]
    fn language_picker_opens_and_applies_the_selected_language() {
        let ctx = egui::Context::default();
        let mut language = Lang::Pl;
        let mut frame = |events| {
            ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        language_picker(ui, &mut language);
                    });
                },
            )
        };
        let click = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let _ = frame(vec![]);
        let _ = frame(click(pos2(35.0, 20.0), true));
        let _ = frame(click(pos2(35.0, 20.0), false));
        let dropdown = frame(vec![]);
        let english = dropdown
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                Shape::Text(text) if text.galley.text() == "English" => {
                    Some(text.pos + vec2(18.0, 5.0))
                }
                _ => None,
            })
            .expect("language list should be open");
        let _ = frame(click(english, true));
        let _ = frame(click(english, false));
        assert_eq!(language, Lang::En);
    }

    #[test]
    fn favorite_badge_does_not_move_neighbor_when_count_gains_digits() {
        let ctx = egui::Context::default();
        let mut positions = Vec::new();
        for count in [1, 9, 10, 99, 100] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        filter_tab(ui, "Ulubione", count, 3, false, None);
                        positions.push(filter_tab(ui, "Minecraft", 42, 2, false, None).rect.left());
                    });
                });
            });
        }
        assert!(positions.windows(2).all(|pair| pair[0] == pair[1]));
    }
}
