use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Write};
use std::path::Path;
use std::process::Command;

use eframe::egui;
use serde_json::Value;

fn title_key(title: &str) -> String {
    title
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn search_term(name: &str) -> &str {
    let name = name.trim();
    let stem = name
        .rsplit_once('.')
        .filter(|(_, ext)| ext.eq_ignore_ascii_case("exe"));
    match stem.map_or(name, |(stem, _)| stem.trim()) {
        "Overwatch 2" => "Overwatch",
        title => title,
    }
}

fn encode_path_segment(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn matching_icon(results: &Value, name: &str) -> Option<String> {
    let key = title_key(search_term(name));
    let mut matches = results.as_array()?.iter().filter(|game| {
        game["name"]
            .as_str()
            .is_some_and(|title| title_key(title) == key)
    });
    let found = matches.next()?;
    if matches.next().is_some() {
        return None;
    }

    let app_id = match &found["appid"] {
        Value::String(id) => id.parse::<u32>().ok()?,
        id => u32::try_from(id.as_u64()?).ok()?,
    };
    let icon = found["icon"].as_str()?.strip_prefix("https://")?;
    let (host, path) = icon.split_once('/')?;
    if !host.ends_with(".steamstatic.com") && !host.ends_with(".akamaihd.net") {
        return None;
    }
    let file = path
        .strip_prefix("community_assets/images/apps/")
        .or_else(|| path.strip_prefix("steamcommunity/public/images/apps/"))?
        .strip_prefix(&format!("{app_id}/"))?;
    let (hash, extension) = file.rsplit_once('.')?;
    if hash.len() != 40
        || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !matches!(extension, "jpg" | "png")
    {
        return None;
    }
    Some(format!(
        "https://shared.fastly.steamstatic.com/community_assets/images/apps/{app_id}/{file}"
    ))
}

fn get(url: &str) -> Option<Vec<u8>> {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let path = std::path::PathBuf::from(std::env::var_os("SystemRoot")?)
            .join("System32")
            .join("curl.exe");
        let mut command = Command::new(path);
        command.creation_flags(cutgut::ffmpeg::CREATE_NO_WINDOW);
        command
    };
    #[cfg(not(windows))]
    let mut command = Command::new("curl");

    let output = command
        .args([
            "-q",
            "--fail",
            "--silent",
            "--proto",
            "=https",
            "--connect-timeout",
            "3",
            "--max-time",
            "6",
            "--max-filesize",
            "262144",
            "--",
            url,
        ])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn decode(bytes: &[u8]) -> Option<egui::ColorImage> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(512);
    limits.max_image_height = Some(512);
    reader.limits(limits);
    let rgba = reader.decode().ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, &rgba))
}

pub fn load(name: &str, cache: &Path) -> Option<egui::ColorImage> {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let path = cache.join(format!("{:016x}.img", hasher.finish()));
    if let Some(icon) = fs::read(&path).ok().and_then(|bytes| decode(&bytes)) {
        return Some(icon);
    }

    let term = search_term(name);
    if title_key(term).is_empty() {
        return None;
    }
    let url = format!(
        "https://steamcommunity.com/actions/SearchApps/{}?l=english",
        encode_path_segment(term)
    );
    let results = serde_json::from_slice(&get(&url)?).ok()?;
    let bytes = get(&matching_icon(&results, name)?)?;
    let icon = decode(&bytes)?;
    if fs::create_dir_all(cache).is_ok() {
        if let Ok(mut file) = tempfile::NamedTempFile::new_in(cache) {
            if file.write_all(&bytes).is_ok() {
                let _ = file.persist(&path);
            }
        }
    }
    Some(icon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_steam_titles_without_using_similar_games() {
        let icon = "https://shared.fastly.steamstatic.com/community_assets/images/apps/2357570/0123456789012345678901234567890123456789.jpg";
        let results = serde_json::json!([
            {"appid": "12", "name": "Overwatch Playtest", "icon": icon},
            {"appid": 2357570, "name": "Overwatch®", "icon": icon}
        ]);
        assert_eq!(
            matching_icon(&results, "Overwatch 2").as_deref(),
            Some(icon)
        );

        let results = serde_json::json!([{"appid": "1", "name": "Minecraft Dungeons"}]);
        assert_eq!(matching_icon(&results, "Minecraft"), None);
        assert_eq!(search_term("Some Game.EXE"), "Some Game");
        assert_eq!(encode_path_segment("RV There Yet?"), "RV%20There%20Yet%3F");
    }

    #[test]
    #[ignore = "requires access to Steam Community and the Steam CDN"]
    fn downloads_and_caches_official_steam_icons() {
        let cache = tempfile::tempdir().unwrap();
        let steam_games = [
            "Aimlabs",
            "Among Us",
            "Counter-Strike 2",
            "ASKA",
            "How to Fish",
            "Overwatch 2",
            "Paladins",
            "RV There Yet?",
            "Shift At Midnight",
        ];
        for name in steam_games {
            assert!(load(name, cache.path()).is_some(), "{name}");
        }
        assert_eq!(
            fs::read_dir(cache.path()).unwrap().count(),
            steam_games.len()
        );
        for name in ["Minecraft", "Valorant"] {
            assert!(load(name, cache.path()).is_none(), "{name}");
        }
    }
}
