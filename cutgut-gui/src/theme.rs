use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, FontFamily, FontId, Frame, Rect, RichText, Stroke,
    TextStyle, Ui, Visuals,
};
use std::{
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    time::Instant,
};

pub const GOLD: Color32 = Color32::from_rgb(240, 194, 109);
/// Favourite pink, used wherever a clip is starred.
pub const PINK: Color32 = Color32::from_rgb(255, 122, 163);
/// Softer pink that keeps the same hue readable on light themes.
pub const PINK_LIGHT: Color32 = Color32::from_rgb(214, 69, 125);

pub struct Reveal {
    previous: egui::TextureHandle,
    origin: egui::Pos2,
    started: Instant,
}

impl Reveal {
    pub fn new(
        ctx: &egui::Context,
        previous: std::sync::Arc<egui::ColorImage>,
        origin: egui::Pos2,
    ) -> Self {
        Self {
            previous: ctx.load_texture("previous_theme", previous, egui::TextureOptions::LINEAR),
            origin,
            started: Instant::now(),
        }
    }

    pub fn paint(&self, ctx: &egui::Context) -> bool {
        let progress = self.started.elapsed().as_secs_f32() / 0.32;
        if progress >= 1.0 {
            return false;
        }
        self.paint_frame(ctx, progress);
        ctx.request_repaint();
        true
    }

    fn paint_frame(&self, ctx: &egui::Context, progress: f32) {
        let rect = ctx.viewport_rect();
        let origin = rect.clamp(self.origin);
        let reach = [
            rect.left_top(),
            rect.right_top(),
            rect.left_bottom(),
            rect.right_bottom(),
        ]
        .into_iter()
        .map(|corner| corner.distance(origin))
        .fold(0.0, f32::max);
        let feather = (reach * 0.16).clamp(60.0, 220.0);
        let eased = progress * (2.0 - progress);
        let radius = egui::lerp(-feather..=(reach + feather), eased);
        let cols = (rect.width() / 26.0).ceil().clamp(1.0, 128.0) as u32;
        let rows = (rect.height() / 26.0).ceil().clamp(1.0, 128.0) as u32;
        let mut mesh = egui::Mesh::with_texture(self.previous.id());
        for y in 0..=rows {
            for x in 0..=cols {
                let uv = pos2(x as f32 / cols as f32, y as f32 / rows as f32);
                let pos = rect.min + vec2(uv.x * rect.width(), uv.y * rect.height());
                let fade =
                    ((pos.distance(origin) - radius + feather) / (2.0 * feather)).clamp(0.0, 1.0);
                let alpha = fade * fade * (3.0 - 2.0 * fade);
                mesh.vertices.push(egui::epaint::Vertex {
                    pos,
                    uv,
                    color: Color32::WHITE.gamma_multiply(alpha),
                });
                if x > 0 && y > 0 {
                    let bottom_right = y * (cols + 1) + x;
                    let top_right = bottom_right - cols - 1;
                    mesh.add_triangle(top_right - 1, top_right, bottom_right);
                    mesh.add_triangle(top_right - 1, bottom_right, bottom_right - 1);
                }
            }
        }
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Debug,
            egui::Id::new("theme_reveal"),
        ))
        .add(egui::Shape::mesh(mesh));
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub glass: bool,
    pub canvas: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub well: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_tint: Color32,
    pub good: Color32,
    pub danger: Color32,
}

impl Palette {
    pub fn get(ui: &Ui) -> Self {
        Self::for_context(ui.ctx(), ui.visuals().dark_mode)
    }

    pub fn for_context(ctx: &egui::Context, dark: bool) -> Self {
        let glass = ctx.data(|data| data.get_temp::<bool>(egui::Id::new("glass_surfaces")));
        Self::with_glass(dark, glass.unwrap_or_default())
    }

    fn with_glass(dark: bool, glass: bool) -> Self {
        let mut palette = Self::new(dark);
        if glass {
            palette.glass = true;
            palette.canvas = palette.canvas.gamma_multiply(0.27);
            palette.panel = palette.panel.gamma_multiply(0.43);
            palette.raised = palette.raised.gamma_multiply(0.50);
            palette.well = palette.well.gamma_multiply(0.39);
            palette.accent_tint = palette.accent_tint.gamma_multiply(0.65);
            palette.line = Color32::from_white_alpha(if dark { 54 } else { 136 });
        }
        palette
    }

    fn new(dark: bool) -> Self {
        let rgb = Color32::from_rgb;
        if dark {
            Self {
                dark,
                glass: false,
                canvas: rgb(17, 19, 21),
                panel: rgb(25, 28, 31),
                raised: rgb(35, 39, 43),
                well: rgb(13, 15, 17),
                line: rgb(53, 58, 62),
                text: rgb(243, 241, 236),
                muted: rgb(167, 173, 179),
                accent: GOLD,
                accent_tint: rgb(57, 46, 31),
                good: rgb(130, 208, 173),
                danger: rgb(255, 149, 145),
            }
        } else {
            Self {
                dark,
                glass: false,
                canvas: rgb(238, 236, 230),
                panel: rgb(251, 249, 244),
                raised: rgb(240, 237, 229),
                well: rgb(255, 253, 249),
                line: rgb(210, 206, 195),
                text: rgb(33, 35, 38),
                muted: rgb(89, 92, 97),
                accent: rgb(129, 76, 13),
                accent_tint: rgb(248, 226, 185),
                good: rgb(36, 108, 78),
                danger: rgb(159, 49, 46),
            }
        }
    }
}

pub fn load_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    for (name, bytes) in [
        (
            "Manrope",
            include_bytes!("../assets/Manrope.ttf").as_slice(),
        ),
        (
            "Redaction",
            include_bytes!("../assets/Redaction-Regular.ttf").as_slice(),
        ),
        (
            "Instrument Serif",
            include_bytes!("../assets/InstrumentSerif-Regular.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
    }
    for (family, name) in [
        (FontFamily::Proportional, "Manrope"),
        (FontFamily::Monospace, "Manrope"),
        (FontFamily::Name("Redaction".into()), "Redaction"),
        (
            FontFamily::Name("Instrument Serif".into()),
            "Instrument Serif",
        ),
    ] {
        let mut names = vec![name.to_owned()];
        if name != "Manrope" {
            names.push("Manrope".into());
        }
        fonts.families.insert(family, names);
    }
    ctx.set_fonts(fonts);
}

pub fn configure(ctx: &egui::Context, dark: bool, glass: bool) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("glass_surfaces"), glass));
    let palette = Palette::with_glass(dark, glass);
    let theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(theme);
    let title_theme = if dark {
        egui::SystemTheme::Dark
    } else {
        egui::SystemTheme::Light
    };
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(title_theme));
    let mut style = (*ctx.style_of(theme)).clone();
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.panel_fill = palette.canvas;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.well;
    visuals.text_edit_bg_color = Some(palette.well);
    visuals.faint_bg_color = palette.raised;
    visuals.override_text_color = Some(palette.text);
    visuals.weak_text_color = Some(palette.muted);
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.accent;
    visuals.error_fg_color = palette.danger;
    visuals.selection.bg_fill = palette.accent_tint;
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.window_corner_radius = CornerRadius::same(16);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.window_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.window_shadow = shadow(palette);
    visuals.popup_shadow = shadow(palette);
    if glass {
        // A deeper tint keeps text legible when a menu overlaps other controls.
        visuals.window_fill = Palette::new(dark).panel.gamma_multiply(0.86);
    }

    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = palette.raised;
        widget.weak_bg_fill = palette.raised;
        widget.bg_stroke = Stroke::new(1.0_f32, palette.line);
        widget.fg_stroke.color = palette.text;
        widget.corner_radius = CornerRadius::same(7);
        widget.expansion = 0.0;
    }
    visuals.widgets.hovered.bg_stroke.color = palette.muted;
    visuals.widgets.active.bg_stroke.color = palette.accent;
    visuals.widgets.open.bg_stroke.color = palette.accent;
    visuals.widgets.active.weak_bg_fill = palette.accent_tint;
    visuals.widgets.open.weak_bg_fill = palette.accent_tint;
    style.visuals = visuals;
    style.animation_time = 0.16;
    style.spacing.item_spacing = vec2(5.0, 4.0);
    style.spacing.button_padding = vec2(5.0, 4.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.window_margin = egui::Margin::same(5);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(13.5));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(21.0));
    ctx.set_style_of(theme, style);
}

pub fn surface(ui: &Ui) -> Frame {
    let palette = Palette::get(ui);
    Frame::new()
        .fill(palette.panel)
        .stroke(Stroke::new(1.0_f32, palette.line))
        .corner_radius(14)
        .inner_margin(5)
}

/// The window fill with full alpha.
///
/// Glass themes scale the fill with `gamma_multiply`, which also scales alpha, so
/// the blurred wallpaper would show through a panel that must stay solid.
pub fn opaque_window_fill(style: &egui::Style) -> Color32 {
    let fill = style.visuals.window_fill;
    Color32::from_rgb(fill.r(), fill.g(), fill.b())
}

pub fn dialog(style: &egui::Style) -> Frame {
    Frame::window(style).inner_margin(5).corner_radius(16)
}

fn shadow(palette: Palette) -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 12],
        blur: 34,
        spread: 2,
        color: Color32::from_black_alpha(if palette.dark { 125 } else { 48 }),
    }
}

pub fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text).family(FontFamily::Monospace)
}

pub fn background(ui: &Ui) {
    let rect = ui.ctx().content_rect();
    let palette = Palette::get(ui);
    let painter = ui.ctx().layer_painter(ui.layer_id());
    crate::icons::radial_glow(
        &painter,
        pos2(rect.right() - rect.width() * 0.24, rect.top() + 40.0),
        rect.width().min(950.0) * 0.50,
        Color32::from_rgba_unmultiplied(204, 147, 65, if palette.dark { 17 } else { 26 }),
    );
}

pub struct BackgroundPreset {
    pub id: &'static str,
    pub name: &'static str,
    jpeg: &'static [u8],
}

pub const BACKGROUND_PRESETS: &[BackgroundPreset] = &[
    BackgroundPreset {
        id: "custom-media",
        name: "Custom media",
        jpeg: &[],
    },
    BackgroundPreset {
        id: "orchid-dusk",
        name: "Orchid Dusk",
        jpeg: include_bytes!("../assets/backgrounds/orchid-dusk.jpg"),
    },
    BackgroundPreset {
        id: "ember-dusk",
        name: "Ember Dusk",
        jpeg: include_bytes!("../assets/backgrounds/ember-dusk.jpg"),
    },
    BackgroundPreset {
        id: "paper-mesh-gradient-beach",
        name: "Mesh Gradient Beach",
        jpeg: include_bytes!("../assets/backgrounds/paper-mesh-gradient-beach.jpg"),
    },
    BackgroundPreset {
        id: "aurora-blobs",
        name: "Aurora Blobs",
        jpeg: include_bytes!("../assets/backgrounds/aurora-blobs.jpg"),
    },
    BackgroundPreset {
        id: "1960s-rust",
        name: "1960s Rust",
        jpeg: include_bytes!("../assets/backgrounds/1960s-rust.jpg"),
    },
    BackgroundPreset {
        id: "paper-static-mesh-gradient-1960s",
        name: "Static Mesh Gradient 1960s",
        jpeg: include_bytes!("../assets/backgrounds/paper-static-mesh-gradient-1960s.jpg"),
    },
    BackgroundPreset {
        id: "paper-smoke-ring-cloud",
        name: "Smoke Ring Cloud",
        jpeg: include_bytes!("../assets/backgrounds/paper-smoke-ring-cloud.jpg"),
    },
    BackgroundPreset {
        id: "light-leak",
        name: "Light Leak",
        jpeg: include_bytes!("../assets/backgrounds/light-leak.jpg"),
    },
    BackgroundPreset {
        id: "halo-veil",
        name: "Halo Veil",
        jpeg: include_bytes!("../assets/backgrounds/halo-veil.jpg"),
    },
    BackgroundPreset {
        id: "plasma-pink",
        name: "Plasma Pink",
        jpeg: include_bytes!("../assets/backgrounds/plasma-pink.jpg"),
    },
    BackgroundPreset {
        id: "cloud-sky",
        name: "Cloud Sky",
        jpeg: include_bytes!("../assets/backgrounds/cloud-sky.jpg"),
    },
    BackgroundPreset {
        id: "paper-neuro-noise-ghost",
        name: "Neuro Noise Ghost",
        jpeg: include_bytes!("../assets/backgrounds/paper-neuro-noise-ghost.jpg"),
    },
    BackgroundPreset {
        id: "paper-warp-live-ink",
        name: "Warp Live Ink",
        jpeg: include_bytes!("../assets/backgrounds/paper-warp-live-ink.jpg"),
    },
    BackgroundPreset {
        id: "paper-warp-cauldron-pot",
        name: "Warp Cauldron Pot",
        jpeg: include_bytes!("../assets/backgrounds/paper-warp-cauldron-pot.jpg"),
    },
    BackgroundPreset {
        id: "ribbed-sunset",
        name: "Ribbed Sunset",
        jpeg: include_bytes!("../assets/backgrounds/ribbed-sunset.jpg"),
    },
    BackgroundPreset {
        id: "paper-fluted-glass-default",
        name: "Fluted Glass",
        jpeg: include_bytes!("../assets/backgrounds/paper-fluted-glass-default.jpg"),
    },
    BackgroundPreset {
        id: "paper-water-slow-mo",
        name: "Water Slow Mo",
        jpeg: include_bytes!("../assets/backgrounds/paper-water-slow-mo.jpg"),
    },
    BackgroundPreset {
        id: "halftone-sunset",
        name: "Halftone Sunset",
        jpeg: include_bytes!("../assets/backgrounds/halftone-sunset.jpg"),
    },
    BackgroundPreset {
        id: "paper-halftone-cmyk-vintage",
        name: "Halftone CMYK Vintage",
        jpeg: include_bytes!("../assets/backgrounds/paper-halftone-cmyk-vintage.jpg"),
    },
    BackgroundPreset {
        id: "crimson-dither",
        name: "Crimson Dither",
        jpeg: include_bytes!("../assets/backgrounds/crimson-dither.jpg"),
    },
    BackgroundPreset {
        id: "gold-drift-dither",
        name: "Gold Drift Dither",
        jpeg: include_bytes!("../assets/backgrounds/gold-drift-dither.jpg"),
    },
    BackgroundPreset {
        id: "paper-dithering-swirl",
        name: "Dithering Swirl",
        jpeg: include_bytes!("../assets/backgrounds/paper-dithering-swirl.jpg"),
    },
    BackgroundPreset {
        id: "paper-image-dithering-default",
        name: "Image Dithering",
        jpeg: include_bytes!("../assets/backgrounds/paper-image-dithering-default.jpg"),
    },
    BackgroundPreset {
        id: "paper-swirl-default",
        name: "Swirl",
        jpeg: include_bytes!("../assets/backgrounds/paper-swirl-default.jpg"),
    },
    BackgroundPreset {
        id: "paper-spiral-jungle",
        name: "Spiral Jungle",
        jpeg: include_bytes!("../assets/backgrounds/paper-spiral-jungle.jpg"),
    },
    BackgroundPreset {
        id: "paper-gem-smoke-infrared",
        name: "Gem Smoke Infrared",
        jpeg: include_bytes!("../assets/backgrounds/paper-gem-smoke-infrared.jpg"),
    },
    BackgroundPreset {
        id: "paper-warp-passion",
        name: "Warp Passion",
        jpeg: include_bytes!("../assets/backgrounds/paper-warp-passion.jpg"),
    },
    BackgroundPreset {
        id: "ripple-cursor",
        name: "Ripple Cursor",
        jpeg: include_bytes!("../assets/backgrounds/ripple-cursor.jpg"),
    },
    BackgroundPreset {
        id: "dot-swell",
        name: "Dot Swell",
        jpeg: include_bytes!("../assets/backgrounds/dot-swell.jpg"),
    },
    BackgroundPreset {
        id: "paper-grain-gradient-truchet",
        name: "Grain Gradient Truchet",
        jpeg: include_bytes!("../assets/backgrounds/paper-grain-gradient-truchet.jpg"),
    },
    BackgroundPreset {
        id: "grain-gradient-ember",
        name: "Grain Gradient Ember",
        jpeg: include_bytes!("../assets/backgrounds/grain-gradient-ember.jpg"),
    },
];

pub fn background_preset(id: &str) -> Option<&'static BackgroundPreset> {
    BACKGROUND_PRESETS.iter().find(|preset| preset.id == id)
}

#[derive(Clone, Copy)]
enum ImageEffect {
    Plain,
    Water,
    FlutedGlass,
}

impl BackgroundPreset {
    /// Tiny 16:9 preview of the theme, drawn in front of its name in the picker.
    pub fn thumbnail(&self) -> Option<&'static [u8]> {
        let bytes: &'static [u8] = match self.id {
            "orchid-dusk" => include_bytes!("../assets/backgrounds/thumbs/orchid-dusk.jpg"),
            "ember-dusk" => include_bytes!("../assets/backgrounds/thumbs/ember-dusk.jpg"),
            "paper-mesh-gradient-beach" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-mesh-gradient-beach.jpg")
            }
            "aurora-blobs" => include_bytes!("../assets/backgrounds/thumbs/aurora-blobs.jpg"),
            "1960s-rust" => include_bytes!("../assets/backgrounds/thumbs/1960s-rust.jpg"),
            "paper-static-mesh-gradient-1960s" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-static-mesh-gradient-1960s.jpg")
            }
            "paper-smoke-ring-cloud" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-smoke-ring-cloud.jpg")
            }
            "light-leak" => include_bytes!("../assets/backgrounds/thumbs/light-leak.jpg"),
            "halo-veil" => include_bytes!("../assets/backgrounds/thumbs/halo-veil.jpg"),
            "plasma-pink" => include_bytes!("../assets/backgrounds/thumbs/plasma-pink.jpg"),
            "cloud-sky" => include_bytes!("../assets/backgrounds/thumbs/cloud-sky.jpg"),
            "paper-neuro-noise-ghost" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-neuro-noise-ghost.jpg")
            }
            "paper-warp-live-ink" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-warp-live-ink.jpg")
            }
            "paper-warp-cauldron-pot" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-warp-cauldron-pot.jpg")
            }
            "ribbed-sunset" => include_bytes!("../assets/backgrounds/thumbs/ribbed-sunset.jpg"),
            "paper-fluted-glass-default" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-fluted-glass-default.jpg")
            }
            "paper-water-slow-mo" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-water-slow-mo.jpg")
            }
            "halftone-sunset" => include_bytes!("../assets/backgrounds/thumbs/halftone-sunset.jpg"),
            "paper-halftone-cmyk-vintage" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-halftone-cmyk-vintage.jpg")
            }
            "crimson-dither" => include_bytes!("../assets/backgrounds/thumbs/crimson-dither.jpg"),
            "gold-drift-dither" => {
                include_bytes!("../assets/backgrounds/thumbs/gold-drift-dither.jpg")
            }
            "paper-dithering-swirl" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-dithering-swirl.jpg")
            }
            "paper-image-dithering-default" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-image-dithering-default.jpg")
            }
            "paper-swirl-default" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-swirl-default.jpg")
            }
            "paper-spiral-jungle" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-spiral-jungle.jpg")
            }
            "paper-gem-smoke-infrared" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-gem-smoke-infrared.jpg")
            }
            "paper-warp-passion" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-warp-passion.jpg")
            }
            "ripple-cursor" => include_bytes!("../assets/backgrounds/thumbs/ripple-cursor.jpg"),
            "dot-swell" => include_bytes!("../assets/backgrounds/thumbs/dot-swell.jpg"),
            "paper-grain-gradient-truchet" => {
                include_bytes!("../assets/backgrounds/thumbs/paper-grain-gradient-truchet.jpg")
            }
            "grain-gradient-ember" => {
                include_bytes!("../assets/backgrounds/thumbs/grain-gradient-ember.jpg")
            }
            _ => return None,
        };
        Some(bytes)
    }

    fn image_effect(&self) -> Option<ImageEffect> {
        match self.id {
            "custom-media" => Some(ImageEffect::Plain),
            "paper-water-slow-mo" => Some(ImageEffect::Water),
            "paper-fluted-glass-default" => Some(ImageEffect::FlutedGlass),
            _ => None,
        }
    }

    pub fn supports_custom_image(&self) -> bool {
        self.image_effect().is_some()
    }

    fn custom_image_path(&self) -> PathBuf {
        crate::settings::config_dir()
            .join("backgrounds")
            .join(format!("{}.jpg", self.id))
    }

    pub fn has_custom_image(&self) -> bool {
        self.supports_custom_image()
            && (self.active_recent().is_some() || self.custom_image_path().is_file())
    }

    pub fn label(&self, lang: crate::i18n::Lang) -> &'static str {
        if self.id == "custom-media" && lang == crate::i18n::Lang::Pl {
            "Własne tło"
        } else {
            self.name
        }
    }

    fn active_reference(&self) -> PathBuf {
        self.custom_image_path().with_extension("active")
    }

    fn active_recent(&self) -> Option<PathBuf> {
        let name = std::fs::read_to_string(self.active_reference()).ok()?;
        let name = Path::new(&name);
        if name.file_name()? != name.as_os_str() || name.extension()? != "jpg" {
            return None;
        }
        let path = self.history_dir().join(name);
        path.is_file().then_some(path)
    }

    fn history_dir(&self) -> PathBuf {
        crate::settings::config_dir()
            .join("backgrounds")
            .join(self.id)
    }

    pub fn recent_images(&self) -> io::Result<Vec<(String, PathBuf)>> {
        self.keep_legacy_image()?;
        Ok(self.list_recent())
    }

    fn list_recent(&self) -> Vec<(String, PathBuf)> {
        let mut images: Vec<_> = std::fs::read_dir(self.history_dir())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let name = path.file_stem()?.to_str()?.split_once("--")?.1.to_owned();
                if path.extension()? != "jpg" || !entry.file_type().ok()?.is_file() {
                    return None;
                }
                Some((entry.metadata().ok()?.modified().ok()?, name, path))
            })
            .collect();
        images.sort_unstable_by(|a, b| b.cmp(a));
        images
            .into_iter()
            .map(|(_, name, path)| (name, path))
            .collect()
    }

    fn save_recent(&self, name: &str, bytes: &[u8], motion: Option<&Path>) -> io::Result<PathBuf> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hash);
        if let Some(source) = motion {
            let metadata = std::fs::metadata(source)?;
            source.hash(&mut hash);
            metadata.len().hash(&mut hash);
            metadata.modified().ok().hash(&mut hash);
        }
        let name: String = name.chars().take(80).collect();
        let path = self
            .history_dir()
            .join(format!("{:016x}--{name}.jpg", hash.finish()));
        if let Some(source) = motion {
            let folder = self.history_dir();
            let target = path.with_extension("motion");
            if !target.is_file() {
                std::fs::create_dir_all(&folder)?;
                let mut file = tempfile::NamedTempFile::new_in(folder)?;
                io::copy(&mut std::fs::File::open(source)?, &mut file)?;
                file.persist(target)?;
            }
        }
        write_background(&path, bytes)?;
        Ok(path)
    }

    fn keep_legacy_image(&self) -> io::Result<()> {
        if self.custom_image_path().is_file() && self.list_recent().is_empty() {
            self.save_recent(
                "Custom image",
                &std::fs::read(self.custom_image_path())?,
                None,
            )?;
        }
        Ok(())
    }
}

fn write_background(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use io::Write;
    let folder = path.parent().expect("custom background directory");
    std::fs::create_dir_all(folder)?;
    let mut file = tempfile::NamedTempFile::new_in(folder)?;
    file.write_all(bytes)?;
    file.persist(path)?;
    Ok(())
}

fn render_custom_image(source: &image::RgbImage, effect: ImageEffect) -> image::RgbImage {
    let uv = cover_uv(
        vec2(source.width() as f32, source.height() as f32),
        vec2(1920.0, 1080.0),
    );
    image::RgbImage::from_fn(1920, 1080, |x, y| {
        let (x, y) = (x as f32 / 1919.0, y as f32 / 1079.0);
        let (u, v, light) = effect.sample(x, y);
        let u = egui::lerp(uv.left()..=uv.right(), u.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let v = egui::lerp(uv.top()..=uv.bottom(), v.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let pixel = image::imageops::sample_bilinear(source, u, v).unwrap_or(image::Rgb([0; 3]));
        image::Rgb(
            pixel
                .0
                .map(|channel| (f32::from(channel) * light).round().clamp(0.0, 255.0) as u8),
        )
    })
}

impl ImageEffect {
    fn sample(self, x: f32, y: f32) -> (f32, f32, f32) {
        match self {
            ImageEffect::Plain => (x, y, 1.0),
            ImageEffect::Water => {
                let wave = (27.0 * y + (11.0 * x - 5.0 * y).sin() * 1.8).sin();
                let crossing = (17.0 * x - 23.0 * y + (9.0 * y).sin()).sin();
                (
                    x + 0.014 * wave + 0.008 * crossing,
                    y + 0.012 * crossing,
                    1.0 + 0.09 * (wave * crossing).max(0.0),
                )
            }
            ImageEffect::FlutedGlass => {
                let rib = std::f32::consts::TAU * 26.0 * x;
                (x + 0.012 * rib.sin(), y, 0.94 + 0.11 * rib.cos())
            }
        }
    }
}

#[derive(Default)]
pub struct Background {
    swirl: Option<std::sync::Arc<crate::swirl::Swirl>>,
    texture: Option<(&'static str, egui::TextureHandle)>,
    recent_previews: Option<(&'static str, Vec<RecentBackground>)>,
    motion: Option<crate::background_motion::MotionBackground>,
    motion_file: Option<(PathBuf, ImageEffect)>,
    built_in_loop: Option<tempfile::NamedTempFile>,
    next_motion_texture: Option<egui::TextureHandle>,
    motion_started: bool,
}

pub struct RecentBackground {
    pub name: String,
    pub path: PathBuf,
    pub texture: egui::TextureHandle,
}

impl Background {
    pub fn new(gl: Option<std::sync::Arc<eframe::glow::Context>>) -> Self {
        Self {
            swirl: gl.and_then(crate::swirl::Swirl::new),
            ..Self::default()
        }
    }

    pub fn recent_previews(
        &mut self,
        preset: &BackgroundPreset,
        ctx: &egui::Context,
    ) -> io::Result<&[RecentBackground]> {
        if self
            .recent_previews
            .as_ref()
            .is_none_or(|(id, _)| *id != preset.id)
        {
            let previews = preset
                .recent_images()?
                .into_iter()
                .filter_map(|(name, path)| {
                    let image = image::open(&path).ok()?.thumbnail(80, 60).into_rgba8();
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    );
                    let texture = ctx.load_texture(&name, image, egui::TextureOptions::LINEAR);
                    Some(RecentBackground {
                        name,
                        path,
                        texture,
                    })
                })
                .take(4)
                .collect();
            self.recent_previews = Some((preset.id, previews));
        }
        Ok(&self
            .recent_previews
            .as_ref()
            .expect("previews were loaded")
            .1)
    }

    pub fn set_custom_image(&mut self, preset: &BackgroundPreset, source: &Path) -> io::Result<()> {
        use image::ImageDecoder;

        let effect = preset.image_effect().ok_or(io::ErrorKind::InvalidInput)?;
        let motion = source
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
            || cutgut::media::is_video_file(&source.to_string_lossy());
        let bytes = if motion {
            crate::background_motion::poster(source)?
        } else {
            let mut decoder = image::ImageReader::open(source)?
                .with_guessed_format()?
                .into_decoder()
                .map_err(io::Error::other)?;
            let orientation = decoder.orientation().map_err(io::Error::other)?;
            let mut image = image::DynamicImage::from_decoder(decoder).map_err(io::Error::other)?;
            image.apply_orientation(orientation);
            let image = render_custom_image(&image.into_rgb8(), effect);
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95)
                .encode_image(&image)
                .map_err(io::Error::other)?;
            bytes
        };
        let name = source.file_name().unwrap_or_default().to_string_lossy();
        preset.keep_legacy_image()?;
        let entry = preset.save_recent(&name, &bytes, motion.then_some(source))?;
        self.use_recent_image(preset, &entry)
    }

    pub fn use_recent_image(&mut self, preset: &BackgroundPreset, path: &Path) -> io::Result<()> {
        if !preset.supports_custom_image() || path.parent() != Some(preset.history_dir().as_path())
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        std::fs::File::options()
            .write(true)
            .open(path)?
            .set_modified(std::time::SystemTime::now())?;
        write_background(&preset.active_reference(), name.as_bytes())?;
        self.invalidate();
        Ok(())
    }

    pub fn reset_custom_image(&mut self, preset: &BackgroundPreset) -> io::Result<()> {
        preset.keep_legacy_image()?;
        for path in [preset.custom_image_path(), preset.active_reference()] {
            if let Err(error) = std::fs::remove_file(path) {
                if error.kind() != io::ErrorKind::NotFound {
                    return Err(error);
                }
            }
        }
        self.invalidate();
        Ok(())
    }

    fn invalidate(&mut self) {
        self.texture = None;
        self.stop_motion();
        self.motion_file = None;
        self.built_in_loop = None;
        self.recent_previews = None;
    }

    fn stop_motion(&mut self) {
        self.motion = None;
        self.next_motion_texture = None;
        self.motion_started = false;
    }

    pub fn paint(&mut self, ctx: &egui::Context, selected: &str, dark: bool) {
        let preset = background_preset(selected);
        let opacity =
            ctx.animate_bool_with_time(egui::Id::new("background_preset"), preset.is_some(), 0.24);
        if opacity <= 0.0 {
            self.texture = None;
            self.stop_motion();
            self.motion_file = None;
            self.built_in_loop = None;
            return;
        }
        if let Some(preset) = preset {
            if self.texture.as_ref().is_none_or(|(id, _)| *id != preset.id) {
                self.stop_motion();
                self.built_in_loop = None;
                let active = preset.active_recent();
                self.motion_file = active.as_ref().and_then(|entry| {
                    let motion = entry.with_extension("motion");
                    preset
                        .image_effect()
                        .filter(|_| motion.is_file())
                        .map(|effect| (motion, effect))
                });
                let custom = preset
                    .supports_custom_image()
                    .then(|| image::open(active.unwrap_or_else(|| preset.custom_image_path())).ok())
                    .flatten();
                if custom.is_none()
                    && self.motion_file.is_none()
                    && preset.id != "paper-swirl-default"
                {
                    if let Some(bytes) = crate::background_loops::preset_loop(preset.id) {
                        if let Ok(file) = crate::background_motion::open_built_in(bytes) {
                            self.motion_file =
                                Some((file.path().to_path_buf(), ImageEffect::Plain));
                            self.built_in_loop = Some(file);
                        }
                    }
                }
                let pixels = match custom.or_else(|| {
                    image::load_from_memory_with_format(preset.jpeg, image::ImageFormat::Jpeg).ok()
                }) {
                    Some(image) => {
                        let image = image.into_rgba8();
                        egui::ColorImage::from_rgba_unmultiplied(
                            [image.width() as usize, image.height() as usize],
                            image.as_raw(),
                        )
                    }
                    None => egui::ColorImage::filled([1, 1], Palette::new(dark).canvas),
                };
                self.texture = Some((
                    preset.id,
                    ctx.load_texture(preset.id, pixels, egui::TextureOptions::LINEAR),
                ));
            }
        } else {
            self.stop_motion();
            self.motion_file = None;
            self.built_in_loop = None;
        }
        let mut mix = 0.0;
        // Nothing is watching while the window is minimized or in the background,
        // yet the animated background would keep decoding frames and repainting
        // at full speed. Stopping the loop lets the app go idle instead.
        let inactive =
            ctx.input(|input| input.viewport().minimized.unwrap_or(false) || !input.focused);
        if inactive {
            self.stop_motion();
        } else if let Some((path, _)) = &self.motion_file {
            let interpolate = self.built_in_loop.is_some();
            let motion = self.motion.get_or_insert_with(|| {
                if interpolate {
                    crate::background_motion::MotionBackground::start_built_in(ctx, path)
                } else {
                    crate::background_motion::MotionBackground::start(ctx, path)
                }
            });
            if let Some(frame) = motion.poll(ctx) {
                if let Some((_, texture)) = &mut self.texture {
                    if interpolate && self.motion_started {
                        if let Some(next) = &mut self.next_motion_texture {
                            std::mem::swap(texture, next);
                            next.set(frame, egui::TextureOptions::LINEAR);
                        } else {
                            self.next_motion_texture = Some(ctx.load_texture(
                                "Next background frame",
                                frame,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                    } else {
                        texture.set(frame, egui::TextureOptions::LINEAR);
                        self.motion_started = true;
                    }
                }
            }
            if self.next_motion_texture.is_some() {
                mix = motion.interpolate(ctx);
            }
        }
        let Some((_, texture)) = &self.texture else {
            return;
        };
        let area = ctx.content_rect();
        if area.is_positive() {
            let painter = ctx.layer_painter(egui::LayerId::background());
            let canvas = Palette::new(dark).canvas;
            painter.rect_filled(area, 0, canvas);
            let tint = Color32::from_white_alpha((255.0 * opacity).round() as u8);
            if selected == "paper-swirl-default" {
                let angle =
                    ctx.input(|input| (input.time * 0.18).rem_euclid(std::f64::consts::TAU)) as f32;
                if let Some(swirl) = &self.swirl {
                    swirl.paint(&painter, area, angle * 3.0, opacity);
                } else {
                    paint_swirl(&painter, area, texture.id(), angle, opacity);
                }
                if !ctx.input(|input| input.viewport().minimized.unwrap_or(false)) {
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(1.0 / 60.0));
                }
            } else if let Some((_, effect @ (ImageEffect::Water | ImageEffect::FlutedGlass))) =
                &self.motion_file
            {
                paint_motion_effect(&painter, area, texture.id(), *effect, opacity);
            } else {
                painter.image(
                    texture.id(),
                    area,
                    Rect::from_min_max(egui::Pos2::ZERO, pos2(1.0, 1.0)),
                    tint,
                );
                if let Some(next) = &self.next_motion_texture {
                    painter.image(
                        next.id(),
                        area,
                        Rect::from_min_max(egui::Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::from_white_alpha((255.0 * opacity * mix).round() as u8),
                    );
                }
            }
            let veil = if dark { 0.52 } else { 0.62 };
            painter.rect_filled(
                area,
                0,
                Color32::from_rgba_unmultiplied(
                    canvas.r(),
                    canvas.g(),
                    canvas.b(),
                    (255.0 * opacity * veil).round() as u8,
                ),
            );
        }
    }
}

fn swirl_uv(x: f32, y: f32, angle: f32) -> egui::Pos2 {
    // Fit the rotating rectangle inside the source image so its edges
    // never reach the texture sampler's clamped border.
    let dx = (x - 0.5) * 0.70;
    let dy = (y - 0.5) * 0.70;
    let (sin, cos) = angle.sin_cos();
    pos2(0.5 + dx * cos - dy * sin, 0.5 + dx * sin + dy * cos)
}

fn paint_swirl(painter: &egui::Painter, area: Rect, id: egui::TextureId, angle: f32, opacity: f32) {
    let mut mesh = egui::Mesh::with_texture(id);
    for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: area.min + area.size() * vec2(x, y),
            uv: swirl_uv(x, y, angle),
            color: Color32::from_white_alpha((255.0 * opacity).round() as u8),
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_motion_effect(
    painter: &egui::Painter,
    area: Rect,
    id: egui::TextureId,
    effect: ImageEffect,
    opacity: f32,
) {
    const COLUMNS: u32 = 104;
    const ROWS: u32 = 28;
    let mut mesh = egui::Mesh::with_texture(id);
    for row in 0..=ROWS {
        for column in 0..=COLUMNS {
            let x = column as f32 / COLUMNS as f32;
            let y = row as f32 / ROWS as f32;
            let (u, v, gain) = effect.sample(x, y);
            let color = (255.0 * opacity * (gain * 0.94).min(1.0)).round() as u8;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: area.min + area.size() * vec2(x, y),
                uv: pos2(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)),
                color: Color32::from_rgba_premultiplied(
                    color,
                    color,
                    color,
                    (255.0 * opacity).round() as u8,
                ),
            });
            if row > 0 && column > 0 {
                let right = row * (COLUMNS + 1) + column;
                let above = right - COLUMNS - 1;
                mesh.add_triangle(above - 1, above, right);
                mesh.add_triangle(above - 1, right, right - 1);
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

pub fn rounded_image(
    painter: &egui::Painter,
    rect: Rect,
    radius: CornerRadius,
    id: egui::TextureId,
    uv: Rect,
) {
    rounded_image_tinted(painter, rect, radius, id, uv, Color32::WHITE);
}

/// Same as [`rounded_image`] but the texture is multiplied by `tint` (fade-in).
pub fn rounded_image_tinted(
    painter: &egui::Painter,
    rect: Rect,
    radius: CornerRadius,
    id: egui::TextureId,
    uv: Rect,
    tint: Color32,
) {
    painter.add(egui::epaint::RectShape::filled(rect, radius, tint).with_texture(id, uv));
}

pub fn cover_uv(image: egui::Vec2, area: egui::Vec2) -> Rect {
    let ratio = (area.x / area.y) / (image.x / image.y);
    Rect::from_center_size(pos2(0.5, 0.5), vec2(ratio.min(1.0), (1.0 / ratio).min(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn custom_background_effects_render_the_photo_differently() {
        let source = image::RgbImage::from_pixel(10, 10, image::Rgb([120, 70, 30]));
        let water = render_custom_image(&source, ImageEffect::Water);
        let glass = render_custom_image(&source, ImageEffect::FlutedGlass);
        assert_eq!(water.dimensions(), (1920, 1080));
        assert_eq!(glass.dimensions(), water.dimensions());
        assert_ne!(glass.get_pixel(0, 0), glass.get_pixel(35, 0));
        assert_ne!(water.get_pixel(0, 0), glass.get_pixel(0, 0));
    }

    #[test]
    fn every_background_preset_decodes() {
        for preset in BACKGROUND_PRESETS
            .iter()
            .filter(|preset| !preset.jpeg.is_empty())
        {
            let image = image::load_from_memory_with_format(preset.jpeg, image::ImageFormat::Jpeg)
                .unwrap_or_else(|error| panic!("{}: {error}", preset.id));
            assert_eq!(
                (image.width(), image.height()),
                (1920, 1080),
                "{}",
                preset.id
            );
        }
    }

    #[test]
    fn theme_reveal_clears_the_old_frame_first_near_the_switch() {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0));
        let reveal = Reveal::new(
            &ctx,
            Arc::new(egui::ColorImage::filled([2, 2], Color32::WHITE)),
            screen.right_top(),
        );
        let frame = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| reveal.paint_frame(ctx, 0.33),
        );
        let mesh = frame
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == reveal.previous.id() => Some(mesh),
                _ => None,
            })
            .unwrap();
        let closest = mesh
            .vertices
            .iter()
            .find(|vertex| vertex.pos == screen.right_top())
            .unwrap();
        let farthest = mesh
            .vertices
            .iter()
            .find(|vertex| vertex.pos == screen.left_bottom())
            .unwrap();
        assert_eq!(closest.color.a(), 0);
        assert_eq!(farthest.color.a(), 255);
    }
}
