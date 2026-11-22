//! Media probing, thumbnails, filmstrips, output naming and gallery scanning.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ffmpeg;
use crate::i18n;
use crate::plan::{CropBox, VideoInfo};

pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "webm", "flv", "ts", "m4v", "wmv", "mpg", "mpeg", "m2ts", "mts",
    "3gp",
];

/// Metadata probes give up here; a slower answer means a stuck filesystem.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// A single decoded frame, or the whole filmstrip grid, must be quick.
const THUMBNAIL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
const FILMSTRIP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);

pub fn get_base_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("CUTGUT_BASE_DIR") {
        let p = PathBuf::from(custom);
        if p.is_dir() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.to_path_buf();
        }
    }
    PathBuf::from(".")
}

pub fn get_default_output_dir() -> PathBuf {
    let dir = get_base_dir().join("outputs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn get_temp_dir() -> PathBuf {
    let dir = get_base_dir().join("temp");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Deletes leftover `.cg_tmp_*` intermediates from `dir`.
///
/// An encode writes into a hidden temp file next to the output and renames it on
/// success, so a crash or a kill leaves it behind in the user's output folder
/// forever. Only files at least `max_age` old are removed, which keeps a second
/// running instance from losing an encode that is still in flight.
pub fn sweep_stale_temp_files(dir: &Path, max_age: std::time::Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let now = std::time::SystemTime::now();
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(".cg_tmp_") || !path.is_file() {
            continue;
        }
        let age = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok());
        if age.map(|age| age >= max_age).unwrap_or(false) && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Returns the NVIDIA clips folder (`Videos/NVIDIA`) only when it exists.
pub fn nvidia_gallery_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)?;
    let dir = home.join("Videos").join("NVIDIA");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

pub fn default_gallery_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let nvidia = home.join("Videos").join("NVIDIA");
    if nvidia.is_dir() {
        return nvidia;
    }
    let videos = home.join("Videos");
    if videos.is_dir() {
        return videos;
    }
    home
}

pub fn is_video_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .map(|e| VIDEO_EXTS.contains(&e.as_str()))
        .unwrap_or(false)
}

/// Why a proposed file name cannot be used as-is on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProblem {
    /// Empty, or a bare `.` / `..`.
    Empty,
    /// Contains one of `<>:"/\\|?*` or a control character.
    IllegalChar,
    /// Ends with a dot or a space, which Windows silently strips.
    TrailingDot,
    /// A DOS device name such as `CON`, `NUL` or `COM1`.
    Reserved,
}

/// Validates a single file-name component (no directory part).
///
/// Used before renaming clips: every name accepted here can be created with
/// `fs::rename`, so the caller never has to retry dozens of times against an
/// error that can never succeed.
pub fn check_file_name(name: &str) -> Result<(), NameProblem> {
    if name.trim().is_empty() || name.trim() == "." || name.trim() == ".." {
        return Err(NameProblem::Empty);
    }
    if name.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return Err(NameProblem::IllegalChar);
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(NameProblem::TrailingDot);
    }
    // `CON.mp4` is still the console device, so only the stem before the first
    // dot decides, and Windows compares it case-insensitively.
    let stem = name.split('.').next().unwrap_or(name);
    let stem = stem.trim_end().to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return Err(NameProblem::Reserved);
    }
    if stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.as_bytes()[3].is_ascii_digit()
    {
        return Err(NameProblem::Reserved);
    }
    Ok(())
}

pub fn read_cutgut_game(path: &Path) -> Option<String> {
    probe_cutgut_game(path).ok().flatten()
}

pub fn probe_cutgut_game(path: &Path) -> Result<Option<String>, String> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("mp4") && !extension.eq_ignore_ascii_case("mkv") {
        return Ok(None);
    }
    let args = [
        "-v",
        "error",
        "-show_entries",
        "format_tags=cutgut_game",
        "-of",
        "default=noprint_wrappers=1",
        "-i",
        &path.to_string_lossy(),
    ]
    .map(str::to_owned);
    let output = ffmpeg::run_ffprobe(&args)?;
    let Some((key, value)) = output.trim().split_once('=') else {
        return Ok(None);
    };
    let game = value.trim();
    if key.eq_ignore_ascii_case("TAG:cutgut_game")
        && !game.is_empty()
        && game.len() <= 512
        && !game.chars().any(char::is_control)
    {
        Ok(Some(game.to_owned()))
    } else {
        Ok(None)
    }
}

/// Parses `ffprobe -of flat` output into a key/value map.
pub fn parse_flat(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            value[1..value.len() - 1].replace("\\\"", "\"")
        } else {
            value.to_string()
        };
        map.insert(key.trim().to_string(), value);
    }
    map
}

pub fn parse_fps(text: &str) -> Option<f64> {
    let text = text.trim();
    if let Some((num, den)) = text.split_once('/') {
        let num: f64 = num.trim().parse().ok()?;
        let den: f64 = den.trim().parse().ok()?;
        if den > 0.0 {
            return Some(num / den);
        }
        return None;
    }
    text.parse::<f64>().ok()
}

/// Duration of a media file in seconds, or `None` when it cannot be probed.
/// Unlike [`probe_video`] this never guesses (no fallback duration), so a
/// missing value reliably means "unknown" (e.g. for gallery badges).
pub fn probe_duration(path: &Path) -> Option<f64> {
    if !path.is_file() {
        return None;
    }
    let args: Vec<String> = [
        "-v",
        "error",
        "-show_entries",
        "format=duration:stream=duration",
        "-of",
        "flat",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(std::iter::once(path.to_string_lossy().into_owned()))
    .collect();
    let output = ffmpeg::run_ffprobe(&args).ok()?;
    duration_from_flat(&parse_flat(&output))
}

fn duration_from_flat(map: &HashMap<String, String>) -> Option<f64> {
    let mut duration = map
        .get("format.duration")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    if duration <= 0.0 {
        for index in 0..32 {
            let value = map
                .get(&format!("streams.stream.{index}.duration"))
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            if value > 0.0 {
                duration = value;
                break;
            }
        }
    }
    (duration > 0.0 && duration.is_finite()).then_some(duration)
}

/// `ffprobe` arguments used by both probe flavours.
///
/// Rotation only shows up in `stream_side_data`; `side_data` is left out for the
/// fallback call because very old `ffprobe` builds reject the section and would
/// fail the whole probe over a field only used to swap width and height.
fn probe_args(path: &str, side_data: bool) -> Vec<String> {
    let stream = "stream=codec_name,codec_type,width,height,r_frame_rate,duration,pix_fmt,\
                  color_transfer,color_primaries,color_space,channels";
    let entries = if side_data {
        format!("{stream}:stream_side_data:format=duration,bit_rate")
    } else {
        format!("{stream}:format=duration,bit_rate")
    };
    [
        "-v",
        "error",
        "-show_entries",
        &entries,
        "-of",
        "flat",
        path,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Flat `ffprobe` map for `path`, with one retry that drops the side-data section.
fn probe_flat(path: &str) -> Result<HashMap<String, String>, String> {
    match ffmpeg::run_ffprobe(&probe_args(path, true)) {
        Ok(out) => Ok(parse_flat(&out)),
        Err(_) => ffmpeg::run_ffprobe(&probe_args(path, false)).map(|out| parse_flat(&out)),
    }
}

/// Where the picture really starts and how long it runs.
///
/// Used to check a stream-copied cut: `-c copy` can only start on a keyframe, so
/// a trim that misses one comes back with extra footage in front and less at the
/// end while still reporting the requested duration.
pub fn content_span(path: &str) -> Result<(f64, f64), String> {
    let args: Vec<String> = [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=duration",
        "-show_entries",
        "format=duration",
        "-show_entries",
        "packet=pts_time",
        "-read_intervals",
        "%+#1",
        "-of",
        "flat",
        path,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let map = parse_flat(&ffmpeg::run_ffprobe(&args)?);
    // The video stream's own duration is the honest one: a container duration
    // that includes a slightly longer audio track must not look like drift.
    let duration = map
        .get("streams.stream.0.duration")
        .or_else(|| map.get("format.duration"))
        .and_then(|v| v.parse::<f64>().ok())
        .ok_or_else(|| format!("{}: {path}", i18n::t("err_probe_failed")))?;
    let start = map
        .get("packets.packet.0.pts_time")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    Ok((start, duration))
}

/// Probes a video for display purposes (gallery badges, thumbnails).
///
/// Never fails on unreadable files: it falls back to guesses parsed out of
/// FFmpeg's own banner. Use [`probe_video_strict`] wherever the numbers drive
/// an encode.
pub fn probe_video(path: &str) -> Result<VideoInfo, String> {
    if !Path::new(path).exists() {
        return Err(format!("{}: {path}", i18n::t("err_file_missing")));
    }

    match probe_flat(path) {
        Ok(map) => match info_from_flat(&map) {
            Some(info) => Ok(info),
            None => Ok(fallback_probe(path)),
        },
        Err(_) => Ok(fallback_probe(path)),
    }
}

/// Probes a video for encoding decisions.
///
/// Unlike [`probe_video`] this never invents metadata: the target-size maths,
/// the stream-copy decision and the retry scaling are all derived from these
/// numbers, so a failed probe has to abort the job instead of quietly producing
/// a file that breaks the guaranteed size limit.
pub fn probe_video_strict(path: &str) -> Result<VideoInfo, String> {
    if !Path::new(path).exists() {
        return Err(format!("{}: {path}", i18n::t("err_file_missing")));
    }

    let map = probe_flat(path)?;
    let info =
        info_from_flat(&map).ok_or_else(|| format!("{}: {path}", i18n::t("err_probe_failed")))?;
    if !(info.duration.is_finite() && info.duration > 0.0) {
        return Err(format!("{}: {path}", i18n::t("err_no_duration")));
    }
    Ok(info)
}

fn info_from_flat(map: &HashMap<String, String>) -> Option<VideoInfo> {
    let mut video_index = None;
    let mut audio_codec = None;
    let mut audio_channels = 0u32;

    for index in 0..32 {
        let prefix = format!("streams.stream.{index}");
        let Some(kind) = map.get(&format!("{prefix}.codec_type")) else {
            continue;
        };
        if kind == "video" && video_index.is_none() {
            video_index = Some(index);
        }
        if kind == "audio" && audio_codec.is_none() {
            audio_codec = map.get(&format!("{prefix}.codec_name")).cloned();
            audio_channels = map
                .get(&format!("{prefix}.channels"))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
        }
    }

    let index = video_index?;
    let prefix = format!("streams.stream.{index}");

    let width = map
        .get(&format!("{prefix}.width"))
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1920)
        .max(1);
    let height = map
        .get(&format!("{prefix}.height"))
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1080)
        .max(1);
    let codec = map
        .get(&format!("{prefix}.codec_name"))
        .cloned()
        .unwrap_or_else(|| "h264".into());

    let fps = map
        .get(&format!("{prefix}.r_frame_rate"))
        .and_then(|v| parse_fps(v))
        .unwrap_or(60.0);

    let mut duration = map
        .get("format.duration")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    if duration <= 0.0 {
        duration = map
            .get(&format!("{prefix}.duration"))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
    }

    let bitrate = map
        .get("format.bit_rate")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    Some(VideoInfo {
        duration,
        width,
        height,
        fps: (fps * 100.0).round() / 100.0,
        bitrate,
        codec,
        audio_codec,
        rotation: rotation_of(map, &prefix),
        pix_fmt: field(map, &prefix, "pix_fmt"),
        color_transfer: field(map, &prefix, "color_transfer"),
        audio_channels,
    })
}

fn field(map: &HashMap<String, String>, prefix: &str, name: &str) -> String {
    map.get(&format!("{prefix}.{name}"))
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Rotation from the display matrix (phones write -90/90 for portrait clips),
/// with the legacy `rotate` tag as a fallback for older muxers.
fn rotation_of(map: &HashMap<String, String>, prefix: &str) -> i32 {
    for index in 0..4 {
        let key = format!("{prefix}.side_data_list.side_data.{index}.rotation");
        if let Some(value) = map.get(&key).and_then(|v| v.trim().parse::<i32>().ok()) {
            return value;
        }
    }
    map.get(&format!("{prefix}.tags.rotate"))
        .and_then(|v| v.trim().parse::<i32>().ok())
        .unwrap_or(0)
}

fn fallback_probe(path: &str) -> VideoInfo {
    let args: Vec<String> = vec!["-i".into(), path.to_string()];
    let out = ffmpeg::run_process_timeout(&ffmpeg::ffmpeg_path(), &args, PROBE_TIMEOUT);
    let stderr = out
        .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
        .unwrap_or_default();

    let mut duration = 60.0;
    let mut width = 1920u32;
    let mut height = 1080u32;
    let mut fps = 60.0;
    let mut codec = "h264".to_string();
    let mut audio_codec = None;

    for line in stderr.lines() {
        if line.contains("Duration:") {
            if let Some(rest) = line.split("Duration:").nth(1) {
                let value = rest.trim().split(',').next().unwrap_or("").trim();
                duration = parse_hms(value).unwrap_or(duration);
            }
        }

        if line.contains("Stream #") && line.contains("Video:") {
            let lower = line.to_ascii_lowercase();
            if let Some((w, h)) = parse_dimensions(line) {
                width = w;
                height = h;
            }
            if let Some(f) = parse_fps_near(line) {
                fps = f;
            }
            if lower.contains("hevc") || lower.contains("h265") {
                codec = "hevc".into();
            } else if lower.contains("av1") {
                codec = "av1".into();
            } else if lower.contains("h264") {
                codec = "h264".into();
            }
        }

        if line.contains("Stream #") && line.contains("Audio:") {
            audio_codec = Some("aac".into());
        }
    }

    VideoInfo {
        duration,
        width,
        height,
        fps,
        bitrate: 0,
        codec,
        audio_codec,
        rotation: 0,
        pix_fmt: String::new(),
        color_transfer: String::new(),
        audio_channels: 0,
    }
}

fn parse_hms(value: &str) -> Option<f64> {
    let mut parts = value.split(':');
    let hours: f64 = parts.next()?.trim().parse().ok()?;
    let minutes: f64 = parts.next()?.trim().parse().ok()?;
    let seconds: f64 = parts.next()?.trim().parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn parse_dimensions(line: &str) -> Option<(u32, u32)> {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index < bytes.len() && (bytes[index] == b'x' || bytes[index] == b'X') {
                let w: u32 = line[start..index].parse().ok()?;
                let h_start = index + 1;
                let mut h_end = h_start;
                while h_end < bytes.len() && bytes[h_end].is_ascii_digit() {
                    h_end += 1;
                }
                if h_end > h_start {
                    if let Ok(h) = line[h_start..h_end].parse::<u32>() {
                        if w >= 100 && h >= 100 {
                            return Some((w, h));
                        }
                    }
                }
            }
        } else {
            index += 1;
        }
    }
    None
}

fn parse_fps_near(line: &str) -> Option<f64> {
    let index = line.find(" fps")?;
    let prefix = &line[..index];
    let token = prefix.split_whitespace().last()?;
    token.parse::<f64>().ok()
}

pub fn extract_frame_png(
    input_path: &str,
    timestamp_s: f64,
    output_path: &str,
    crop_box: Option<&CropBox>,
) -> Result<PathBuf, String> {
    if !Path::new(input_path).exists() {
        return Err(format!("{}: {input_path}", i18n::t("err_file_not_found")));
    }
    if let Some(parent) = Path::new(output_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut args: Vec<String> = vec![
        "-y".into(),
        "-ss".into(),
        format!("{:.3}", timestamp_s.max(0.0)),
        "-i".into(),
        input_path.into(),
    ];
    if let Some(crop) = crop_box {
        if crop.ratio_type != "original" {
            args.extend(["-vf".into(), crop.to_filter()]);
        }
    }
    args.extend([
        "-vframes".into(),
        "1".into(),
        "-q:v".into(),
        "1".into(),
        output_path.into(),
    ]);

    let out = ffmpeg::run_process_timeout(&ffmpeg::ffmpeg_path(), &args, THUMBNAIL_TIMEOUT)?;
    if !out.status.success() || !Path::new(output_path).exists() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{} {}", i18n::t("err_thumbnail"), err.trim()));
    }
    Ok(PathBuf::from(output_path))
}

pub fn generate_thumbnail(
    input_path: &str,
    output_path: &str,
    timestamp_s: Option<f64>,
) -> Result<PathBuf, String> {
    if !Path::new(input_path).exists() {
        return Err(format!("{}: {input_path}", i18n::t("err_file_not_found")));
    }
    if let Some(parent) = Path::new(output_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let at = match timestamp_s {
        Some(t) => t,
        None => {
            let duration = probe_video(input_path).map(|i| i.duration).unwrap_or(0.0);
            if duration > 0.0 {
                (duration * 0.15).clamp(0.4, (duration - 0.1).max(0.4))
            } else {
                0.5
            }
        }
    };

    let args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-threads".into(),
        "2".into(),
        "-ss".into(),
        format!("{at:.3}"),
        "-i".into(),
        input_path.into(),
        "-frames:v".into(),
        "1".into(),
        "-filter_threads".into(),
        "1".into(),
        "-vf".into(),
        "scale=480:-2:flags=bilinear".into(),
        "-threads".into(),
        "1".into(),
        "-q:v".into(),
        "4".into(),
        output_path.into(),
    ];

    let out = ffmpeg::run_process_timeout(&ffmpeg::ffmpeg_path(), &args, THUMBNAIL_TIMEOUT)?;
    if !out.status.success() || !Path::new(output_path).exists() {
        return Err(i18n::t("err_thumbnail_failed").to_string());
    }
    Ok(PathBuf::from(output_path))
}

pub fn generate_filmstrip(
    input_path: &str,
    output_path: &str,
    count: u32,
    thumb_w: u32,
) -> Result<PathBuf, String> {
    if !Path::new(input_path).exists() {
        return Err(format!("{}: {input_path}", i18n::t("err_file_not_found")));
    }
    if let Some(parent) = Path::new(output_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let info = probe_video_strict(input_path)?;
    let duration = info.duration.max(0.1);
    let count = count.max(1);
    let fps = count as f64 / duration;

    // Keep the native aspect ratio (16:9, 4:3, vertical...) for the thumbnails.
    // Autorotation runs before the filters, so a rotated clip is already portrait
    // by the time the grid is drawn.
    let (frame_w, frame_h) = info.display_size();
    let aspect = frame_w as f64 / frame_h.max(1) as f64;
    let thumb_h = ((thumb_w as f64 / aspect.max(0.1)).max(1.0)) as u32;
    let thumb_w = if thumb_w % 2 == 0 {
        thumb_w
    } else {
        thumb_w - 1
    };
    let thumb_h = if thumb_h % 2 == 0 {
        thumb_h
    } else {
        thumb_h - 1
    };

    let vf = format!(
        "fps={fps:.6},scale={thumb_w}:{thumb_h}:force_original_aspect_ratio=increase,\
         crop={thumb_w}:{thumb_h},tile={count}x1"
    );

    let args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-i".into(),
        input_path.into(),
        "-vf".into(),
        vf,
        "-frames:v".into(),
        "1".into(),
        output_path.into(),
    ];

    let out = ffmpeg::run_process_timeout(&ffmpeg::ffmpeg_path(), &args, FILMSTRIP_TIMEOUT)?;
    if !out.status.success() || !Path::new(output_path).exists() {
        return Err(i18n::t("err_filmstrip_failed").to_string());
    }
    Ok(PathBuf::from(output_path))
}

// ---------------------------------------------------------------------------
// Output naming
// ---------------------------------------------------------------------------

pub fn sanitize_base_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "CutGut".into()
    } else {
        cleaned
    }
}

pub fn generate_output_filepath(output_dir: &str, base_name: &str, extension: &str) -> PathBuf {
    generate_output_filepath_dated(output_dir, base_name, extension, &today_string())
}

/// Like [`generate_output_filepath`], but with an explicit date stamp.
///
/// Exports of an existing recording keep that recording's date instead of the
/// day the cut was made, so the output sorts and reads like the source.
pub fn generate_output_filepath_dated(
    output_dir: &str,
    base_name: &str,
    extension: &str,
    date: &str,
) -> PathBuf {
    let target_dir = if !output_dir.is_empty() && Path::new(output_dir).exists() {
        PathBuf::from(output_dir)
    } else {
        get_default_output_dir()
    };
    let _ = std::fs::create_dir_all(&target_dir);

    let mut ext = extension.trim().to_string();
    if !ext.starts_with('.') {
        ext = format!(".{ext}");
    }

    let base = sanitize_base_name(base_name);
    let filename = format!("{base} - CG {date}{ext}");

    let candidate = target_dir.join(&filename);
    if !candidate.exists() {
        return candidate;
    }

    let root = filename.trim_end_matches(&ext).to_string();
    let mut counter = 2;
    loop {
        let candidate = target_dir.join(format!("{root} ({counter}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Formats an epoch timestamp (seconds) as `YYYY-MM-DD HH:MM` (UTC).
pub fn format_date_time(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let rem = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// Returns the current local date as `YYYY-MM-DD` (UTC days converted to a civil date).
pub fn today_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Gallery scanning
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Clip {
    pub path: PathBuf,
    pub group: String,
    pub size: u64,
    pub mtime: u64,
}

impl Clip {
    pub fn from_video_path(path: &Path, group: &str) -> Option<Self> {
        if path.is_file() && is_video_file(&path.to_string_lossy()) {
            Some(make_clip(path, group))
        } else {
            None
        }
    }

    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
    }
}

pub fn is_gallery_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["png", "jpg", "jpeg"]
                .iter()
                .any(|kind| ext.eq_ignore_ascii_case(kind))
        })
}

pub fn is_gallery_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            [
                "mp3", "m4a", "aac", "opus", "ogg", "wav", "flac", "wma", "aiff", "aif",
            ]
            .iter()
            .any(|kind| ext.eq_ignore_ascii_case(kind))
        })
}
pub fn scan_gallery(root: &Path) -> Vec<(String, Vec<Clip>)> {
    scan_gallery_entries(root, false)
}

pub fn scan_gallery_media(root: &Path) -> Vec<(String, Vec<Clip>)> {
    scan_gallery_entries(root, true)
}

fn scan_gallery_entries(root: &Path, include_media: bool) -> Vec<(String, Vec<Clip>)> {
    let mut groups: Vec<(String, Vec<Clip>)> = Vec::new();
    if !root.is_dir() {
        return groups;
    }

    let root_name = root
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Clips".into());

    let mut root_clips: Vec<Clip> = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    entries.sort_by_key(|p| {
        p.file_name()
            .map(|f| f.to_string_lossy().to_ascii_lowercase())
    });

    for path in entries {
        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            let clips = collect_clips(&path, &name, include_media);
            if !clips.is_empty() {
                groups.push((name, clips));
            }
        } else if path.is_file()
            && (is_video_file(&name)
                || (include_media && (is_gallery_image(&path) || is_gallery_audio(&path))))
        {
            root_clips.push(make_clip(&path, &root_name));
        }
    }

    if !root_clips.is_empty() {
        root_clips.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        groups.insert(0, (root_name, root_clips));
    }

    groups.sort_by_key(|(name, _)| name.to_ascii_lowercase());
    groups
}

fn collect_clips(folder: &Path, group: &str, include_media: bool) -> Vec<Clip> {
    let mut clips = Vec::new();
    let mut stack = vec![folder.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file()
                && (is_video_file(&name)
                    || (include_media && (is_gallery_image(&path) || is_gallery_audio(&path))))
            {
                clips.push(make_clip(&path, group));
            }
        }
    }

    clips.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    clips
}

fn make_clip(path: &Path, group: &str) -> Clip {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Clip {
        path: std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
        group: group.to_string(),
        size,
        mtime,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_parsing() {
        assert!((parse_fps("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse_fps("60"), Some(60.0));
    }

    #[test]
    fn display_matrix_rotation_and_hdr_fields_are_read() {
        let raw = "streams.stream.0.codec_name=\"hevc\"
                   streams.stream.0.codec_type=\"video\"
                   streams.stream.0.width=1920
                   streams.stream.0.height=1080
                   streams.stream.0.pix_fmt=\"yuv420p10le\"
                   streams.stream.0.color_transfer=\"smpte2084\"
                   streams.stream.0.color_primaries=\"bt2020\"
                   streams.stream.0.side_data_list.side_data.0.side_data_type=\"Display Matrix\"
                   streams.stream.0.side_data_list.side_data.0.rotation=-90
                   streams.stream.1.codec_type=\"audio\"
                   streams.stream.1.codec_name=\"aac\"
                   streams.stream.1.channels=6
                   format.duration=12.5
";
        let info = info_from_flat(&parse_flat(raw)).expect("video info");
        assert_eq!(info.rotation, -90);
        assert_eq!(info.display_size(), (1080, 1920));
        assert_eq!(info.audio_channels, 6);
        assert!(info.is_hdr());
        assert!(info.is_deep_color());
        // nv12 is 8-bit despite the "12" in its name.
        let mut eight_bit = info.clone();
        eight_bit.pix_fmt = "nv12".into();
        assert!(!eight_bit.is_deep_color());
    }

    #[test]
    fn legacy_rotate_tag_is_still_understood() {
        let raw = "streams.stream.0.codec_type=\"video\"
                   streams.stream.0.width=1280
                   streams.stream.0.height=720
                   streams.stream.0.tags.rotate=90
                   format.duration=3
";
        let info = info_from_flat(&parse_flat(raw)).expect("video info");
        assert_eq!(info.rotation, 90);
        assert_eq!(info.display_size(), (720, 1280));
        assert!(!info.is_hdr());
    }

    #[test]
    fn flat_probe_parsing() {
        let raw = "streams.stream.0.codec_name=\"h264\"\n\
                   streams.stream.0.codec_type=\"video\"\n\
                   streams.stream.0.width=1920\n\
                   streams.stream.0.height=1080\n\
                   streams.stream.0.r_frame_rate=\"60/1\"\n\
                   streams.stream.1.codec_type=\"audio\"\n\
                   streams.stream.1.codec_name=\"aac\"\n\
                   format.duration=12.5\n\
                   format.bit_rate=8000000\n";
        let map = parse_flat(raw);
        let info = info_from_flat(&map).expect("video info");
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
        assert_eq!(info.fps, 60.0);
        assert_eq!(info.duration, 12.5);
        assert_eq!(info.audio_codec.as_deref(), Some("aac"));
        assert!(info.has_audio());
    }

    #[test]
    fn duration_parsing_prefers_format_and_falls_back_to_stream() {
        let map = parse_flat("format.duration=12.5\nstreams.stream.0.duration=99.0\n");
        assert_eq!(duration_from_flat(&map), Some(12.5));
        let map = parse_flat("streams.stream.0.duration=99.0\n");
        assert_eq!(duration_from_flat(&map), Some(99.0));
        let map = parse_flat("format.duration=N/A\n");
        assert_eq!(duration_from_flat(&map), None);
        assert_eq!(probe_duration(Path::new("does-not-exist.mp4")), None);
    }

    #[test]
    fn naming_uses_cg_suffix_and_avoids_collisions() {
        let dir = std::env::temp_dir().join(format!("cutgut_naming_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let dir_str = dir.to_string_lossy().to_string();

        let first = generate_output_filepath(&dir_str, "clip", ".mp4");
        assert!(first.to_string_lossy().contains(" - CG "));
        std::fs::write(&first, b"x").unwrap();

        let second = generate_output_filepath(&dir_str, "clip", ".mp4");
        assert_ne!(first, second);
        assert!(second.to_string_lossy().contains("(2)"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_temp_files_are_swept_but_live_ones_are_kept() {
        let dir = std::env::temp_dir().join(format!("cutgut_sweep_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stale = dir.join(".cg_tmp_1_2_3_clip.mp4");
        let live = dir.join(".cg_tmp_4_5_6_clip.mp4");
        let user_file = dir.join("clip.mp4");
        for path in [&stale, &live, &user_file] {
            std::fs::write(path, b"x").unwrap();
        }

        // A generous age keeps everything: an in-flight encode must survive.
        assert_eq!(
            sweep_stale_temp_files(&dir, std::time::Duration::from_secs(3600)),
            0
        );
        assert!(live.exists() && stale.exists());

        // Zero age matches every leftover, but never a normal output file.
        assert_eq!(sweep_stale_temp_files(&dir, std::time::Duration::ZERO), 2);
        assert!(!stale.exists() && !live.exists());
        assert!(user_file.exists(), "user data must never be touched");

        assert_eq!(sweep_stale_temp_files(&dir, std::time::Duration::ZERO), 0);
        assert_eq!(
            sweep_stale_temp_files(&dir.join("missing"), std::time::Duration::ZERO),
            0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_name_uses_the_recording_date() {
        let dir = std::env::temp_dir().join(format!("cutgut_dated_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir_str = dir.to_string_lossy().to_string();

        let path = generate_output_filepath_dated(&dir_str, "Clip", ".mp4", "2025-03-04");
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.contains("CG 2025-03-04"), "{name}");
        assert!(!name.contains(&today_string()) || today_string() == "2025-03-04");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn date_is_iso_like() {
        let date = today_string();
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
    }

    #[test]
    fn acceptable_file_names_pass() {
        for name in [
            "Błyskawica - CG 2026 (2).mp4",
            "clip_01.mp4",
            "astral",
            "CONSOLE.mp4",
            "com10.mp4",
            "console.mp4",
        ] {
            assert_eq!(check_file_name(name), Ok(()), "{name} should be accepted");
        }
    }

    #[test]
    fn illegal_file_names_are_rejected() {
        for name in ["", "   ", ".", "..", "a/b.mp4", "a\\b.mp4", "C:clip.mp4"] {
            assert!(check_file_name(name).is_err(), "{name} should be rejected");
        }
        for name in [
            "what?.mp4",
            "star*.mp4",
            "quote\".mp4",
            "pipe|.mp4",
            "less<more>.mp4",
            "bell\u{7}.mp4",
        ] {
            assert_eq!(
                check_file_name(name),
                Err(NameProblem::IllegalChar),
                "{name}"
            );
        }
        assert_eq!(check_file_name("trailing."), Err(NameProblem::TrailingDot));
        assert_eq!(check_file_name("trailing "), Err(NameProblem::TrailingDot));
        for name in ["CON", "con.mp4", "NUL", "aux.txt", "COM1.mp4", "lpt9.mp4"] {
            assert_eq!(check_file_name(name), Err(NameProblem::Reserved), "{name}");
        }
        assert_eq!(check_file_name("CONS.mp4"), Ok(()));
        assert_eq!(check_file_name("COM.mp4"), Ok(()));
    }
}
