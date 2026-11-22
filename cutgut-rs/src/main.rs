//! CutGut Rust engine — CLI.
//!
//! The CLI exposes the same planning/encoding pipeline that the GUI uses:
//! probe, plan, encode (strict hard size limit), audio export, thumbnails,
//! filmstrips, gallery scanning and source cleanup.
//!
//! All output is bilingual: `--lang pl` (default) prints Polish, `--lang en`
//! prints English.

use cutgut::{cli_text, ffmpeg, i18n, media, plan};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ffmpeg::{CancellationToken, ProgressUpdate};
use plan::{CropBox, VideoCodec};

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    apply_language(&raw_args);
    let args = strip_global_flags(raw_args);
    if args.len() < 2 || args[1] == "menu" {
        interactive_menu();
        return;
    }

    let result = match args[1].as_str() {
        "probe" => cmd_probe(&args[2..]),
        "engines" => cmd_engines(&args[2..]),
        "plan" => cmd_plan(&args[2..]),
        "encode" => cmd_encode(&args[2..]),
        "audio" => cmd_audio(&args[2..]),
        "thumb" => cmd_thumb(&args[2..]),
        "filmstrip" => cmd_filmstrip(&args[2..]),
        "gallery" => cmd_gallery(&args[2..]),
        "cleanup" => cmd_cleanup(&args[2..]),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("{}: {other}", cli_text::t("unknown_command"))),
    };

    if let Err(err) = result {
        eprintln!("{}: {err}", cli_text::t("error"));
        std::process::exit(1);
    }
}

/// Removes the global language flag so it can be given before or after the
/// command name.
fn strip_global_flags(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--lang" {
            index += 2;
            continue;
        }
        if args[index].starts_with("--lang=") {
            index += 1;
            continue;
        }
        out.push(args[index].clone());
        index += 1;
    }
    out
}

/// Reads the language code from the raw arguments and activates it.
fn apply_language(args: &[String]) {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--lang" {
            if let Some(code) = args.get(index + 1) {
                i18n::set_lang(code);
                return;
            }
        }
        if let Some(code) = args[index].strip_prefix("--lang=") {
            i18n::set_lang(code);
            return;
        }
        index += 1;
    }
}

fn read_line() -> String {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_string()
}

fn prompt(label: &str, default: &str) -> String {
    if default.is_empty() {
        print!("{label}: ");
    } else {
        print!("{label} [{default}]: ");
    }
    let value = read_line();
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

fn interactive_menu() {
    loop {
        println!();
        println!("==============================================");
        println!("  {}", cli_text::t("menu_title"));
        println!("==============================================");
        println!("  {}", cli_text::t("menu_gallery"));
        println!("  {}", cli_text::t("menu_plan"));
        println!("  {}", cli_text::t("menu_encode"));
        println!("  {}", cli_text::t("menu_audio"));
        println!("  {}", cli_text::t("menu_still"));
        println!("  {}", cli_text::t("menu_engines"));
        println!("  {}", cli_text::t("menu_exit"));
        print!("{}: ", cli_text::t("choice"));

        match read_line().as_str() {
            "1" => {
                let dir = prompt(cli_text::t("prompt_gallery_dir"), "");
                let mut args = Vec::new();
                if !dir.is_empty() {
                    args.push(dir);
                }
                if let Err(e) = cmd_gallery(&args) {
                    println!("{}: {e}", cli_text::t("error"));
                }
            }
            "2" => {
                let input = prompt(cli_text::t("prompt_video"), "");
                if input.is_empty() {
                    println!("{}", cli_text::t("no_file"));
                    continue;
                }
                let mut args = vec![input];
                push_common_args(&mut args);
                if let Err(e) = cmd_plan(&args) {
                    println!("{}: {e}", cli_text::t("error"));
                }
            }
            "3" => {
                let input = prompt(cli_text::t("prompt_video"), "");
                if input.is_empty() {
                    println!("{}", cli_text::t("no_file"));
                    continue;
                }
                let mut args = vec![input.clone()];
                push_common_args(&mut args);
                let out = prompt(cli_text::t("prompt_output_next"), "");
                let out = if out.is_empty() {
                    let stem = Path::new(&input)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "clip".into());
                    let parent = Path::new(&input).parent().unwrap_or(Path::new("."));
                    parent
                        .join(format!("{stem} - CG.mp4"))
                        .to_string_lossy()
                        .to_string()
                } else {
                    out
                };
                args.push("--out".into());
                args.push(out);
                args.push("--quiet".into());
                eprintln!();
                if let Err(e) = cmd_encode(&args) {
                    println!("{}: {e}", cli_text::t("error"));
                }
            }
            "4" => {
                let input = prompt(cli_text::t("prompt_video"), "");
                if input.is_empty() {
                    continue;
                }
                let mut args = vec![input.clone()];
                push_common_args(&mut args);
                let out = prompt(cli_text::t("prompt_output"), "");
                if out.is_empty() {
                    continue;
                }
                let format = prompt(cli_text::t("prompt_format"), "m4a");
                args.push("--out".into());
                args.push(out);
                args.push("--format".into());
                args.push(format);
                if let Err(e) = cmd_audio(&args) {
                    println!("{}: {e}", cli_text::t("error"));
                }
            }
            "5" => {
                let input = prompt(cli_text::t("prompt_video"), "");
                if input.is_empty() {
                    continue;
                }
                let mode = prompt(cli_text::t("prompt_what"), "t");
                let out = prompt(cli_text::t("prompt_output_image"), "");
                if out.is_empty() {
                    continue;
                }
                let mut args = vec![input];
                args.push("--out".into());
                args.push(out);
                let result = if mode.eq_ignore_ascii_case("f") {
                    cmd_filmstrip(&args)
                } else {
                    cmd_thumb(&args)
                };
                if let Err(e) = result {
                    println!("{}: {e}", cli_text::t("error"));
                }
            }
            "6" => {
                if let Err(e) = cmd_engines(&[]) {
                    println!("{}: {e}", cli_text::t("error"));
                }
            }
            "0" | "q" | "exit" => break,
            other => println!("{}: {other}", cli_text::t("unknown_choice")),
        }
    }
}

fn push_common_args(args: &mut Vec<String>) {
    let start = prompt(cli_text::t("prompt_start"), "0");
    let end = prompt(cli_text::t("prompt_end"), "");
    let target = prompt(cli_text::t("prompt_target"), "20");
    let codec = prompt(cli_text::t("prompt_codec"), "h264");
    let speed = prompt(cli_text::t("prompt_speed"), "hq");
    let cpu = prompt(cli_text::t("prompt_cpu"), "N");

    args.extend(["--start".into(), start]);
    if !end.is_empty() {
        args.extend(["--end".into(), end]);
    }
    args.extend(["--target".into(), target]);
    args.extend(["--codec".into(), codec]);
    args.extend(["--speed".into(), speed]);
    if cpu.eq_ignore_ascii_case("t") || cpu.eq_ignore_ascii_case("y") {
        args.push("--cpu".into());
    }
}

fn print_usage() {
    println!("{}", cli_text::t("usage"));
}

fn parse_args(args: &[String]) -> (Vec<String>, HashMap<String, String>) {
    let mut positional = Vec::new();
    let mut flags = HashMap::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if let Some(key) = arg.strip_prefix("--") {
            if let Some(next) = args.get(index + 1) {
                if !next.starts_with("--") {
                    flags.insert(key.to_string(), next.clone());
                    index += 2;
                    continue;
                }
            }
            flags.insert(key.to_string(), "true".into());
            index += 1;
        } else {
            positional.push(arg.clone());
            index += 1;
        }
    }
    (positional, flags)
}

fn flag_f64(flags: &HashMap<String, String>, key: &str, default: f64) -> f64 {
    flags
        .get(key)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

fn flag_str<'a>(flags: &'a HashMap<String, String>, key: &str, default: &'a str) -> &'a str {
    flags.get(key).map(|s| s.as_str()).unwrap_or(default)
}

fn flag_bool(flags: &HashMap<String, String>, key: &str) -> bool {
    flags
        .get(key)
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false)
}

fn codec_from(flags: &HashMap<String, String>) -> Result<VideoCodec, String> {
    let raw = flag_str(flags, "codec", "h264");
    VideoCodec::parse(raw)
        .ok_or_else(|| format!("{}: {raw} (h264/hevc/av1)", cli_text::t("unknown_codec")))
}

fn crop_from(info: &plan::VideoInfo, flags: &HashMap<String, String>) -> Option<CropBox> {
    let ratio = flag_str(flags, "crop", "original");
    if ratio == "original" {
        return None;
    }
    let (width, height) = info.display_size();
    Some(CropBox::calculate_default(
        width as i32,
        height as i32,
        ratio,
        flag_str(flags, "align", "center"),
    ))
}

/// Hardware encoding is the default; `--cpu` forces software encoding and wins
/// over `--gpu`. Shared by `plan` and `encode` so both always pick the same
/// encoder for the same flags.
fn prefer_hw_from(flags: &HashMap<String, String>) -> bool {
    !flag_bool(flags, "cpu")
}

fn cmd_probe(args: &[String]) -> Result<(), String> {
    let (positional, _flags) = parse_args(args);
    let path = positional
        .first()
        .ok_or_else(|| cli_text::t("need_path_probe").to_string())?;
    let info = media::probe_video_strict(path)?;
    println!("{}:        {}", cli_text::t("label_file"), path);
    let (width, height) = info.display_size();
    println!(
        "{}:       {}x{} @ {:.2} FPS",
        cli_text::t("label_image"),
        width,
        height,
        info.fps
    );
    println!(
        "{}:        {:.2} s",
        cli_text::t("label_duration"),
        info.duration
    );
    println!("{}:       {}", cli_text::t("label_codec"), info.codec);
    println!(
        "{}:       {}",
        cli_text::t("label_audio"),
        info.audio_codec
            .clone()
            .unwrap_or_else(|| cli_text::t("none").into())
    );
    if info.bitrate > 0 {
        println!(
            "{}:     {} kb/s",
            cli_text::t("label_bitrate"),
            info.bitrate / 1000
        );
    }
    Ok(())
}

fn cmd_engines(_args: &[String]) -> Result<(), String> {
    println!("{}", cli_text::t("detect_encoders"));
    let names = [
        "h264_nvenc",
        "hevc_nvenc",
        "av1_nvenc",
        "h264_amf",
        "hevc_amf",
        "av1_amf",
        "libx264",
        "libx265",
        "libsvtav1",
    ];

    for name in names {
        let ok = ffmpeg::test_encoder_support(name);
        println!("  {name:<14} {}", if ok { "OK" } else { "-" });
    }

    for encoder in ["h264_nvenc", "hevc_nvenc", "av1_nvenc"] {
        if ffmpeg::test_encoder_support(encoder) {
            let opts = ffmpeg::get_nvenc_quality_options(encoder, "hq");
            println!(
                "{} {encoder}: {}",
                cli_text::t("nvenc_flags"),
                opts.join(" ")
            );
        }
    }
    Ok(())
}

fn cmd_plan(args: &[String]) -> Result<(), String> {
    let (positional, flags) = parse_args(args);
    let path = positional
        .first()
        .ok_or_else(|| cli_text::t("need_path_plan").to_string())?;

    let info = media::probe_video_strict(path)?;
    let start = flag_f64(&flags, "start", 0.0);
    let end = flag_f64(&flags, "end", info.duration);
    let target = flag_f64(&flags, "target", 20.0);
    let codec = codec_from(&flags)?;
    let speed = flag_str(&flags, "speed", "hq").to_string();
    let prefer_hw = prefer_hw_from(&flags);
    let lang = i18n::lang_code().to_string();
    let crop = crop_from(&info, &flags);

    let p = plan::calculate_plan(
        &info,
        start,
        end,
        target,
        codec,
        &speed,
        prefer_hw,
        path,
        &lang,
        crop.as_ref(),
    )?;

    println!(
        "{}:       {} ({})",
        cli_text::t("label_encoder"),
        p.encoder_name,
        p.codec.upper()
    );
    println!(
        "{}:    {:.2} s",
        cli_text::t("label_clip_duration"),
        p.duration_s
    );
    println!(
        "{}:  {:.2} MB",
        cli_text::t("label_hard_limit"),
        p.hard_limit_bytes as f64 / 1_000_000.0
    );
    println!(
        "{}:   {:.2} MB",
        cli_text::t("label_budget"),
        p.target_bytes as f64 / 1_000_000.0
    );
    println!(
        "{}:         {}x{} @ {:.0} FPS, {} kb/s",
        cli_text::t("label_video"),
        p.out_width,
        p.out_height,
        p.out_fps,
        p.video_kbps
    );
    if p.has_audio {
        println!(
            "{}:         AAC {} kb/s",
            cli_text::t("label_audio"),
            p.audio_kbps
        );
    } else {
        println!(
            "{}:         {}",
            cli_text::t("label_audio"),
            cli_text::t("none")
        );
    }
    println!(
        "{}:         {}",
        cli_text::t("label_remux"),
        if p.is_remux {
            cli_text::t("yes")
        } else {
            cli_text::t("no")
        }
    );
    if p.bitrate_capped && !p.is_remux {
        println!("  {}", cli_text::t("limit_capped"));
    }
    if p.hdr_tonemapped || p.hdr_untouched {
        println!(
            "{}:            {}",
            cli_text::t("label_hdr"),
            if p.hdr_tonemapped {
                cli_text::t("hdr_tonemapped")
            } else {
                cli_text::t("hdr_untouched")
            }
        );
    }
    println!(
        "{}:        {} ({})",
        cli_text::t("label_quality"),
        p.quality.label,
        p.quality.color
    );
    println!("  {}", p.quality.description);
    if !p.quality.tip.is_empty() {
        println!("  {}: {}", cli_text::t("label_tip"), p.quality.tip);
    }
    if p.is_fallback {
        if let Some(reason) = &p.fallback_reason {
            println!("Fallback:      {reason}");
        }
    }
    Ok(())
}

fn cmd_encode(args: &[String]) -> Result<(), String> {
    let (positional, flags) = parse_args(args);
    let input = positional
        .first()
        .ok_or_else(|| cli_text::t("need_input_encode").to_string())?
        .clone();
    let output = flags
        .get("out")
        .ok_or_else(|| cli_text::t("missing_out").to_string())?
        .clone();

    let info = media::probe_video_strict(&input)?;
    let start = flag_f64(&flags, "start", 0.0);
    let end = flag_f64(&flags, "end", info.duration);
    let target = flag_f64(&flags, "target", 20.0);
    let codec = codec_from(&flags)?;
    let speed = flag_str(&flags, "speed", "hq").to_string();
    let prefer_hw = prefer_hw_from(&flags);
    let crop = crop_from(&info, &flags);
    let quiet = flag_bool(&flags, "quiet");

    let cancel = CancellationToken::new();
    let mut last_percent = -10.0f64;
    let mut progress = |p: &ProgressUpdate| {
        if quiet {
            return;
        }
        if p.percent - last_percent >= 1.0 || p.percent >= 100.0 {
            last_percent = p.percent;
            eprint!(
                "\r[{:<32}] {:5.1}%  {:>6}  ETA {:>5.0}s   ",
                truncate(&p.stage, 32),
                p.percent,
                p.speed,
                p.eta_s
            );
        }
    };

    println!("{}:  {input}", cli_text::t("label_input"));
    println!("{}:  {output}", cli_text::t("label_output"));
    println!(
        "{}:    {target:.1} MB · {} {codec:?} · {speed}",
        cli_text::t("label_limit"),
        cli_text::t("label_codec")
    );

    let result = ffmpeg::encode_video(
        &input,
        &output,
        start,
        end,
        target,
        "auto",
        Some(codec),
        &speed,
        Some(prefer_hw),
        crop.as_ref(),
        &mut progress,
        &cancel,
    )?;

    if !quiet {
        eprintln!();
    }

    let size = std::fs::metadata(&result).map(|m| m.len()).unwrap_or(0);
    println!(
        "{}: {} ({:.2} MB)",
        cli_text::t("done"),
        result.display(),
        size as f64 / 1_000_000.0
    );
    Ok(())
}

fn cmd_audio(args: &[String]) -> Result<(), String> {
    let (positional, flags) = parse_args(args);
    let input = positional
        .first()
        .ok_or_else(|| cli_text::t("need_input_audio").to_string())?
        .clone();
    let output = flags
        .get("out")
        .ok_or_else(|| cli_text::t("missing_out").to_string())?
        .clone();
    let format = flag_str(&flags, "format", "m4a").to_string();
    let start = flag_f64(&flags, "start", 0.0);
    let end = flag_f64(&flags, "end", 0.0);
    let target = flags.get("target").and_then(|v| v.parse::<f64>().ok());
    let bitrate = flags.get("bitrate").and_then(|v| v.parse::<i32>().ok());
    let quiet = flag_bool(&flags, "quiet");

    let info = media::probe_video_strict(&input)?;
    let end = if end > 0.0 { end } else { info.duration };

    let cancel = CancellationToken::new();
    let mut progress = |p: &ProgressUpdate| {
        if quiet {
            return;
        }
        eprint!("\r[{:<28}] {:5.1}%   ", truncate(&p.stage, 28), p.percent);
    };

    let result = ffmpeg::encode_audio(
        &input,
        &output,
        start,
        end,
        target,
        &format,
        bitrate,
        &mut progress,
        &cancel,
    )?;
    if !quiet {
        eprintln!();
    }
    let size = std::fs::metadata(&result).map(|m| m.len()).unwrap_or(0);
    println!(
        "{}: {} ({:.2} MB)",
        cli_text::t("done"),
        result.display(),
        size as f64 / 1_000_000.0
    );
    Ok(())
}

fn cmd_thumb(args: &[String]) -> Result<(), String> {
    let (positional, flags) = parse_args(args);
    let input = positional
        .first()
        .ok_or_else(|| cli_text::t("need_input_thumb").to_string())?;
    let output = flags
        .get("out")
        .ok_or_else(|| cli_text::t("missing_out_png").to_string())?;
    let at = flags.get("at").and_then(|v| v.parse::<f64>().ok());
    let path = media::generate_thumbnail(input, output, at)?;
    println!("{}: {}", cli_text::t("thumbnail"), path.display());
    Ok(())
}

fn cmd_filmstrip(args: &[String]) -> Result<(), String> {
    let (positional, flags) = parse_args(args);
    let input = positional
        .first()
        .ok_or_else(|| cli_text::t("need_input_filmstrip").to_string())?;
    let output = flags
        .get("out")
        .ok_or_else(|| cli_text::t("missing_out_png").to_string())?;
    let count = flags
        .get("count")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(20);
    let width = flags
        .get("width")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(160);
    let path = media::generate_filmstrip(input, output, count, width)?;
    println!("{}: {}", cli_text::t("filmstrip"), path.display());
    Ok(())
}

fn cmd_gallery(args: &[String]) -> Result<(), String> {
    let (positional, _flags) = parse_args(args);
    let root = positional
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(media::default_gallery_dir);

    if !root.is_dir() {
        return Err(format!(
            "{}: {}",
            cli_text::t("folder_missing"),
            root.display()
        ));
    }

    let groups = media::scan_gallery(&root);
    if groups.is_empty() {
        println!("{} {}", cli_text::t("no_clips"), root.display());
        return Ok(());
    }

    let total: usize = groups.iter().map(|(_, clips)| clips.len()).sum();
    println!(
        "{}: {} ({} {})",
        cli_text::t("gallery"),
        root.display(),
        total,
        plural_clips(total)
    );

    for (name, clips) in groups {
        println!(
            "\n[{}] ({} {})",
            name,
            clips.len(),
            plural_clips(clips.len())
        );
        for clip in clips {
            println!(
                "  {}  ({:.1} MB)",
                clip.name(),
                clip.size as f64 / 1_000_000.0
            );
        }
    }
    Ok(())
}

fn cmd_cleanup(args: &[String]) -> Result<(), String> {
    let (positional, flags) = parse_args(args);
    if positional.len() < 2 {
        return Err(cli_text::t("cleanup_usage").to_string());
    }
    let policy = match flag_str(&flags, "policy", "never") {
        "delete" | "delete_permanently" => "delete_permanently",
        "trash" => "trash",
        _ => "never",
    };
    let message = ffmpeg::cleanup_source_file(&positional[0], &positional[1], policy)?;
    println!("{message}");
    Ok(())
}

/// Plural form of "clip" — Polish needs three forms, English needs one.
fn plural_clips(count: usize) -> &'static str {
    if !i18n::is_pl() {
        return "clips";
    }
    if count == 1 {
        return "klip";
    }
    let last = count % 10;
    let last_two = count % 100;
    if (2..=4).contains(&last) && !(12..=14).contains(&last_two) {
        "klipy"
    } else {
        "klipów"
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
}

#[allow(dead_code)]
fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
}
