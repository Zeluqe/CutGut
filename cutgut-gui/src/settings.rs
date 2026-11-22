//! Persistent GUI settings stored as a tiny key=value file in %APPDATA%.

use std::{io::Write, path::PathBuf};

use cutgut::plan::VideoCodec;

use crate::i18n::Lang;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OriginalCleanup {
    Off,
    #[default]
    Ask,
    Automatic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportSettings {
    pub profile: usize,
    pub crop: usize,
    pub limit: usize,
    pub custom_mb: f64,
    pub codec: VideoCodec,
    pub engine: usize,
    pub format: usize,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            profile: 0,
            crop: 0,
            limit: 1,
            custom_mb: 30.0,
            codec: VideoCodec::H264,
            engine: 0,
            format: 0,
        }
    }
}

impl ExportSettings {
    fn read(&mut self, key: &str, value: &str) {
        let index = |limit: usize| value.parse::<usize>().ok().filter(|&index| index < limit);
        match key {
            "export_profile" => self.profile = index(6).unwrap_or(self.profile),
            "export_crop" => self.crop = index(4).unwrap_or(self.crop),
            "export_limit" => self.limit = index(5).unwrap_or(self.limit),
            "export_engine" => self.engine = index(4).unwrap_or(self.engine),
            "export_format" => self.format = index(5).unwrap_or(self.format),
            "export_codec" => self.codec = VideoCodec::parse(value).unwrap_or(self.codec),
            "export_custom_mb" => {
                if let Ok(mb) = value.parse::<f64>() {
                    if (1.0..=2000.0).contains(&mb) {
                        self.custom_mb = mb;
                    }
                }
            }
            _ => {}
        }
    }

    fn encode(self) -> String {
        format!(
            "export_profile={}\nexport_crop={}\nexport_limit={}\nexport_custom_mb={}\nexport_codec={}\nexport_engine={}\nexport_format={}\n",
            self.profile, self.crop, self.limit, self.custom_mb, self.codec.as_str(), self.engine, self.format,
        )
    }
}

impl OriginalCleanup {
    pub const OPTIONS: [(Self, &'static str); 3] = [
        (Self::Ask, "cleanup_ask"),
        (Self::Automatic, "cleanup_automatic"),
        (Self::Off, "cleanup_off"),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ask => "cleanup_ask",
            Self::Automatic => "cleanup_automatic",
            Self::Off => "cleanup_off",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Automatic => "automatic",
            Self::Off => "off",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub version: u32,
    pub lang: Lang,
    pub dark_theme: bool,
    pub background_theme: String,
    pub output_dir: String,
    pub pause_overlay: bool,
    pub gallery_on_startup: bool,
    pub gallery_dir: String,
    /// Every folder the gallery shows. `gallery_dir` is always the first entry.
    pub gallery_dirs: Vec<String>,
    pub original_cleanup: OriginalCleanup,
    /// Gallery grouping mode and thumbnail density (indexes into the enums).
    pub gallery_group: u8,
    pub gallery_card: u8,
    pub gallery_sort: u8,
    pub gallery_desc: bool,
    pub export: ExportSettings,
    /// Preview volume in percent (0–100), shared by the editor and the gallery.
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 2,
            lang: Lang::Pl,
            dark_theme: true,
            background_theme: String::new(),
            output_dir: cutgut::media::get_default_output_dir()
                .to_string_lossy()
                .to_string(),
            pause_overlay: true,
            gallery_on_startup: true,
            gallery_dir: cutgut::media::default_gallery_dir()
                .to_string_lossy()
                .to_string(),
            gallery_dirs: vec![cutgut::media::default_gallery_dir()
                .to_string_lossy()
                .to_string()],
            original_cleanup: OriginalCleanup::default(),
            gallery_group: 0,
            gallery_card: 1,
            gallery_sort: 0,
            gallery_desc: true,
            export: ExportSettings::default(),
            volume: 100.0,
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|h| PathBuf::from(h).join("AppData").join("Roaming"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("CutGut")
}

impl Settings {
    pub fn load() -> Self {
        let Ok(text) = std::fs::read_to_string(config_dir().join("settings.ini")) else {
            return Settings::default();
        };
        Settings::parse(&text)
    }

    /// Reads the ini format; kept separate from the file so tests can round-trip
    /// the whole file through it.
    fn parse(text: &str) -> Self {
        let mut settings = Settings::default();
        let mut saw_version = false;
        for line in text.trim_start_matches('\u{feff}').lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "version" => {
                    saw_version = true;
                    settings.version = value.trim().parse().unwrap_or(2);
                }
                "lang" => settings.lang = Lang::from_code(value),
                "dark_theme" => settings.dark_theme = parse_bool(value, true),
                "background_theme" => {
                    settings.background_theme = crate::theme::background_preset(value.trim())
                        .map_or_else(String::new, |preset| preset.id.to_owned());
                }
                "output_dir" => {
                    if !value.trim().is_empty() {
                        settings.output_dir = value.trim().to_string();
                    }
                }
                "pause_overlay" => settings.pause_overlay = parse_bool(value, true),
                "gallery_on_startup" => settings.gallery_on_startup = parse_bool(value, false),
                "gallery_dir" => {
                    if !value.trim().is_empty() {
                        settings.gallery_dir = value.trim().to_string();
                        if settings.gallery_dirs.is_empty() {
                            settings.gallery_dirs.push(settings.gallery_dir.clone());
                        }
                    }
                }
                "gallery_dirs" => {
                    let dirs: Vec<String> = value
                        .split(';')
                        .map(str::trim)
                        .filter(|dir| !dir.is_empty())
                        .map(str::to_owned)
                        .collect();
                    if !dirs.is_empty() {
                        settings.gallery_dirs = dirs;
                    }
                }
                "volume" => {
                    settings.volume = value
                        .trim()
                        .parse::<f32>()
                        .map(|volume| volume.clamp(0.0, 100.0))
                        .unwrap_or(100.0);
                }
                "gallery_group" => {
                    settings.gallery_group = value.trim().parse().unwrap_or(0);
                }
                "gallery_card" => {
                    settings.gallery_card = value.trim().parse().unwrap_or(1);
                }
                "gallery_sort" => {
                    settings.gallery_sort = value.trim().parse().unwrap_or(0);
                }
                "gallery_desc" => settings.gallery_desc = parse_bool(value, true),
                "original_cleanup" => {
                    settings.original_cleanup = match value.trim() {
                        "automatic" => OriginalCleanup::Automatic,
                        "off" => OriginalCleanup::Off,
                        _ => OriginalCleanup::Ask,
                    };
                }
                key => settings.export.read(key, value.trim()),
            }
        }
        if !saw_version {
            // Older builds defaulted this to false; enable the automatic gallery
            // once when migrating to the versioned settings file.
            settings.version = 2;
            settings.gallery_on_startup = true;
        }
        settings
    }

    /// Folder list without duplicates, with `gallery_dir` first.
    pub fn gallery_folders(&self) -> Vec<String> {
        let mut dirs: Vec<String> = Vec::new();
        for dir in std::iter::once(&self.gallery_dir).chain(self.gallery_dirs.iter()) {
            if !dir.trim().is_empty() && !dirs.iter().any(|known| known == dir) {
                dirs.push(dir.clone());
            }
        }
        dirs
    }

    pub fn save(&self) {
        let dir = config_dir();
        let text = self.encode();
        if std::fs::create_dir_all(&dir).is_ok() {
            if let Ok(mut pending) = tempfile::NamedTempFile::new_in(&dir) {
                if pending.write_all(text.as_bytes()).is_ok() {
                    let _ = pending.persist(dir.join("settings.ini"));
                }
            }
        }
    }

    /// The settings file contents, so tests can check every key without disk.
    fn encode(&self) -> String {
        let mut text = format!(
            "version={}\nlang={}\ndark_theme={}\noutput_dir={}\npause_overlay={}\ngallery_on_startup={}\ngallery_dir={}\ngallery_dirs={}\nvolume={:.0}\noriginal_cleanup={}\n",
            self.version,
            self.lang.code(),
            self.dark_theme,
            self.output_dir,
            self.pause_overlay,
            self.gallery_on_startup,
            self.gallery_dir,
            self.gallery_dirs.join(";"),
            self.volume.clamp(0.0, 100.0),
            self.original_cleanup.code(),
        );
        text.push_str(&format!(
            "gallery_group={}\ngallery_card={}\ngallery_sort={}\ngallery_desc={}\n",
            self.gallery_group, self.gallery_card, self.gallery_sort, self.gallery_desc
        ));
        text.push_str(&format!("background_theme={}\n", self.background_theme));
        text.push_str(&self.export.encode());
        text
    }
}

fn parse_bool(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gallery_folders_keep_the_primary_folder_first() {
        let mut settings = Settings {
            gallery_dir: "C:/a".to_owned(),
            gallery_dirs: vec!["C:/a".to_owned(), "C:/b".to_owned()],
            ..Settings::default()
        };
        assert_eq!(settings.gallery_folders(), vec!["C:/a", "C:/b"]);
        // The primary folder is never duplicated, even when it is listed twice.
        settings.gallery_dirs.insert(0, "C:/a".to_owned());
        assert_eq!(settings.gallery_folders(), vec!["C:/a", "C:/b"]);
        settings.gallery_dirs = vec!["  ".to_owned()];
        assert_eq!(settings.gallery_folders(), vec!["C:/a"]);
    }

    #[test]
    fn gallery_layout_and_sort_round_trip() {
        let settings = Settings {
            lang: Lang::En,
            gallery_group: 2,
            gallery_card: 0,
            gallery_sort: 3,
            gallery_desc: false,
            ..Settings::default()
        };
        let title = settings.encode();
        for key in [
            "gallery_group=2",
            "gallery_card=0",
            "gallery_sort=3",
            "gallery_desc=false",
        ] {
            assert!(title.contains(key), "missing {key}");
        }
        let restored = Settings::parse(&title);
        assert_eq!(restored.gallery_group, 2);
        assert_eq!(restored.gallery_card, 0);
        assert_eq!(restored.gallery_sort, 3);
        assert!(!restored.gallery_desc);
        assert_eq!(restored.lang, Lang::En);
    }

    #[test]
    fn export_preferences_round_trip_and_ignore_invalid_values() {
        let chosen = ExportSettings {
            profile: 5,
            crop: 3,
            limit: 4,
            custom_mb: 73.5,
            codec: VideoCodec::Av1,
            engine: 2,
            format: 1,
        };
        let mut loaded = ExportSettings::default();
        for line in chosen.encode().lines() {
            let (key, value) = line.split_once('=').unwrap();
            loaded.read(key, value);
        }
        loaded.read("export_format", "500");
        loaded.read("export_custom_mb", "NaN");
        assert_eq!(loaded, chosen);
    }
}
