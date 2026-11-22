use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui::{self, pos2, Align2, Color32, FontId, Rect, Stroke, TextureHandle, Vec2};

use crate::{settings, steam_icons, theme};

const ART: &[(&str, &[u8])] = &[
    ("Aimlabs", include_bytes!("../assets/games/aimlabs.png")),
    ("Among Us", include_bytes!("../assets/games/among-us.png")),
    ("ASKA", include_bytes!("../assets/games/aska.png")),
    (
        "Counter-Strike 2",
        include_bytes!("../assets/games/counter-strike-2.png"),
    ),
    (
        "How to Fish",
        include_bytes!("../assets/games/how-to-fish.png"),
    ),
    ("Minecraft", include_bytes!("../assets/games/minecraft.png")),
    (
        "Overwatch 2",
        include_bytes!("../assets/games/overwatch-2.png"),
    ),
    ("Paladins", include_bytes!("../assets/games/paladins.png")),
    (
        "RV There Yet?",
        include_bytes!("../assets/games/rv-there-yet.png"),
    ),
    (
        "Shift At Midnight",
        include_bytes!("../assets/games/shift-at-midnight.png"),
    ),
    ("Valorant", include_bytes!("../assets/games/valorant.png")),
];

pub fn game_name(folder: &str) -> &str {
    let folder_key = folder.trim().to_ascii_lowercase();
    match folder_key.as_str() {
        "minecraft" | "base profile" => "Minecraft",
        _ if folder_key.starts_with("java-runtime-") => "Minecraft",
        "aim lab" | "aimlabs" => "Aimlabs",
        "among us" => "Among Us",
        "aska" | "aska.exe" => "ASKA",
        "counter-strike 2" | "counter strike 2" | "cs2" => "Counter-Strike 2",
        "how to fish" => "How to Fish",
        "valorant" => "Valorant",
        "overwatch" | "overwatch 2" => "Overwatch 2",
        "paladins" => "Paladins",
        "rv there yet" | "rv there yet?" => "RV There Yet?",
        "shift at midnight" => "Shift At Midnight",
        "desktop" => "Desktop",
        _ => folder,
    }
}

pub struct GameIcons {
    textures: HashMap<String, GameArt>,
    steam_textures: HashMap<String, GameArt>,
    pending: HashSet<String>,
    requests: Sender<String>,
    results: Receiver<(String, Option<egui::ColorImage>)>,
}

struct GameArt {
    texture: TextureHandle,
    color: Color32,
}

impl GameArt {
    fn new(ctx: &egui::Context, name: &str, image: egui::ColorImage) -> Self {
        Self {
            color: dominant_color(&image.pixels),
            texture: ctx.load_texture(name, image, egui::TextureOptions::LINEAR),
        }
    }
}

impl GameIcons {
    pub fn new(ctx: &egui::Context) -> Self {
        let textures = ART
            .iter()
            .filter_map(|&(name, png)| {
                let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
                    .ok()?
                    .to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let pixels = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                Some((name.to_owned(), GameArt::new(ctx, name, pixels)))
            })
            .collect();
        let (requests, queued) = mpsc::channel::<String>();
        let (completed, results) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let cache = settings::config_dir().join("steam-icons");
            for name in queued {
                let icon = steam_icons::load(&name, &cache);
                if completed.send((name, icon)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self {
            textures,
            steam_textures: HashMap::new(),
            pending: HashSet::new(),
            requests,
            results,
        }
    }

    pub fn request_steam<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        for name in names {
            if name != "Desktop"
                && !self.steam_textures.contains_key(name)
                && self.pending.insert(name.to_owned())
                && self.requests.send(name.to_owned()).is_err()
            {
                self.pending.remove(name);
            }
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((name, icon)) = self.results.try_recv() {
            self.pending.remove(&name);
            if let Some(icon) = icon {
                let art = GameArt::new(ctx, &name, icon);
                self.steam_textures.insert(name, art);
            }
        }
    }

    fn art(&self, name: &str) -> Option<&GameArt> {
        self.steam_textures
            .get(name)
            .or_else(|| self.textures.get(name))
    }

    pub fn color(&self, name: &str) -> Option<Color32> {
        if name == "Desktop" {
            return Some(fallback_color(name));
        }
        self.art(name).map(|art| art.color)
    }

    pub fn paint(&self, painter: &egui::Painter, name: &str, rect: Rect) {
        if let Some(art) = self.art(name) {
            theme::rounded_image(
                painter,
                rect,
                egui::CornerRadius::same(4),
                art.texture.id(),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            );
        } else {
            fallback(painter, name, rect);
        }
    }
}

fn dominant_color(pixels: &[Color32]) -> Color32 {
    let mut hues = [[0.0_f32; 4]; 12];
    let mut neutral = [0.0_f32; 4];
    for pixel in pixels {
        let [r, g, b, a] = pixel.to_srgba_unmultiplied().map(f32::from);
        if a < 64.0 {
            continue;
        }
        let max = r.max(g).max(b);
        let chroma = max - r.min(g).min(b);
        let (bucket, weight) = if chroma >= 24.0 && max >= 40.0 {
            let hue = if max == r {
                (g - b) / chroma
            } else if max == g {
                2.0 + (b - r) / chroma
            } else {
                4.0 + (r - g) / chroma
            };
            let index = (2.0 * hue).round().rem_euclid(12.0) as usize;
            (&mut hues[index], a * chroma / 255.0)
        } else {
            (&mut neutral, a)
        };
        bucket[0] += r * weight;
        bucket[1] += g * weight;
        bucket[2] += b * weight;
        bucket[3] += weight;
    }
    let [r, g, b, weight] = hues
        .into_iter()
        .filter(|bucket| bucket[3] > 0.0)
        .max_by(|a, b| a[3].total_cmp(&b[3]))
        .unwrap_or(neutral);
    if weight == 0.0 {
        return Color32::GRAY;
    }
    Color32::from_rgb(
        (r / weight).round() as u8,
        (g / weight).round() as u8,
        (b / weight).round() as u8,
    )
}

fn fallback_color(name: &str) -> Color32 {
    let palette = [
        Color32::from_rgb(54, 70, 95),
        Color32::from_rgb(75, 61, 100),
        Color32::from_rgb(45, 89, 80),
        Color32::from_rgb(100, 63, 53),
    ];
    let index = name.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(usize::from(byte))
    }) % palette.len();
    palette[index]
}

fn fallback(painter: &egui::Painter, name: &str, rect: Rect) {
    painter.rect_filled(rect, 4, fallback_color(name));

    if name == "Desktop" {
        let p = |x, y| {
            pos2(
                rect.left() + x * rect.width(),
                rect.top() + y * rect.height(),
            )
        };
        let stroke = Stroke::new(1.35_f32, Color32::WHITE);
        painter.rect_stroke(
            Rect::from_min_max(p(0.19, 0.23), p(0.81, 0.64)),
            2,
            stroke,
            egui::StrokeKind::Inside,
        );
        painter.line_segment([p(0.39, 0.76), p(0.61, 0.76)], stroke);
    } else if let Some(initial) = name.chars().next() {
        painter.text(
            rect.center() + Vec2::new(0.0, -0.5),
            Align2::CENTER_CENTER,
            initial.to_uppercase().to_string(),
            FontId::proportional(rect.height() * 0.58),
            Color32::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognized_games_have_embedded_art() {
        let icons = GameIcons::new(&egui::Context::default());
        assert_eq!(icons.textures.len(), ART.len());
        for folder in [
            "Aim Lab",
            "Among Us",
            "Aska.exe",
            "Base Profile",
            "Counter-strike 2",
            "How to Fish",
            "java-runtime-epsilon",
            "Minecraft",
            "Overwatch 2",
            "Paladins",
            "RV There Yet",
            "Shift At Midnight",
            "Valorant",
        ] {
            let art = &icons.textures[game_name(folder)];
            assert_eq!(art.texture.size(), [64, 64]);
        }
    }

    #[test]
    fn hue_prefers_the_visible_accent_over_transparency_and_gray() {
        let mut pixels = vec![Color32::TRANSPARENT; 25];
        pixels.extend([Color32::from_gray(30); 20]);
        let accent = Color32::from_rgb(255, 65, 80);
        pixels.extend([accent; 5]);
        assert_eq!(dominant_color(&pixels), accent);
    }
}
