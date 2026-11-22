use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::settings::config_dir;

#[derive(Clone, Default)]
pub struct Library {
    pub imported: BTreeSet<PathBuf>,
    pub favorites: BTreeSet<PathBuf>,
    pub export_games: BTreeMap<PathBuf, String>,
}

impl Library {
    pub fn load() -> Self {
        let path = config_dir().join("gallery.txt");
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut library = Self::default();
        for line in text.lines() {
            if let Some((tag, path)) = line.split_once('\t') {
                if tag == "export" {
                    if let Ok((path, game)) = serde_json::from_str::<(PathBuf, String)>(path) {
                        library.export_games.insert(path, game);
                    }
                    continue;
                }
                let path = PathBuf::from(path);
                if path.is_absolute() {
                    match tag {
                        "import" => library.imported.insert(path),
                        "favorite" => library.favorites.insert(path),
                        _ => false,
                    };
                }
            }
        }
        library
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        let pending = dir.join(format!("gallery.{}.tmp", std::process::id()));
        let mut text = String::new();
        for (tag, paths) in [("import", &self.imported), ("favorite", &self.favorites)] {
            use std::fmt::Write as _;
            for path in paths {
                writeln!(text, "{tag}\t{}", path.display()).expect("writing to String");
            }
        }
        for (path, game) in &self.export_games {
            if let Ok(entry) = serde_json::to_string(&(path, game)) {
                text.push_str(&format!("export\t{entry}\n"));
            }
        }
        std::fs::write(&pending, text)?;
        let saved = std::fs::rename(&pending, dir.join("gallery.txt"));
        if saved.is_err() {
            let _ = std::fs::remove_file(&pending);
        }
        saved
    }

    pub fn replace_path(&mut self, from: &Path, to: Option<&Path>) {
        if let Some(game) = self.export_games.remove(from) {
            if let Some(path) = to {
                self.export_games.insert(path.to_path_buf(), game);
            }
        }
        for paths in [&mut self.imported, &mut self.favorites] {
            if paths.remove(from) {
                if let Some(path) = to {
                    paths.insert(path.to_path_buf());
                }
            }
        }
    }
}
