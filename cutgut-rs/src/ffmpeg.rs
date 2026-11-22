//! FFmpeg orchestration: encoder detection, argument building, encode with
//! strict size verification and retries, audio export, cleanup policies.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::i18n;
use crate::plan::{self, AudioPlan, CropBox, Plan, VideoCodec};

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Upper bound for a metadata probe; anything slower is a stuck filesystem.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
/// Upper bound for a lossless remux of a full clip.
const REMUX_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn find_binary(name: &str) -> Option<String> {
    let exe_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };

    // A copy placed next to CutGut.exe wins over PATH: it lets users pick a
    // lean/fast FFmpeg build (a 200 MB static build costs ~0.5 s per spawn,
    // which is the whole seek latency).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(&exe_name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(&exe_name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
            let plain = dir.join(name);
            if plain.is_file() {
                return Some(plain.to_string_lossy().to_string());
            }
        }
    }

    None
}

pub fn ffmpeg_path() -> String {
    if let Ok(custom) = std::env::var("CUTGUT_FFMPEG") {
        if Path::new(&custom).is_file() {
            return custom;
        }
    }
    find_binary("ffmpeg").unwrap_or_else(|| "ffmpeg".into())
}

pub fn ffplay_path() -> String {
    find_binary("ffplay").unwrap_or_else(|| "ffplay".into())
}

pub fn ffprobe_path() -> String {
    if let Ok(custom) = std::env::var("CUTGUT_FFPROBE") {
        if Path::new(&custom).is_file() {
            return custom;
        }
    }
    find_binary("ffprobe").unwrap_or_else(|| "ffprobe".into())
}

pub fn run_process(program: &str, args: &[String]) -> Result<std::process::Output, String> {
    let mut cmd = command(program);
    cmd.args(args);
    cmd.output()
        .map_err(|e| format!("{} {program}: {e}", i18n::t("err_run_program")))
}

/// Runs `program` and gives up after `timeout`, killing it first.
///
/// Plain [`run_process`] waits forever: a hung FFmpeg on a network share, a
/// disconnected drive or a stuck filter would block a worker thread (and the
/// encode job) until the app is closed. Output is drained on separate threads so
/// a chatty process can never deadlock on a full pipe.
pub fn run_process_timeout(
    program: &str,
    args: &[String],
    timeout: Duration,
) -> Result<Output, String> {
    let mut cmd = command(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("{} {program}: {e}", i18n::t("err_run_program")))?;

    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        pipe.map(|mut pipe| {
            std::thread::spawn(move || {
                let mut buffer = Vec::new();
                let _ = pipe.read_to_end(&mut buffer);
                buffer
            })
        })
    };
    let stdout = reader(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let stderr = reader(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                kill_pid(child.id());
                let _ = child.wait();
                return Err(format!("{}: {e}", i18n::t("err_wait_ffmpeg")));
            }
        }
        if Instant::now() >= deadline {
            kill_pid(child.id());
            let _ = child.wait();
            // The reader threads are deliberately *not* joined: a grandchild can
            // still hold the inherited pipe open, and waiting for it here would
            // block exactly like the call that just timed out. They exit on EOF.
            return Err(format!(
                "{} ({} s)",
                i18n::t("err_timeout"),
                timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    let stdout = stdout
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

pub fn run_ffprobe(args: &[String]) -> Result<String, String> {
    let out = run_process_timeout(&ffprobe_path(), args, PROBE_TIMEOUT)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("ffprobe: {}", err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let pid = *self.pid.lock().unwrap();
        if let Some(pid) = pid {
            kill_pid(pid);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn set_pid(&self, pid: u32) {
        *self.pid.lock().unwrap() = Some(pid);
    }

    fn clear_pid(&self) {
        *self.pid.lock().unwrap() = None;
    }
}

fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.output();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
    }
}

#[derive(Clone, Debug)]
pub struct ProgressUpdate {
    pub stage: String,
    pub percent: f64,
    pub speed: String,
    pub fps: f64,
    pub eta_s: f64,
    pub elapsed_s: f64,
}

// ---------------------------------------------------------------------------
// Encoder detection
// ---------------------------------------------------------------------------

fn support_cache() -> &'static Mutex<HashMap<String, bool>> {
    static CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn nvenc_cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Verifies an encoder by actually encoding a single 256x256 frame.
pub fn test_encoder_support(encoder_name: &str) -> bool {
    if let Some(value) = support_cache().lock().unwrap().get(encoder_name) {
        return *value;
    }

    let args: Vec<String> = [
        "-y",
        "-hide_banner",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=256x256:d=0.04",
        "-c:v",
        encoder_name,
        "-f",
        "null",
        "-",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let supported = match run_process(&ffmpeg_path(), &args) {
        Ok(out) => out.status.success(),
        Err(_) => false,
    };

    support_cache()
        .lock()
        .unwrap()
        .insert(encoder_name.to_string(), supported);
    supported
}

pub fn check_nvenc_support() -> bool {
    test_encoder_support("h264_nvenc")
}
pub fn check_nvenc_hevc_support() -> bool {
    test_encoder_support("hevc_nvenc")
}
pub fn check_nvenc_av1_support() -> bool {
    test_encoder_support("av1_nvenc")
}
pub fn check_amf_support() -> bool {
    test_encoder_support("h264_amf")
}

/// Detects which advanced NVENC quality flags the current driver/GPU supports.
pub fn get_nvenc_quality_options(encoder_name: &str, speed: &str) -> Vec<String> {
    let key = format!("{encoder_name}/{speed}");
    if let Some(opts) = nvenc_cache().lock().unwrap().get(&key) {
        return opts.clone();
    }

    // Richest first: each set is probed with a one-frame encode, so an older
    // driver or ffmpeg build falls through to the next instead of failing the
    // export. Measured on a 1080p60 game clip at 4.6 Mb/s (VMAF against the
    // source): the ultra-quality tune with B-frame references is worth about
    // half a point over the plain set, at the same file size.
    let mut candidates: Vec<Vec<&str>> = Vec::new();
    if speed == "hq" {
        candidates.push(vec![
            "-tune",
            "uhq",
            "-preset",
            "p7",
            "-multipass",
            "fullres",
            "-b_ref_mode",
            "middle",
            "-spatial-aq",
            "1",
            "-temporal-aq",
            "1",
            "-rc-lookahead",
            "32",
        ]);
        candidates.push(vec![
            "-tune",
            "uhq",
            "-multipass",
            "fullres",
            "-b_ref_mode",
            "middle",
            "-spatial-aq",
            "1",
            "-temporal-aq",
            "1",
            "-rc-lookahead",
            "32",
        ]);
    }
    candidates.extend([
        vec![
            "-multipass",
            "fullres",
            "-b_ref_mode",
            "middle",
            "-spatial-aq",
            "1",
            "-temporal-aq",
            "1",
            "-rc-lookahead",
            "32",
        ],
        vec![
            "-multipass",
            "fullres",
            "-spatial-aq",
            "1",
            "-temporal-aq",
            "1",
            "-rc-lookahead",
            "32",
        ],
        vec![
            "-multipass",
            "fullres",
            "-spatial-aq",
            "1",
            "-rc-lookahead",
            "32",
        ],
        vec!["-multipass", "fullres"],
        vec![],
    ]);

    let mut chosen: Vec<String> = Vec::new();
    for cand in candidates {
        let mut args: Vec<String> = [
            "-y",
            "-hide_banner",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=256x256:d=0.04",
            "-c:v",
            encoder_name,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.extend(cand.iter().map(|s| s.to_string()));
        args.extend(["-f", "null", "-"].iter().map(|s| s.to_string()));

        if let Ok(out) = run_process(&ffmpeg_path(), &args) {
            if out.status.success() {
                chosen = cand.iter().map(|s| s.to_string()).collect();
                break;
            }
        }
    }

    nvenc_cache().lock().unwrap().insert(key, chosen.clone());
    chosen
}

#[derive(Clone, Debug)]
pub struct ResolvedEncoder {
    pub encoder_name: String,
    pub codec: VideoCodec,
    pub is_hardware: bool,
    pub is_fallback: bool,
    pub fallback_reason: Option<String>,
}

pub fn resolve_encoder(codec: VideoCodec, speed: &str, prefer_hardware: bool) -> ResolvedEncoder {
    let _ = speed;

    match codec {
        VideoCodec::Av1 => {
            if prefer_hardware && test_encoder_support("av1_nvenc") {
                return resolved("av1_nvenc", codec, true, false, None);
            }
            if prefer_hardware && test_encoder_support("av1_amf") {
                return resolved("av1_amf", codec, true, false, None);
            }
            if test_encoder_support("libsvtav1") {
                return resolved(
                    "libsvtav1",
                    codec,
                    false,
                    prefer_hardware,
                    prefer_hardware.then(|| {
                        "GPU does not support hardware AV1 encoding — using CPU (libsvtav1)."
                            .to_string()
                    }),
                );
            }
            if prefer_hardware && test_encoder_support("hevc_nvenc") {
                return resolved(
                    "hevc_nvenc",
                    VideoCodec::Hevc,
                    true,
                    true,
                    Some("AV1 encoder unavailable — switched to HEVC NVENC.".into()),
                );
            }
            resolved(
                "libx264",
                VideoCodec::H264,
                false,
                true,
                Some("AV1 encoder unavailable — switched to H.264 CPU.".into()),
            )
        }
        VideoCodec::Hevc => {
            if prefer_hardware && test_encoder_support("hevc_nvenc") {
                return resolved("hevc_nvenc", codec, true, false, None);
            }
            if prefer_hardware && test_encoder_support("hevc_amf") {
                return resolved("hevc_amf", codec, true, false, None);
            }
            if test_encoder_support("libx265") {
                return resolved(
                    "libx265",
                    codec,
                    false,
                    prefer_hardware,
                    prefer_hardware.then(|| {
                        "GPU does not support hardware HEVC encoding — using CPU (libx265)."
                            .to_string()
                    }),
                );
            }
            resolved(
                "libx264",
                VideoCodec::H264,
                false,
                true,
                Some("HEVC encoder unavailable — switched to H.264 CPU.".into()),
            )
        }
        VideoCodec::H264 => {
            if prefer_hardware && test_encoder_support("h264_nvenc") {
                return resolved("h264_nvenc", codec, true, false, None);
            }
            if prefer_hardware && test_encoder_support("h264_amf") {
                return resolved("h264_amf", codec, true, false, None);
            }
            resolved(
                "libx264",
                codec,
                false,
                prefer_hardware,
                prefer_hardware.then(|| {
                    "No hardware H.264 encoder (NVENC/AMF) — using CPU (libx264).".to_string()
                }),
            )
        }
    }
}

fn resolved(
    name: &str,
    codec: VideoCodec,
    hw: bool,
    fallback: bool,
    reason: Option<String>,
) -> ResolvedEncoder {
    ResolvedEncoder {
        encoder_name: name.to_string(),
        codec,
        is_hardware: hw,
        is_fallback: fallback,
        fallback_reason: reason,
    }
}

pub fn parse_preset_mode(preset_mode: &str) -> (VideoCodec, String, bool) {
    let m = preset_mode.trim().to_ascii_lowercase();
    let codec = if m.contains("av1") {
        VideoCodec::Av1
    } else if m.contains("hevc") || m.contains("h265") || m.contains("x265") {
        VideoCodec::Hevc
    } else {
        VideoCodec::H264
    };

    let is_cpu = m.contains("cpu") || m.starts_with("lib") || m.contains("svt");
    let prefer_hw = !is_cpu;

    let speed = if m.contains("fast") || m.contains("veryfast") || m.contains("speed") {
        "fast"
    } else if m.contains("balanced") || m.contains("medium") {
        "balanced"
    } else {
        "hq"
    };

    (codec, speed.to_string(), prefer_hw)
}

// ---------------------------------------------------------------------------
// Argument building
// ---------------------------------------------------------------------------

pub fn container_mux_args(output_path: &str) -> Vec<String> {
    let ext = Path::new(output_path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(ext.as_str(), "mp4" | "m4v" | "mov" | "m4a") {
        vec!["-movflags".into(), "+faststart".into()]
    } else {
        Vec::new()
    }
}

/// Whether a stream copy really covers only the requested range.
///
/// `-c copy` can only start on a keyframe, so a cut that misses one arrives with
/// extra footage in front and less at the end while still reporting the wanted
/// duration. Checking the real span keeps the trim honest: a miss falls back to
/// the frame-accurate re-encode instead of shipping the wrong seconds.
fn trim_is_exact(path: &Path, expected_s: f64, fps: f64) -> bool {
    if expected_s <= 0.0 {
        return false;
    }
    let Ok((start, duration)) = crate::media::content_span(&path.to_string_lossy()) else {
        return false;
    };
    // Two frames of slack, but never more than half a second: a slow slideshow
    // source must not turn a clear miss into a pass.
    let tolerance = (2.0 / fps.max(1.0)).clamp(0.12, 0.5);
    start.abs() <= tolerance && (duration - expected_s).abs() <= tolerance
}

pub fn build_audio_stream_args(has_audio: bool, a_kbps: i32) -> Vec<String> {
    if has_audio && a_kbps > 0 {
        vec![
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            "0:a:0?".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            format!("{a_kbps}k"),
            // Game captures are often 5.1; social platforms (and the size plan,
            // which budgets for stereo) expect a stereo track. Squeezing 5.1 into
            // the stereo bitrate audibly wrecks the sound.
            "-ac".into(),
            "2".into(),
        ]
    } else {
        vec!["-map".into(), "0:v:0".into(), "-an".into()]
    }
}

/// Whether this ffmpeg build carries a filter, checked once per process.
///
/// Tonemapping needs `zscale` (libzimg), which not every build ships; asking
/// first means an HDR clip exports with a working chain or reports that it
/// could not be mapped, instead of failing in the middle of an encode.
pub fn supports_filter(name: &str) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(known) = cache.lock().unwrap().get(name) {
        return *known;
    }
    let available = run_process(
        &ffmpeg_path(),
        &[
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            "error".to_string(),
            "-h".to_string(),
            format!("filter={name}"),
        ],
    )
    .map(|out| {
        let text = String::from_utf8_lossy(&out.stdout);
        text.contains(&format!("Filter {name}"))
    })
    .unwrap_or(false);
    cache.lock().unwrap().insert(name.to_string(), available);
    available
}

pub fn build_encoder_video_args(
    encoder_name: &str,
    v_kbps: i32,
    speed: &str,
    pass_num: u8,
    stats_file: &str,
) -> Vec<String> {
    let enc = encoder_name.to_ascii_lowercase();
    let s = |v: &str| v.to_string();

    if enc.contains("nvenc") {
        let preset = if speed == "hq" {
            "p6"
        } else if speed == "fast" {
            "p3"
        } else {
            "p4"
        };
        let maxrate = (v_kbps as f64 * 1.5) as i32;
        let bufsize = (v_kbps as f64 * 2.0) as i32;
        let mut args: Vec<String> = vec![
            s("-c:v"),
            s(encoder_name),
            s("-preset"),
            s(preset),
            s("-tune"),
            s("hq"),
            s("-rc:v"),
            s("vbr"),
            s("-b:v"),
            format!("{v_kbps}k"),
            s("-maxrate"),
            format!("{maxrate}k"),
            s("-bufsize"),
            format!("{bufsize}k"),
        ];
        args.extend(get_nvenc_quality_options(encoder_name, speed));
        args.extend([s("-pix_fmt"), s("yuv420p")]);
        return args;
    }

    if enc.contains("amf") {
        let quality = if speed == "hq" { "quality" } else { "speed" };
        let maxrate = (v_kbps as f64 * 1.5) as i32;
        let bufsize = (v_kbps as f64 * 2.0) as i32;
        return vec![
            s("-c:v"),
            s(encoder_name),
            s("-usage"),
            s("transcoding"),
            s("-quality"),
            s(quality),
            s("-rc"),
            s("vbr_peak"),
            s("-b:v"),
            format!("{v_kbps}k"),
            s("-maxrate"),
            format!("{maxrate}k"),
            s("-bufsize"),
            format!("{bufsize}k"),
            s("-pix_fmt"),
            s("yuv420p"),
        ];
    }

    if enc.contains("x264") {
        let cpu_preset = if speed == "hq" {
            "slow"
        } else if speed == "fast" {
            "veryfast"
        } else {
            "medium"
        };
        let maxrate = (v_kbps as f64 * 1.35) as i32;
        let bufsize = (v_kbps as f64 * 2.0) as i32;

        if pass_num == 1 {
            return vec![
                s("-c:v"),
                s("libx264"),
                s("-b:v"),
                format!("{v_kbps}k"),
                s("-preset"),
                s(cpu_preset),
                s("-pass"),
                s("1"),
                s("-passlogfile"),
                s(stats_file),
            ];
        }
        if pass_num == 2 {
            return vec![
                s("-c:v"),
                s("libx264"),
                s("-b:v"),
                format!("{v_kbps}k"),
                s("-maxrate"),
                format!("{maxrate}k"),
                s("-bufsize"),
                format!("{bufsize}k"),
                s("-preset"),
                s(cpu_preset),
                s("-pass"),
                s("2"),
                s("-passlogfile"),
                s(stats_file),
                s("-pix_fmt"),
                s("yuv420p"),
            ];
        }
        return vec![
            s("-c:v"),
            s("libx264"),
            s("-b:v"),
            format!("{v_kbps}k"),
            s("-maxrate"),
            format!("{maxrate}k"),
            s("-bufsize"),
            format!("{bufsize}k"),
            s("-preset"),
            s(cpu_preset),
            s("-pix_fmt"),
            s("yuv420p"),
        ];
    }

    if enc.contains("x265") {
        let cpu_preset = if speed == "hq" {
            "slow"
        } else if speed == "fast" {
            "fast"
        } else {
            "medium"
        };
        let stats_opt = stats_file.replace('\\', "/");
        let maxrate = (v_kbps as f64 * 1.35) as i32;
        let bufsize = (v_kbps as f64 * 2.0) as i32;

        if pass_num == 1 {
            return vec![
                s("-c:v"),
                s("libx265"),
                s("-b:v"),
                format!("{v_kbps}k"),
                s("-preset"),
                s(cpu_preset),
                s("-x265-params"),
                format!("pass=1:stats={stats_opt}"),
            ];
        }
        if pass_num == 2 {
            return vec![
                s("-c:v"),
                s("libx265"),
                s("-b:v"),
                format!("{v_kbps}k"),
                s("-preset"),
                s(cpu_preset),
                s("-x265-params"),
                format!("pass=2:stats={stats_opt}:vbv-maxrate={maxrate}:vbv-bufsize={bufsize}"),
                s("-pix_fmt"),
                s("yuv420p"),
            ];
        }
        return vec![
            s("-c:v"),
            s("libx265"),
            s("-b:v"),
            format!("{v_kbps}k"),
            s("-preset"),
            s(cpu_preset),
            s("-x265-params"),
            format!("vbv-maxrate={maxrate}:vbv-bufsize={bufsize}"),
            s("-pix_fmt"),
            s("yuv420p"),
        ];
    }

    if enc.contains("svt") || enc.contains("av1") {
        let svt_preset = if speed == "hq" {
            "5"
        } else if speed == "fast" {
            "8"
        } else {
            "6"
        };
        if pass_num == 1 {
            return vec![
                s("-c:v"),
                s("libsvtav1"),
                s("-b:v"),
                format!("{v_kbps}k"),
                s("-preset"),
                s(svt_preset),
                s("-pass"),
                s("1"),
                s("-passlogfile"),
                s(stats_file),
            ];
        }
        if pass_num == 2 {
            return vec![
                s("-c:v"),
                s("libsvtav1"),
                s("-b:v"),
                format!("{v_kbps}k"),
                s("-preset"),
                s(svt_preset),
                s("-pass"),
                s("2"),
                s("-passlogfile"),
                s(stats_file),
                s("-pix_fmt"),
                s("yuv420p"),
            ];
        }
        return vec![
            s("-c:v"),
            s("libsvtav1"),
            s("-b:v"),
            format!("{v_kbps}k"),
            s("-preset"),
            s(svt_preset),
            s("-pix_fmt"),
            s("yuv420p"),
        ];
    }

    vec![
        s("-c:v"),
        s("libx264"),
        s("-b:v"),
        format!("{v_kbps}k"),
        s("-preset"),
        s("medium"),
        s("-pix_fmt"),
        s("yuv420p"),
    ]
}

// ---------------------------------------------------------------------------
// Progress runner
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
/// Owns a running child process and guarantees it is killed *and* reaped if the
/// function unwinds or returns early. Without it, every `?` between `spawn()` and
/// `wait()` leaked a running FFmpeg that kept writing the temp file.
struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child already reaped")
    }

    /// Hands the child over to the caller, which then has to reap it itself.
    fn take(&mut self) -> Option<Child> {
        self.child.take()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Seek/trim arguments shared by every video encode attempt.
///
/// `-ss` is an *input* option: FFmpeg jumps to the nearest keyframe before
/// `start_s` and decodes only the clip, instead of decoding the whole file up to
/// `start_s` on every pass. Because the output timeline starts at zero when the
/// seek happens on the input, the length is expressed with `-t` rather than the
/// absolute `-to`. Seeking accuracy is unaffected (verified frame-by-frame on a
/// timecoded source).
fn video_trim_args(input_path: &str, start_s: f64, dur_s: f64) -> Vec<String> {
    vec![
        "-ss".into(),
        format!("{start_s:.3}"),
        "-i".into(),
        input_path.to_string(),
        "-t".into(),
        format!("{dur_s:.3}"),
    ]
}

#[allow(clippy::too_many_arguments)]
fn run_ffmpeg_progress(
    args: Vec<String>,
    total_duration_s: f64,
    stage: &str,
    progress: &mut dyn FnMut(&ProgressUpdate),
    cancel: &CancellationToken,
    stage_weight: f64,
    stage_offset: f64,
) -> Result<(), String> {
    let mut cmd = command(&ffmpeg_path());
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| format!("{}: {e}", i18n::t("err_run_ffmpeg")))?;
    let mut guard = ChildGuard::new(child);
    let pid = guard.child_mut().id();
    cancel.set_pid(pid);

    let stderr = guard
        .child_mut()
        .stderr
        .take()
        .ok_or_else(|| i18n::t("err_no_stderr").to_string())?;
    let tail = Arc::new(Mutex::new(VecDeque::<String>::new()));
    let tail_thread = tail.clone();
    let stderr_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut guard = tail_thread.lock().unwrap();
            if guard.len() >= 80 {
                guard.pop_front();
            }
            guard.push_back(line);
        }
    });

    let stdout = guard
        .child_mut()
        .stdout
        .take()
        .ok_or_else(|| i18n::t("err_no_stdout").to_string())?;
    let started = Instant::now();
    let mut out_time_us: i64 = 0;
    let mut speed_str = "1.0x".to_string();
    let mut fps_val = 0.0f64;
    let mut was_cancelled = false;

    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if cancel.is_cancelled() {
            was_cancelled = true;
            // Kill through the handle instead of `taskkill /PID`: the PID may
            // already belong to an unrelated process by now.
            let _ = guard.child_mut().kill();
            break;
        }
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        match key {
            "out_time_us" | "out_time_ms" => {
                if let Ok(us) = value.parse::<i64>() {
                    out_time_us = us;
                }
            }
            "speed" => speed_str = value.to_string(),
            "fps" => {
                if let Ok(fps) = value.parse::<f64>() {
                    fps_val = fps;
                }
            }
            _ => {}
        }

        let current_s = out_time_us as f64 / 1_000_000.0;
        let sub = (current_s / total_duration_s.max(0.1)).clamp(0.0, 1.0);
        let overall = stage_offset + sub * stage_weight * 100.0;
        let speed_factor = speed_str
            .trim_end_matches('x')
            .parse::<f64>()
            .unwrap_or(1.0)
            .max(0.01);
        let eta = ((total_duration_s - current_s).max(0.0)) / speed_factor;

        progress(&ProgressUpdate {
            stage: stage.to_string(),
            percent: overall,
            speed: speed_str.clone(),
            fps: fps_val,
            eta_s: eta,
            elapsed_s: started.elapsed().as_secs_f64(),
        });
    }

    let status = match guard.take() {
        Some(mut child) => child
            .wait()
            .map_err(|e| format!("{}: {e}", i18n::t("err_wait_ffmpeg")))?,
        None => return Err(i18n::t("err_cancelled").to_string()),
    };
    cancel.clear_pid();
    let _ = stderr_thread.join();

    if was_cancelled || cancel.is_cancelled() {
        return Err(i18n::t("err_cancelled").to_string());
    }

    if !status.success() {
        let stderr_text = tail
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let snippet = tail_chars(stderr_text.trim(), 1800);
        return Err(format!("{}\n{snippet}", i18n::t("err_ffmpeg")));
    }

    progress(&ProgressUpdate {
        stage: stage.to_string(),
        percent: stage_offset + stage_weight * 100.0,
        speed: speed_str,
        fps: fps_val,
        eta_s: 0.0,
        elapsed_s: started.elapsed().as_secs_f64(),
    });
    Ok(())
}

/// Returns at most the last `max_chars` characters of `text`.
///
/// Slicing by byte length panics when the cut lands inside a multi-byte UTF-8
/// sequence, which happens routinely with non-ASCII paths echoed by FFmpeg
/// (e.g. `C:\\Users\\użytkownik\\...`), so the trim is done on character boundaries.
fn tail_chars(text: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return &text[text.len()..];
    }
    let mut chars = 0;
    for (index, _) in text.char_indices().rev() {
        chars += 1;
        if chars == max_chars {
            return &text[index..];
        }
    }
    text
}

// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

fn same_file(a: &str, b: &str) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(pa), Ok(pb)) => pa == pb,
        _ => {
            let na = std::path::absolute(a)
                .map(|p| p.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|_| a.to_ascii_lowercase());
            let nb = std::path::absolute(b)
                .map(|p| p.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|_| b.to_ascii_lowercase());
            na == nb
        }
    }
}

#[cfg(windows)]
fn replace_file(src: &Path, dst: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;

    let src_w: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
    let dst_w: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe { MoveFileExW(src_w.as_ptr(), dst_w.as_ptr(), MOVEFILE_REPLACE_EXISTING) };
    if ok == 0 {
        Err(format!(
            "{}: {}",
            i18n::t("err_replace_output"),
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        std::fs::remove_file(dst).map_err(|e| e.to_string())?;
    }
    std::fs::rename(src, dst).map_err(|e| e.to_string())
}

/// Gives `target` the modification time of `source`.
///
/// A trimmed export is a brand new file, but it is the same *recording*: keeping
/// the source timestamp means the gallery, the date sorter and Windows Explorer
/// all still show when the clip was captured instead of when it was cut.
fn copy_modified_time(source: &Path, target: &Path) {
    let Ok(modified) = std::fs::metadata(source).and_then(|meta| meta.modified()) else {
        return;
    };
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(target) {
        let _ = file.set_modified(modified);
    }
}

fn temp_tag() -> String {
    // Two jobs started in the same millisecond from the same process (parallel
    // queue) would otherwise share temp files and overwrite each other.
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}_{}", millis, std::process::id(), sequence)
}

// ---------------------------------------------------------------------------
// Video encoding
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn encode_video(
    input_path: &str,
    output_path: &str,
    start_s: f64,
    end_s: f64,
    target_mb: f64,
    preset_mode: &str,
    codec: Option<VideoCodec>,
    speed: &str,
    prefer_hardware: Option<bool>,
    crop_box: Option<&CropBox>,
    progress: &mut dyn FnMut(&ProgressUpdate),
    cancel: &CancellationToken,
) -> Result<PathBuf, String> {
    encode_video_with_game(
        input_path,
        output_path,
        start_s,
        end_s,
        target_mb,
        preset_mode,
        codec,
        speed,
        prefer_hardware,
        crop_box,
        None,
        progress,
        cancel,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_video_with_game(
    input_path: &str,
    output_path: &str,
    start_s: f64,
    end_s: f64,
    target_mb: f64,
    preset_mode: &str,
    codec: Option<VideoCodec>,
    speed: &str,
    prefer_hardware: Option<bool>,
    crop_box: Option<&CropBox>,
    game: Option<&str>,
    progress: &mut dyn FnMut(&ProgressUpdate),
    cancel: &CancellationToken,
) -> Result<PathBuf, String> {
    if !Path::new(input_path).exists() {
        return Err(format!("{}: {input_path}", i18n::t("err_input_not_found")));
    }
    if end_s <= start_s {
        return Err(i18n::t("err_end_before_start").to_string());
    }
    if same_file(input_path, output_path) {
        return Err(i18n::t("err_output_is_source").to_string());
    }

    let out_dir = Path::new(output_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let video_info = crate::media::probe_video_strict(input_path)?;

    let mut actual_codec_choice = codec;
    let mut speed_choice = speed.to_string();
    let mut prefer_hw = prefer_hardware.unwrap_or(true);

    if actual_codec_choice.is_none() {
        let (parsed_codec, parsed_speed, parsed_hw) = parse_preset_mode(preset_mode);
        actual_codec_choice = Some(parsed_codec);
        if speed.is_empty() || speed == "hq" {
            speed_choice = parsed_speed;
        }
        if prefer_hardware.is_none() {
            prefer_hw = parsed_hw;
        }
    } else if prefer_hardware.is_none() {
        let lower = preset_mode.to_ascii_lowercase();
        prefer_hw = !(lower.contains("cpu") || lower.starts_with("lib"));
    }

    let codec_choice = actual_codec_choice.unwrap_or(VideoCodec::H264);
    let resolved = resolve_encoder(codec_choice, &speed_choice, prefer_hw);
    let encoder_name = resolved.encoder_name.clone();
    let actual_codec = resolved.codec;

    let plan: Plan = plan::calculate_plan(
        &video_info,
        start_s,
        end_s,
        target_mb,
        actual_codec,
        &speed_choice,
        prefer_hw,
        input_path,
        i18n::lang_code(),
        crop_box,
    )?;

    let dur_s = plan.duration_s;
    let audio_bytes = plan.audio_bytes;
    let overhead_bytes = plan.overhead_bytes;
    let has_audio = plan.has_audio;
    let hard_limit_bytes = plan.hard_limit_bytes;
    let filter_str = plan.filter_str.clone();
    let a_kbps = plan.audio_kbps;

    let ffmpeg = ffmpeg_path();
    let tag = temp_tag();
    let file_name = Path::new(output_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "output.mp4".into());
    let temp_output = out_dir.join(format!(".cg_tmp_{tag}_{file_name}"));
    let mut mux_args = container_mux_args(output_path);
    if let Some(game) = game {
        // MP4 needs the mdta metadata format for custom keys. Matroska supports them directly.
        if let [_, flags] = mux_args.as_mut_slice() {
            flags.push_str("+use_metadata_tags");
        }
        mux_args.extend(["-metadata".into(), format!("cutgut_game={game}")]);
    }
    mux_args.push(temp_output.to_string_lossy().into_owned());
    let stats_base = std::env::temp_dir().join(format!("cg_stats_{tag}"));

    let mut cleanup: Vec<PathBuf> = vec![
        temp_output.clone(),
        PathBuf::from(format!("{}-0.log", stats_base.to_string_lossy())),
        PathBuf::from(format!("{}-0.log.mbtree", stats_base.to_string_lossy())),
        PathBuf::from(format!("{}.log", stats_base.to_string_lossy())),
        PathBuf::from(format!("{}.log.mbtree", stats_base.to_string_lossy())),
    ];

    let result = (|| -> Result<PathBuf, String> {
        // Fast lossless remux path
        if plan.is_remux {
            progress(&ProgressUpdate {
                stage: i18n::t("stage_remux").to_string(),
                percent: 30.0,
                speed: i18n::t("speed_direct").to_string(),
                fps: 0.0,
                eta_s: 0.5,
                elapsed_s: 0.1,
            });

            // A copied 5.1 track is not what the plan promised, and some
            // platforms reject it. The video still stays a copy; only the audio
            // is re-encoded, which costs a fraction of a second.
            let codec_args: Vec<String> = if has_audio && video_info.audio_channels > 2 {
                let mut args = vec!["-c:v".to_string(), "copy".to_string()];
                args.extend(build_audio_stream_args(has_audio, a_kbps));
                args
            } else {
                vec!["-c".to_string(), "copy".to_string()]
            };

            let mut args: Vec<String> = vec![
                "-y".into(),
                "-hide_banner".into(),
                "-ss".into(),
                start_s.to_string(),
                "-to".into(),
                end_s.to_string(),
                "-i".into(),
                input_path.to_string(),
            ];
            args.extend(codec_args);
            args.extend(mux_args.iter().cloned());

            if let Ok(out) = run_process_timeout(&ffmpeg, &args, REMUX_TIMEOUT) {
                if out.status.success() && temp_output.exists() {
                    let size = std::fs::metadata(&temp_output)
                        .map(|m| m.len())
                        .unwrap_or(0) as i64;
                    // A whole-clip copy has nothing to verify; a trimmed one does.
                    let trimmed = start_s > 0.05 || end_s < video_info.duration - 0.05;
                    let exact = !trimmed || trim_is_exact(&temp_output, dur_s, video_info.fps);
                    if size > 1000 && size <= hard_limit_bytes && exact {
                        replace_file(&temp_output, Path::new(output_path))?;
                        copy_modified_time(Path::new(input_path), Path::new(output_path));
                        progress(&ProgressUpdate {
                            stage: i18n::t("stage_done").to_string(),
                            percent: 100.0,
                            speed: i18n::t("speed_direct").to_string(),
                            fps: 0.0,
                            eta_s: 0.0,
                            elapsed_s: 0.2,
                        });
                        return Ok(PathBuf::from(output_path));
                    }
                }
            }
            // The copy stays a copy only when the cut really landed on a keyframe.
            // Anything else falls through to a re-encode, which is frame accurate.
            if temp_output.exists() {
                let _ = std::fs::remove_file(&temp_output);
            }
        }

        let vf_args: Vec<String> = match &filter_str {
            Some(f) => vec!["-vf".into(), f.clone()],
            None => Vec::new(),
        };
        // A tone-mapped export is plain SDR, so it has to say so: untagged frames
        // are guessed by players, and players guess badly.
        let color_args: Vec<String> = if plan.hdr_tonemapped {
            [
                "-colorspace",
                "bt709",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        } else {
            Vec::new()
        };
        let audio_args = build_audio_stream_args(has_audio, a_kbps);

        let mut current_vkbps = plan.video_kbps;
        let max_attempts = 3i32;
        let mut attempt = 1i32;

        while attempt <= max_attempts {
            if cancel.is_cancelled() {
                return Err(i18n::t("err_cancelled").to_string());
            }

            let is_retry = attempt > 1;
            let hw_desc = if encoder_name.contains("nvenc") {
                format!("NVIDIA NVENC {}", actual_codec.upper())
            } else if encoder_name.contains("amf") {
                format!("AMD AMF {}", actual_codec.upper())
            } else {
                format!("CPU {}", actual_codec.upper())
            };
            let stage_prefix = if is_retry {
                format!(
                    "{} {attempt}/{max_attempts}) - {encoder_name}",
                    i18n::t("stage_size_fix")
                )
            } else {
                format!("{} {hw_desc}", i18n::t("stage_compress"))
            };

            let is_cpu_2pass = (encoder_name.contains("x264")
                || encoder_name.contains("x265")
                || encoder_name == "libsvtav1")
                && speed_choice == "hq";

            // Input-side seek, so a clip taken from the end of a long recording
            // does not decode everything before it (measured: ~1.2 s saved per
            // job on a 1080p60 source, once per pass).
            let common = video_trim_args(input_path, start_s, dur_s);

            if is_cpu_2pass {
                let stats = stats_base.to_string_lossy().to_string();
                let pass1 = build_encoder_video_args(
                    &encoder_name,
                    current_vkbps,
                    &speed_choice,
                    1,
                    &stats,
                );
                let mut args: Vec<String> = vec![
                    "-y".into(),
                    "-hide_banner".into(),
                    "-progress".into(),
                    "pipe:1".into(),
                ];
                args.extend(common.clone());
                args.extend(vf_args.clone());
                args.extend(color_args.clone());
                args.extend(pass1);
                args.extend(["-an".into(), "-f".into(), "null".into(), "-".into()]);

                run_ffmpeg_progress(
                    args,
                    dur_s,
                    &format!("{stage_prefix} [{} 1/2]", i18n::t("stage_pass")),
                    progress,
                    cancel,
                    0.35,
                    0.0,
                )?;

                if cancel.is_cancelled() {
                    return Err(i18n::t("err_cancelled").to_string());
                }

                let pass2 = build_encoder_video_args(
                    &encoder_name,
                    current_vkbps,
                    &speed_choice,
                    2,
                    &stats,
                );
                let mut args: Vec<String> = vec![
                    "-y".into(),
                    "-hide_banner".into(),
                    "-progress".into(),
                    "pipe:1".into(),
                ];
                args.extend(common);
                args.extend(vf_args.clone());
                args.extend(color_args.clone());
                args.extend(pass2);
                args.extend(audio_args.clone());
                args.extend(mux_args.iter().cloned());

                run_ffmpeg_progress(
                    args,
                    dur_s,
                    &format!("{stage_prefix} [{} 2/2]", i18n::t("stage_pass")),
                    progress,
                    cancel,
                    0.65,
                    35.0,
                )?;
            } else {
                let single = build_encoder_video_args(
                    &encoder_name,
                    current_vkbps,
                    &speed_choice,
                    0,
                    &stats_base.to_string_lossy(),
                );
                let mut args: Vec<String> = vec![
                    "-y".into(),
                    "-hide_banner".into(),
                    "-progress".into(),
                    "pipe:1".into(),
                ];
                args.extend(common);
                args.extend(vf_args.clone());
                args.extend(color_args.clone());
                args.extend(single);
                args.extend(audio_args.clone());
                args.extend(mux_args.iter().cloned());

                run_ffmpeg_progress(args, dur_s, &stage_prefix, progress, cancel, 1.0, 0.0)?;
            }

            if !temp_output.exists() {
                return Err(i18n::t("err_encode_not_created").to_string());
            }

            let actual_size = std::fs::metadata(&temp_output)
                .map(|m| m.len())
                .unwrap_or(0) as i64;
            let safe_limit = (hard_limit_bytes as f64 * 0.995) as i64;

            if actual_size <= safe_limit
                || (actual_size <= hard_limit_bytes && attempt >= max_attempts)
            {
                if actual_size < 1000 {
                    return Err(i18n::t("err_encode_corrupt").to_string());
                }
                replace_file(&temp_output, Path::new(output_path))?;
                copy_modified_time(Path::new(input_path), Path::new(output_path));
                return Ok(PathBuf::from(output_path));
            }

            if attempt < max_attempts {
                let _ = std::fs::remove_file(&temp_output);
                let fixed_overhead = audio_bytes + overhead_bytes;
                let measured_video = (actual_size - fixed_overhead).max(1000);
                let safety = if attempt == 1 { 0.96 } else { 0.93 };
                let target_video =
                    ((hard_limit_bytes as f64 * safety) as i64 - fixed_overhead).max(1000);
                let scale = (target_video as f64 / measured_video as f64).min(0.95);
                current_vkbps = ((current_vkbps as f64 * scale) as i32).max(25);
                attempt += 1;
            } else {
                let _ = std::fs::remove_file(&temp_output);
                return Err(i18n::fill(
                    i18n::t("err_output_too_large"),
                    &[
                        &format!("{:.2}", actual_size as f64 / 1_000_000.0),
                        &format!("{target_mb:.2}"),
                        &format!("{:.2}", hard_limit_bytes as f64 / 1_000_000.0),
                        &max_attempts.to_string(),
                        &actual_codec.upper(),
                        i18n::t("err_encode_too_large"),
                    ],
                ));
            }
        }

        Err(i18n::t("err_encode_failed").to_string())
    })();

    for path in cleanup.drain(..) {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Audio encoding
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn encode_audio(
    input_path: &str,
    output_path: &str,
    start_s: f64,
    end_s: f64,
    target_mb: Option<f64>,
    audio_format: &str,
    bitrate_kbps: Option<i32>,
    progress: &mut dyn FnMut(&ProgressUpdate),
    cancel: &CancellationToken,
) -> Result<PathBuf, String> {
    if !Path::new(input_path).exists() {
        return Err(format!("{}: {input_path}", i18n::t("err_input_not_found")));
    }
    if end_s <= start_s {
        return Err(i18n::t("err_end_before_start").to_string());
    }
    if same_file(input_path, output_path) {
        return Err(i18n::t("err_output_is_source").to_string());
    }

    let video_info = crate::media::probe_video_strict(input_path)?;
    let plan: AudioPlan = plan::calculate_audio_plan(
        &video_info,
        start_s,
        end_s,
        target_mb,
        audio_format,
        bitrate_kbps,
    );
    if !plan.has_audio {
        return Err(i18n::t("err_no_audio_track").to_string());
    }

    let out_dir = Path::new(output_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let tag = temp_tag();
    let file_name = Path::new(output_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "output.m4a".into());
    let temp_output = out_dir.join(format!(".cg_tmp_{tag}_{file_name}"));

    let dur_s = plan.duration_s;
    let mut kbps = plan.audio_kbps;
    let hard_limit = plan.hard_limit_bytes;
    let codec = plan.codec.clone();
    let fmt = plan.format.clone();

    let result = (|| -> Result<PathBuf, String> {
        let mut attempt = 1;
        loop {
            if cancel.is_cancelled() {
                return Err(i18n::t("err_cancelled").to_string());
            }

            let mut args: Vec<String> = vec![
                "-y".into(),
                "-hide_banner".into(),
                "-progress".into(),
                "pipe:1".into(),
                "-ss".into(),
                start_s.to_string(),
                "-to".into(),
                end_s.to_string(),
                "-i".into(),
                input_path.to_string(),
                "-map".into(),
                "0:a:0".into(),
                "-vn".into(),
                "-sn".into(),
                "-dn".into(),
                "-c:a".into(),
                codec.clone(),
                "-b:a".into(),
                format!("{kbps}k"),
            ];
            if fmt == "opus" {
                args.extend(["-vbr".into(), "on".into()]);
            }
            args.extend(container_mux_args(&temp_output.to_string_lossy()));
            args.push(temp_output.to_string_lossy().to_string());

            run_ffmpeg_progress(
                args,
                dur_s,
                &format!("{}{})", i18n::t("stage_audio_export"), fmt.to_uppercase()),
                progress,
                cancel,
                1.0,
                0.0,
            )?;

            if !temp_output.exists() {
                return Err(i18n::t("err_audio_not_created").to_string());
            }
            let actual_size = std::fs::metadata(&temp_output)
                .map(|m| m.len())
                .unwrap_or(0) as i64;
            if actual_size < 256 {
                return Err(i18n::t("err_audio_empty").to_string());
            }

            let within_limit = match hard_limit {
                Some(limit) => actual_size <= limit,
                None => true,
            };

            if within_limit {
                replace_file(&temp_output, Path::new(output_path))?;
                copy_modified_time(Path::new(input_path), Path::new(output_path));
                return Ok(PathBuf::from(output_path));
            }

            if attempt >= 2 {
                return Err(i18n::fill(
                    i18n::t("err_audio_too_large"),
                    &[
                        &format!("{:.2}", actual_size as f64 / 1_000_000.0),
                        &format!("{:.2}", target_mb.unwrap_or(0.0)),
                        i18n::t("err_audio_too_large_advice"),
                    ],
                ));
            }

            kbps = ((kbps as f64 * 0.92) as i32).max(16);
            attempt += 1;
            let _ = std::fs::remove_file(&temp_output);
        }
    })();

    if temp_output.exists() {
        let _ = std::fs::remove_file(&temp_output);
    }
    result
}

// ---------------------------------------------------------------------------
// Cleanup policies
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn send_to_recycle_bin(path: &Path, warn_permanent: bool) -> bool {
    use std::os::windows::ffi::OsStrExt;

    #[repr(C)]
    struct ShFileOpStructW {
        hwnd: *mut core::ffi::c_void,
        w_func: u32,
        p_from: *const u16,
        p_to: *const u16,
        f_flags: u16,
        f_any_operations_aborted: i32,
        h_name_mappings: *mut core::ffi::c_void,
        lpsz_progress_title: *const u16,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHFileOperationW(op: *mut ShFileOpStructW) -> i32;
    }

    const FO_DELETE: u32 = 3;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_WANTNUKEWARNING: u16 = 0x4000;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    wide.push(0);

    let mut op = ShFileOpStructW {
        hwnd: std::ptr::null_mut(),
        w_func: FO_DELETE,
        p_from: wide.as_ptr(),
        p_to: std::ptr::null(),
        f_flags: FOF_ALLOWUNDO
            | FOF_NOCONFIRMATION
            | FOF_SILENT
            | if warn_permanent {
                FOF_WANTNUKEWARNING
            } else {
                0
            },
        f_any_operations_aborted: 0,
        h_name_mappings: std::ptr::null_mut(),
        lpsz_progress_title: std::ptr::null(),
    };

    let res = unsafe { SHFileOperationW(&mut op) };
    res == 0 && op.f_any_operations_aborted == 0 && !path.exists()
}

#[cfg(not(windows))]
fn send_to_recycle_bin(_path: &Path, _warn_permanent: bool) -> bool {
    false
}

fn powershell_recycle(path: &Path) -> bool {
    let escaped = path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName Microsoft.VisualBasic; \
         [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile('{escaped}', \
         'OnlyErrorDialogs', 'SendToRecycleBin')"
    );
    let args: Vec<String> = vec![
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-Command".into(),
        script,
    ];
    if let Ok(out) = run_process("powershell", &args) {
        out.status.success() && !path.exists()
    } else {
        false
    }
}

/// Sends a file to the Windows Recycle Bin (with PowerShell fallback).
pub fn recycle_path(path: &str) -> bool {
    let p = Path::new(path);
    if !p.exists() {
        return true;
    }
    send_to_recycle_bin(p, false) || powershell_recycle(p)
}

pub fn cleanup_source_file(
    input_path: &str,
    output_path: &str,
    policy: &str,
) -> Result<String, String> {
    if policy == "never" {
        return Ok(i18n::t("cleanup_kept").to_string());
    }
    if input_path.is_empty() || output_path.is_empty() {
        return Err(i18n::t("err_no_path").to_string());
    }
    if same_file(input_path, output_path) {
        return Err(i18n::t("err_same_path").to_string());
    }
    if !Path::new(input_path).exists() {
        return Ok(i18n::t("err_source_missing").to_string());
    }
    if !Path::new(output_path).exists() {
        return Err(i18n::t("err_output_missing").to_string());
    }

    match policy {
        "trash" => {
            let absolute = std::path::absolute(input_path).map_err(|error| error.to_string())?;
            if send_to_recycle_bin(&absolute, true) {
                return Ok(i18n::t("cleanup_trashed").to_string());
            }
            Err(i18n::t("err_trash_failed").to_string())
        }
        "delete_permanently" => std::fs::remove_file(input_path)
            .map(|_| i18n::t("cleanup_deleted").to_string())
            .map_err(|e| format!("{} {e}", i18n::t("err_delete_failed"))),
        other => Err(format!("{}: {other}", i18n::t("err_unknown_policy"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_tail_never_splits_a_multibyte_character() {
        // FFmpeg echoes paths, and Windows user names are frequently non-ASCII:
        // slicing this by byte length used to panic inside the stderr thread.
        let line = "[mov @ 0x1] Error opening C:\\Users\\użytkownik\\Zażółć gęślą jaźń.mp4";
        let tail = tail_chars(line, 12);
        assert_eq!(tail.chars().count(), 12);
        assert!(line.ends_with(tail));
        assert!(tail.is_char_boundary(0));

        // Every window length must stay on a character boundary.
        for max in 0..=line.chars().count() + 2 {
            let tail = tail_chars(line, max);
            assert!(tail.chars().count() <= max);
            assert!(line.ends_with(tail));
        }
    }

    #[test]
    fn short_stderr_is_returned_unchanged() {
        assert_eq!(tail_chars("błąd", 1800), "błąd");
        assert_eq!(tail_chars("", 1800), "");
    }

    #[test]
    fn video_seek_is_an_input_option_and_uses_a_length_not_a_timestamp() {
        let args = video_trim_args("in.mp4", 90.5, 12.25);
        let seek = args.iter().position(|arg| arg == "-ss").expect("-ss");
        let input = args.iter().position(|arg| arg == "-i").expect("-i");
        assert!(seek < input, "-ss must come before -i: {args:?}");
        assert_eq!(args[input + 1], "in.mp4");
        assert!(args.iter().any(|arg| arg == "-t"), "{args:?}");
        assert!(
            !args.iter().any(|arg| arg == "-to"),
            "-to is absolute and would trim at the wrong point: {args:?}"
        );
        assert_eq!(args.last().map(String::as_str), Some("12.250"));
        assert_eq!(args[seek + 1], "90.500");
    }

    #[test]
    fn timeout_kills_a_hanging_process() {
        let (program, args) = if cfg!(windows) {
            (
                "cmd".to_string(),
                vec!["/C".to_string(), "ping -n 20 127.0.0.1 > nul".to_string()],
            )
        } else {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "sleep 20".to_string()],
            )
        };
        let started = Instant::now();
        let result = run_process_timeout(&program, &args, Duration::from_millis(250));
        assert!(result.is_err(), "a hanging process must not report success");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the timeout did not cut the wait short"
        );
    }

    #[test]
    fn timeout_returns_output_of_a_fast_process() {
        let (program, args) = if cfg!(windows) {
            (
                "cmd".to_string(),
                vec!["/C".to_string(), "echo hej".to_string()],
            )
        } else {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "echo hej".to_string()],
            )
        };
        let out = run_process_timeout(&program, &args, Duration::from_secs(30)).expect("output");
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("hej"));
    }

    #[test]
    fn nvenc_args_include_vbr_buffers() {
        let args = build_encoder_video_args("h264_nvenc", 2000, "hq", 0, "stats");
        let joined = args.join(" ");
        assert!(joined.contains("-rc:v vbr"));
        assert!(joined.contains("-b:v 2000k"));
        assert!(joined.contains("-maxrate 3000k"));
        assert!(joined.contains("-bufsize 4000k"));
    }

    #[test]
    fn x264_two_pass_args() {
        let p1 = build_encoder_video_args("libx264", 1500, "hq", 1, "base").join(" ");
        let p2 = build_encoder_video_args("libx264", 1500, "hq", 2, "base").join(" ");
        assert!(p1.contains("-pass 1"));
        assert!(p1.contains("-passlogfile base"));
        assert!(p2.contains("-pass 2"));
        assert!(p2.contains("-maxrate"));
    }

    #[test]
    fn svtav1_two_pass_args() {
        let p1 = build_encoder_video_args("libsvtav1", 900, "hq", 1, "base").join(" ");
        assert!(p1.contains("-preset 5"));
        assert!(p1.contains("-pass 1"));
    }

    #[test]
    fn container_mux_for_mp4() {
        assert_eq!(container_mux_args("a.mp4"), vec!["-movflags", "+faststart"]);
        assert!(container_mux_args("a.mkv").is_empty());
    }

    #[test]
    fn audio_is_downmixed_for_social_exports() {
        let args = build_audio_stream_args(true, 128).join(" ");
        assert!(args.contains("-ac 2"), "{args}");
        assert!(args.contains("-c:a aac"));
        assert!(!build_audio_stream_args(false, 128)
            .join(" ")
            .contains("-ac"));
    }

    /// End-to-end checks that need a real ffmpeg: they build a tiny clip and run
    /// the same code the app runs, so a broken cut or a blown size limit fails
    /// here instead of in a user's export.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cutgut-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn ffmpeg_ready() -> bool {
        run_process(&ffmpeg_path(), &["-version".to_string()])
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Six seconds, 60 fps, keyframes every 2 s (0/2/4) and 5.1 sound.
    fn sample_clip(dir: &Path) -> PathBuf {
        let out = dir.join("clip.mp4");
        let args = [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=60:duration=6",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=6",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-g",
            "120",
            "-c:a",
            "aac",
            "-ac",
            "6",
        ]
        .iter()
        .map(|s| s.to_string())
        .chain([out.to_string_lossy().into_owned()])
        .collect::<Vec<String>>();
        let output = run_process(&ffmpeg_path(), &args).expect("build sample");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        out
    }

    fn cut(input: &Path, out: &Path, start: f64, end: f64, target_mb: f64) {
        let mut progress = |_: &ProgressUpdate| {};
        encode_video_with_game(
            &input.to_string_lossy(),
            &out.to_string_lossy(),
            start,
            end,
            target_mb,
            "cpu_balanced",
            Some(VideoCodec::H264),
            "balanced",
            Some(false),
            None,
            None,
            &mut progress,
            &CancellationToken::new(),
        )
        .expect("export");
    }

    #[test]
    fn a_cut_between_keyframes_is_re_encoded_to_the_requested_seconds() {
        if !ffmpeg_ready() {
            eprintln!("ffmpeg missing, skipping export test");
            return;
        }
        let dir = scratch("misaligned");
        let input = sample_clip(&dir);
        let out = dir.join("cut.mp4");
        // 2.5 s is half a GOP away from the keyframe at 2.0, so a stream copy
        // used to start 0.5 s early and end 0.5 s short.
        cut(&input, &out, 2.5, 4.5, 5.0);

        let (start, duration) = crate::media::content_span(&out.to_string_lossy()).expect("span");
        assert!(start.abs() < 0.06, "starts at {start:.3} s");
        assert!(
            (duration - 2.0).abs() < 0.15,
            "runs {duration:.3} s instead of 2.0"
        );
        let info = crate::media::probe_video_strict(&out.to_string_lossy()).expect("probe");
        assert_eq!(info.audio_channels, 2, "5.1 sources are downmixed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cut_on_a_keyframe_stays_a_stream_copy() {
        if !ffmpeg_ready() {
            eprintln!("ffmpeg missing, skipping export test");
            return;
        }
        let dir = scratch("aligned");
        let input = sample_clip(&dir);
        let out = dir.join("cut.mp4");
        cut(&input, &out, 4.0, 6.0, 5.0);

        let source = crate::media::probe_video_strict(&input.to_string_lossy()).expect("source");
        let copied = crate::media::probe_video_strict(&out.to_string_lossy()).expect("copy");
        // A copy keeps the source bitrate; a re-encode would be resized to fit
        // the multi-megabit budget the plan asks for.
        let ratio = copied.bitrate as f64 / source.bitrate.max(1) as f64;
        assert!(
            (0.9..1.1).contains(&ratio),
            "bitrate {ratio:.2}× the source: not a copy"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stream_copy_still_hands_over_stereo_audio() {
        if !ffmpeg_ready() {
            eprintln!("ffmpeg missing, skipping export test");
            return;
        }
        let dir = scratch("copy-stereo");
        let input = sample_clip(&dir);
        let out = dir.join("cut.mp4");
        cut(&input, &out, 4.0, 6.0, 5.0);

        let source = crate::media::probe_video_strict(&input.to_string_lossy()).expect("source");
        assert_eq!(source.audio_channels, 6, "the fixture is 5.1");
        let copied = crate::media::probe_video_strict(&out.to_string_lossy()).expect("copy");
        // The video is copied, but a 5.1 track must not survive a remux: some
        // platforms reject it and the plan budgeted for stereo.
        assert_eq!(copied.audio_channels, 2);
        let ratio = copied.bitrate as f64 / source.bitrate.max(1) as f64;
        assert!((0.9..1.1).contains(&ratio), "video was not copied");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_huge_limit_on_a_short_clip_still_exports() {
        if !ffmpeg_ready() {
            eprintln!("ffmpeg missing, skipping export test");
            return;
        }
        let dir = scratch("huge-limit");
        let input = sample_clip(&dir);
        let out = dir.join("cut.mp4");
        // A "500 MB" preset on a few seconds of video must not ask the encoder
        // for a bitrate it cannot open.
        cut(&input, &out, 0.0, 5.0, 500.0);
        let size = std::fs::metadata(&out).expect("output").len();
        assert!(size > 10_000 && size < 500_000_000, "{size} bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_export_never_exceeds_the_hard_limit() {
        if !ffmpeg_ready() {
            eprintln!("ffmpeg missing, skipping export test");
            return;
        }
        let dir = scratch("limit");
        let input = sample_clip(&dir);
        let out = dir.join("cut.mp4");
        cut(&input, &out, 0.0, 5.0, 0.5);

        let size = std::fs::metadata(&out).expect("output").len();
        assert!(size <= 500_000, "{size} bytes for a 0.5 MB limit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preset_parsing() {
        let (codec, speed, hw) = parse_preset_mode("CPU_HEVC_FAST");
        assert_eq!(codec, VideoCodec::Hevc);
        assert_eq!(speed, "fast");
        assert!(!hw);

        let (codec, _, hw) = parse_preset_mode("AV1_NVENC_HQ");
        assert_eq!(codec, VideoCodec::Av1);
        assert!(hw);
    }
}
