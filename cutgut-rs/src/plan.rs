//! Planning layer: hard size limits, quality assessment, audio planning.
//!
//! This is a faithful port of the Python `calculate_plan`, `assess_quality`
//! and `calculate_audio_plan` functions so both editions produce identical
//! FFmpeg instructions.

use std::path::Path;

use crate::ffmpeg;
use crate::i18n;

pub const TARGET_MARGIN: f64 = 0.960;
pub const TARGET_MARGIN_AV1: f64 = 0.940;

/// Bits per pixel above which a bigger target stops buying quality and starts
/// asking hardware encoders for a bitrate they refuse to open: a 4 s 360p clip
/// under a 500 MB limit works out to ~1 Gb/s, and NVENC/AMF reject the job with
/// "invalid param". Half a bit per pixel is already visually lossless.
pub const MAX_BITS_PER_PIXEL: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
}

impl VideoCodec {
    pub fn as_str(self) -> &'static str {
        match self {
            VideoCodec::H264 => "h264",
            VideoCodec::Hevc => "hevc",
            VideoCodec::Av1 => "av1",
        }
    }

    pub fn upper(self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::Hevc => "HEVC",
            VideoCodec::Av1 => "AV1",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "h264" | "avc" | "h.264" => Some(VideoCodec::H264),
            "hevc" | "h265" | "h.265" => Some(VideoCodec::Hevc),
            "av1" => Some(VideoCodec::Av1),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VideoInfo {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bitrate: u64,
    pub codec: String,
    pub audio_codec: Option<String>,
    /// Degrees from the display matrix; phones write ±90 for portrait clips.
    pub rotation: i32,
    /// Pixel format reported by the probe, e.g. `yuv420p10le`.
    pub pix_fmt: String,
    /// Transfer characteristics, e.g. `bt709`, `smpte2084` (HDR10), `arib-std-b67`.
    pub color_transfer: String,
    /// Channels of the first audio stream (0 when the clip has no sound).
    pub audio_channels: u32,
}

impl VideoInfo {
    pub fn has_audio(&self) -> bool {
        match &self.audio_codec {
            Some(codec) => {
                let c = codec.trim().to_ascii_lowercase();
                !c.is_empty() && c != "none"
            }
            None => false,
        }
    }

    /// Size the picture is actually shown at.
    ///
    /// FFmpeg applies the rotation metadata before any filter runs, so cropping,
    /// scaling and preview maths all have to work on these numbers — using the
    /// coded size of a portrait phone clip cuts the wrong part of the frame.
    pub fn display_size(&self) -> (u32, u32) {
        if self.rotation % 180 != 0 {
            (self.height.max(1), self.width.max(1))
        } else {
            (self.width.max(1), self.height.max(1))
        }
    }

    pub fn is_deep_color(&self) -> bool {
        // Bare "12" would also match nv12, so look for the markers that only
        // deep-colour formats carry.
        let fmt = self.pix_fmt.to_ascii_lowercase();
        ["p10", "p12", "p16", "10le", "10be", "12le", "16le"]
            .iter()
            .any(|mark| fmt.contains(mark))
    }

    /// HDR (PQ or HLG) needs tonemapping; converting it with a plain 8-bit
    /// `yuv420p` conversion is what makes those clips look washed out.
    pub fn is_hdr(&self) -> bool {
        let transfer = self.color_transfer.as_str();
        // PQ (HDR10/HDR10+) and HLG; ffprobe has used a few spellings for HLG.
        transfer.contains("2084") || transfer.contains("std-b67") || transfer == "hlg"
    }
}

#[derive(Clone, Debug)]
pub struct CropBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub ratio_type: String,
}

impl CropBox {
    pub fn original(width: i32, height: i32) -> Self {
        Self {
            x: 0,
            y: 0,
            w: even(width),
            h: even(height),
            ratio_type: "original".into(),
        }
    }

    pub fn to_filter(&self) -> String {
        format!(
            "crop={}:{}:{}:{}",
            even(self.w),
            even(self.h),
            even(self.x).max(0),
            even(self.y).max(0)
        )
    }

    pub fn calculate_default(src_w: i32, src_h: i32, ratio_type: &str, alignment: &str) -> Self {
        if ratio_type == "original" || src_w <= 0 || src_h <= 0 {
            return Self::original(src_w, src_h);
        }

        let (mut w, mut h) = match ratio_type {
            "16:9" => {
                let target = 16.0 / 9.0;
                let src = src_w as f64 / src_h as f64;
                if (src - target).abs() < 0.02 {
                    (src_w, src_h)
                } else if src > target {
                    ((src_h as f64 * target) as i32, src_h)
                } else {
                    (src_w, (src_w as f64 / target) as i32)
                }
            }
            "9:16" => {
                let target = 9.0 / 16.0;
                let mut w = (src_h as f64 * target) as i32;
                let mut h = src_h;
                if w > src_w {
                    w = src_w;
                    h = (src_w as f64 / target) as i32;
                }
                (w, h)
            }
            "1:1" => {
                let side = src_w.min(src_h);
                (side, side)
            }
            _ => return Self::original(src_w, src_h),
        };

        w = even(w);
        h = even(h);

        let x = match alignment {
            "left" => 0,
            "right" => (src_w - w).max(0),
            _ => ((src_w - w) / 2).max(0),
        };
        let y = ((src_h - h) / 2).max(0);

        Self {
            x,
            y,
            w,
            h,
            ratio_type: ratio_type.to_string(),
        }
    }
}

fn even(value: i32) -> i32 {
    if value % 2 == 0 {
        value
    } else {
        value - 1
    }
}

#[derive(Clone, Debug)]
pub struct QualityAssessment {
    pub rating: String,
    pub label: String,
    pub color: String,
    pub bppf: f64,
    pub description: String,
    pub tip: String,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub duration_s: f64,
    pub source_duration_s: f64,
    pub target_bytes: i64,
    pub hard_limit_bytes: i64,
    pub video_kbps: i32,
    pub audio_kbps: i32,
    pub audio_bytes: i64,
    pub overhead_bytes: i64,
    pub has_audio: bool,
    pub codec: VideoCodec,
    pub encoder_name: String,
    pub is_hardware: bool,
    pub is_fallback: bool,
    pub fallback_reason: Option<String>,
    pub filter_str: Option<String>,
    pub is_remux: bool,
    pub out_width: i32,
    pub out_height: i32,
    pub out_fps: f64,
    pub quality: QualityAssessment,
    pub crop_box: Option<CropBox>,
    /// Set when the source was HDR and the filter chain maps it to SDR.
    pub hdr_tonemapped: bool,
    /// Set when the source was HDR but the ffmpeg build cannot tonemap it.
    pub hdr_untouched: bool,
    /// Audio channels in the export (multichannel sources are downmixed).
    pub audio_channels: u32,
    /// Set when the size limit asked for more bitrate than the clip can use.
    pub bitrate_capped: bool,
}

pub fn get_hard_limit_bytes(target_mb: f64) -> i64 {
    (target_mb * 1_000_000.0) as i64
}

pub fn calculate_target_bytes(target_mb: f64, codec: VideoCodec) -> i64 {
    let margin = if codec == VideoCodec::Av1 {
        TARGET_MARGIN_AV1
    } else {
        TARGET_MARGIN
    };
    (target_mb * 1_000_000.0 * margin) as i64
}

pub fn can_stream_copy(
    video_info: &VideoInfo,
    input_path: &str,
    start_s: f64,
    end_s: f64,
    target_mb: f64,
    target_codec: Option<VideoCodec>,
) -> bool {
    if input_path.is_empty() || !Path::new(input_path).exists() {
        return false;
    }

    if let Some(codec) = target_codec {
        let src = video_info.codec.to_ascii_lowercase();
        let matches = match codec {
            VideoCodec::Av1 => src.contains("av1"),
            VideoCodec::Hevc => src.contains("hevc") || src.contains("h265"),
            VideoCodec::H264 => src.contains("h264") || src.contains("avc"),
        };
        if !matches {
            return false;
        }
    }

    let hard_limit = get_hard_limit_bytes(target_mb);
    let total_size = std::fs::metadata(input_path).map(|m| m.len()).unwrap_or(0) as i64;
    let dur_s = (end_s - start_s).max(0.1);

    if start_s <= 0.05 && end_s >= video_info.duration - 0.05 {
        return total_size <= hard_limit;
    }

    if video_info.duration > 0.0 && total_size > 0 {
        let est = ((dur_s / video_info.duration.max(0.1)) * total_size as f64 * 1.04) as i64;
        return est <= hard_limit;
    }
    if video_info.bitrate > 0 {
        let est = (dur_s * video_info.bitrate as f64 / 8.0 * 1.04) as i64;
        return est <= hard_limit;
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn assess_quality(
    video_kbps: i32,
    width: i32,
    height: i32,
    fps: f64,
    is_remux: bool,
    dur_s: f64,
    target_mb: f64,
    codec: VideoCodec,
    lang: &str,
) -> QualityAssessment {
    let code = if lang.trim().is_empty() {
        i18n::lang_code()
    } else {
        lang
    };
    let t = |key: &str| i18n::t_for(code, key);

    if is_remux {
        return QualityAssessment {
            rating: "great".into(),
            label: t("q_remux_label").into(),
            color: "#22c55e".into(),
            bppf: 0.15,
            description: t("q_remux_desc").into(),
            tip: t("q_remux_tip").into(),
        };
    }

    let eff_w = width.max(320) as f64;
    let eff_h = height.max(240) as f64;
    let eff_fps = fps.max(1.0);
    let bppf = (video_kbps as f64 * 1000.0) / (eff_w * eff_h * eff_fps);
    let _ = target_mb;

    let mut q = if bppf >= 0.075 {
        QualityAssessment {
            rating: "great".into(),
            label: t("q_great_label").into(),
            color: "#22c55e".into(),
            bppf,
            description: String::new(),
            tip: t("q_great_tip").into(),
        }
    } else if bppf >= 0.038 {
        QualityAssessment {
            rating: "good".into(),
            label: t("q_good_label").into(),
            color: "#84cc16".into(),
            bppf,
            description: String::new(),
            tip: t("q_good_tip").into(),
        }
    } else if bppf >= 0.018 {
        QualityAssessment {
            rating: "ok".into(),
            label: t("q_ok_label").into(),
            color: "#eab308".into(),
            bppf,
            description: String::new(),
            tip: if dur_s > 30.0 {
                i18n::fill(
                    t("q_ok_tip_long"),
                    &[&((dur_s * 0.3) as i32).max(5).to_string()],
                )
            } else {
                t("q_ok_tip_short").to_string()
            },
        }
    } else if bppf >= 0.009 {
        QualityAssessment {
            rating: "low".into(),
            label: t("q_low_label").into(),
            color: "#f97316".into(),
            bppf,
            description: String::new(),
            tip: i18n::fill(
                t("q_low_tip"),
                &[&((dur_s * 0.4) as i32).max(5).to_string()],
            ),
        }
    } else {
        QualityAssessment {
            rating: "very_low".into(),
            label: t("q_very_low_label").into(),
            color: "#ef4444".into(),
            bppf,
            description: String::new(),
            tip: i18n::fill(
                t("q_very_low_tip"),
                &[&((dur_s * 0.6) as i32).max(5).to_string()],
            ),
        }
    };

    let codec_name = codec.upper();
    q.description = i18n::fill(t("q_description"), &[&format!("{bppf:.4}"), &codec_name]);
    q.bppf = (bppf * 10000.0).round() / 10000.0;
    q
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_plan(
    video_info: &VideoInfo,
    start_s: f64,
    end_s: f64,
    target_mb: f64,
    codec: VideoCodec,
    speed: &str,
    prefer_hardware: bool,
    input_path: &str,
    lang: &str,
    crop_box: Option<&CropBox>,
) -> Result<Plan, String> {
    let resolved = ffmpeg::resolve_encoder(codec, speed, prefer_hardware);
    let actual_codec = resolved.codec;

    let dur_s = (end_s - start_s).max(0.1);
    let hard_limit_bytes = get_hard_limit_bytes(target_mb);
    let target_bytes = calculate_target_bytes(target_mb, actual_codec);
    let (source_w, source_h) = video_info.display_size();

    let has_crop = crop_box
        .map(|c| c.ratio_type != "original" && (c.w < source_w as i32 || c.h < source_h as i32))
        .unwrap_or(false);

    let is_remux = if has_crop {
        false
    } else if !input_path.is_empty() {
        can_stream_copy(
            video_info,
            input_path,
            start_s,
            end_s,
            target_mb,
            Some(actual_codec),
        )
    } else {
        false
    };

    let has_audio = video_info.has_audio();
    let audio_bps = if has_audio {
        if matches!(actual_codec, VideoCodec::Hevc | VideoCodec::Av1) {
            96_000.0
        } else {
            128_000.0
        }
    } else {
        0.0
    };
    let audio_kbps = (audio_bps / 1000.0) as i32;

    let overhead_bytes = 30_000 + (dur_s * 350.0) as i64;
    let audio_bytes = if has_audio {
        (audio_bps * dur_s / 8.0) as i64
    } else {
        0
    };
    let net_video_bytes = target_bytes - audio_bytes - overhead_bytes;

    if net_video_bytes <= 0 || (net_video_bytes as f64 * 8.0 / dur_s) < 30_000.0 {
        return Err(format!(
            "{} ({} MB) - {} ({} s). {}",
            i18n::t_for(lang, "plan_target_too_small_title"),
            format!("{target_mb:.1}"),
            i18n::t_for(lang, "plan_target_too_small_clip"),
            format!("{dur_s:.1}"),
            i18n::t_for(lang, "plan_target_too_small")
        ));
    }

    let wanted_kbps = (((net_video_bytes as f64 * 8.0 / dur_s) / 1000.0) as i32).max(30);
    // The output is never larger than the source, so the source area is the right
    // ceiling: a huge limit on a short clip cannot be spent on pixels that are
    // not there.
    let bpp_ceiling =
        (source_w as f64 * source_h as f64 * video_info.fps.max(1.0) * MAX_BITS_PER_PIXEL / 1000.0)
            as i32;
    let bitrate_capped = wanted_kbps > bpp_ceiling.max(30);
    let video_kbps = wanted_kbps.min(bpp_ceiling.max(30));

    let mut filters: Vec<String> = Vec::new();

    // HDR cannot just be dropped into 8-bit: PQ/HLG has to be flattened into
    // linear light and tone mapped first, or the export comes out grey and flat.
    // `zscale` (libzimg) is not in every ffmpeg build, so the chain is only
    // added when it is actually there; otherwise the export keeps its HDR
    // numbers and the plan says so instead of silently wrecking the colours.
    let hdr_tonemapped = video_info.is_hdr() && ffmpeg::supports_filter("zscale");
    let hdr_untouched = video_info.is_hdr() && !hdr_tonemapped;
    if hdr_tonemapped {
        filters.extend([
            "zscale=t=linear:npl=100".to_string(),
            "format=gbrpf32le".to_string(),
            "zscale=p=bt709".to_string(),
            "tonemap=tonemap=hable:desat=0".to_string(),
            "zscale=t=bt709:m=bt709:r=tv".to_string(),
            "format=yuv420p".to_string(),
        ]);
    }

    let (base_w, base_h) = if has_crop {
        let crop = crop_box.unwrap();
        filters.push(crop.to_filter());
        (crop.w, crop.h)
    } else {
        (source_w as i32, source_h as i32)
    };

    let mut out_w = base_w;
    let mut out_h = base_h;
    let mut out_fps = video_info.fps;

    if video_kbps < 450 {
        if base_h > 480 {
            filters.push("scale=-2:480".into());
            out_h = 480;
            out_w = if base_h > 0 {
                base_w * 480 / base_h
            } else {
                854
            };
            out_w = if out_w % 2 == 0 { out_w } else { out_w + 1 };
        }
        if video_info.fps > 30.0 {
            filters.push("fps=30".into());
            out_fps = 30.0;
        }
    } else if video_kbps < 900 {
        if base_h > 720 {
            filters.push("scale=-2:720".into());
            out_h = 720;
            out_w = if base_h > 0 {
                base_w * 720 / base_h
            } else {
                1280
            };
            out_w = if out_w % 2 == 0 { out_w } else { out_w + 1 };
        }
        if video_info.fps > 45.0 {
            filters.push("fps=30".into());
            out_fps = 30.0;
        }
    }
    let _ = base_w;

    let filter_str = if filters.is_empty() {
        None
    } else {
        Some(filters.join(","))
    };

    let quality = assess_quality(
        video_kbps,
        out_w,
        out_h,
        out_fps,
        is_remux,
        dur_s,
        target_mb,
        actual_codec,
        lang,
    );

    Ok(Plan {
        duration_s: dur_s,
        source_duration_s: video_info.duration,
        target_bytes,
        hard_limit_bytes,
        video_kbps,
        audio_kbps,
        audio_bytes,
        overhead_bytes,
        has_audio,
        codec: actual_codec,
        encoder_name: resolved.encoder_name,
        is_hardware: resolved.is_hardware,
        is_fallback: resolved.is_fallback,
        fallback_reason: resolved.fallback_reason,
        filter_str,
        is_remux,
        out_width: out_w,
        out_height: out_h,
        out_fps,
        quality,
        crop_box: crop_box.cloned(),
        hdr_tonemapped,
        hdr_untouched,
        audio_channels: if has_audio { 2 } else { 0 },
        bitrate_capped,
    })
}

#[derive(Clone, Debug)]
pub struct AudioPlan {
    pub format: String,
    pub extension: String,
    pub codec: String,
    pub audio_kbps: i32,
    pub default_kbps: i32,
    pub duration_s: f64,
    pub has_audio: bool,
    pub hard_limit_bytes: Option<i64>,
    pub audio_bytes: i64,
    pub overhead_bytes: i64,
}

pub fn normalize_export_format(fmt: &str) -> String {
    let f = fmt.trim().trim_start_matches('.').to_ascii_lowercase();
    match f.as_str() {
        "mp4" | "mkv" | "m4a" | "opus" | "mp3" => f,
        _ => "mp4".into(),
    }
}

pub fn is_audio_export_format(fmt: &str) -> bool {
    matches!(
        normalize_export_format(fmt).as_str(),
        "m4a" | "opus" | "mp3"
    )
}

pub fn get_export_extension(fmt: &str) -> String {
    match normalize_export_format(fmt).as_str() {
        "m4a" => ".m4a".into(),
        "opus" => ".opus".into(),
        "mp3" => ".mp3".into(),
        "mkv" => ".mkv".into(),
        _ => ".mp4".into(),
    }
}

pub fn audio_spec(fmt: &str) -> (&'static str, &'static str, i32, i32) {
    match normalize_export_format(fmt).as_str() {
        "opus" => ("libopus", ".opus", 96, 192),
        "mp3" => ("libmp3lame", ".mp3", 192, 320),
        _ => ("aac", ".m4a", 160, 256),
    }
}

pub fn calculate_audio_plan(
    video_info: &VideoInfo,
    start_s: f64,
    end_s: f64,
    target_mb: Option<f64>,
    audio_format: &str,
    bitrate_kbps: Option<i32>,
) -> AudioPlan {
    let fmt = normalize_export_format(audio_format);
    let (codec, extension, default_kbps, max_kbps) = audio_spec(&fmt);
    let dur_s = (end_s - start_s).max(0.1);

    let has_audio = video_info.has_audio();
    let overhead_bytes = 4096 + (dur_s * 64.0) as i64;
    let hard_limit_bytes = target_mb.map(get_hard_limit_bytes);

    let mut kbps = bitrate_kbps.unwrap_or(default_kbps);
    if let Some(limit) = hard_limit_bytes {
        let budget = ((limit - overhead_bytes) as f64 * 8.0 / dur_s / 1000.0) as i32;
        kbps = kbps.min(budget.max(16));
    }
    kbps = kbps.clamp(16, max_kbps);

    AudioPlan {
        format: fmt,
        extension: extension.into(),
        codec: codec.into(),
        audio_kbps: kbps,
        default_kbps,
        duration_s: dur_s,
        has_audio,
        hard_limit_bytes,
        audio_bytes: (kbps as f64 * 1000.0 * dur_s / 8.0) as i64,
        overhead_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(w: u32, h: u32, dur: f64, fps: f64, audio: bool) -> VideoInfo {
        VideoInfo {
            duration: dur,
            width: w,
            height: h,
            fps,
            bitrate: 0,
            codec: "h264".into(),
            audio_codec: if audio { Some("aac".into()) } else { None },
            rotation: 0,
            pix_fmt: "yuv420p".into(),
            color_transfer: "bt709".into(),
            audio_channels: if audio { 2 } else { 0 },
        }
    }

    #[test]
    fn audio_plan_respects_hard_limit() {
        let plan = calculate_audio_plan(
            &info(1920, 1080, 60.0, 60.0, true),
            0.0,
            60.0,
            Some(20.0),
            "m4a",
            None,
        );
        assert!(plan.audio_bytes <= plan.hard_limit_bytes.unwrap());
        assert!(plan.audio_kbps >= 16);
    }

    #[test]
    fn video_bitrate_fits_budget() {
        let plan = calculate_plan(
            &info(1920, 1080, 60.0, 60.0, true),
            0.0,
            60.0,
            20.0,
            VideoCodec::H264,
            "hq",
            false,
            "",
            "pl",
            None,
        )
        .expect("plan");
        let estimated = plan.video_kbps as i64 * 1000 / 8 * plan.duration_s as i64;
        assert!(estimated <= plan.target_bytes);
        assert!(plan.hard_limit_bytes == 20_000_000);
    }

    #[test]
    fn av1_uses_stricter_margin() {
        assert!(
            calculate_target_bytes(20.0, VideoCodec::Av1)
                < calculate_target_bytes(20.0, VideoCodec::H264)
        );
    }

    #[test]
    fn crop_16_9_from_4_3() {
        let c = CropBox::calculate_default(1440, 1080, "16:9", "center");
        assert_eq!(c.w, 1440);
        assert_eq!(c.h, 810);
        assert!(c.to_filter().starts_with("crop="));
    }

    #[test]
    fn tiny_target_is_rejected() {
        let err = calculate_plan(
            &info(1920, 1080, 60.0, 60.0, true),
            0.0,
            3600.0,
            20.0,
            VideoCodec::H264,
            "hq",
            false,
            "",
            "pl",
            None,
        )
        .unwrap_err();
        assert!(err.contains("nierealny"));
    }

    fn sample_info() -> VideoInfo {
        VideoInfo {
            duration: 22.0,
            width: 1920,
            height: 1080,
            fps: 60.0,
            bitrate: 6_000_000,
            codec: "h264".into(),
            audio_codec: Some("aac".into()),
            rotation: 0,
            pix_fmt: "yuv420p".into(),
            color_transfer: "bt709".into(),
            audio_channels: 2,
        }
    }

    #[test]
    fn portrait_rotation_is_planned_on_the_displayed_frame() {
        let mut info = sample_info();
        info.width = 1920;
        info.height = 1080;
        info.rotation = -90;
        let crop = CropBox::calculate_default(1080, 1920, "9:16", "center");
        let plan = calculate_plan(
            &info,
            0.0,
            10.0,
            20.0,
            VideoCodec::H264,
            "hq",
            false,
            "",
            "pl",
            Some(&crop),
        )
        .expect("plan");
        // 9:16 of a portrait picture is the picture itself, not a slice of the
        // pre-rotation landscape frame.
        assert_eq!((plan.out_width, plan.out_height), (1080, 1920));
    }

    #[test]
    fn hdr_sources_get_a_tonemap_chain() {
        let mut info = sample_info();
        info.pix_fmt = "yuv420p10le".into();
        info.color_transfer = "smpte2084".into();
        let plan = calculate_plan(
            &info,
            0.0,
            10.0,
            20.0,
            VideoCodec::H264,
            "hq",
            false,
            "",
            "pl",
            None,
        )
        .expect("plan");
        let filters = plan.filter_str.clone().unwrap_or_default();
        if crate::ffmpeg::supports_filter("zscale") {
            assert!(plan.hdr_tonemapped && !plan.hdr_untouched);
            assert!(filters.contains("tonemap"), "{filters}");
            assert!(filters.contains("format=yuv420p"), "{filters}");
        } else {
            // Without libzimg the colours cannot be mapped; the plan has to say so
            // instead of pretending the export is fine.
            assert!(plan.hdr_untouched && !plan.hdr_tonemapped);
            assert!(!filters.contains("tonemap"));
        }
    }

    #[test]
    fn a_huge_limit_is_capped_at_bits_per_pixel_not_encoder_limits() {
        // Four seconds of 360p under a 500 MB limit works out to ~1 Gb/s, which
        // NVENC/AMF refuse to open ("invalid param"), so the plan has to cap it.
        let plan = calculate_plan(
            &info(640, 360, 4.0, 60.0, true),
            0.0,
            4.0,
            500.0,
            VideoCodec::H264,
            "hq",
            true,
            "",
            "pl",
            None,
        )
        .expect("plan");
        let ceiling = (640.0 * 360.0 * 60.0 * MAX_BITS_PER_PIXEL / 1000.0) as i32;
        assert_eq!(plan.video_kbps, ceiling);
        assert!(plan.bitrate_capped);
        // 1080p60 with a normal limit is untouched.
        let normal = calculate_plan(
            &info(1920, 1080, 20.0, 60.0, true),
            0.0,
            20.0,
            20.0,
            VideoCodec::H264,
            "hq",
            true,
            "",
            "pl",
            None,
        )
        .expect("plan");
        assert!(!normal.bitrate_capped);
    }

    #[test]
    fn audio_channels_are_stereo_in_the_plan() {
        let mut info = sample_info();
        info.audio_channels = 6;
        let plan = calculate_plan(
            &info,
            0.0,
            10.0,
            20.0,
            VideoCodec::H264,
            "hq",
            false,
            "",
            "pl",
            None,
        )
        .expect("plan");
        assert_eq!(plan.audio_channels, 2);
    }

    #[test]
    fn quality_assessment_is_translated() {
        let pl = assess_quality(
            4000,
            1920,
            1080,
            60.0,
            false,
            40.0,
            20.0,
            VideoCodec::Av1,
            "pl",
        );
        let en = assess_quality(
            4000,
            1920,
            1080,
            60.0,
            false,
            40.0,
            20.0,
            VideoCodec::Av1,
            "en",
        );
        assert_eq!(pl.label, "OK do wysłania");
        assert_eq!(en.label, "OK for Sharing");
        assert_ne!(pl.tip, en.tip);
        assert!(pl.description.contains("bppf"));
        assert!(en.description.contains("bppf"));
    }

    #[test]
    fn plan_uses_the_requested_language() {
        let pl = calculate_plan(
            &sample_info(),
            0.0,
            20.0,
            20.0,
            VideoCodec::H264,
            "hq",
            false,
            "clip.mp4",
            "pl",
            None,
        )
        .unwrap();
        let en = calculate_plan(
            &sample_info(),
            0.0,
            20.0,
            20.0,
            VideoCodec::H264,
            "hq",
            false,
            "clip.mp4",
            "en",
            None,
        )
        .unwrap();
        assert_ne!(pl.quality.label, en.quality.label);
        assert!(!pl.quality.description.is_empty());
        assert!(!en.quality.description.is_empty());
    }
}
