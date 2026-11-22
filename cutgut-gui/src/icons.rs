//! Hand-drawn vector icons, gradient helpers and small custom widgets used by
//! the gallery/editor UI (no external icon font needed).

use eframe::egui::{self, pos2, Color32, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    Pencil,
    Trash,
    Folder,
    Refresh,
    Search,
    Close,
    Scissors,
    Film,
    Image,
    Audio,
    Settings,
    Camera,
    Gallery,
    Star,
    Upload,
    Loop,
    MarkIn,
    MarkOut,
    Filter,
    Sort,
    Previous,
    Next,
    Check,
    Copy,
    Fullscreen,
    FullscreenExit,
    Calendar,
    Clock,
    Text,
    Storage,
    Ascending,
    Descending,
    BarChart,
    PieChart,
    Heart,
    HeartOutline,
    Warning,
}

/// Rotates `point` around `center`, so a glyph can tilt its own parts.
fn rotated(point: Pos2, center: Pos2, angle: f32) -> Pos2 {
    let (sin, cos) = angle.sin_cos();
    let d = point - center;
    center + egui::vec2(d.x * cos - d.y * sin, d.x * sin + d.y * cos)
}

/// Paints an icon with a hover animation. `hover` runs from 0 (rest) to 1
/// (fully hovered) and each glyph has its own small move: the folder lifts and
/// shows its contents, the bin lid swings open, the play head steps forward,
/// the scissors open, the sheets separate, the pencil wiggles.
pub fn paint_animated(
    painter: &egui::Painter,
    icon: Icon,
    rect: Rect,
    color: Color32,
    width: f32,
    hover: f32,
) {
    if hover <= 0.01 {
        paint(painter, icon, rect, color, width);
        return;
    }
    let ease = 1.0 - (1.0 - hover).powi(2);
    // Every glyph grows a hair, which makes the button feel alive on its own.
    let rect = Rect::from_center_size(rect.center(), rect.size() * (1.0 + 0.07 * ease));
    let p = |x: f32, y: f32| {
        Pos2::new(
            rect.left() + x * rect.width(),
            rect.top() + y * rect.height(),
        )
    };
    let stroke = Stroke::new(width, color);
    let gentle = Stroke::new(width * 0.9, color.gamma_multiply(0.55 + 0.45 * ease));
    match icon {
        Icon::Folder => {
            // The folder rises and its contents slide into view.
            let lift = 0.04 * ease;
            let shifted = rect.translate(egui::vec2(0.0, -lift * rect.height()));
            paint(painter, Icon::Folder, shifted, color, width);
            painter.line_segment([p(0.27, 0.52), p(0.73, 0.52)], gentle);
            painter.line_segment([p(0.27, 0.63), p(0.60, 0.63)], gentle);
        }
        Icon::Trash => {
            // The lid swings open around its left corner.
            let left = p(0.20, 0.28 - 0.075 * ease);
            let angle = -0.20 * ease;
            let right = rotated(p(0.80, 0.28 - 0.075 * ease), left, angle);
            painter.line_segment([left, right], stroke);
            painter.line_segment(
                [
                    rotated(p(0.40, 0.20 - 0.075 * ease), left, angle),
                    rotated(p(0.60, 0.20 - 0.075 * ease), left, angle),
                ],
                Stroke::new(width * 1.2, color),
            );
            painter.add(Shape::line(
                vec![p(0.28, 0.32), p(0.30, 0.82), p(0.70, 0.82), p(0.72, 0.32)],
                stroke,
            ));
            painter.line_segment([p(0.42, 0.42), p(0.42, 0.72)], gentle);
            painter.line_segment([p(0.58, 0.42), p(0.58, 0.72)], gentle);
        }
        Icon::Play => {
            // Play head steps forward, as if the clip already started.
            let dx = 0.05 * ease;
            painter.add(Shape::convex_polygon(
                vec![p(0.32 + dx, 0.18), p(0.82 + dx, 0.5), p(0.32 + dx, 0.82)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Scissors => {
            // Blades open around the hinge.
            let pivot = p(0.36, 0.5);
            let open = 0.16 * ease;
            painter.circle_stroke(p(0.26, 0.28), rect.width() * 0.10, stroke);
            painter.circle_stroke(p(0.26, 0.72), rect.width() * 0.10, stroke);
            painter.line_segment([p(0.36, 0.36), rotated(p(0.82, 0.78), pivot, open)], stroke);
            painter.line_segment(
                [p(0.36, 0.64), rotated(p(0.82, 0.22), pivot, -open)],
                stroke,
            );
        }
        Icon::Pencil => {
            // A small nod, hinged at the tip.
            let tip = p(0.22, 0.82);
            let angle = 0.14 * ease;
            painter.add(Shape::convex_polygon(
                vec![
                    rotated(p(0.30, 0.70), tip, angle),
                    rotated(p(0.72, 0.28), tip, angle),
                    rotated(p(0.80, 0.36), tip, angle),
                    rotated(p(0.38, 0.78), tip, angle),
                ],
                color,
                Stroke::NONE,
            ));
            painter.add(Shape::convex_polygon(
                vec![
                    rotated(p(0.22, 0.82), tip, angle),
                    rotated(p(0.26, 0.68), tip, angle),
                    rotated(p(0.34, 0.78), tip, angle),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Copy => {
            // The two sheets pull apart.
            let spread = 0.035 * ease;
            painter.rect_stroke(
                Rect::from_min_max(
                    p(0.16 - spread, 0.16 - spread),
                    p(0.66 - spread, 0.66 - spread),
                ),
                egui::CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    p(0.34 + spread, 0.34 + spread),
                    p(0.84 + spread, 0.84 + spread),
                ),
                egui::CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Upload => {
            let up = 0.05 * ease;
            painter.add(Shape::line(
                vec![p(0.14, 0.61), p(0.14, 0.84), p(0.86, 0.84), p(0.86, 0.61)],
                stroke,
            ));
            painter.line_segment([p(0.5, 0.15 - up), p(0.5, 0.67 - up)], stroke);
            painter.add(Shape::line(
                vec![p(0.30, 0.35 - up), p(0.5, 0.15 - up), p(0.70, 0.35 - up)],
                stroke,
            ));
        }
        Icon::Fullscreen => {
            // The frame lets go: brackets step outwards.
            let out = 0.035 * ease;
            for (x, y) in [(0.15, 0.15), (0.85, 0.15), (0.15, 0.85), (0.85, 0.85)] {
                let dx = if x < 0.5 { -out } else { out };
                let dy = if y < 0.5 { -out } else { out };
                let corner = p(x + dx, y + dy);
                let (arm_x, arm_y) = if x < 0.5 { (0.23, 0.0) } else { (-0.23, 0.0) };
                let (down_x, down_y) = if y < 0.5 { (0.0, 0.23) } else { (0.0, -0.23) };
                painter.line_segment([corner, p(x + dx + arm_x, y + dy + arm_y)], stroke);
                painter.line_segment([corner, p(x + dx + down_x, y + dy + down_y)], stroke);
            }
        }
        Icon::Previous => {
            painter.add(Shape::line(
                vec![
                    p(0.62 - 0.05 * ease, 0.22),
                    p(0.36 - 0.05 * ease, 0.5),
                    p(0.62 - 0.05 * ease, 0.78),
                ],
                stroke,
            ));
        }
        Icon::Next => {
            painter.add(Shape::line(
                vec![
                    p(0.38 + 0.05 * ease, 0.22),
                    p(0.64 + 0.05 * ease, 0.5),
                    p(0.38 + 0.05 * ease, 0.78),
                ],
                stroke,
            ));
        }
        Icon::Refresh => refresh(painter, rect, color, width, ease * 0.85),
        _ => paint(painter, icon, rect, color, width),
    }
}

pub fn paint(painter: &egui::Painter, icon: Icon, rect: Rect, color: Color32, width: f32) {
    let p = |x: f32, y: f32| {
        Pos2::new(
            rect.left() + x * rect.width(),
            rect.top() + y * rect.height(),
        )
    };
    let rr = |x1: f32, y1: f32, x2: f32, y2: f32| Rect::from_min_max(p(x1, y1), p(x2, y2));
    let stroke = Stroke::new(width, color);
    let cr = ((rect.width() * 0.08).round() as u8).max(1);
    let center = p(0.5, 0.5);

    match icon {
        Icon::Sort => {
            painter.line_segment([p(0.30, 0.78), p(0.30, 0.20)], stroke);
            painter.add(Shape::line(
                vec![p(0.14, 0.37), p(0.30, 0.20), p(0.46, 0.37)],
                stroke,
            ));
            painter.line_segment([p(0.70, 0.22), p(0.70, 0.80)], stroke);
            painter.add(Shape::line(
                vec![p(0.54, 0.63), p(0.70, 0.80), p(0.86, 0.63)],
                stroke,
            ));
        }
        Icon::Audio => {
            for (x, top, bottom) in [
                (0.17, 0.43, 0.57),
                (0.34, 0.29, 0.71),
                (0.5, 0.15, 0.85),
                (0.66, 0.33, 0.67),
                (0.83, 0.42, 0.58),
            ] {
                painter.line_segment([p(x, top), p(x, bottom)], stroke);
            }
        }
        Icon::Image => {
            painter.rect_stroke(
                rr(0.12, 0.17, 0.88, 0.83),
                cr,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.circle_filled(p(0.34, 0.38), rect.width() * 0.07, color);
            painter.add(Shape::line(
                vec![
                    p(0.15, 0.75),
                    p(0.42, 0.53),
                    p(0.56, 0.66),
                    p(0.70, 0.48),
                    p(0.85, 0.64),
                ],
                stroke,
            ));
        }
        Icon::Fullscreen => {
            // Wide brackets sitting on the corners, arms pointing inwards.
            for (x, y) in [(0.15, 0.15), (0.85, 0.15), (0.15, 0.85), (0.85, 0.85)] {
                let dx = if x < 0.5 { 0.23 } else { -0.23 };
                let dy = if y < 0.5 { 0.23 } else { -0.23 };
                painter.line_segment([p(x, y), p(x + dx, y)], stroke);
                painter.line_segment([p(x, y), p(x, y + dy)], stroke);
            }
        }
        Icon::FullscreenExit => {
            // The same brackets folded back in towards the middle.
            for (x, y) in [(0.34, 0.34), (0.66, 0.34), (0.34, 0.66), (0.66, 0.66)] {
                let dx = if x < 0.5 { -0.15 } else { 0.15 };
                let dy = if y < 0.5 { -0.15 } else { 0.15 };
                painter.line_segment([p(x, y), p(x + dx, y)], stroke);
                painter.line_segment([p(x, y), p(x, y + dy)], stroke);
            }
        }
        Icon::Calendar => {
            painter.rect_stroke(rr(0.16, 0.24, 0.84, 0.84), cr, stroke, StrokeKind::Inside);
            painter.line_segment([p(0.16, 0.40), p(0.84, 0.40)], stroke);
            painter.line_segment([p(0.32, 0.14), p(0.32, 0.28)], stroke);
            painter.line_segment([p(0.68, 0.14), p(0.68, 0.28)], stroke);
        }
        Icon::Clock => {
            painter.circle_stroke(center, rect.width() * 0.33, stroke);
            painter.line_segment([center, p(0.5, 0.28)], stroke);
            painter.line_segment([center, p(0.66, 0.56)], stroke);
        }
        Icon::Text => {
            painter.line_segment([p(0.18, 0.30), p(0.82, 0.30)], stroke);
            painter.line_segment([p(0.18, 0.50), p(0.82, 0.50)], stroke);
            painter.line_segment([p(0.18, 0.70), p(0.62, 0.70)], stroke);
        }
        Icon::Storage => {
            painter.rect_stroke(rr(0.14, 0.30, 0.86, 0.70), cr, stroke, StrokeKind::Inside);
            painter.circle_filled(p(0.74, 0.50), rect.width() * 0.035, color);
        }
        Icon::Ascending | Icon::Descending => {
            let up = matches!(icon, Icon::Ascending);
            let (tail, head) = if up { (0.76, 0.22) } else { (0.22, 0.76) };
            painter.line_segment([p(0.30, tail), p(0.30, head)], stroke);
            let dir = if up { -1.0 } else { 1.0 };
            painter.add(Shape::line(
                vec![
                    p(0.18, head + 0.13 * dir),
                    p(0.30, head),
                    p(0.42, head + 0.13 * dir),
                ],
                stroke,
            ));
            for (index, width) in [0.34_f32, 0.52, 0.70].into_iter().enumerate() {
                let y = 0.34 + index as f32 * 0.15;
                painter.line_segment([p(0.56, y), p(0.56 + width * 0.36, y)], stroke);
            }
        }
        Icon::BarChart => {
            for (x, top) in [(0.26_f32, 0.56_f32), (0.5, 0.36), (0.74, 0.46)] {
                painter.rect_filled(rr(x - 0.075, top, x + 0.075, 0.82), cr, color);
            }
        }
        Icon::PieChart => {
            painter.circle_stroke(center, rect.width() * 0.32, stroke);
            // A filled quarter, from twelve o'clock to three o'clock.
            let r = rect.width() * 0.30;
            let mut points = vec![center];
            for step in 0..=8 {
                let angle = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::FRAC_PI_2 * (step as f32 / 8.0);
                points.push(center + Vec2::new(angle.cos(), angle.sin()) * r);
            }
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
        Icon::Previous | Icon::Next => {
            let (outer, tip) = if matches!(icon, Icon::Previous) {
                (0.63, 0.35)
            } else {
                (0.37, 0.65)
            };
            painter.add(Shape::line(
                vec![p(outer, 0.22), p(tip, 0.50), p(outer, 0.78)],
                stroke,
            ));
        }
        Icon::Loop => {
            painter.add(Shape::line(
                vec![p(0.18, 0.43), p(0.18, 0.34), p(0.28, 0.25), p(0.82, 0.25)],
                stroke,
            ));
            painter.add(Shape::line(
                vec![p(0.67, 0.10), p(0.82, 0.25), p(0.67, 0.40)],
                stroke,
            ));
            painter.add(Shape::line(
                vec![p(0.82, 0.57), p(0.82, 0.66), p(0.72, 0.75), p(0.18, 0.75)],
                stroke,
            ));
            painter.add(Shape::line(
                vec![p(0.33, 0.60), p(0.18, 0.75), p(0.33, 0.90)],
                stroke,
            ));
        }
        Icon::MarkIn | Icon::MarkOut => {
            let flip = |x| {
                if matches!(icon, Icon::MarkIn) {
                    x
                } else {
                    1.0 - x
                }
            };
            let m = |x, y| p(flip(x), y);
            painter.add(Shape::line(
                vec![m(0.43, 0.18), m(0.24, 0.18), m(0.24, 0.82), m(0.43, 0.82)],
                stroke,
            ));
            painter.line_segment([m(0.38, 0.50), m(0.80, 0.50)], stroke);
            painter.add(Shape::line(
                vec![m(0.64, 0.34), m(0.80, 0.50), m(0.64, 0.66)],
                stroke,
            ));
        }
        Icon::Filter => {
            for (y, left, right) in [(0.25, 0.18, 0.82), (0.50, 0.29, 0.71), (0.75, 0.42, 0.58)] {
                painter.line_segment([p(left, y), p(right, y)], stroke);
            }
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![p(0.32, 0.18), p(0.82, 0.5), p(0.32, 0.82)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            painter.rect_filled(
                rr(0.30, 0.2, 0.44, 0.8),
                egui::CornerRadius::same(cr),
                color,
            );
            painter.rect_filled(
                rr(0.56, 0.2, 0.70, 0.8),
                egui::CornerRadius::same(cr),
                color,
            );
        }
        Icon::Pencil => {
            painter.add(Shape::convex_polygon(
                vec![p(0.30, 0.70), p(0.72, 0.28), p(0.80, 0.36), p(0.38, 0.78)],
                color,
                Stroke::NONE,
            ));
            painter.add(Shape::convex_polygon(
                vec![p(0.22, 0.82), p(0.26, 0.68), p(0.34, 0.78)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Trash => {
            painter.line_segment([p(0.20, 0.28), p(0.80, 0.28)], stroke);
            painter.line_segment(
                [p(0.40, 0.20), p(0.60, 0.20)],
                Stroke::new(width * 1.2, color),
            );
            painter.add(Shape::line(
                vec![p(0.28, 0.32), p(0.30, 0.82), p(0.70, 0.82), p(0.72, 0.32)],
                stroke,
            ));
            painter.line_segment(
                [p(0.42, 0.42), p(0.42, 0.72)],
                Stroke::new(width * 0.8, color),
            );
            painter.line_segment(
                [p(0.58, 0.42), p(0.58, 0.72)],
                Stroke::new(width * 0.8, color),
            );
        }
        Icon::Folder => {
            painter.add(Shape::line(
                vec![
                    p(0.16, 0.74),
                    p(0.16, 0.28),
                    p(0.42, 0.28),
                    p(0.50, 0.38),
                    p(0.84, 0.38),
                    p(0.84, 0.74),
                    p(0.16, 0.74),
                ],
                stroke,
            ));
        }
        Icon::Refresh => refresh(painter, rect, color, width, 0.0),
        Icon::Search => {
            painter.circle_stroke(p(0.42, 0.42), rect.width() * 0.24, stroke);
            painter.line_segment([p(0.60, 0.60), p(0.82, 0.82)], stroke);
        }
        Icon::Warning => {
            // Triangle with an exclamation: the one glyph every tool needs.
            painter.add(Shape::closed_line(
                vec![p(0.50, 0.14), p(0.90, 0.84), p(0.10, 0.84)],
                stroke,
            ));
            painter.line_segment([p(0.50, 0.40), p(0.50, 0.62)], stroke);
            painter.line_segment([p(0.50, 0.72), p(0.50, 0.735)], stroke);
        }
        Icon::Close => {
            painter.line_segment([p(0.26, 0.26), p(0.74, 0.74)], stroke);
            painter.line_segment([p(0.74, 0.26), p(0.26, 0.74)], stroke);
        }
        Icon::Copy => {
            // Two offset sheets, the classic "copy" mark.
            painter.rect_stroke(
                Rect::from_min_max(p(0.16, 0.16), p(0.66, 0.66)),
                egui::CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.34, 0.34), p(0.84, 0.84)),
                egui::CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Check => {
            painter.add(Shape::line(
                vec![p(0.22, 0.52), p(0.42, 0.72), p(0.78, 0.30)],
                stroke,
            ));
        }
        Icon::Star => {
            let mut points = Vec::new();
            for i in 0..10 {
                let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let radius = if i % 2 == 0 { 0.42 } else { 0.18 };
                points.push(Pos2::new(
                    center.x + rect.width() * radius * angle.cos(),
                    center.y + rect.width() * radius * angle.sin(),
                ));
            }
            fan(painter, center, &points, color);
        }
        Icon::Heart => heart(painter, rect, color, 1.0),
        Icon::HeartOutline => heart(painter, rect, color, 0.0),
        Icon::Upload => {
            painter.add(Shape::line(
                vec![p(0.14, 0.61), p(0.14, 0.84), p(0.86, 0.84), p(0.86, 0.61)],
                stroke,
            ));
            painter.line_segment([p(0.5, 0.15), p(0.5, 0.67)], stroke);
            painter.add(Shape::line(
                vec![p(0.30, 0.35), p(0.5, 0.15), p(0.70, 0.35)],
                stroke,
            ));
        }
        Icon::Gallery => {
            for (x, y) in [(0.18, 0.18), (0.54, 0.18), (0.18, 0.54), (0.54, 0.54)] {
                painter.rect_stroke(
                    rr(x, y, x + 0.28, y + 0.28),
                    egui::CornerRadius::same(cr),
                    stroke,
                    egui::StrokeKind::Inside,
                );
                painter.circle_filled(p(x + 0.08, y + 0.08), rect.width() * 0.02, color);
            }
        }
        Icon::Scissors => {
            painter.circle_stroke(p(0.26, 0.28), rect.width() * 0.10, stroke);
            painter.circle_stroke(p(0.26, 0.72), rect.width() * 0.10, stroke);
            painter.line_segment([p(0.36, 0.36), p(0.82, 0.78)], stroke);
            painter.line_segment([p(0.36, 0.64), p(0.82, 0.22)], stroke);
        }
        Icon::Film => {
            painter.rect_stroke(
                rr(0.16, 0.24, 0.84, 0.76),
                egui::CornerRadius::same(cr),
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [p(0.34, 0.24), p(0.34, 0.76)],
                Stroke::new(width * 0.8, color),
            );
            painter.line_segment(
                [p(0.66, 0.24), p(0.66, 0.76)],
                Stroke::new(width * 0.8, color),
            );
            for i in 0..3 {
                let y = 0.32 + i as f32 * 0.14;
                painter.line_segment([p(0.34, y), p(0.66, y)], Stroke::new(width * 0.7, color));
            }
        }
        Icon::Settings => {
            let tooth_angle = std::f32::consts::TAU / 8.0;
            let outline = (0..8)
                .flat_map(|i| {
                    [(-0.40, 0.33), (-0.24, 0.44), (0.24, 0.44), (0.40, 0.33)].map(
                        move |(turn, radius)| {
                            let angle = (i as f32 + turn) * tooth_angle;
                            p(0.5 + radius * angle.cos(), 0.5 + radius * angle.sin())
                        },
                    )
                })
                .collect();
            painter.add(Shape::closed_line(outline, stroke));
            painter.circle_stroke(center, rect.width() * 0.15, stroke);
        }
        Icon::Camera => {
            painter.add(Shape::closed_line(
                vec![
                    p(0.12, 0.36),
                    p(0.19, 0.29),
                    p(0.33, 0.29),
                    p(0.40, 0.18),
                    p(0.60, 0.18),
                    p(0.67, 0.29),
                    p(0.81, 0.29),
                    p(0.88, 0.36),
                    p(0.88, 0.74),
                    p(0.81, 0.81),
                    p(0.19, 0.81),
                    p(0.12, 0.74),
                ],
                stroke,
            ));
            painter.circle_stroke(p(0.5, 0.55), rect.width() * 0.15, stroke);
        }
    }
}

pub fn heart(painter: &egui::Painter, rect: Rect, color: Color32, fill: f32) {
    let center = rect.center();
    let points: Vec<_> = (0..40)
        .map(|step| {
            let angle = step as f32 / 40.0 * std::f32::consts::TAU;
            let x = angle.sin().powi(3) * 16.0;
            let y = 13.0 * angle.cos()
                - 5.0 * (2.0 * angle).cos()
                - 2.0 * (3.0 * angle).cos()
                - (4.0 * angle).cos();
            center + egui::vec2(x, -y - 2.0) * (rect.width() / 38.0)
        })
        .collect();
    if fill > 0.0 {
        fan(painter, center, &points, color.gamma_multiply(fill));
    }
    painter.add(Shape::closed_line(points, Stroke::new(1.45_f32, color)));
}

fn fan(painter: &egui::Painter, center: Pos2, points: &[Pos2], color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color);
    for &point in points {
        mesh.colored_vertex(point, color);
    }
    for i in 0..points.len() as u32 {
        mesh.add_triangle(0, i + 1, (i + 1) % points.len() as u32 + 1);
    }
    painter.add(Shape::mesh(mesh));
}

pub fn refresh(painter: &egui::Painter, rect: Rect, color: Color32, width: f32, progress: f32) {
    let center = rect.center();
    let radius = rect.width() * 0.31;
    let start = 0.15 - progress * std::f32::consts::PI * 1.5;
    let sweep = std::f32::consts::TAU * 0.83;
    let on_arc = |angle: f32| center + egui::vec2(angle.cos(), angle.sin()) * radius;
    let points: Vec<_> = (0..=24)
        .map(|i| on_arc(start + sweep * i as f32 / 24.0))
        .collect();
    let end = start + sweep;
    let tip = on_arc(end);
    let tangent = egui::vec2(-end.sin(), end.cos());
    let radial = egui::vec2(end.cos(), end.sin());
    let stroke = Stroke::new(width, color);
    let base = tip - tangent * radius * 0.55;
    painter.add(Shape::line(points, stroke));
    painter.add(Shape::line(
        vec![
            base + radial * radius * 0.43,
            tip,
            base - radial * radius * 0.43,
        ],
        stroke,
    ));
}

pub fn speaker(painter: &egui::Painter, rect: Rect, color: Color32, width: f32, muted: f32) {
    let p = |x, y| {
        pos2(
            rect.left() + rect.width() * x,
            rect.top() + rect.height() * y,
        )
    };
    painter.add(Shape::closed_line(
        vec![
            p(0.12, 0.38),
            p(0.29, 0.38),
            p(0.48, 0.23),
            p(0.48, 0.77),
            p(0.29, 0.62),
            p(0.12, 0.62),
        ],
        Stroke::new(width, color),
    ));
    for radius in [0.18, 0.32] {
        let points = (0..=12)
            .map(|step| {
                let angle = -0.91_f32 + 1.82 * step as f32 / 12.0;
                p(0.53 + radius * angle.cos(), 0.5 + radius * angle.sin())
            })
            .collect();
        if muted < 1.0 {
            painter.add(Shape::line(
                points,
                Stroke::new(width, color.gamma_multiply(1.0 - muted)),
            ));
        }
    }
    if muted > 0.0 {
        painter.line_segment(
            [p(0.16, 0.14), p(0.16, 0.14).lerp(p(0.84, 0.86), muted)],
            Stroke::new(width, color),
        );
    }
}

pub fn rounded_gradient(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    top: Color32,
    bottom: Color32,
) {
    let mut mesh = egui::Mesh::default();
    let color_at = |y: f32| lerp_color(top, bottom, (y - rect.top()) / rect.height());
    let centers = [
        rect.right_bottom() - egui::vec2(radius, radius),
        rect.left_bottom() + egui::vec2(radius, -radius),
        rect.left_top() + egui::vec2(radius, radius),
        rect.right_top() + egui::vec2(-radius, radius),
    ];
    mesh.colored_vertex(rect.center(), color_at(rect.center().y));
    for (corner, center) in centers.into_iter().enumerate() {
        for step in 0..=5 {
            let angle = (corner as f32 + step as f32 / 5.0) * std::f32::consts::FRAC_PI_2;
            let point = center + egui::vec2(angle.cos(), angle.sin()) * radius;
            mesh.colored_vertex(point, color_at(point.y));
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for i in 0..count {
        mesh.add_triangle(0, i + 1, (i + 1) % count + 1);
    }
    painter.add(Shape::mesh(mesh));
}

pub fn radial_glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    let mut mesh = egui::Mesh::default();
    let segments = 48;
    let rings = 9;
    mesh.colored_vertex(center, color);
    for ring in 1..=rings {
        let distance = ring as f32 / rings as f32;
        let opacity = ((-4.5 * distance * distance).exp() - (-4.5_f32).exp()).max(0.0);
        let tint = color.gamma_multiply(opacity);
        for i in 0..segments {
            let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(
                center + egui::vec2(angle.cos(), angle.sin()) * radius * distance,
                tint,
            );
        }
        let start = 1 + (ring - 1) * segments;
        for i in 0..segments {
            let next = (i + 1) % segments;
            if ring == 1 {
                mesh.add_triangle(0, start + i, start + next);
            } else {
                let inner = start - segments;
                mesh.add_triangle(inner + i, start + i, start + next);
                mesh.add_triangle(inner + i, start + next, inner + next);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}
