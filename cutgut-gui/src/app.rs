//! Main CutGut window (egui): preview, timeline, export panel, quality plan,
//! gallery and settings.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use cutgut::ffmpeg::{self, CancellationToken, ProgressUpdate};
use cutgut::media;
use cutgut::plan::{self, AudioPlan, CropBox, Plan, VideoCodec, VideoInfo};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke,
    TextureHandle, Vec2,
};

use crate::gallery::Gallery;
use crate::i18n::{t, Lang};
use crate::icons::{self, Icon};
use crate::player::{self, PlaybackRate, Player, PreviewSize};
use crate::scrub::ScrubCache;
use crate::settings::{OriginalCleanup, Settings};
use crate::still_preview::StillPreview;
use crate::theme::{self, Palette};
use crate::widgets::{self, BtnKind, Volume};

const LIMIT_VALUES: [f64; 5] = [10.0, 20.0, 50.0, 500.0, 0.0];
/// Quiet period before a settings change is written to disk.
const SETTINGS_SAVE_DELAY: std::time::Duration = std::time::Duration::from_millis(400);
const FORMATS: [&str; 5] = ["mp4", "mkv", "m4a", "opus", "mp3"];

struct VideoState {
    path: String,
    name: String,
    info: VideoInfo,
}

enum EncodeMsg {
    Progress(ProgressUpdate),
    Done(PathBuf, u64),
    Error(String),
}

struct EncodeState {
    rx: Receiver<EncodeMsg>,
    cancel: CancellationToken,
    game: Option<String>,
    original: Option<OriginalFile>,
}

#[derive(PartialEq, Eq)]
struct OriginalFile {
    path: String,
    size: u64,
    modified: std::time::SystemTime,
    created: Option<std::time::SystemTime>,
}

impl OriginalFile {
    fn read(path: &str) -> Option<Self> {
        let metadata = std::fs::symlink_metadata(path).ok()?;
        metadata.is_file().then_some(Self {
            path: path.to_owned(),
            size: metadata.len(),
            modified: metadata.modified().ok()?,
            created: metadata.created().ok(),
        })
    }
}

struct CleanupRequest {
    original: OriginalFile,
    output: PathBuf,
    output_size: u64,
}

impl CleanupRequest {
    fn files_match(&self) -> bool {
        OriginalFile::read(&self.original.path).as_ref() == Some(&self.original)
            && std::fs::metadata(&self.output).is_ok_and(|file| {
                file.is_file() && file.len() == self.output_size && file.len() > 0
            })
    }
}

enum Cleanup {
    Ask(CleanupRequest),
    Running(Receiver<(String, bool)>),
}

struct PendingTheme {
    capture: egui::UserData,
    origin: egui::Pos2,
    requested: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Playback {
    Full,
    Trim,
}

impl Playback {
    fn range(self, duration: f64, in_s: f64, out_s: f64) -> (f64, f64) {
        match self {
            Self::Full => (0.0, duration),
            Self::Trim => (in_s, out_s),
        }
    }

    fn start_at(self, position: f64, bounds: (f64, f64)) -> f64 {
        let (start, end) = bounds;
        if self == Self::Trim || position >= end - 0.015 {
            start
        } else {
            position
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DragMode {
    None,
    In,
    Out,
    Playhead,
}

pub struct CutGutApp {
    lang: Lang,
    logo: TextureHandle,
    background: theme::Background,
    settings: Settings,
    /// Set when the settings file needs rewriting; the write is deferred so that
    /// dragging a slider does not rewrite `settings.ini` on every frame.
    settings_dirty: bool,
    settings_changed_at: Option<Instant>,

    video: Option<VideoState>,
    position: f64,
    in_s: f64,
    out_s: f64,
    loop_play: bool,
    playback: Option<Playback>,
    playback_rate: PlaybackRate,
    preview_press: widgets::PreviewPress,
    playback_end: f64,
    pending_video: Option<Receiver<(String, Result<VideoInfo, String>)>>,
    snapshot: Option<Receiver<Result<(PathBuf, Option<egui::ColorImage>), String>>>,
    ctx: egui::Context,
    player: Player,
    volume: Volume,
    /// Transient play/pause badge: `true` = paused (||), `false` = playing (>).
    overlay: Option<(bool, Instant)>,
    /// Speed badge that slides in from the top while the picture is held down
    /// (or right after the speed was picked from the menu).
    rate_osd: Option<(PlaybackRate, Instant)>,
    /// Held down: the badge stays until the button comes back up.
    rate_osd_held: bool,

    preview_tex: Option<TextureHandle>,
    /// Frames of the open clip, decoded once: scrubbing reads them from memory
    /// instead of starting a decoder per request.
    scrub: ScrubCache,
    /// Which cached frame is on screen, so a drag uploads only when it changes.
    preview_cached: Option<usize>,
    /// Timeline zoom (1 = the whole clip) and the left edge of what is shown.
    zoom: f64,
    view_start: f64,
    still: StillPreview,
    last_frame_request: Option<(Instant, f64)>,
    drag_mode: DragMode,
    /// Playback mode to restore after scrubbing the playhead with the mouse.
    seek_resume: Option<Playback>,
    marker_grab_offset: f32,

    plan: Option<Plan>,
    audio_plan: Option<AudioPlan>,
    plan_error: Option<String>,
    plan_rx: Option<Receiver<Result<Plan, String>>>,
    plan_dirty: bool,

    encoding: Option<EncodeState>,
    cleanup: Option<Cleanup>,
    progress: f32,
    stage: String,
    status: String,
    last_output: Option<PathBuf>,
    /// ffmpeg's report from the last failed job, shown in a dialog: a toast
    /// cannot carry a page of stderr. The second field is the environment plus
    /// the log tail, read once when the job failed.
    export_error: Option<(String, String)>,

    show_gallery: bool,
    view_entered: Option<Instant>,
    pending_theme: Option<PendingTheme>,
    theme_reveal: Option<theme::Reveal>,
    show_settings: bool,
    /// Storage section shows a pie chart instead of the bar.
    storage_pie: bool,
    /// Small textures of the background themes, decoded the first time the picker
    /// is opened and reused afterwards.
    theme_thumbs: std::collections::HashMap<&'static str, egui::TextureHandle>,
    show_keybinds: bool,
    gallery: Gallery,
    toast: Option<(String, Instant)>,
    screenshot_path: Option<PathBuf>,
    screenshot_at: Instant,
    screenshot_requested: bool,
}

impl CutGutApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        theme::load_fonts(&cc.egui_ctx);
        theme::configure(
            &cc.egui_ctx,
            settings.dark_theme,
            !settings.background_theme.is_empty(),
        );
        // `+`/`-`/`0` belong to the timeline here. Left to itself, egui also uses
        // them (with Ctrl) to rescale the whole window, which looks like the app
        // is broken: fixed-width rows overlap and every layout shifts.
        cc.egui_ctx
            .options_mut(|options| options.zoom_with_keyboard = false);
        let lang = settings.lang;
        cutgut::i18n::set_lang(lang.code());
        // An encode that was killed mid-way leaves `.cg_tmp_*` files in the
        // output folder; clear the ones that can no longer belong to a job.
        cutgut::media::sweep_stale_temp_files(
            std::path::Path::new(&settings.output_dir),
            std::time::Duration::from_secs(6 * 60 * 60),
        );
        let ctx = cc.egui_ctx.clone();
        let mut gallery = Gallery::new(settings.gallery_dir.clone(), ctx.clone());
        gallery.set_output_dir(&settings.output_dir);
        gallery.set_volume(settings.volume);
        gallery.set_layout(
            crate::gallery::GroupBy::from_index(settings.gallery_group),
            crate::gallery::CardSize::from_index(settings.gallery_card),
        );
        gallery.set_sort(settings.gallery_sort, settings.gallery_desc);
        let mut volume = Volume::default();
        volume.set_percent(settings.volume);
        let icon = crate::app_icon();
        let logo = ctx.load_texture(
            "CutGut logo",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        let mut app = Self {
            lang,
            logo,
            background: theme::Background::new(cc.gl.clone()),
            video: None,
            position: 0.0,
            in_s: 0.0,
            out_s: 0.0,
            loop_play: false,
            playback: None,
            playback_rate: PlaybackRate::default(),
            preview_press: widgets::PreviewPress::default(),
            playback_end: 0.0,
            pending_video: None,
            snapshot: None,
            ctx: ctx.clone(),
            player: Player::new(ctx.clone()),
            volume,
            overlay: None,
            rate_osd: None,
            rate_osd_held: false,
            preview_tex: None,
            scrub: ScrubCache::default(),
            preview_cached: None,
            zoom: 1.0,
            view_start: 0.0,
            still: StillPreview::new(ctx),
            last_frame_request: None,
            drag_mode: DragMode::None,
            seek_resume: None,
            marker_grab_offset: 0.0,
            plan: None,
            audio_plan: None,
            plan_error: None,
            plan_rx: None,
            plan_dirty: false,
            encoding: None,
            cleanup: None,
            progress: 0.0,
            stage: String::new(),
            status: t(lang, "ready").to_string(),
            last_output: None,
            export_error: None,
            show_gallery: false,
            view_entered: None,
            pending_theme: None,
            theme_reveal: None,
            show_settings: false,
            storage_pie: false,
            theme_thumbs: std::collections::HashMap::new(),
            show_keybinds: false,
            gallery,
            toast: None,
            settings,
            settings_dirty: false,
            settings_changed_at: None,
            screenshot_path: None,
            screenshot_at: Instant::now(),
            screenshot_requested: false,
        };
        app.player.set_volume(app.volume.percent());
        let args: Vec<String> = std::env::args().collect();
        let video_arg = match args.get(1).map(String::as_str) {
            Some("--screenshot") => args.get(3).is_some_and(|path| Path::new(path).is_file()),
            Some(path) => Path::new(path).is_file(),
            None => false,
        };
        if app.settings.gallery_on_startup && !video_arg {
            let folders = app.settings.gallery_folders();
            if !folders.iter().any(|dir| std::path::Path::new(dir).is_dir()) {
                if let Some(dir) = media::nvidia_gallery_dir() {
                    app.settings.gallery_dir = dir.to_string_lossy().to_string();
                }
            }
            app.show_gallery = true;
            app.gallery.set_dirs(app.settings.gallery_folders());
        }
        if args.len() >= 3 && args[1] == "--screenshot" {
            app.screenshot_path = Some(PathBuf::from(&args[2]));
            app.screenshot_at = Instant::now();
            app.show_settings = args.iter().any(|arg| arg == "--settings");
            app.show_keybinds = args.iter().any(|arg| arg == "--keybinds");
            if args.get(3).map(|s| s.as_str()) == Some("--gallery") {
                if let Some(dir) = args.get(4) {
                    app.settings.gallery_dir = dir.clone();
                    app.settings.gallery_dirs = vec![dir.clone()];
                    app.gallery.set_dir(dir.clone());
                    app.show_gallery = true;
                    if args.iter().any(|arg| arg == "--open-first") {
                        app.gallery.open_first();
                    }
                    if args.iter().any(|arg| arg == "--play-first") {
                        app.gallery.play_selected();
                    }
                }
            } else if let Some(video) = args.get(3) {
                if std::path::Path::new(video).is_file() {
                    app.load_video(video);
                }
            }
            if let Some(position) = args.iter().position(|arg| arg == "--crop") {
                if let Some(value) = args.get(position + 1) {
                    app.settings.export.crop = match value.as_str() {
                        "9:16" => 1,
                        "1:1" => 2,
                        "16:9" => 3,
                        _ => 0,
                    };
                    app.plan_dirty = true;
                }
            }
        } else if let Some(arg) = args.get(1) {
            if std::path::Path::new(arg).is_file() {
                app.load_video(arg);
            }
        }
        if app.pending_video.is_some() {
            app.show_gallery = false;
        }
        app
    }

    fn target_mb(&self) -> f64 {
        if self.settings.export.limit >= LIMIT_VALUES.len() {
            self.settings.export.custom_mb
        } else if self.settings.export.limit == 4 {
            self.settings.export.custom_mb
        } else {
            LIMIT_VALUES[self.settings.export.limit]
        }
    }

    fn crop_box(&self) -> Option<CropBox> {
        let video = self.video.as_ref()?;
        if self.settings.export.crop == 0 {
            return None;
        }
        let ratio = match self.settings.export.crop {
            1 => "9:16",
            2 => "1:1",
            _ => "16:9",
        };
        // FFmpeg applies rotation metadata before any filter, so the crop box
        // has to be computed in the picture's real (displayed) size.
        let (width, height) = video.info.display_size();
        Some(CropBox::calculate_default(
            width as i32,
            height as i32,
            ratio,
            "center",
        ))
    }

    fn speed_hw(&self) -> (&'static str, bool) {
        match self.settings.export.engine {
            0 => ("hq", true),
            1 => ("fast", true),
            2 => ("hq", false),
            _ => ("fast", false),
        }
    }

    fn is_audio_export(&self) -> bool {
        self.settings.export.format >= 2
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(t(self.lang, "media_files"), media::VIDEO_EXTS)
            .pick_file()
        {
            self.load_video(&path.to_string_lossy());
        }
    }

    fn load_video(&mut self, path: &str) {
        self.pause_playback();
        let ctx = self.ctx.clone();
        let path = path.to_owned();
        let (tx, rx) = mpsc::channel();
        self.pending_video = Some(rx);
        std::thread::spawn(move || {
            let info = media::probe_video(&path);
            let _ = tx.send((path, info));
            ctx.request_repaint();
        });
    }

    fn poll_video(&mut self) {
        let Some(rx) = &self.pending_video else {
            return;
        };
        let (path, result) = match rx.try_recv() {
            Ok(message) => message,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.pending_video = None;
                return;
            }
        };
        self.pending_video = None;
        let Ok(info) = result else {
            self.show_toast(format!("{} · {path}", t(self.lang, "media_missing")));
            return;
        };
        if !Path::new(&path).is_file() {
            return;
        }
        self.pause_playback();
        self.still.clear();
        self.last_frame_request = None;
        self.drag_mode = DragMode::None;
        self.overlay = None;
        self.position = 0.0;
        self.in_s = 0.0;
        self.out_s = info.duration.max(0.001);
        self.preview_tex = None;
        self.plan = None;
        self.audio_plan = None;
        self.plan_error = None;
        self.last_output = None;

        let name = Path::new(&path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        self.video = Some(VideoState {
            path: path.to_string(),
            name,
            info,
        });
        self.status = t(self.lang, "ready").to_string();

        self.zoom = 1.0;
        self.view_start = 0.0;
        self.preview_cached = None;
        self.request_frame(true);
        if let Some(video) = &self.video {
            self.scrub.open(&self.ctx, &video.path, &video.info);
        }
        self.request_plan();
    }

    /// Closes the clip and goes back to the empty editor: decoders stopped, the
    /// frame cache dropped, nothing left holding a file handle.
    fn unload_video(&mut self) {
        self.player.stop();
        self.playback = None;
        self.playback_end = 0.0;
        self.scrub.clear();
        self.still.clear();
        self.preview_tex = None;
        self.preview_cached = None;
        self.overlay = None;
        self.rate_osd = None;
        self.rate_osd_held = false;
        self.video = None;
        self.plan = None;
        self.audio_plan = None;
        self.plan_error = None;
        self.position = 0.0;
        self.in_s = 0.0;
        self.out_s = 0.0;
        self.zoom = 1.0;
        self.view_start = 0.0;
        self.drag_mode = DragMode::None;
        self.seek_resume = None;
        self.status = t(self.lang, "no_file").to_string();
        self.toast = Some((t(self.lang, "video_unloaded").to_string(), Instant::now()));
    }

    fn request_frame(&mut self, immediate: bool) {
        let Some(video) = &self.video else { return };
        let last_frame = (video.info.duration - 1.0 / video.info.fps.max(1.0)).max(0.0);
        let time = self.position.min(last_frame).max(0.0);

        // The cached frame is what makes a drag feel instant: it is already
        // decoded, so the picture follows the playhead at screen rate. Only the
        // moment the user actually stops on is worth a fresh decoder, and even
        // then the cached frame stays up until the sharp one arrives.
        if let Some((index, image)) = self.scrub.frame_at(time) {
            if self.preview_cached != Some(index) {
                player::upload(
                    &self.ctx,
                    &mut self.preview_tex,
                    "preview",
                    (*image).clone(),
                );
                self.preview_cached = Some(index);
            }
            if !immediate {
                return;
            }
        }

        let debounce = if immediate {
            Duration::ZERO
        } else {
            Duration::from_millis(120)
        };
        if let Some((when, previous)) = self.last_frame_request {
            if (previous - time).abs() < 0.005 || when.elapsed() < debounce {
                return;
            }
        }
        self.last_frame_request = Some((Instant::now(), time));
        self.preview_cached = None;
        self.still
            .request(&video.path, time, PreviewSize::still(&video.info));
    }

    fn request_plan(&mut self) {
        self.plan_dirty = true;
    }

    fn update_plan(&mut self) {
        if !self.plan_dirty
            || self.plan_rx.is_some()
            || matches!(self.drag_mode, DragMode::In | DragMode::Out)
        {
            return;
        }
        let Some(video) = &self.video else {
            self.plan_dirty = false;
            return;
        };

        let target = self.target_mb();
        let crop = self.crop_box();
        let format = FORMATS[self.settings.export.format];

        if self.is_audio_export() {
            self.plan_dirty = false;
            self.plan = None;
            self.audio_plan = Some(plan::calculate_audio_plan(
                &video.info,
                self.in_s,
                self.out_s,
                Some(target),
                format,
                None,
            ));
            self.plan_error = None;
            return;
        }

        self.plan_dirty = false;
        let info = video.info.clone();
        let path = video.path.clone();
        let start = self.in_s;
        let end = self.out_s;
        let codec = self.settings.export.codec;
        let (speed, hw) = self.speed_hw();
        let speed = speed.to_string();
        let (tx, rx) = mpsc::channel();
        self.plan_rx = Some(rx);
        let ctx = self.ctx.clone();
        let lang_code = self.lang.code().to_string();
        std::thread::spawn(move || {
            let result = plan::calculate_plan(
                &info,
                start,
                end,
                target,
                codec,
                &speed,
                hw,
                &path,
                &lang_code,
                crop.as_ref(),
            );
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn poll_plan(&mut self) {
        let Some(rx) = &self.plan_rx else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.plan_rx = None;
                if self.plan_dirty {
                    return;
                }
                match result {
                    Ok(plan) => {
                        self.plan = Some(plan);
                        self.plan_error = None;
                    }
                    Err(error) => {
                        self.plan = None;
                        self.plan_error = Some(error);
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.plan_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    fn toggle_play(&mut self, mode: Playback) {
        if self.playback == Some(mode) {
            self.pause_playback();
            self.overlay = Some((true, Instant::now()));
            return;
        }
        let bounds = mode.range(self.duration(), self.in_s, self.out_s);
        self.start_playback(mode, mode.start_at(self.position, bounds), bounds.1);
        if self.playback.is_some() {
            self.overlay = Some((false, Instant::now()));
        }
    }

    /// Marks the settings file as stale. Writing is deferred: a slider drag
    /// changes the value on every frame, and an atomic rewrite per frame would
    /// hammer the disk for nothing.
    fn queue_settings_save(&mut self) {
        self.settings_dirty = true;
        self.settings_changed_at = Some(Instant::now());
    }

    /// Writes the deferred settings once the user stopped tweaking, or straight
    /// away when the window is closing.
    fn flush_settings(&mut self, ctx: &egui::Context) {
        if !self.settings_dirty {
            return;
        }
        let closing = ctx.input(|input| input.viewport().close_requested());
        let waited = self
            .settings_changed_at
            .map(|changed| changed.elapsed())
            .unwrap_or_default();
        if closing || waited >= SETTINGS_SAVE_DELAY {
            self.settings.save();
            self.settings_dirty = false;
            self.settings_changed_at = None;
        } else {
            ctx.request_repaint_after(SETTINGS_SAVE_DELAY - waited);
        }
    }

    /// Volume picked by the user in the editor or the gallery becomes the shared
    /// default for every clip (and survives restarts).
    fn save_volume(&mut self, percent: f32) {
        let percent = percent.clamp(0.0, 100.0);
        if (self.settings.volume - percent).abs() < 0.5 {
            return;
        }
        self.settings.volume = percent;
        self.volume.set_percent(percent);
        self.player.set_volume(percent);
        self.gallery.set_volume(percent);
        self.queue_settings_save();
    }

    fn start_playback(&mut self, mode: Playback, start: f64, end: f64) {
        let Some(video) = &self.video else { return };
        self.still.clear();
        self.last_frame_request = None;
        self.player.set_volume(self.volume.percent());
        self.player
            .start(&video.path, &video.info, start, end, self.playback_rate);
        self.position = start;
        self.playback_end = end;
        self.playback = Some(mode);
    }

    fn pause_playback(&mut self) {
        if self.playback.take().is_some() {
            self.player.stop();
        }
    }

    fn poll_playback(&mut self, ctx: &egui::Context) {
        let Some(mode) = self.playback else { return };
        if let Some(frame) = self.player.poll() {
            self.position = frame.time;
            self.preview_cached = None;
            player::upload(ctx, &mut self.preview_tex, "preview", frame.image);
        }
        if !self.player.is_active() {
            self.playback = None;
            // EOF at the selected end. A decoder failure must not restart the loop.
            if self.playback_end - self.position <= 0.12 {
                self.position = self.playback_end;
                if self.loop_play {
                    let (start, end) = mode.range(self.duration(), self.in_s, self.out_s);
                    self.start_playback(mode, start, end);
                }
            }
        }
    }

    fn show_toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    fn set_in_point(&mut self) {
        if self.video.is_some() {
            let time = self.snap(self.position);
            self.in_s = time.clamp(0.0, (self.out_s - 0.05).max(0.0));
            self.trim_changed();
        }
    }

    fn set_out_point(&mut self) {
        if self.video.is_some() {
            let duration = self.duration();
            let time = self.snap(self.position);
            self.out_s = time.clamp((self.in_s + 0.05).min(duration), duration);
            self.trim_changed();
        }
    }

    /// The nearest frame boundary: a cut can only happen there, so the trim
    /// points should not sit a fraction of a frame off and then be rounded by
    /// the encoder anyway.
    fn snap(&self, time: f64) -> f64 {
        let fps = self
            .video
            .as_ref()
            .map_or(30.0, |video| video.info.fps.max(1.0));
        let snapped = (time * fps).round() / fps;
        snapped.clamp(0.0, self.duration())
    }

    fn trim_changed(&mut self) {
        if self.playback == Some(Playback::Trim) {
            self.pause_playback();
        }
        self.request_plan();
    }

    fn duration(&self) -> f64 {
        self.video
            .as_ref()
            .map(|v| v.info.duration.max(0.001))
            .unwrap_or(0.0)
    }

    fn save_png(&mut self) {
        let Some(video) = &self.video else { return };
        if self.snapshot.is_some() {
            return;
        }
        let path = video.path.clone();
        let last_frame = (video.info.duration - 1.0 / video.info.fps.max(1.0)).max(0.0);
        let time = self.position.min(last_frame);
        let base = Path::new(&video.name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let name = format!("{base} - {time:.2}s");
        let output_dir = self.settings.output_dir.clone();
        let ctx = self.ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.snapshot = Some(rx);
        std::thread::spawn(move || {
            let out = media::generate_output_filepath(&output_dir, &name, ".png");
            let result =
                media::extract_frame_png(&path, time, &out.to_string_lossy(), None).map(|path| {
                    let image = image::open(&path).ok().map(|image| {
                        let rgba = image.to_rgba8();
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        )
                    });
                    (path, image)
                });
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn poll_snapshot(&mut self) {
        let Some(rx) = &self.snapshot else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.snapshot = None;
                match result {
                    Ok((path, image)) => {
                        self.gallery.reload();
                        let copied = image.is_some();
                        if let Some(image) = image {
                            self.ctx.copy_image(image);
                        }
                        self.show_toast(format!(
                            "{} · {}",
                            t(
                                self.lang,
                                if copied {
                                    "snapshot_clipboard"
                                } else {
                                    "snapshot"
                                }
                            ),
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                    }
                    Err(error) => self.show_toast(error),
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.snapshot = None,
        }
    }

    fn start_export(&mut self) {
        if self.encoding.is_some() || self.cleanup.is_some() {
            return;
        }
        let Some(video) = &self.video else {
            self.show_toast(t(self.lang, "select_video"));
            return;
        };

        let format = FORMATS[self.settings.export.format];
        let extension = plan::get_export_extension(format);
        let base = Path::new(&video.name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "clip".into());
        // The export belongs to the original recording, so it keeps that date
        // instead of being stamped with the day it was cut.
        let date = self.gallery.recording_day(Path::new(&video.path));
        let output = media::generate_output_filepath_dated(
            &self.settings.output_dir,
            &base,
            &extension,
            &date,
        );

        let path = video.path.clone();
        let game = self.gallery.source_game(Path::new(&path));
        let start = self.in_s;
        let end = self.out_s;
        let target = self.target_mb();
        let codec = self.settings.export.codec;
        let (speed, hw) = self.speed_hw();
        let speed = speed.to_string();
        let crop = self.crop_box();
        let audio_only = self.is_audio_export();
        let trimmed = start > 0.001 || end < video.info.duration - 0.001;
        let original = if !audio_only && trimmed {
            OriginalFile::read(&path)
        } else {
            None
        };

        let crop_note = crop.as_ref().map_or(String::new(), |crop| {
            format!(
                " · crop {} {}x{}+{}+{}",
                crop.ratio_type, crop.w, crop.h, crop.x, crop.y
            )
        });
        crate::log::write(
            "export",
            format!(
                "{path} -> {} · {start:.3}-{end:.3} s · limit {target} MB · codec {codec:?} · {speed}{} · {}{crop_note}",
                output.display(),
                if hw { "GPU" } else { "CPU" },
                if audio_only { "audio only" } else { "video" },
            ),
        );

        let (tx, rx) = mpsc::channel();
        let cancel = CancellationToken::new();
        let cancel_thread = cancel.clone();
        let output_for_thread = output.clone();
        let game_for_thread = game.clone();

        std::thread::spawn(move || {
            let mut progress = |update: &ProgressUpdate| {
                let _ = tx.send(EncodeMsg::Progress(update.clone()));
            };
            let result = if audio_only {
                ffmpeg::encode_audio(
                    &path,
                    &output_for_thread.to_string_lossy(),
                    start,
                    end,
                    Some(target),
                    format,
                    None,
                    &mut progress,
                    &cancel_thread,
                )
            } else {
                let game = media::read_cutgut_game(Path::new(&path)).or(game_for_thread);
                ffmpeg::encode_video_with_game(
                    &path,
                    &output_for_thread.to_string_lossy(),
                    start,
                    end,
                    target,
                    "auto",
                    Some(codec),
                    &speed,
                    Some(hw),
                    crop.as_ref(),
                    game.as_deref(),
                    &mut progress,
                    &cancel_thread,
                )
            };

            match result {
                Ok(path) => {
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let _ = tx.send(EncodeMsg::Done(path, size));
                }
                Err(error) => {
                    let _ = tx.send(EncodeMsg::Error(error));
                }
            }
        });

        self.encoding = Some(EncodeState {
            rx,
            cancel,
            game,
            original,
        });
        self.progress = 0.0;
        self.stage = t(self.lang, "exporting").to_string();
        self.status = self.stage.clone();
    }

    fn poll_encoding(&mut self) {
        let mut messages = Vec::new();
        let mut disconnected = false;
        if let Some(state) = &self.encoding {
            loop {
                match state.rx.try_recv() {
                    Ok(message) => messages.push(message),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if disconnected && messages.is_empty() {
            self.encoding = None;
        }
        for message in messages {
            match message {
                EncodeMsg::Progress(update) => {
                    self.progress = update.percent as f32;
                    self.stage = update.stage;
                }
                EncodeMsg::Done(path, size) => {
                    crate::log::write(
                        "export",
                        format!(
                            "done: {} ({:.2} MB)",
                            path.display(),
                            size as f64 / 1_000_000.0
                        ),
                    );
                    let state = self.encoding.take();
                    let (game, original) = state.map_or((None, None), |state| {
                        (
                            state.game,
                            if state.cancel.is_cancelled() {
                                None
                            } else {
                                state.original
                            },
                        )
                    });
                    let exported = cutgut::media::is_video_file(&path.to_string_lossy());
                    if exported {
                        self.gallery.export_finished(path.clone(), game, self.lang);
                    }
                    self.progress = 100.0;
                    self.stage = t(self.lang, "done").to_string();
                    self.status = format!(
                        "{}: {} ({:.2} MB)",
                        t(self.lang, "done"),
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        size as f64 / 1_000_000.0
                    );
                    self.last_output = Some(path.clone());
                    self.show_toast(self.status.clone());
                    if exported {
                        // Show the fresh export in the gallery, highlighted
                        // until the user opens it.
                        self.switch_view(true);
                    }
                    if let Some(original) = original {
                        let request = CleanupRequest {
                            original,
                            output: path,
                            output_size: size,
                        };
                        match self.settings.original_cleanup {
                            OriginalCleanup::Off => {}
                            OriginalCleanup::Ask => self.cleanup = Some(Cleanup::Ask(request)),
                            OriginalCleanup::Automatic => self.recycle_original(request),
                        }
                    }
                }
                EncodeMsg::Error(error) => {
                    crate::log::write("export", format!("failed: {error}"));
                    self.encoding = None;
                    self.progress = 0.0;
                    self.stage = t(self.lang, "error").to_string();
                    self.status = error.clone();
                    self.export_error = Some((error, crate::log::diagnostics()));
                }
            }
        }
    }

    fn recycle_original(&mut self, request: CleanupRequest) {
        let source = &request.original.path;
        self.gallery.release_source(Path::new(source));
        if self
            .video
            .as_ref()
            .is_some_and(|video| video.path == *source)
        {
            self.pause_playback();
            self.still.clear();
            self.scrub.clear();
        }
        let (tx, rx) = mpsc::channel();
        let ctx = self.ctx.clone();
        self.cleanup = Some(Cleanup::Running(rx));
        std::thread::spawn(move || {
            let recycled = request.files_match()
                && ffmpeg::cleanup_source_file(
                    &request.original.path,
                    &request.output.to_string_lossy(),
                    "trash",
                )
                .is_ok();
            let _ = tx.send((request.original.path, recycled));
            ctx.request_repaint();
        });
    }

    fn poll_cleanup(&mut self) {
        let Some(Cleanup::Running(rx)) = &self.cleanup else {
            return;
        };
        let (source, recycled) = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => (String::new(), false),
        };
        self.cleanup = None;
        if !recycled {
            if let Some(video) = self.video.as_ref().filter(|video| video.path == source) {
                self.scrub.open(&self.ctx, &video.path, &video.info);
            }
            self.show_toast(t(self.lang, "original_cleanup_failed"));
            return;
        }
        self.gallery.clip_removed(Path::new(&source), self.lang);
        if self
            .video
            .as_ref()
            .is_some_and(|video| video.path == source)
        {
            self.pause_playback();
            self.video = None;
            self.still.clear();
            self.scrub.clear();
            self.preview_tex = None;
            self.preview_cached = None;
            self.plan = None;
            self.audio_plan = None;
            self.plan_error = None;
            self.plan_rx = None;
        }
        self.show_toast(t(self.lang, "original_recycled"));
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        let paths: Vec<_> = dropped.into_iter().filter_map(|file| file.path).collect();
        if paths.is_empty() {
            return;
        }
        if self.show_settings {
            return;
        }
        if self.show_gallery {
            self.gallery.import(paths, self.lang);
        } else {
            self.load_video(&paths[0].to_string_lossy());
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.show_settings
            || matches!(self.cleanup, Some(Cleanup::Ask(_)))
            || self.gallery.has_modal()
            || egui::Popup::is_any_open(ctx)
            || ctx.wants_keyboard_input()
        {
            return;
        }
        if self.show_gallery {
            let (back, import, select_all) = ctx.input(|input| {
                (
                    input.key_pressed(egui::Key::Escape) || input.key_pressed(egui::Key::G),
                    input.modifiers.ctrl && input.key_pressed(egui::Key::O),
                    (input.modifiers.ctrl || input.modifiers.command)
                        && input.key_pressed(egui::Key::A),
                )
            });
            if import {
                self.gallery.import_dialog(self.lang);
            } else if select_all {
                self.gallery.mark_all(self.lang);
            } else if back {
                // Escape drops a selection first, and only then leaves the gallery.
                if !self.gallery.clear_marks() {
                    self.switch_view(false);
                }
            }
            return;
        }

        let (space, key_i, key_o, key_g, key_f, key_s, left, right, ctrl_o) = ctx.input(|input| {
            (
                input.key_pressed(egui::Key::Space),
                input.key_pressed(egui::Key::I),
                input.key_pressed(egui::Key::O),
                input.key_pressed(egui::Key::G),
                input.key_pressed(egui::Key::F),
                input.key_pressed(egui::Key::S),
                // Shift+arrow is a frame step, so the one-second jump must opt out.
                input.key_pressed(egui::Key::ArrowLeft) && !input.modifiers.shift,
                input.key_pressed(egui::Key::ArrowRight) && !input.modifiers.shift,
                input.modifiers.ctrl && input.key_pressed(egui::Key::O),
            )
        });
        let (frame_back, frame_forward, to_start, to_end, zoom_in, zoom_out, zoom_reset) = ctx
            .input(|input| {
                let shift = input.modifiers.shift;
                (
                    input.key_pressed(egui::Key::Comma)
                        || input.key_pressed(egui::Key::OpenBracket)
                        || (shift
                            && !input.modifiers.ctrl
                            && input.key_pressed(egui::Key::ArrowLeft)),
                    input.key_pressed(egui::Key::Period)
                        || input.key_pressed(egui::Key::CloseBracket)
                        || (shift
                            && !input.modifiers.ctrl
                            && input.key_pressed(egui::Key::ArrowRight)),
                    input.key_pressed(egui::Key::Home),
                    input.key_pressed(egui::Key::End),
                    input.key_pressed(egui::Key::Plus) || input.key_pressed(egui::Key::Equals),
                    input.key_pressed(egui::Key::Minus),
                    input.key_pressed(egui::Key::Num0),
                )
            });
        if ctrl_o {
            self.open_dialog();
        }
        if space {
            self.toggle_play(self.playback.unwrap_or(Playback::Full));
        }
        if key_i {
            self.set_in_point();
        }
        if key_o && !ctrl_o {
            self.set_out_point();
        }
        if key_g {
            self.switch_view(true);
        }
        if key_f {
            let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        if key_s {
            self.save_png();
        }
        // Shift turns the arrow keys into frame steps: a second is a lot when you
        // are trying to land on one specific frame.
        let shift = ctx.input(|input| input.modifiers.shift);
        if frame_back {
            self.step_frames(-1);
        } else if left && !shift {
            self.pause_playback();
            self.position = self.snap((self.position - 1.0).max(0.0));
            self.request_frame(true);
        }
        if frame_forward {
            self.step_frames(1);
        } else if right && !shift {
            self.pause_playback();
            self.position = self.snap((self.position + 1.0).min(self.duration()));
            self.request_frame(true);
        }
        if to_start {
            self.pause_playback();
            self.position = 0.0;
            self.request_frame(true);
        }
        if to_end {
            self.pause_playback();
            let duration = self.duration();
            self.position = self.snap(duration);
            self.request_frame(true);
        }
        let duration = self.duration();
        if zoom_in {
            self.zoom_timeline(1.6, self.position, duration);
        }
        if zoom_out {
            self.zoom_timeline(1.0 / 1.6, self.position, duration);
        }
        if zoom_reset {
            self.zoom = 1.0;
            self.view_start = 0.0;
        }
    }

    /// Moves the playhead by whole frames, so repeated presses cannot drift.
    fn step_frames(&mut self, count: i32) {
        self.pause_playback();
        let fps = self
            .video
            .as_ref()
            .map_or(30.0, |video| video.info.fps.max(1.0));
        let time = frame_step_position(self.position, fps, count, self.duration());
        if (time - self.position).abs() < 1e-9 {
            return;
        }
        self.position = time;
        self.request_frame(true);
    }
}

impl eframe::App for CutGutApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Stop every child process (preview decoder and ffplay audio) before
        // the window is destroyed, so no audio can outlive the application.
        self.player.stop();
        self.playback = None;
        self.gallery.stop_all();
        if let Some(state) = &self.encoding {
            state.cancel.cancel();
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.flush_settings(ctx);
        self.poll_theme(ctx);
        self.poll_plan();
        self.poll_encoding();
        self.poll_cleanup();
        self.poll_video();
        self.poll_snapshot();
        if let Some(image) = self.still.poll() {
            self.preview_cached = None;
            player::upload(ctx, &mut self.preview_tex, "preview", image);
        }
        self.poll_playback(ctx);
        self.update_plan();
        self.handle_shortcuts(ctx);
        self.handle_dropped_files(ctx);
        self.gallery.sync_window(ctx);

        let glass = !self.settings.background_theme.is_empty();
        self.background.paint(
            ctx,
            &self.settings.background_theme,
            self.settings.dark_theme,
        );
        // A fullscreen preview owns the whole window; the top bar would only
        // peek through at the edges.
        let fullscreen_preview = self.show_gallery
            && self.gallery.has_modal()
            && ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
        if !fullscreen_preview {
            self.top_bar(ctx);
        }
        let margin = if self.show_gallery {
            egui::Margin {
                bottom: 10,
                ..egui::Margin::symmetric(5, 4)
            }
        } else {
            egui::Margin::symmetric(5, 4)
        };
        let mut central_frame = egui::Frame::central_panel(&ctx.style()).inner_margin(margin);
        if glass {
            central_frame.fill = Color32::TRANSPARENT;
        }
        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ctx, |ui| {
                if let Some(start) = self.view_entered {
                    let remaining = (1.0 - start.elapsed().as_secs_f32() / 0.19).max(0.0);
                    if remaining > 0.0 {
                        let ease = remaining * remaining;
                        ui.set_opacity(1.0 - 0.30 * ease);
                        ui.add_space(6.0 * ease);
                        ctx.request_repaint();
                    } else {
                        self.view_entered = None;
                    }
                }
                if self.show_gallery && !glass {
                    theme::background(ui);
                }
                if self.show_gallery {
                    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                    self.gallery.ui(
                        ui,
                        ctx,
                        self.lang,
                        self.settings.pause_overlay,
                        self.show_settings
                            || self.show_keybinds
                            || matches!(self.cleanup, Some(Cleanup::Ask(_))),
                    );
                    if let Some(volume) = self.gallery.take_volume_change() {
                        self.save_volume(volume);
                    }
                    let (group, card) = (
                        self.gallery.group_by().index(),
                        self.gallery.card_size().index(),
                    );
                    let (sort, descending) = self.gallery.sort_state();
                    if (group, card) != (self.settings.gallery_group, self.settings.gallery_card)
                        || (sort, descending)
                            != (self.settings.gallery_sort, self.settings.gallery_desc)
                    {
                        self.settings.gallery_group = group;
                        self.settings.gallery_card = card;
                        self.settings.gallery_sort = sort;
                        self.settings.gallery_desc = descending;
                        self.queue_settings_save();
                    }
                    if let Some(message) = self.gallery.take_notice() {
                        self.show_toast(message);
                    }
                    if let Some(path) = self.gallery.take_edit() {
                        self.switch_view(false);
                        self.load_video(&path);
                    }
                } else {
                    self.editor_ui(ui);
                }
            });

        self.settings_window(ctx);
        self.keybinds_window(ctx);
        self.cleanup_window(ctx);
        self.error_window(ctx);
        self.toast_window(ctx);

        if let Some(reveal) = &self.theme_reveal {
            if !reveal.paint(ctx) {
                self.theme_reveal = None;
            }
        }

        if let Some(path) = self.screenshot_path.clone() {
            if !self.screenshot_requested {
                let remaining =
                    Duration::from_millis(4200).saturating_sub(self.screenshot_at.elapsed());
                if remaining.is_zero() {
                    self.screenshot_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                } else {
                    ctx.request_repaint_after(remaining);
                }
            }
            let events = ctx.input(|input| input.events.clone());
            for event in events {
                if let egui::Event::Screenshot {
                    image, user_data, ..
                } = event
                {
                    if user_data.data.is_some() {
                        continue;
                    }
                    let [width, height] = image.size;
                    let mut bytes = Vec::with_capacity(width * height * 4);
                    for pixel in &image.pixels {
                        bytes.extend_from_slice(&pixel.to_array());
                    }
                    if let Some(buffer) =
                        image::RgbaImage::from_raw(width as u32, height as u32, bytes)
                    {
                        let _ = buffer.save(&path);
                    }
                    self.screenshot_path = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        if self.encoding.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl CutGutApp {
    fn switch_view(&mut self, gallery: bool) {
        if self.show_gallery == gallery {
            return;
        }
        if self.preview_press.cancel() {
            self.playback_rate = PlaybackRate::Normal;
        }
        self.show_gallery = gallery;
        self.view_entered = Some(Instant::now());
        if gallery {
            self.pause_playback();
            self.gallery.set_dirs(self.settings.gallery_folders());
        } else {
            self.gallery.close_modal();
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        let lang = self.lang;
        let horizontal_margin = if self.show_gallery { 9 } else { 5 };
        let palette = Palette::for_context(ctx, self.settings.dark_theme);
        let mut frame = egui::Frame::side_top_panel(&ctx.style())
            .inner_margin(egui::Margin::symmetric(horizontal_margin, 5));
        if palette.glass {
            frame.fill = palette.panel;
        }
        egui::TopBottomPanel::top("topbar")
            .frame(frame)
            .show(ctx, |ui| {
                let palette = Palette::get(ui);
                ui.horizontal_centered(|ui| {
                    ui.image((self.logo.id(), Vec2::splat(32.0)));
                    ui.label(
                        RichText::new("CutGut")
                            .family(egui::FontFamily::Name("Instrument Serif".into()))
                            .size(26.0),
                    );
                    ui.add_space(5.0);
                    if widgets::tab(
                        ui,
                        t(lang, "editor"),
                        !self.show_gallery,
                        Some(Icon::Scissors),
                    )
                    .clicked()
                    {
                        self.switch_view(false);
                    }
                    if widgets::tab(
                        ui,
                        t(lang, "gallery"),
                        self.show_gallery,
                        Some(Icon::Gallery),
                    )
                    .clicked()
                    {
                        self.switch_view(true);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_square(ui, Icon::Settings, t(lang, "settings"), 34.0)
                            .clicked()
                        {
                            self.show_settings = true;
                        }
                        let theme_switch =
                            widgets::theme_switch(ui, self.settings.dark_theme, lang);
                        if theme_switch.clicked() {
                            self.toggle_theme(ctx, theme_switch.rect.center());
                        }
                        if widgets::language_picker(ui, &mut self.lang) {
                            self.settings.lang = self.lang;
                            self.queue_settings_save();
                            cutgut::i18n::set_lang(self.lang.code());
                            self.request_plan();
                        }
                        ui.add_space(5.0);
                        if self.show_gallery {
                            if widgets::icon_button(
                                ui,
                                Icon::Upload,
                                t(lang, "import_clips"),
                                BtnKind::Secondary,
                                0.0,
                            )
                            .clicked()
                            {
                                self.gallery.import_dialog(lang);
                            }
                        } else if self.video.is_some() {
                            let mut changed = false;
                            if widgets::icon_button(
                                ui,
                                Icon::Folder,
                                t(lang, "change"),
                                BtnKind::Secondary,
                                0.0,
                            )
                            .clicked()
                            {
                                self.open_dialog();
                                changed = true;
                            }
                            if !changed
                                && widgets::icon_button(
                                    ui,
                                    Icon::Close,
                                    t(lang, "unload"),
                                    BtnKind::Ghost,
                                    0.0,
                                )
                                .clicked()
                            {
                                self.unload_video();
                            }
                        }
                    });
                });
                let rect = ui.min_rect().expand2(egui::vec2(20.0, 10.0));
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom(),
                    Stroke::new(1.0_f32, palette.line),
                );
            });
    }

    fn toggle_theme(&mut self, ctx: &egui::Context, origin: egui::Pos2) {
        if self.pending_theme.take().is_some() {
            return;
        }
        let capture = egui::UserData::new("theme_reveal");
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(capture.clone()));
        self.pending_theme = Some(PendingTheme {
            capture,
            origin,
            requested: Instant::now(),
        });
        ctx.request_repaint();
    }

    fn poll_theme(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending_theme else {
            return;
        };
        let viewport = ctx.viewport_id();
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot {
                    viewport_id,
                    user_data,
                    image,
                } if *viewport_id == viewport && user_data == &pending.capture => {
                    Some(image.clone())
                }
                _ => None,
            })
        });
        let elapsed = pending.requested.elapsed();
        let timeout = Duration::from_millis(160);
        if image.is_none() && elapsed < timeout {
            ctx.request_repaint_after(timeout - elapsed);
            return;
        }
        let texture_limit = ctx.input(|input| input.max_texture_side);
        self.theme_reveal = image
            .filter(|image| image.size.iter().all(|&length| length <= texture_limit))
            .map(|image| theme::Reveal::new(ctx, image, pending.origin));
        self.pending_theme = None;
        self.settings.dark_theme = !self.settings.dark_theme;
        theme::configure(
            ctx,
            self.settings.dark_theme,
            !self.settings.background_theme.is_empty(),
        );
        self.queue_settings_save();
        ctx.request_repaint();
    }

    fn editor_ui(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let view_height = ui.available_height();
        egui::ScrollArea::vertical()
            .scroll_source(
                if self.show_settings
                    || self.show_keybinds
                    || matches!(self.cleanup, Some(Cleanup::Ask(_)))
                {
                    egui::scroll_area::ScrollSource::NONE
                } else {
                    egui::scroll_area::ScrollSource::ALL
                },
            )
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let gap = 5.0;
                let width = ui.available_width();
                ui.spacing_mut().item_spacing.x = gap;
                // Player + timeline take the full window width.
                theme::surface(ui)
                    .corner_radius(0)
                    .inner_margin(5)
                    .show(ui, |ui| {
                        if let Some(video) = &self.video {
                            let file =
                                egui::Label::new(RichText::new(&video.name).size(16.0).strong())
                                    .truncate();
                            let (width, height) = video.info.display_size();
                            ui.add(file).on_hover_text(format!(
                                "{}
{} × {} · {:.0} FPS",
                                video.path, width, height, video.info.fps
                            ));
                            self.preview_ui(ui, (view_height - 205.0).max(180.0));
                            self.timeline_ui(ui);
                            self.controls_ui(ui);
                        } else if self.pending_video.is_some() {
                            ui.add_space(5.0);
                            ui.vertical_centered(|ui| {
                                ui.spinner();
                                ui.label(t(lang, "loading"));
                            });
                            ui.add_space(5.0);
                        } else {
                            ui.vertical_centered(|ui| {
                                if widgets::upload_zone(
                                    ui,
                                    t(lang, "open"),
                                    t(lang, "open_hint"),
                                    ui.available_width(),
                                    (view_height * 0.55).clamp(215.0, 540.0),
                                )
                                .clicked()
                                {
                                    self.open_dialog();
                                }
                            });
                        }
                    });

                // Export and quality sit below the player, so the editor needs a
                // short scroll to reach them.
                ui.add_space(gap);
                let column = ((width - gap) / 2.0).max(300.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(column, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.export_card(ui);
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(column, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.quality_card(ui);
                        },
                    );
                });
                ui.add_space(gap);
            });
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui, height: f32) {
        let palette = Palette::get(ui);
        let aspect = self
            .video
            .as_ref()
            .map(|v| {
                let (width, height) = v.info.display_size();
                width as f32 / height.max(1) as f32
            })
            .unwrap_or(16.0 / 9.0);

        let full_width = ui.available_width();
        let height = height.min(full_width / aspect);
        let image_size = egui::vec2(
            full_width.min(height * aspect),
            height.min(full_width / aspect),
        );
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(full_width, height), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0, Color32::from_rgb(13, 15, 17));

        let image_rect = Rect::from_center_size(rect.center(), image_size);
        if let Some(texture) = &self.preview_tex {
            theme::rounded_image(
                &painter,
                image_rect,
                CornerRadius::ZERO,
                texture.id(),
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            );
        } else {
            painter.text(
                image_rect.center(),
                Align2::CENTER_CENTER,
                t(
                    self.lang,
                    if self.still.pending {
                        "loading"
                    } else {
                        "media_missing"
                    },
                ),
                FontId::proportional(13.0),
                Color32::from_gray(177),
            );
        }

        // Framing overlay (9:16 / 1:1 / 16:9)
        if let Some(video) = &self.video {
            if let Some(crop) = self.crop_box() {
                let (frame_w, frame_h) = video.info.display_size();
                let width = frame_w.max(1) as f32;
                let height = frame_h.max(1) as f32;
                let left = image_rect.left() + crop.x as f32 / width * image_rect.width();
                let top = image_rect.top() + crop.y as f32 / height * image_rect.height();
                let right = left + crop.w as f32 / width * image_rect.width();
                let bottom = top + crop.h as f32 / height * image_rect.height();
                let crop_rect =
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom));
                let mask = Color32::from_black_alpha(150);
                painter.rect_filled(
                    Rect::from_min_max(
                        image_rect.left_top(),
                        egui::pos2(crop_rect.left(), image_rect.bottom()),
                    ),
                    CornerRadius::same(0),
                    mask,
                );
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(crop_rect.right(), image_rect.top()),
                        image_rect.right_bottom(),
                    ),
                    CornerRadius::same(0),
                    mask,
                );
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(crop_rect.left(), image_rect.top()),
                        egui::pos2(crop_rect.right(), crop_rect.top()),
                    ),
                    CornerRadius::same(0),
                    mask,
                );
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(crop_rect.left(), crop_rect.bottom()),
                        egui::pos2(crop_rect.right(), image_rect.bottom()),
                    ),
                    CornerRadius::same(0),
                    mask,
                );
                painter.rect_stroke(
                    crop_rect,
                    CornerRadius::same(0),
                    Stroke::new(1.5_f32, palette.accent),
                    egui::StrokeKind::Inside,
                );
                let third = Color32::from_white_alpha(50);
                for i in 1..3 {
                    let x = crop_rect.left() + crop_rect.width() * i as f32 / 3.0;
                    let y = crop_rect.top() + crop_rect.height() * i as f32 / 3.0;
                    painter.line_segment(
                        [
                            egui::pos2(x, crop_rect.top()),
                            egui::pos2(x, crop_rect.bottom()),
                        ],
                        Stroke::new(1.0_f32, third),
                    );
                    painter.line_segment(
                        [
                            egui::pos2(crop_rect.left(), y),
                            egui::pos2(crop_rect.right(), y),
                        ],
                        Stroke::new(1.0_f32, third),
                    );
                }
            }
        }

        // Play/pause feedback: a short (||) after pausing and (>) after
        // resuming; nothing at all while the clip is running.
        if self.settings.pause_overlay && self.video.is_some() {
            if let Some((paused, created)) = self.overlay {
                const HOLD: f32 = 0.40;
                const FADE: f32 = 0.32;
                let elapsed = created.elapsed().as_secs_f32();
                if elapsed > HOLD + FADE {
                    self.overlay = None;
                } else {
                    let alpha = if elapsed <= HOLD {
                        1.0
                    } else {
                        1.0 - (elapsed - HOLD) / FADE
                    };
                    let radius = (height * 0.09).clamp(24.0, 44.0);
                    let center = image_rect.center();
                    painter.circle_filled(
                        center,
                        radius,
                        Color32::from_black_alpha((150.0 * alpha) as u8),
                    );
                    painter.circle_stroke(
                        center,
                        radius,
                        Stroke::new(1.0_f32, Color32::from_white_alpha((45.0 * alpha) as u8)),
                    );
                    let icon = if paused { Icon::Pause } else { Icon::Play };
                    let icon_color = Color32::from_white_alpha((238.0 * alpha) as u8);
                    if paused {
                        let bar_w = radius * 0.20;
                        let bar_h = radius * 0.86;
                        let gap = radius * 0.16;
                        let left = Rect::from_center_size(
                            center - Vec2::new(gap + bar_w * 0.5, 0.0),
                            Vec2::new(bar_w, bar_h),
                        );
                        let right = Rect::from_center_size(
                            center + Vec2::new(gap + bar_w * 0.5, 0.0),
                            Vec2::new(bar_w, bar_h),
                        );
                        painter.rect_filled(left, CornerRadius::same(2), icon_color);
                        painter.rect_filled(right, CornerRadius::same(2), icon_color);
                    } else {
                        icons::paint(
                            &painter,
                            icon,
                            Rect::from_center_size(center, Vec2::splat(radius * 0.9)),
                            icon_color,
                            2.0,
                        );
                    }
                    ui.ctx().request_repaint();
                }
            }
        }

        // Speed badge: slides down from the top edge of the picture and stays
        // while the picture is held down, then fades. Half transparent, so the
        // footage underneath still reads.
        if let Some((rate, since)) = self.rate_osd {
            const SLIDE: f32 = 0.15;
            const HOLD: f32 = 0.85;
            const FADE: f32 = 0.35;
            let elapsed = since.elapsed().as_secs_f32();
            if !self.rate_osd_held && elapsed > HOLD + FADE {
                self.rate_osd = None;
            } else {
                let slide = (elapsed / SLIDE).clamp(0.0, 1.0).powf(0.65);
                let alpha = if self.rate_osd_held || elapsed <= HOLD {
                    1.0
                } else {
                    (1.0 - (elapsed - HOLD) / FADE).clamp(0.0, 1.0)
                };
                let galley = painter.layout_no_wrap(
                    rate.label().to_owned(),
                    FontId::proportional(28.0),
                    Color32::from_white_alpha((240.0 * alpha) as u8),
                );
                let size = galley.size() + egui::vec2(34.0, 14.0);
                let center = egui::pos2(
                    image_rect.center().x,
                    image_rect.top() + 10.0 + size.y * 0.5 - (1.0 - slide) * (size.y + 14.0),
                );
                let pill = Rect::from_center_size(center, size);
                let cut = Rect::intersect(pill, image_rect);
                if cut.height() > 1.0 {
                    painter.rect_filled(pill, 10, Color32::from_black_alpha((165.0 * alpha) as u8));
                    painter.rect_stroke(
                        pill,
                        10,
                        Stroke::new(1.0_f32, Color32::from_white_alpha((34.0 * alpha) as u8)),
                        egui::StrokeKind::Inside,
                    );
                    painter.galley(pill.center() - galley.size() * 0.5, galley, Color32::WHITE);
                }
                ui.ctx().request_repaint();
            }
        }

        use widgets::PreviewAction;
        let enabled = self.video.is_some()
            && !self.show_settings
            && !self.show_keybinds
            && !matches!(self.cleanup, Some(Cleanup::Ask(_)));
        match self
            .preview_press
            .update(&response, Some(image_rect), enabled)
        {
            PreviewAction::Drag => {
                if let Some(video) = &self.video {
                    crate::drag_out::drag_file_out(Path::new(&video.path));
                }
            }
            PreviewAction::Click => self.toggle_play(self.playback.unwrap_or(Playback::Full)),
            PreviewAction::Hold(rate) => {
                self.change_preview_rate(rate);
                self.rate_osd = Some((rate, Instant::now()));
                self.rate_osd_held = true;
                if self.playback.is_none() {
                    self.toggle_play(Playback::Full);
                }
            }
            PreviewAction::Release => {
                self.rate_osd_held = false;
                let shown = self.rate_osd.map(|(rate, _)| rate);
                self.change_preview_rate(PlaybackRate::Normal);
                // Let the badge fade out instead of vanishing the moment the
                // button comes up.
                if let Some(rate) = shown {
                    self.rate_osd = Some((rate, Instant::now()));
                }
            }
            PreviewAction::None => {}
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                response.enabled(),
                t(
                    self.lang,
                    if self.playback == Some(Playback::Full) {
                        "pause"
                    } else {
                        "play"
                    },
                ),
            )
        });
        response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    /// Ruler band and filmstrip. Split out so a test can prove the frame-step and
    /// zoom buttons sit outside the scrubbing area: an interaction registered later
    /// on top of them swallows their clicks.
    fn timeline_geometry(rect: Rect) -> (Rect, Rect) {
        let ruler = Rect::from_min_max(
            egui::pos2(rect.left() + 7.0, rect.top() + 3.0),
            egui::pos2(rect.right() - 7.0, rect.top() + 26.0),
        );
        let strip = Rect::from_min_max(
            egui::pos2(ruler.left(), ruler.bottom() + 3.0),
            egui::pos2(ruler.right(), rect.bottom() - 6.0),
        );
        (ruler, strip)
    }

    /// Hit rects in the ruler: frame back, frame forward, zoom out, zoom in.
    fn ruler_buttons(ruler: Rect) -> [Rect; 4] {
        let y = ruler.center().y;
        let size = Vec2::splat(20.0);
        [
            Rect::from_center_size(egui::pos2(ruler.left() + 11.0, y), size),
            Rect::from_center_size(egui::pos2(ruler.left() + 33.0, y), size),
            Rect::from_center_size(egui::pos2(ruler.right() - 62.0, y), size),
            Rect::from_center_size(egui::pos2(ruler.right() - 40.0, y), size),
        ]
    }

    /// The part of the clip the timeline shows: a zoomed span and its left edge.
    fn timeline_view(&self, duration: f64) -> (f64, f64) {
        timeline_window(self.zoom, self.view_start, duration)
    }

    /// Zooms the timeline while keeping `anchor` (a clip time) where it is on
    /// screen, which is what makes wheel-zoom feel like it targets the pointer.
    fn zoom_timeline(&mut self, factor: f64, anchor: f64, duration: f64) {
        let (zoom, start) = zoomed_window(self.zoom, self.view_start, duration, factor, anchor);
        self.zoom = zoom;
        self.view_start = start;
    }

    fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        let palette = Palette::get(ui);
        let duration = self.duration();
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 108.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 7, palette.well);

        // Fixed geometry: a ruler band above the filmstrip.
        let (ruler, strip) = Self::timeline_geometry(rect);
        let strip_width = strip.width().max(1.0);

        // Wheel or pinch zooms around the pointer instead of the view centre.
        if ui.rect_contains_pointer(strip) {
            let (zoom, pointer) = ui.input(|input| (input.zoom_delta(), input.pointer.hover_pos()));
            if (zoom - 1.0).abs() > 0.001 {
                let (start, span) = self.timeline_view(duration);
                let fraction = pointer.map_or(0.5, |p| {
                    f64::from(((p.x - strip.left()) / strip_width).clamp(0.0, 1.0))
                });
                self.zoom_timeline(f64::from(zoom), start + fraction * span, duration);
            }
        }

        // A frame step outside a zoomed window must not hide the playhead.
        let (mut view_start, mut view_span) = self.timeline_view(duration);
        if self.position < view_start || self.position > view_start + view_span {
            self.view_start =
                (self.position - view_span * 0.5).clamp(0.0, (duration - view_span).max(0.0));
            (view_start, view_span) = self.timeline_view(duration);
        }

        // --- ruler buttons: frame steps on the left, zoom on the right. They are
        // the discoverable version of `,` `.` `+` `-`, and they work with a mouse.
        let lang = self.lang;
        let button = |center: egui::Pos2, label: Option<Icon>, text: &str, tip: &str| -> bool {
            let hit = Rect::from_center_size(center, Vec2::splat(19.0));
            let response = ui.interact(hit, ui.id().with(tip), Sense::click());
            let base = if response.hovered() {
                palette.raised
            } else {
                palette.raised.gamma_multiply(0.75)
            };
            painter.rect_filled(hit, 4, base);
            let ink = if response.hovered() {
                palette.text
            } else {
                palette.muted
            };
            match label {
                Some(icon) => icons::paint(&painter, icon, hit.shrink(5.0), ink, 1.5),
                None => {
                    painter.text(
                        hit.center(),
                        Align2::CENTER_CENTER,
                        text,
                        FontId::proportional(12.0),
                        ink,
                    );
                }
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            response.on_hover_text(t(lang, tip)).clicked()
        };
        let [back_hit, forward_hit, zoom_out_hit, zoom_in_hit] = Self::ruler_buttons(ruler);
        let step_back = button(back_hit.center(), Some(Icon::Previous), "", "kb_frame_back");
        let step_forward = button(
            forward_hit.center(),
            Some(Icon::Next),
            "",
            "kb_frame_forward",
        );
        let zoom_out = button(zoom_out_hit.center(), None, "-", "kb_zoom_out");
        let zoom_in = button(zoom_in_hit.center(), None, "+", "kb_zoom_in");
        if step_back {
            self.step_frames(-1);
        }
        if step_forward {
            self.step_frames(1);
        }
        let zoom_by = match (zoom_out, zoom_in) {
            (true, _) => 1.0 / 1.6,
            (_, true) => 1.6,
            _ => 1.0_f64,
        };
        if (zoom_by - 1.0).abs() > 0.001 {
            self.zoom_timeline(zoom_by, self.position, duration);
            (view_start, view_span) = self.timeline_view(duration);
        }

        let x_for = |time: f64| {
            strip.left() + (((time - view_start) / view_span).clamp(0.0, 1.0) as f32) * strip_width
        };
        let time_at = |x: f32| {
            view_start + f64::from(((x - strip.left()) / strip_width).clamp(0.0, 1.0)) * view_span
        };

        // --- ruler: ticks on a step that still leaves room for its label
        let step = tick_step(f64::from(strip_width) / view_span);
        let mut tick = (view_start / step).ceil() * step;
        while tick <= view_start + view_span + 1e-9 {
            let x = x_for(tick);
            let index = (tick / step).round();
            let strong = (index as i64) % 5 == 0;
            painter.line_segment(
                [
                    egui::pos2(x, ruler.bottom() - if strong { 7.0 } else { 4.0 }),
                    egui::pos2(x, ruler.bottom()),
                ],
                Stroke::new(
                    1.0_f32,
                    palette.muted.gamma_multiply(if strong { 0.9 } else { 0.5 }),
                ),
            );
            let label = tick_label(tick, step);
            let width = painter
                .layout_no_wrap(label.clone(), FontId::proportional(9.5), palette.muted)
                .size()
                .x;
            // Skip labels that would collide with the buttons or the zoom badge.
            let free = if self.zoom > 1.01 { 96.0 } else { 72.0 };
            if x + width + free < ruler.right() && x > ruler.left() + 46.0 {
                painter.text(
                    egui::pos2(x + 3.0, ruler.top()),
                    Align2::LEFT_TOP,
                    label,
                    FontId::proportional(9.5),
                    palette.muted,
                );
            }
            painter.line_segment(
                [egui::pos2(x, strip.top()), egui::pos2(x, strip.bottom())],
                Stroke::new(
                    1.0_f32,
                    Color32::from_white_alpha(if strong { 24 } else { 10 }),
                ),
            );
            tick += step;
        }

        // --- filmstrip from the scrub cache: real frames at any zoom level
        let ([cell_w, cell_h], cols) = self.scrub.atlas_layout();
        let atlas = self
            .scrub
            .atlas(ui.ctx())
            .map(|texture| (texture.id(), texture.size_vec2()));
        if let Some((texture, atlas_size)) = atlas {
            // One tile per ~76 px: at full zoom a frame per cache tick would be a
            // sliver a few pixels wide, which reads as noise rather than footage.
            let mut tiles = ((strip_width / 76.0).floor() as usize).max(1);
            // Zoomed in past the cache rate, fewer tiles than the strip can hold
            // beats repeating the same frame to fill the row.
            let cached = (view_span / self.scrub.interval).ceil().max(1.0) as usize;
            tiles = tiles.min(cached.max(1));
            let slice = view_span / tiles as f64;
            for tile in 0..tiles {
                let from = view_start + tile as f64 * slice;
                let left = x_for(from);
                let right = x_for(from + slice).max(left + 1.0);
                let cell = Rect::from_min_max(
                    egui::pos2(left, strip.top()),
                    egui::pos2(right, strip.bottom()),
                );
                // A tile waits for the frame that belongs to it. Borrowing the
                // nearest decoded one made the strip shuffle itself while the
                // clip was still loading.
                let index = self
                    .scrub
                    .nearest_index_within(from + slice * 0.5, self.scrub.interval * 1.2);
                let Some(index) = index else {
                    painter.rect_filled(cell, 0, palette.raised.gamma_multiply(0.35));
                    continue;
                };
                let (col, row) = (index % cols, index / cols);
                let uv = Rect::from_min_max(
                    egui::pos2(
                        (col * cell_w) as f32 / atlas_size.x,
                        (row * cell_h) as f32 / atlas_size.y,
                    ),
                    egui::pos2(
                        ((col + 1) * cell_w) as f32 / atlas_size.x,
                        ((row + 1) * cell_h) as f32 / atlas_size.y,
                    ),
                );
                painter.image(texture, cell, uv, Color32::WHITE);
            }
        }
        // How far the decode pass has got; gone once it is done. The number
        // matters: tiles arriving one by one look like a bug otherwise.
        if !self.scrub.finished() {
            let fraction = self.scrub.fill_fraction().clamp(0.0, 1.0);
            if let Some(last) = self.scrub.last_time() {
                let x = x_for(last + self.scrub.interval);
                painter.line_segment(
                    [egui::pos2(x, strip.top()), egui::pos2(x, strip.bottom())],
                    Stroke::new(1.5_f32, palette.accent.gamma_multiply(0.45)),
                );
            }
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(strip.left(), strip.top()),
                    egui::vec2(strip_width * fraction, 2.0),
                ),
                0,
                palette.accent.gamma_multiply(0.5),
            );
            let text =
                t(self.lang, "loading_frames").replace("{}", &format!("{:.0}", fraction * 100.0));
            let galley = painter.layout_no_wrap(text, FontId::proportional(10.0), palette.text);
            let tag = Rect::from_min_size(
                egui::pos2(
                    strip.right() - galley.size().x - 14.0,
                    strip.bottom() - galley.size().y - 5.0,
                ),
                galley.size() + egui::vec2(8.0, 2.0),
            );
            painter.rect_filled(tag, 4, Color32::from_black_alpha(150));
            painter.galley(tag.min + egui::vec2(4.0, 1.0), galley, palette.text);
        }

        let x_in = x_for(self.in_s);
        let x_out = x_for(self.out_s);
        for (start, end) in [(strip.left(), x_in), (x_out, strip.right())] {
            painter.rect_filled(
                Rect::from_min_max(
                    egui::pos2(start, strip.top()),
                    egui::pos2(end, strip.bottom()),
                ),
                0,
                Color32::from_black_alpha(122),
            );
        }
        let x = x_for(self.position);
        painter.line_segment(
            [egui::pos2(x, ruler.top()), egui::pos2(x, strip.bottom())],
            Stroke::new(1.6_f32, theme::GOLD),
        );
        painter.circle_filled(egui::pos2(x, ruler.top() + 2.0), 3.4, theme::GOLD);

        let handle = |x: f32, mode| {
            let bottom = strip.bottom()
                - if mode == DragMode::In && (x_out - x_in).abs() < 26.0 {
                    21.0
                } else {
                    1.0
                };
            let center = x.clamp(rect.left() + 12.0, rect.right() - 12.0);
            Rect::from_center_size(egui::pos2(center, bottom - 10.0), Vec2::new(22.0, 20.0))
        };
        let in_handle = handle(x_in, DragMode::In);
        let out_handle = handle(x_out, DragMode::Out);
        for (x, rect, color, icon) in [
            (x_in, in_handle, palette.good, Icon::MarkIn),
            (x_out, out_handle, palette.danger, Icon::MarkOut),
        ] {
            painter.vline(x, strip.y_range(), Stroke::new(1.7_f32, color));
            painter.rect_filled(rect.shrink2(egui::vec2(2.0, 1.0)), 4, color);
            icons::paint(
                &painter,
                icon,
                Rect::from_center_size(rect.center(), Vec2::splat(14.0)),
                if palette.dark {
                    Color32::from_rgb(18, 19, 20)
                } else {
                    Color32::WHITE
                },
                1.35,
            );
        }

        // Zoom badge; clicking it goes back to the whole clip.
        if self.zoom > 1.01 {
            let text = format!("×{:.1}", self.zoom);
            let galley = painter.layout_no_wrap(text, FontId::proportional(9.5), palette.muted);
            let badge = Rect::from_min_size(
                egui::pos2(ruler.right() - galley.size().x - 8.0, ruler.top()),
                galley.size() + egui::vec2(8.0, 2.0),
            );
            painter.rect_filled(badge, 3, palette.raised);
            painter.galley(badge.min + egui::vec2(4.0, 1.0), galley, palette.text);
            if ui
                .interact(badge, ui.id().with("zoom-reset"), Sense::click())
                .on_hover_text(t(self.lang, "zoom_reset"))
                .clicked()
            {
                self.zoom = 1.0;
                self.view_start = 0.0;
            }
        }

        let mode_at = |p| {
            if in_handle.contains(p) {
                DragMode::In
            } else if out_handle.contains(p) {
                DragMode::Out
            } else {
                DragMode::Playhead
            }
        };
        // Only the filmstrip scrubs: the ruler band belongs to the frame-step
        // and zoom buttons, and a full-rect interaction here would swallow their
        // clicks (they are registered first, so the drag area wins).
        let seek = ui.interact(strip, ui.id().with("seek"), Sense::click_and_drag());
        if seek.hovered() || seek.dragged() {
            let mode = if seek.dragged() {
                self.drag_mode
            } else {
                seek.hover_pos().map_or(DragMode::Playhead, mode_at)
            };
            ui.ctx()
                .set_cursor_icon(if matches!(mode, DragMode::In | DragMode::Out) {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::PointingHand
                });
        }

        let origin = ui.input(|input| input.pointer.press_origin());
        // A plain click jumps straight to the clicked moment and keeps playing
        // (one decoder restart). Dragging pauses so the scrubbed frames can be
        // inspected, and resumes on release.
        let mut clicked = false;
        if seek.clicked() {
            if let Some(pointer) = seek.interact_pointer_pos() {
                if mode_at(pointer) == DragMode::Playhead {
                    clicked = true;
                    self.drag_mode = DragMode::None;
                    self.seek_resume = None;
                    let time = self.snap(time_at(pointer.x));
                    self.position = time;
                    match self.playback {
                        Some(mode) => {
                            let (_, end) = mode.range(self.duration(), self.in_s, self.out_s);
                            self.start_playback(mode, time.min(end), end);
                        }
                        None => self.request_frame(true),
                    }
                }
            }
        }
        if !clicked && seek.drag_started() {
            self.drag_mode = origin.map_or(DragMode::Playhead, mode_at);
            self.marker_grab_offset = match self.drag_mode {
                DragMode::In => origin.map_or(0.0, |p| p.x - x_in),
                DragMode::Out => origin.map_or(0.0, |p| p.x - x_out),
                _ => 0.0,
            };
            if self.drag_mode == DragMode::Playhead {
                self.seek_resume = self.playback;
                self.pause_playback();
            }
        }
        if !clicked && (seek.dragged() || seek.drag_stopped()) {
            if let Some(pointer) = seek.interact_pointer_pos() {
                // Preserve the distance from the marker when grabbing an off-center handle.
                let grab_offset = match self.drag_mode {
                    DragMode::In | DragMode::Out => self.marker_grab_offset,
                    _ => 0.0,
                };
                let time = time_at(pointer.x - grab_offset);
                match self.drag_mode {
                    DragMode::Playhead => {
                        self.position = self.snap(time);
                        // Live scrub feedback: the frame is already in memory, so
                        // following the playhead costs nothing.
                        self.request_frame(seek.drag_stopped());
                    }
                    DragMode::In => {
                        let time = self.snap(time);
                        self.in_s = time.clamp(0.0, (self.out_s - 0.05).max(0.0));
                        self.trim_changed();
                    }
                    DragMode::Out => {
                        let time = self.snap(time);
                        self.out_s = time.clamp((self.in_s + 0.05).min(duration), duration);
                        self.trim_changed();
                    }
                    DragMode::None => {}
                }
            }
            if seek.drag_stopped() {
                self.drag_mode = DragMode::None;
                if let Some(mode) = self.seek_resume.take() {
                    let (_, end) = mode.range(self.duration(), self.in_s, self.out_s);
                    if self.position < end - 0.05 {
                        self.start_playback(mode, self.position, end);
                    }
                } else {
                    self.request_frame(true);
                }
            }
        }
    }

    fn change_preview_rate(&mut self, rate: PlaybackRate) {
        if std::mem::replace(&mut self.playback_rate, rate) == rate {
            return;
        }
        // Whatever changed the speed, say it out loud: 2x is invisible otherwise.
        if rate == PlaybackRate::Normal {
            if !self.rate_osd_held {
                self.rate_osd = None;
            }
        } else {
            self.rate_osd = Some((rate, Instant::now()));
        }
        // A running preview is only re-paced: restarting the decoders here is
        // what used to freeze the picture (and the sound) for half a second.
        if self.playback.is_some() {
            self.player.set_rate(rate);
        }
    }
    fn controls_ui(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let full = self.playback == Some(Playback::Full);
        let trim = self.playback == Some(Playback::Trim);
        let preview = t(lang, if trim { "pause_trim" } else { "play_trim" });
        let preview_width = ui
            .painter()
            .layout_no_wrap(
                preview.to_owned(),
                FontId::proportional(13.0),
                Palette::get(ui).text,
            )
            .size()
            .x
            + 45.0;
        let (row, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
        // Three fixed groups can only overlap, never reflow, so a narrow window
        // (or a zoomed UI) has to switch to tighter furniture. Each group is also
        // clipped to its own rect: overflow is cut instead of painted on top of
        // its neighbour.
        let compact = row.width() < 1090.0;
        let volume_width = if compact { 92.0 } else { 128.0 };
        let middle = Rect::from_center_size(
            row.center(),
            Vec2::new(
                preview_width + 2.0 * (128.0 + ui.spacing().item_spacing.x),
                row.height(),
            ),
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(row.with_max_x(middle.left()))
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.set_clip_rect(ui.max_rect());
                widgets::borderless_controls(ui);
                if widgets::icon_square(
                    ui,
                    if full { Icon::Pause } else { Icon::Play },
                    t(lang, if full { "pause" } else { "play_full" }),
                    31.0,
                )
                .clicked()
                {
                    self.toggle_play(Playback::Full);
                }
                widgets::playback_time(
                    ui,
                    format_time(self.position),
                    format_time(self.duration()),
                    Palette::get(ui).text,
                );
                let mut rate = self.playback_rate;
                if widgets::playback_speed(ui, &mut rate, lang) {
                    self.change_preview_rate(rate);
                }
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(middle)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.set_clip_rect(middle);
                widgets::borderless_controls(ui);
                if widgets::trim_point(
                    ui,
                    Icon::MarkIn,
                    "IN",
                    &format_time(self.in_s),
                    t(lang, "mark_in"),
                )
                .clicked()
                {
                    self.set_in_point();
                }
                if widgets::icon_button(
                    ui,
                    if trim { Icon::Pause } else { Icon::Scissors },
                    preview,
                    BtnKind::Secondary,
                    preview_width,
                )
                .clicked()
                {
                    self.toggle_play(Playback::Trim);
                }
                if widgets::trim_point(
                    ui,
                    Icon::MarkOut,
                    "OUT",
                    &format_time(self.out_s),
                    t(lang, "mark_out"),
                )
                .clicked()
                {
                    self.set_out_point();
                }
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(row.with_min_x(middle.right()))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.set_clip_rect(row.with_min_x(middle.right()));
                widgets::borderless_controls(ui);
                let mut volume = None;
                ui.allocate_ui_with_layout(
                    Vec2::new(volume_width, 28.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        volume = self
                            .volume
                            .show(ui, lang, if compact { 30.0 } else { 54.0 });
                    },
                );
                if let Some(volume) = volume {
                    self.player.set_volume(volume);
                    self.save_volume(volume);
                }
                if widgets::icon_button(
                    ui,
                    Icon::Loop,
                    t(lang, "loop"),
                    if self.loop_play {
                        BtnKind::Secondary
                    } else {
                        BtnKind::Ghost
                    },
                    0.0,
                )
                .clicked()
                {
                    self.loop_play = !self.loop_play;
                }
                if widgets::icon_square(ui, Icon::Camera, t(lang, "snapshot"), 34.0).clicked() {
                    self.save_png();
                }
            },
        );
    }

    fn export_card(&mut self, ui: &mut egui::Ui) {
        let palette = Palette::get(ui);
        theme::surface(ui).corner_radius(0).show(ui, |ui| {
            widgets::borderless_controls(ui);
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::heading(ui, Icon::Upload, t(self.lang, "export"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_square(ui, Icon::Folder, &self.settings.output_dir, 28.0)
                        .clicked()
                    {
                        let _ = std::process::Command::new("explorer")
                            .arg(&self.settings.output_dir)
                            .spawn();
                    }
                });
            });
            ui.add_space(5.0);

            let mut changed = false;
            let field_width = ((ui.available_width() - 5.0) / 2.0).max(95.0);
            egui::Grid::new("export_grid")
                .num_columns(2)
                .spacing(Vec2::new(5.0, 5.0))
                .show(ui, |ui| {
                    changed |= self.profile_field(ui, field_width);
                    changed |= self.crop_field(ui, field_width);
                    ui.end_row();
                    changed |= self.limit_field(ui, field_width);
                    changed |= self.codec_field(ui, field_width);
                    ui.end_row();
                    changed |= self.engine_field(ui, field_width);
                    changed |= self.format_field(ui, field_width);
                    ui.end_row();
                });

            ui.add_space(5.0);
            ui.vertical(|ui| {
                let enabled = self.video.is_some()
                    && self.encoding.is_none()
                    && self.pending_video.is_none()
                    && self.cleanup.is_none();
                let mut export_clicked = false;
                ui.add_enabled_ui(enabled, |ui| {
                    if widgets::icon_button(
                        ui,
                        Icon::Scissors,
                        t(self.lang, "export_btn"),
                        BtnKind::Primary,
                        ui.available_width(),
                    )
                    .clicked()
                    {
                        export_clicked = true;
                    }
                });
                if export_clicked {
                    self.start_export();
                }
                if let Some(output) = &self.last_output {
                    if widgets::icon_button(
                        ui,
                        Icon::Folder,
                        t(self.lang, "open_output"),
                        BtnKind::Secondary,
                        170.0,
                    )
                    .clicked()
                    {
                        if let Some(parent) = output.parent() {
                            let _ = std::process::Command::new("explorer").arg(parent).spawn();
                        }
                    }
                }
            });

            ui.add_space(5.0);
            if let Some(state) = &self.encoding {
                ui.add(
                    egui::ProgressBar::new((self.progress / 100.0).clamp(0.0, 1.0))
                        .fill(palette.accent_tint)
                        .desired_width(ui.available_width())
                        .show_percentage(),
                );
                let mut cancel = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.stage).size(10.5).color(palette.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(
                            ui,
                            Icon::Close,
                            t(self.lang, "cancel"),
                            BtnKind::Danger,
                            96.0,
                        )
                        .clicked()
                        {
                            cancel = true;
                        }
                    });
                });
                if cancel {
                    state.cancel.cancel();
                }
            } else {
                ui.label(RichText::new(&self.status).size(10.5).color(palette.muted));
            }

            if changed {
                self.queue_settings_save();
                self.request_plan();
            }
        });
    }

    fn profile_field(&mut self, ui: &mut egui::Ui, width: f32) -> bool {
        let labels_pl = [
            "Discord Free (20 MB)",
            "Discord Nitro (50 MB)",
            "Shorts HQ (50 MB)",
            "Shorts Small (20 MB)",
            "Square Meme (20 MB)",
            "Własny",
        ];
        let labels_en = [
            "Discord Free (20 MB)",
            "Discord Nitro (50 MB)",
            "Shorts HQ (50 MB)",
            "Shorts Small (20 MB)",
            "Square Meme (20 MB)",
            "Custom",
        ];
        let labels = if self.lang == Lang::Pl {
            labels_pl
        } else {
            labels_en
        };
        let mut changed = false;
        ui.vertical(|ui| {
            field_label(ui, t(self.lang, "profile"));
            let current = labels[self.settings.export.profile.min(labels.len() - 1)];
            egui::ComboBox::from_id_salt("profile_combo")
                .selected_text(current)
                .width(width)
                .truncate()
                .show_ui(ui, |ui| {
                    widgets::animate_menu(ui);
                    for (index, label) in labels.iter().enumerate() {
                        changed |=
                            select_value(ui, &mut self.settings.export.profile, index, label);
                    }
                })
                .response
                .on_hover_text(current);
        });
        if changed {
            match self.settings.export.profile {
                0 => {
                    self.settings.export.crop = 0;
                    self.settings.export.limit = 1;
                }
                1 => {
                    self.settings.export.crop = 0;
                    self.settings.export.limit = 2;
                }
                2 => {
                    self.settings.export.crop = 1;
                    self.settings.export.limit = 2;
                }
                3 => {
                    self.settings.export.crop = 1;
                    self.settings.export.limit = 1;
                }
                4 => {
                    self.settings.export.crop = 2;
                    self.settings.export.limit = 1;
                }
                _ => {}
            }
        }
        changed
    }

    fn crop_field(&mut self, ui: &mut egui::Ui, width: f32) -> bool {
        let options = [
            t(self.lang, "original"),
            t(self.lang, "portrait"),
            t(self.lang, "square"),
            t(self.lang, "landscape"),
        ];
        let mut changed = false;
        ui.vertical(|ui| {
            field_label(ui, t(self.lang, "framing"));
            egui::ComboBox::from_id_salt("crop_combo")
                .selected_text(options[self.settings.export.crop])
                .width(width)
                .truncate()
                .show_ui(ui, |ui| {
                    widgets::animate_menu(ui);
                    for (index, label) in options.iter().enumerate() {
                        changed |= select_value(ui, &mut self.settings.export.crop, index, label);
                    }
                })
                .response
                .on_hover_text(options[self.settings.export.crop]);
        });
        changed
    }

    fn limit_field(&mut self, ui: &mut egui::Ui, width: f32) -> bool {
        let labels = [
            "10 MB".to_string(),
            "20 MB".to_string(),
            "50 MB".to_string(),
            "500 MB".to_string(),
            format!(
                "{} ({:.0} MB)",
                t(self.lang, "custom"),
                self.settings.export.custom_mb
            ),
        ];
        let mut changed = false;
        ui.vertical(|ui| {
            field_label(ui, t(self.lang, "limit"));
            egui::ComboBox::from_id_salt("limit_combo")
                .selected_text(&labels[self.settings.export.limit.min(labels.len() - 1)])
                .width(width)
                .truncate()
                .show_ui(ui, |ui| {
                    widgets::animate_menu(ui);
                    for (index, label) in labels.iter().enumerate() {
                        changed |= select_value(ui, &mut self.settings.export.limit, index, label);
                    }
                })
                .response
                .on_hover_text(&labels[self.settings.export.limit.min(labels.len() - 1)]);
            if self.settings.export.limit == 4 {
                if ui
                    .add(
                        egui::DragValue::new(&mut self.settings.export.custom_mb)
                            .range(1.0..=2000.0)
                            .speed(0.5)
                            .suffix(" MB"),
                    )
                    .changed()
                {
                    changed = true;
                }
            }
        });
        changed
    }

    fn codec_field(&mut self, ui: &mut egui::Ui, width: f32) -> bool {
        let options = ["H.264", "HEVC", "AV1"];
        let mut changed = false;
        ui.vertical(|ui| {
            field_label(ui, t(self.lang, "codec"));
            let selected = match self.settings.export.codec {
                VideoCodec::H264 => 0,
                VideoCodec::Hevc => 1,
                VideoCodec::Av1 => 2,
            };
            let mut value = selected;
            egui::ComboBox::from_id_salt("codec_combo")
                .selected_text(options[selected])
                .width(width)
                .truncate()
                .show_ui(ui, |ui| {
                    widgets::animate_menu(ui);
                    for (index, label) in options.iter().enumerate() {
                        select_value(ui, &mut value, index, label);
                    }
                })
                .response
                .on_hover_text(options[selected]);
            if value != selected {
                self.settings.export.codec = match value {
                    1 => VideoCodec::Hevc,
                    2 => VideoCodec::Av1,
                    _ => VideoCodec::H264,
                };
                changed = true;
            }
        });
        changed
    }

    fn engine_field(&mut self, ui: &mut egui::Ui, width: f32) -> bool {
        let options = [
            t(self.lang, "gpu_hq"),
            t(self.lang, "gpu_fast"),
            t(self.lang, "cpu_hq"),
            t(self.lang, "cpu_fast"),
        ];
        let mut changed = false;
        ui.vertical(|ui| {
            field_label(ui, t(self.lang, "engine"));
            egui::ComboBox::from_id_salt("engine_combo")
                .selected_text(options[self.settings.export.engine])
                .width(width)
                .truncate()
                .show_ui(ui, |ui| {
                    widgets::animate_menu(ui);
                    for (index, label) in options.iter().enumerate() {
                        changed |= select_value(ui, &mut self.settings.export.engine, index, label);
                    }
                })
                .response
                .on_hover_text(options[self.settings.export.engine]);
        });
        changed
    }

    fn format_field(&mut self, ui: &mut egui::Ui, width: f32) -> bool {
        let labels = [
            "MP4 (universal)",
            "MKV (low overhead)",
            "M4A (AAC)",
            "Opus (smallest)",
            "MP3",
        ];
        let mut changed = false;
        ui.vertical(|ui| {
            field_label(ui, t(self.lang, "format"));
            egui::ComboBox::from_id_salt("format_combo")
                .selected_text(labels[self.settings.export.format])
                .width(width)
                .truncate()
                .show_ui(ui, |ui| {
                    widgets::animate_menu(ui);
                    for (index, label) in labels.iter().enumerate() {
                        changed |= select_value(ui, &mut self.settings.export.format, index, label);
                    }
                })
                .response
                .on_hover_text(labels[self.settings.export.format]);
        });
        changed
    }

    fn quality_card(&mut self, ui: &mut egui::Ui) {
        let palette = Palette::get(ui);
        theme::surface(ui).corner_radius(0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.video.is_none()
                || self.is_audio_export()
                || self.plan_error.is_some()
                || self.plan.is_none()
            {
                ui.horizontal(|ui| {
                    widgets::heading(ui, Icon::Star, t(self.lang, "quality"));
                    if self.plan_rx.is_some() {
                        ui.spinner();
                    }
                });
            }

            if self.video.is_none() {
                ui.label(RichText::new(t(self.lang, "select_video")).color(palette.muted));
                return;
            }

            if self.is_audio_export() {
                if let Some(audio) = &self.audio_plan {
                    ui.label(
                        RichText::new(format!(
                            "{} · {} kb/s · {:.1} s",
                            audio.format.to_uppercase(),
                            audio.audio_kbps,
                            audio.duration_s
                        ))
                        .size(12.0)
                        .color(palette.text),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(if self.lang == Lang::Pl {
                            "Opus = najmniejszy plik. M4A = najlepsza zgodność."
                        } else {
                            "Opus = smallest file. M4A = best compatibility."
                        })
                        .size(11.0)
                        .color(palette.muted),
                    );
                } else {
                    ui.label(
                        RichText::new(t(self.lang, "analyzing"))
                            .size(11.0)
                            .color(palette.muted),
                    );
                }
                return;
            }

            if let Some(error) = &self.plan_error {
                ui.label(RichText::new(error).size(11.5).color(palette.danger));
                return;
            }

            let Some(plan) = &self.plan else {
                ui.label(
                    RichText::new(t(self.lang, "analyzing"))
                        .size(11.0)
                        .color(palette.muted),
                );
                return;
            };

            let raw = parse_hex_color(&plan.quality.color);
            let color = if palette.dark {
                raw
            } else {
                icons::lerp_color(raw, palette.text, 0.63)
            };
            ui.horizontal_wrapped(|ui| {
                widgets::heading(ui, Icon::Star, t(self.lang, "quality"));
                ui.label(
                    RichText::new(&plan.quality.label)
                        .size(12.0)
                        .strong()
                        .color(color),
                );
                if self.plan_rx.is_some() {
                    ui.spinner();
                }
            });
            let audio = if plan.has_audio {
                format!("AAC {} kb/s", plan.audio_kbps)
            } else {
                t(self.lang, "no_audio").to_owned()
            };
            let codec = if plan.is_remux {
                "Remux"
            } else {
                plan.codec.upper()
            };
            let density = if plan.is_remux {
                String::new()
            } else {
                format!(" · {:.4} bppf", plan.quality.bppf)
            };
            let metrics = [
                format!(
                    "{}×{} · {:.0} FPS",
                    plan.out_width, plan.out_height, plan.out_fps
                ),
                format!("{codec} · {} kb/s", plan.video_kbps),
                format!("{} MB · {audio}", self.target_mb()),
                format!("{}{density}", plan.encoder_name),
            ];
            egui::Grid::new("quality_metrics")
                .num_columns(2)
                .min_col_width((ui.available_width() - 8.0) * 0.46)
                .spacing([5.0, 3.0])
                .show(ui, |ui| {
                    for pair in metrics.chunks_exact(2) {
                        for value in pair {
                            ui.label(RichText::new(value).size(11.5).color(palette.text));
                        }
                        ui.end_row();
                    }
                });
            if plan.is_remux {
                ui.label(
                    RichText::new(&plan.quality.description)
                        .size(11.0)
                        .color(palette.muted),
                );
            }
            if let Some(reason) = plan.fallback_reason.as_ref().filter(|_| plan.is_fallback) {
                ui.label(RichText::new(reason).size(11.0).color(palette.accent));
            }
            // HDR either gets mapped to SDR (good) or cannot be (worth knowing
            // before the export, not after).
            if plan.bitrate_capped && !plan.is_remux {
                ui.label(
                    RichText::new(t(self.lang, "limit_capped"))
                        .size(10.5)
                        .color(palette.muted),
                );
            }
            if plan.hdr_tonemapped || plan.hdr_untouched {
                let key = if plan.hdr_tonemapped {
                    "hdr_tonemapped"
                } else {
                    "hdr_untouched"
                };
                let color = if plan.hdr_untouched {
                    palette.danger
                } else {
                    palette.accent
                };
                ui.label(RichText::new(t(self.lang, key)).size(10.5).color(color));
            }

            if let Some(key) = codec_hint(plan.codec, plan.quality.bppf, plan.is_remux) {
                ui.label(
                    RichText::new(t(self.lang, key))
                        .size(10.5)
                        .color(palette.accent.gamma_multiply(0.9)),
                );
            }
            if !plan.is_remux && !plan.quality.tip.is_empty() {
                ui.label(
                    RichText::new(&plan.quality.tip)
                        .size(10.5)
                        .color(palette.accent.gamma_multiply(0.9)),
                );
            }
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        if !self.show_gallery {
            self.gallery
                .poll_usage(self.settings.gallery_folders(), ctx);
        }
        let lang = self.lang;
        let had_glass = !self.settings.background_theme.is_empty();
        let mut close = false;
        let mut changed = false;
        let modal = egui::Modal::new(egui::Id::new("settings"))
            .frame(theme::dialog(&ctx.style()))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(5, 2))
                    .show(ui, |ui| {
                        ui.set_width(468.0);
                        ui.horizontal(|ui| {
                            widgets::heading(ui, Icon::Settings, t(lang, "settings_title"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                close =
                                    widgets::icon_square(ui, Icon::Close, t(lang, "close"), 30.0)
                                        .clicked();
                            });
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(t(lang, "language"));
                            changed |= widgets::language_picker(ui, &mut self.lang);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let theme_switch =
                                    widgets::theme_switch(ui, self.settings.dark_theme, lang);
                                if theme_switch.clicked() {
                                    self.toggle_theme(ctx, theme_switch.rect.center());
                                }
                                ui.label(t(lang, "theme"));
                            });
                        });
                        ui.separator();
                        // "Custom media" shows the image it last used, so the entry is
                        // recognisable without opening anything.
                        let custom_thumb = theme::background_preset("custom-media")
                            .and_then(|preset| self.background.recent_previews(preset, ctx).ok())
                            .and_then(|previews| previews.first())
                            .map(|preview| preview.texture.clone());
                        ui.horizontal(|ui| {
                            let (label, default) = if lang == Lang::Pl {
                                ("Motyw tła", "Domyślny")
                            } else {
                                ("Background theme", "Default")
                            };
                            ui.label(label);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let is_default = self.settings.background_theme.is_empty();
                                let selected =
                                    theme::background_preset(&self.settings.background_theme)
                                        .map_or(default, |preset| preset.label(lang));
                                egui::ComboBox::from_id_salt("background_theme")
                                    .width(202.0)
                                    .height(340.0)
                                    .selected_text(selected)
                                    .show_ui(ui, |ui| {
                                        widgets::animate_menu(ui);
                                        ui.set_max_width(316.0);
                                        if widgets::theme_row(ui, None, default, is_default)
                                            .clicked()
                                        {
                                            self.settings.background_theme.clear();
                                            changed = true;
                                        }
                                        for preset in theme::BACKGROUND_PRESETS {
                                            let thumb = if preset.id == "custom-media" {
                                                custom_thumb.clone()
                                            } else {
                                                preset_thumb(ctx, &mut self.theme_thumbs, preset)
                                            };
                                            let selected =
                                                self.settings.background_theme == preset.id;
                                            if widgets::theme_row(
                                                ui,
                                                thumb.as_ref(),
                                                preset.label(lang),
                                                selected,
                                            )
                                            .clicked()
                                                && !selected
                                            {
                                                self.settings.background_theme =
                                                    preset.id.to_owned();
                                                changed = true;
                                            }
                                        }
                                    });
                            });
                        });
                        if let Some(preset) =
                            theme::background_preset(&self.settings.background_theme)
                                .filter(|preset| preset.supports_custom_image())
                        {
                            ui.horizontal(|ui| {
                                let (label, reset, failure) = if lang == Lang::Pl {
                                    ("Plik tła", "Przywróć", "Nie udało się zmienić pliku tła")
                                } else {
                                    (
                                        "Background file",
                                        "Reset",
                                        "Could not change the background file",
                                    )
                                };
                                ui.label(label);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if widgets::icon_button(
                                        ui,
                                        Icon::Image,
                                        t(lang, "choose"),
                                        BtnKind::Secondary,
                                        0.0,
                                    )
                                    .clicked()
                                    {
                                        if let Some(path) = rfd::FileDialog::new()
                                            .add_filter(
                                                "PNG / JPEG / GIF / Video",
                                                &[
                                                    "png", "jpg", "jpeg", "gif", "mp4", "mkv",
                                                    "webm", "mov", "avi", "m4v",
                                                ],
                                            )
                                            .pick_file()
                                        {
                                            if let Err(error) =
                                                self.background.set_custom_image(preset, &path)
                                            {
                                                self.show_toast(format!("{failure} · {error}"));
                                            }
                                        }
                                    }
                                    if preset.has_custom_image()
                                        && ui.add(egui::Button::new(reset).frame(false)).clicked()
                                    {
                                        if let Err(error) =
                                            self.background.reset_custom_image(preset)
                                        {
                                            self.show_toast(format!("{failure} · {error}"));
                                        }
                                    }
                                    let mut picked = None;
                                    match self.background.recent_previews(preset, ctx) {
                                        Ok(previews) => {
                                            let size = Vec2::new(34.0, 26.0);
                                            for preview in previews {
                                                let response = ui
                                                    .add(
                                                        egui::Image::new((
                                                            preview.texture.id(),
                                                            size,
                                                        ))
                                                        .uv(theme::cover_uv(
                                                            preview.texture.size_vec2(),
                                                            size,
                                                        ))
                                                        .corner_radius(5)
                                                        .sense(Sense::click()),
                                                    )
                                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                    .on_hover_text(&preview.name);
                                                response.widget_info(|| {
                                                    egui::WidgetInfo::labeled(
                                                        egui::WidgetType::Button,
                                                        true,
                                                        &preview.name,
                                                    )
                                                });
                                                if response.clicked() {
                                                    picked = Some(preview.path.clone());
                                                }
                                            }
                                        }
                                        Err(error) => {
                                            ui.label(format!("{failure} · {error}"));
                                        }
                                    }
                                    if let Some(path) = picked {
                                        if let Err(error) =
                                            self.background.use_recent_image(preset, &path)
                                        {
                                            self.show_toast(format!("{failure} · {error}"));
                                        }
                                    }
                                });
                            });
                        }
                        ui.separator();
                        if choose_folder(ui, lang, "output_dir", &mut self.settings.output_dir) {
                            self.gallery.set_output_dir(&self.settings.output_dir);
                            changed = true;
                        }
                        let mut folders = self.settings.gallery_folders();
                        if gallery_folders_ui(ui, lang, &mut folders) {
                            if folders.is_empty() {
                                folders.push(
                                    cutgut::media::default_gallery_dir()
                                        .to_string_lossy()
                                        .into_owned(),
                                );
                            }
                            self.settings.gallery_dirs = folders;
                            self.settings.gallery_dir = self
                                .settings
                                .gallery_dirs
                                .first()
                                .cloned()
                                .unwrap_or_default();
                            self.gallery.set_dirs(self.settings.gallery_folders());
                            changed = true;
                        }
                        ui.separator();
                        changed |= widgets::switch_row(
                            ui,
                            &mut self.settings.pause_overlay,
                            t(lang, "pause_overlay"),
                        )
                        .changed();
                        changed |= widgets::switch_row(
                            ui,
                            &mut self.settings.gallery_on_startup,
                            t(lang, "gallery_startup"),
                        )
                        .changed();
                        ui.horizontal(|ui| {
                            ui.label(t(lang, "original_cleanup"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                egui::ComboBox::from_id_salt("original_cleanup_mode")
                                    .width(202.0)
                                    .selected_text(t(lang, self.settings.original_cleanup.label()))
                                    .show_ui(ui, |ui| {
                                        widgets::animate_menu(ui);
                                        ui.set_min_width(202.0);
                                        ui.spacing_mut().item_spacing.y = 2.0;
                                        for (mode, label) in OriginalCleanup::OPTIONS {
                                            let selected = self.settings.original_cleanup == mode;
                                            if widgets::menu_row(ui, None, t(lang, label), selected)
                                                .clicked()
                                                && !selected
                                            {
                                                self.settings.original_cleanup = mode;
                                                changed = true;
                                            }
                                        }
                                    });
                            });
                        });
                        ui.add_space(2.0);
                        ui.separator();
                        storage_usage_ui(ui, &self.gallery, lang, &mut self.storage_pie);
                        ui.add_space(2.0);
                        ui.separator();
                        if widgets::inset_footer_button(ui, t(lang, "keybinds")).clicked() {
                            self.show_keybinds = true;
                        }
                    });
            });
        if close || modal.should_close() {
            self.show_settings = false;
        }
        if changed {
            self.settings.lang = self.lang;
            let glass = !self.settings.background_theme.is_empty();
            if glass != had_glass {
                theme::configure(ctx, self.settings.dark_theme, glass);
            }
            self.queue_settings_save();
            if self.lang != lang || self.plan_error.is_some() {
                self.request_plan();
            }
        }
    }

    /// What went wrong with the export, in full.
    ///
    /// The engine hands back ffmpeg's own stderr (up to 1800 characters), which
    /// is exactly what a bug report needs and exactly what a four-second toast
    /// cannot show.
    fn error_window(&mut self, ctx: &egui::Context) {
        let Some((error, report)) = self.export_error.clone() else {
            return;
        };
        let lang = self.lang;
        let mut close = false;
        let mut copy = false;
        let mut open_logs = false;
        let modal = egui::Modal::new(egui::Id::new("export_error"))
            .frame(theme::dialog(&ctx.style()))
            .show(ctx, |ui| {
                ui.set_width(560.0);
                let palette = Palette::get(ui);
                widgets::heading(ui, Icon::Warning, t(lang, "export_failed"));
                ui.add_space(2.0);
                let text = RichText::new(t(lang, "export_failed_hint"))
                    .size(11.5)
                    .color(palette.muted);
                ui.label(text);
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let combined = format!(
                            "{error}

{report}"
                        );
                        let mut shown = combined.as_str();
                        ui.add(
                            egui::TextEdit::multiline(&mut shown)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(12)
                                .code_editor(),
                        );
                    });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(crate::log::directory().display().to_string())
                        .size(10.5)
                        .color(palette.muted),
                );
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = ui.button(t(lang, "close")).clicked();
                    copy = ui.button(t(lang, "copy_report")).clicked();
                    open_logs = ui.button(t(lang, "open_logs")).clicked();
                });
            });
        if copy {
            ui_copy(
                ctx,
                &format!(
                    "{error}

{report}"
                ),
            );
            self.show_toast(t(lang, "report_copied"));
        }
        if open_logs {
            let _ = std::process::Command::new("explorer")
                .arg(crate::log::directory())
                .spawn();
        }
        if close || modal.should_close() {
            self.export_error = None;
        }
    }

    fn cleanup_window(&mut self, ctx: &egui::Context) {
        let Some(Cleanup::Ask(request)) = &self.cleanup else {
            return;
        };
        let filename = Path::new(&request.original.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let lang = self.lang;
        let mut keep = false;
        let mut recycle = false;
        let modal = egui::Modal::new(egui::Id::new("original_cleanup"))
            .frame(theme::dialog(&ctx.style()))
            .show(ctx, |ui| {
                ui.set_width(365.0);
                widgets::heading(ui, Icon::Trash, t(lang, "original_cleanup_title"));
                ui.add(egui::Label::new(RichText::new(filename.as_ref()).strong()).truncate());
                ui.label(t(lang, "original_cleanup_prompt"));
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    keep = ui.button(t(lang, "keep_original")).clicked();
                    recycle = widgets::icon_button(
                        ui,
                        Icon::Trash,
                        t(lang, "recycle_original"),
                        BtnKind::Danger,
                        0.0,
                    )
                    .clicked();
                });
            });
        if recycle {
            if let Some(Cleanup::Ask(request)) = self.cleanup.take() {
                self.recycle_original(request);
            }
        } else if keep || modal.should_close() {
            self.cleanup = None;
        }
    }

    fn keybinds_window(&mut self, ctx: &egui::Context) {
        if !self.show_keybinds {
            return;
        }
        let lang = self.lang;
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("keybinds"))
            .frame(theme::dialog(&ctx.style()))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(14, 2))
                    .show(ui, |ui| {
                        ui.set_width(430.0);
                        let palette = Palette::get(ui);
                        ui.horizontal(|ui| {
                            widgets::heading(ui, Icon::Settings, t(lang, "keybinds"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                close =
                                    widgets::icon_square(ui, Icon::Close, t(lang, "close"), 30.0)
                                        .clicked();
                            });
                        });
                        ui.separator();
                        keybind_row(ui, t(lang, "kb_play"), &[&["SPACE"]], palette);
                        keybind_row(ui, t(lang, "kb_trim"), &[&["I"], &["O"]], palette);
                        keybind_row(ui, t(lang, "kb_step"), &[&["←"], &["→"]], palette);
                        keybind_row(
                            ui,
                            t(lang, "kb_frame_step"),
                            &[&[",", ".", "[", "]"], &["SHIFT", "←", "SHIFT", "→"]],
                            palette,
                        );
                        keybind_row(ui, t(lang, "kb_jump"), &[&["HOME"], &["END"]], palette);
                        keybind_row(
                            ui,
                            t(lang, "kb_zoom"),
                            &[&["+", "-", "0"], &["CTRL", "SCROLL"]],
                            palette,
                        );
                        keybind_row(
                            ui,
                            t(lang, "kb_gallery_navigation"),
                            &[&["←"], &["→"]],
                            palette,
                        );
                        keybind_row(ui, t(lang, "kb_open"), &[&["CTRL", "O"]], palette);
                        keybind_row(ui, t(lang, "kb_import"), &[&["CTRL", "O"]], palette);
                        keybind_row(
                            ui,
                            t(lang, "kb_gallery_select"),
                            &[&["CTRL", "+"], &["SHIFT", "+"]],
                            palette,
                        );
                        keybind_row(ui, t(lang, "kb_select_all"), &[&["CTRL", "A"]], palette);
                        keybind_row(ui, t(lang, "kb_gallery"), &[&["G"]], palette);
                        keybind_row(ui, t(lang, "kb_fullscreen"), &[&["F"]], palette);
                        keybind_row(ui, t(lang, "kb_snapshot"), &[&["S"]], palette);
                        keybind_row(ui, t(lang, "kb_close"), &[&["ESC"]], palette);
                    });
            });
        if close || modal.should_close() {
            self.show_keybinds = false;
        }
    }

    fn toast_window(&mut self, ctx: &egui::Context) {
        let Some((message, created)) = &self.toast else {
            return;
        };
        if created.elapsed() > Duration::from_secs(4) {
            self.toast = None;
            return;
        }
        let text = message.clone();
        egui::Area::new(egui::Id::new("toast"))
            .anchor(Align2::RIGHT_TOP, Vec2::new(-18.0, 64.0))
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                theme::surface(ui).show(ui, |ui| {
                    ui.set_max_width(420.0);
                    ui.label(RichText::new(text).size(13.0));
                });
            });
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

fn field_label(ui: &mut egui::Ui, text: &str) {
    let palette = Palette::get(ui);
    ui.label(RichText::new(text).size(11.5).strong().color(palette.muted));
}

/// Puts text on the system clipboard (egui keeps no state for this).
fn ui_copy(ctx: &egui::Context, text: &str) {
    ctx.copy_text(text.to_owned());
}

fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let minutes = (total / 60.0).floor() as u64;
    let secs = total - minutes as f64 * 60.0;
    format!("{minutes:02}:{secs:05.2}")
}

fn parse_hex_color(text: &str) -> Color32 {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(160);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(160);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(160);
        Color32::from_rgb(r, g, b)
    } else {
        Color32::from_rgb(160, 160, 170)
    }
}

/// Gallery folder list: one row per folder, each removable, plus "Add folder".
fn gallery_folders_ui(ui: &mut egui::Ui, lang: Lang, folders: &mut Vec<String>) -> bool {
    let palette = Palette::get(ui);
    let mut changed = false;
    let mut remove = None;
    let mut add = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(t(lang, "gallery_dir")).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Folder, t(lang, "add_folder"), BtnKind::Ghost, 0.0)
                .clicked()
            {
                add = true;
            }
        });
    });
    for (index, dir) in folders.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            let (slot, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
            icons::paint(ui.painter(), Icon::Folder, slot, palette.accent, 1.3);
            // The bin sits at the far right, one per folder, so any root can go.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_square(ui, Icon::Trash, t(lang, "remove_folder"), 24.0).clicked() {
                    remove = Some(index);
                }
                let width = (ui.available_width() - 4.0).max(40.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(width, 16.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(dir).size(11.5).color(palette.muted))
                                .truncate()
                                .sense(Sense::hover()),
                        )
                        .on_hover_text(dir.as_str());
                    },
                );
            });
        });
    }
    if let Some(index) = remove {
        folders.remove(index);
        changed = true;
    }
    if add {
        if let Some(folder) = rfd::FileDialog::new()
            .set_directory(
                folders
                    .first()
                    .map(String::as_str)
                    .unwrap_or_else(|| "C:\\"),
            )
            .pick_folder()
        {
            let folder = folder.to_string_lossy().into_owned();
            if !folders.iter().any(|known| known == &folder) {
                folders.push(folder);
                changed = true;
            }
        }
    }
    changed
}

fn choose_folder(ui: &mut egui::Ui, lang: Lang, label: &str, path: &mut String) -> bool {
    let palette = Palette::get(ui);
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(330.0);
            ui.label(RichText::new(t(lang, label)).strong());
            ui.add(
                egui::Label::new(RichText::new(path.as_str()).size(11.5).color(palette.muted))
                    .truncate(),
            )
            .on_hover_text(path.as_str());
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Folder, t(lang, "choose"), BtnKind::Secondary, 0.0)
                .clicked()
            {
                picked = rfd::FileDialog::new()
                    .set_directory(path.as_str())
                    .pick_folder();
            }
        });
    });
    if let Some(folder) = picked {
        *path = folder.to_string_lossy().into_owned();
        true
    } else {
        false
    }
}

/// Interactively links the segments to the category icons and their totals.
/// Decodes (once) the small preview of a background theme for the settings picker.
fn preset_thumb(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<&'static str, egui::TextureHandle>,
    preset: &theme::BackgroundPreset,
) -> Option<egui::TextureHandle> {
    if let Some(texture) = cache.get(preset.id) {
        return Some(texture.clone());
    }
    let bytes = preset.thumbnail()?;
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).ok()?;
    let image = image.into_rgba8();
    let pixels = egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    );
    let texture = ctx.load_texture(preset.id, pixels, egui::TextureOptions::LINEAR);
    cache.insert(preset.id, texture.clone());
    Some(texture)
}

fn storage_usage_ui(ui: &mut egui::Ui, gallery: &Gallery, lang: Lang, pie: &mut bool) {
    let palette = Palette::get(ui);
    let mut usage = gallery.usage(lang);
    usage.retain(|item| item.size > 0);
    let total: u64 = usage.iter().map(|item| item.size).sum();
    if total == 0 {
        return;
    }

    ui.horizontal(|ui| {
        let (icon, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
        icons::paint(ui.painter(), Icon::Gallery, icon, palette.muted, 1.4);
        ui.label(
            RichText::new(t(lang, "storage"))
                .size(12.0)
                .color(palette.muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format_size(total))
                    .size(17.0)
                    .strong()
                    .color(palette.text),
            );
            ui.add_space(6.0);
            widgets::chart_toggle(ui, pie, lang);
        });
    });

    let state_id = ui.make_persistent_id("storage_category");
    let previous = ui.data(|data| data.get_temp::<String>(state_id));
    let mut selected = usage
        .iter()
        .position(|item| Some(&item.name) == previous.as_ref())
        .unwrap_or(0);
    let mut active = selected;
    if *pie {
        if let Some(picked) = storage_pie_ui(ui, &usage, total, selected, state_id, gallery, lang) {
            active = picked;
            ui.data_mut(|data| data.insert_temp(state_id, usage[picked].name.clone()));
        }
    } else {
        const ICON: f32 = 16.0;
        const ICON_INSET: f32 = 4.0;
        let (track, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), ICON + 2.0 * ICON_INSET),
            Sense::hover(),
        );
        let mut cumulative = 0;
        let mut left = track.left();
        let mut hovered = None;
        let mut segments = Vec::with_capacity(usage.len());
        for (index, item) in usage.iter().enumerate() {
            cumulative += item.size;
            let right = track.left() + track.width() * (cumulative as f64 / total as f64) as f32;
            let rect = Rect::from_min_max(
                egui::pos2(left, track.top()),
                egui::pos2(right, track.bottom()),
            );
            left = right;
            let response = ui
                .interact(rect, state_id.with(("segment", index)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                selected = index;
                ui.data_mut(|data| data.insert_temp(state_id, item.name.clone()));
            }
            if response.hovered() {
                hovered = Some(index);
            }
            segments.push((rect, response));
        }

        active = hovered.unwrap_or(selected);
        let painter = ui.painter();
        painter.rect_filled(track, 6, palette.well);
        for (index, (item, (segment, response))) in usage.iter().zip(segments).enumerate() {
            let emphasis =
                ui.ctx()
                    .animate_bool_with_time(state_id.with(&item.name), active == index, 0.15);
            let is_first = index == 0;
            let is_last = index + 1 == usage.len();
            let radius = egui::CornerRadius {
                nw: if is_first { 6 } else { 0 },
                sw: if is_first { 6 } else { 0 },
                ne: if is_last { 6 } else { 0 },
                se: if is_last { 6 } else { 0 },
            };
            let gap = if is_last {
                0.0
            } else {
                (segment.width() * 0.2).min(2.0)
            };
            let segment = Rect::from_min_max(segment.min, segment.max - Vec2::new(gap, 0.0));
            let fill = icons::lerp_color(item.color, palette.panel, 0.34 * (1.0 - emphasis));
            painter.rect_filled(segment, radius, fill);
            if segment.width() >= 12.0 {
                let inset = if is_first || is_last { 5.0 } else { 2.0 };
                painter.hline(
                    (segment.left() + inset)..=(segment.right() - inset),
                    segment.top() + 2.0,
                    Stroke::new(
                        1.0_f32,
                        Color32::from_white_alpha((24.0 + 48.0 * emphasis) as u8),
                    ),
                );
            }

            let share = item.size as f64 / total as f64 * 100.0;
            let details = format!(
                "{} · {:.1}% · {} {}",
                format_size(item.size),
                share,
                item.count,
                t(lang, "clips_short")
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    response.enabled(),
                    selected == index,
                    format!("{} · {details}", item.name),
                )
            });
            response
                .clone()
                .on_hover_text(format!("{}\n{details}", item.name));
            if response.has_focus() {
                painter.rect_stroke(
                    segment,
                    radius,
                    Stroke::new(1.0_f32, palette.text),
                    egui::StrokeKind::Inside,
                );
            }
            if segment.width() >= ICON + 4.0 {
                let icon = Rect::from_center_size(segment.center(), Vec2::splat(ICON));
                match &item.game {
                    Some(game) => gallery.paint_game_icon(painter, game, icon),
                    None => icons::paint(painter, Icon::Upload, icon, Color32::from_gray(24), 1.4),
                }
            }
        }
    }
    let item = &usage[active];
    let share = item.size as f64 / total as f64 * 100.0;
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.0, item.color);
        let metrics = format!(
            "{}  ·  {:.1}%  ·  {} {}",
            format_size(item.size),
            share,
            item.count,
            t(lang, "clips_short"),
        );
        let metrics_width = ui
            .painter()
            .layout_no_wrap(metrics.clone(), FontId::proportional(11.0), palette.muted)
            .size()
            .x;
        let name_width =
            (ui.available_width() - metrics_width - ui.spacing().item_spacing.x).max(0.0);
        ui.allocate_ui_with_layout(
            Vec2::new(name_width, 15.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(name_width);
                ui.add(egui::Label::new(RichText::new(&item.name).size(12.0).strong()).truncate());
            },
        );
        ui.label(RichText::new(metrics).size(11.0).color(palette.muted));
    });
}

/// Donut chart of the categories; returns the slice the user just clicked.
fn storage_pie_ui(
    ui: &mut egui::Ui,
    usage: &[crate::gallery::CategoryUsage],
    total: u64,
    selected: usize,
    state_id: egui::Id,
    gallery: &Gallery,
    lang: Lang,
) -> Option<usize> {
    const SIZE: f32 = 132.0;
    let palette = Palette::get(ui);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), SIZE + 14.0), Sense::click());
    let center = rect.center();
    let radius = SIZE * 0.5;
    // Slices start at twelve o'clock and run clockwise.
    let start = -std::f32::consts::FRAC_PI_2;
    let mut bounds = Vec::with_capacity(usage.len());
    let mut angle = start;
    for item in usage {
        let sweep = (item.size as f64 / total as f64) as f32 * std::f32::consts::TAU;
        bounds.push((angle, sweep));
        angle += sweep;
    }

    let pointer = response.hover_pos();
    let hovered = pointer.and_then(|pos| {
        let offset = pos - center;
        if offset.length() > radius + 3.0 {
            return None;
        }
        let mut turn = offset.y.atan2(offset.x) - start;
        while turn < 0.0 {
            turn += std::f32::consts::TAU;
        }
        let mut reach = 0.0;
        for (index, (_, sweep)) in bounds.iter().enumerate() {
            reach += sweep;
            if turn < reach {
                return Some(index);
            }
        }
        None
    });

    let picked = if response.clicked() { hovered } else { None };
    let active = hovered.unwrap_or(selected);
    let painter = ui.painter().with_clip_rect(rect.union(response.rect));

    for (index, (item, (from, sweep))) in usage.iter().zip(&bounds).enumerate() {
        let emphasis =
            ui.ctx()
                .animate_bool_with_time(state_id.with(&item.name), active == index, 0.15);
        if *sweep <= 0.0 {
            continue;
        }
        let fill = icons::lerp_color(item.color, palette.text, 0.12 * emphasis);
        // The hovered slice lifts out of the pie by a couple of pixels.
        let mid = from + sweep * 0.5;
        let lift = Vec2::new(mid.cos(), mid.sin()) * (3.0 * emphasis) - Vec2::splat(1.0);
        let steps = ((sweep / 0.05).ceil() as usize).max(3);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(center + lift, fill);
        for step in 0..=steps {
            let a = from + sweep * (step as f32 / steps as f32);
            let (sin, cos) = a.sin_cos();
            mesh.colored_vertex(center + lift + Vec2::new(cos, sin) * radius, fill);
        }
        for step in 0..steps {
            let first = 1 + step as u32;
            mesh.add_triangle(0, first, first + 1);
        }
        painter.add(egui::Shape::mesh(mesh));

        // Every slice carries its game's own icon, as long as it is wide enough
        // to hold one without touching the edges.
        let icon_size = (sweep * radius * 0.46).min(28.0) + 2.0 * emphasis;
        if icon_size >= 13.0 {
            if let Some(game) = &item.game {
                let slot = Rect::from_center_size(
                    center + Vec2::new(mid.cos(), mid.sin()) * (radius * 0.64),
                    Vec2::splat(icon_size),
                );
                gallery.paint_game_icon(&painter, game, slot);
            }
        }
    }

    // Thin seams between the slices, like a printed pie chart.
    for (from, sweep) in &bounds {
        let a = from + sweep;
        painter.line_segment(
            [
                center + Vec2::new(a.cos(), a.sin()) * 2.0,
                center + Vec2::new(a.cos(), a.sin()) * radius,
            ],
            Stroke::new(1.4_f32, palette.panel),
        );
    }

    // Hovering a slice slides out a callout: a leader line and a small card with
    // the name on top and its share, clip count and size underneath.
    if let Some(index) = hovered {
        let (from, sweep) = bounds[index];
        let mid = from + sweep * 0.5;
        let (sin, cos) = mid.sin_cos();
        let dir = Vec2::new(cos, sin);
        let edge = center + dir * (radius + 1.0);
        let right = dir.x >= 0.0;
        let item = &usage[index];
        let slide =
            ui.ctx()
                .animate_bool_with_time(state_id.with(("pie_callout", &item.name)), true, 0.16);
        let name = item.name.clone();
        let details = format!(
            "{:.1}%  ·  {} {}  ·  {}",
            item.size as f64 / total as f64 * 100.0,
            item.count,
            t(lang, "clips_short"),
            format_size(item.size)
        );
        let name_galley = painter.layout_no_wrap(name, FontId::proportional(12.0), palette.text);
        let info_galley =
            painter.layout_no_wrap(details, FontId::proportional(10.5), palette.muted);
        let width = name_galley.size().x.max(info_galley.size().x) + 20.0;
        let height = name_galley.size().y + info_galley.size().y + 13.0;
        let gap = 22.0;
        let origin_x = if right {
            center.x + radius + gap
        } else {
            center.x - radius - gap - width
        };
        let origin_y =
            (edge.y - height * 0.5).clamp(rect.top() + 2.0, rect.bottom() - height - 2.0);
        let card = Rect::from_min_size(
            egui::pos2(
                origin_x + if right { -1.0 } else { 1.0 } * (1.0 - slide) * 10.0,
                origin_y,
            ),
            Vec2::new(width, height),
        );
        let layer = egui::LayerId::new(egui::Order::Tooltip, state_id.with("pie_callout_layer"));
        let painter = ui.ctx().layer_painter(layer);
        // The leader line grows out of the slice.
        let anchor = egui::pos2(
            if right { card.left() } else { card.right() },
            card.center().y,
        );
        let elbow = egui::pos2(
            anchor.x + if right { -10.0 } else { 10.0 } * slide,
            anchor.y,
        );
        painter.line_segment(
            [edge, elbow],
            Stroke::new(1.0_f32, palette.line.gamma_multiply(slide)),
        );
        painter.line_segment(
            [elbow, anchor],
            Stroke::new(1.0_f32, palette.line.gamma_multiply(slide)),
        );
        painter.circle_filled(edge, 2.5, item.color);
        painter.rect_filled(card, 8, palette.raised.gamma_multiply(slide));
        painter.rect_stroke(
            card,
            8,
            Stroke::new(1.0_f32, palette.line.gamma_multiply(slide)),
            egui::StrokeKind::Inside,
        );
        painter.galley(
            card.min + Vec2::new(10.0, 6.0),
            name_galley.clone(),
            palette.text.gamma_multiply(slide),
        );
        painter.galley(
            card.min + Vec2::new(10.0, 6.0 + name_galley.size().y + 0.5),
            info_galley,
            palette.muted.gamma_multiply(slide),
        );
    }

    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            response.enabled(),
            true,
            format!(
                "{} · {}",
                usage[active].name,
                format_size(usage[active].size)
            ),
        )
    });
    picked
}

fn format_size(size: u64) -> String {
    let mb = size as f64 / 1_000_000.0;
    if mb >= 1_000.0 {
        format!("{:.2} GB", mb / 1_000.0)
    } else {
        format!("{mb:.1} MB")
    }
}

/// Discord-style shortcut row: label on the left, key caps on the right.
/// Below this many bits per pixel an export is visibly starved, and the codec
/// choice starts to matter more than anything else in the plan.
const STARVED_BPPF: f64 = 0.055;

/// Which codec hint (if any) a plan deserves.
///
/// Measured on a 1080p60 game clip at the same ~20 MB budget, through CutGut's
/// own pipeline: AV1 95.2 VMAF, HEVC 94.4, H.264 93.3 on CPU and 88.9 on the
/// hardware encoder. A starved H.264 export is exactly the case worth flagging.
fn codec_hint(codec: cutgut::plan::VideoCodec, bppf: f64, is_remux: bool) -> Option<&'static str> {
    use cutgut::plan::VideoCodec;
    if is_remux || bppf >= STARVED_BPPF {
        return None;
    }
    match codec {
        VideoCodec::H264 => Some("codec_hint_h264"),
        VideoCodec::Hevc => Some("codec_hint_hevc"),
        VideoCodec::Av1 => None,
    }
}

/// Visible part of the clip: a zoomed span and its left edge.
fn timeline_window(zoom: f64, view_start: f64, duration: f64) -> (f64, f64) {
    let duration = duration.max(0.001);
    let span = (duration / zoom.max(1.0)).clamp(0.05, duration);
    let start = view_start.clamp(0.0, (duration - span).max(0.0));
    (start, span)
}

/// Zoom factor and left edge after zooming, keeping `anchor` under the same
/// point on screen.
fn zoomed_window(
    zoom: f64,
    view_start: f64,
    duration: f64,
    factor: f64,
    anchor: f64,
) -> (f64, f64) {
    let (start, span) = timeline_window(zoom, view_start, duration);
    let zoomed = (zoom * factor).clamp(1.0, 240.0);
    let duration = duration.max(0.001);
    let spanned = (duration / zoomed).clamp(0.05, duration);
    let fraction = if span > 0.0 {
        ((anchor - start) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let start = (anchor - fraction * spanned).clamp(0.0, (duration - spanned).max(0.0));
    (zoomed, start)
}

/// Playhead position `count` frames away from `position`, clamped to the clip.
fn frame_step_position(position: f64, fps: f64, count: i32, duration: f64) -> f64 {
    let fps = fps.max(1.0);
    let index = (position * fps).round() as i64 + i64::from(count);
    (index as f64 / fps).clamp(0.0, duration.max(0.0))
}

/// Smallest ruler step that still fits a label, in seconds.
fn tick_step(px_per_second: f64) -> f64 {
    const STEPS: [f64; 15] = [
        0.05, 0.1, 0.2, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0,
    ];
    for step in STEPS {
        if step * px_per_second >= 72.0 {
            return step;
        }
    }
    STEPS[STEPS.len() - 1]
}

/// `m:ss` for whole seconds, `m:ss.d` when the ruler is finer than that.
fn tick_label(time: f64, step: f64) -> String {
    let minutes = (time / 60.0).floor();
    let seconds = time - minutes * 60.0;
    let minutes = minutes as i64;
    // More decimals the closer the ruler is zoomed in, so two ticks never read
    // the same.
    if step >= 1.0 {
        format!("{minutes}:{seconds:02.0}")
    } else if step >= 0.1 {
        format!("{minutes}:{seconds:04.1}")
    } else {
        format!("{minutes}:{seconds:05.2}")
    }
}

fn keybind_row(ui: &mut egui::Ui, label: &str, shortcuts: &[&[&str]], palette: Palette) {
    let height = 10.0 + 23.0 * shortcuts.len() as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter();
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0_f32, palette.line),
    );
    painter.text(
        rect.left_center() + Vec2::new(2.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        palette.text,
    );

    for (row, keys) in shortcuts.iter().enumerate() {
        let mut right = rect.right() - 2.0;
        let y = rect.top() + 5.0 + 23.0 * row as f32;
        for key in keys.iter().rev() {
            let galley =
                painter.layout_no_wrap(key.to_string(), FontId::proportional(11.0), palette.text);
            let width = (galley.size().x + 10.0).max(22.0);
            let cap = Rect::from_min_size(egui::pos2(right - width, y), Vec2::new(width, 20.0));
            painter.rect_filled(cap, 4, palette.line);
            let face = cap.with_max_y(cap.bottom() - 2.0);
            painter.rect_filled(face, 4, palette.raised);
            painter.rect_stroke(
                face,
                4,
                Stroke::new(1.0_f32, palette.line),
                egui::StrokeKind::Inside,
            );
            if let "←" | "→" = *key {
                let center = face.center();
                let sign = if *key == "←" { -1.0 } else { 1.0 };
                let tip = center + Vec2::new(sign * 4.0, 0.0);
                let stroke = Stroke::new(1.25_f32, palette.text);
                painter.line_segment([center - Vec2::new(sign * 4.0, 0.0), tip], stroke);
                painter.add(egui::Shape::line(
                    vec![
                        tip + Vec2::new(-sign * 3.0, -3.0),
                        tip,
                        tip + Vec2::new(-sign * 3.0, 3.0),
                    ],
                    stroke,
                ));
            } else {
                painter.galley(face.center() - galley.size() * 0.5, galley, palette.text);
            }
            right = cap.left() - 3.0;
        }
    }
}

fn select_value<T: PartialEq>(ui: &mut egui::Ui, value: &mut T, item: T, label: &str) -> bool {
    let selected = *value == item;
    if widgets::choice(ui, label, selected).clicked() {
        *value = item;
        ui.close();
        !selected
    } else {
        false
    }
}

impl Drop for CutGutApp {
    fn drop(&mut self) {
        self.player.stop();
        self.gallery.stop_all();
        if let Some(state) = &self.encoding {
            state.cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        codec_hint, frame_step_position, tick_label, tick_step, timeline_window, zoomed_window,
        CleanupRequest, OriginalFile, Playback,
    };

    /// The scrub area used to be the whole timeline rect, registered after the
    /// frame-step buttons, so every click on them was eaten by the drag area and
    /// the buttons looked dead.
    #[test]
    fn the_ruler_buttons_never_overlap_the_scrub_area() {
        use eframe::egui::{pos2, vec2, Rect};
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 111.0));
        let (ruler, strip) = crate::app::CutGutApp::timeline_geometry(rect);
        for hit in crate::app::CutGutApp::ruler_buttons(ruler) {
            assert!(ruler.contains_rect(hit), "{hit:?} is outside the ruler");
            assert!(
                !strip.intersects(hit),
                "{hit:?} overlaps the filmstrip and would lose its clicks"
            );
            assert!(hit.height() >= 18.0, "a {hit:?} tall target is hard to hit");
        }
    }

    #[test]
    fn cleanup_requires_the_original_and_the_completed_export() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("original.mp4");
        let output = dir.path().join("trim.mp4");
        std::fs::write(&input, b"original").unwrap();
        let request = CleanupRequest {
            original: OriginalFile::read(input.to_str().unwrap()).unwrap(),
            output: output.clone(),
            output_size: 4,
        };
        assert!(!request.files_match());
        std::fs::write(&output, b"trim").unwrap();
        assert!(request.files_match());
        std::fs::write(&input, b"replacement").unwrap();
        assert!(!request.files_match());
    }

    #[test]
    fn full_playback_preserves_positions_outside_the_trim() {
        let full = Playback::Full.range(10.0, 3.0, 7.0);
        assert_eq!(full, (0.0, 10.0));
        assert_eq!(Playback::Full.start_at(1.0, full), 1.0);
        assert_eq!(Playback::Full.start_at(8.0, full), 8.0);
        assert_eq!(Playback::Full.start_at(10.0, full), 0.0);
    }

    #[test]
    fn a_starved_export_points_at_a_better_codec() {
        use cutgut::plan::VideoCodec;
        // A 20 MB cap on a minute of 1080p60 is the case this is for.
        assert_eq!(
            codec_hint(VideoCodec::H264, 0.02, false),
            Some("codec_hint_h264")
        );
        assert_eq!(
            codec_hint(VideoCodec::Hevc, 0.02, false),
            Some("codec_hint_hevc")
        );
        // AV1 is already the best choice, and a remux has no bitrate to argue about.
        assert_eq!(codec_hint(VideoCodec::Av1, 0.02, false), None);
        assert_eq!(codec_hint(VideoCodec::H264, 0.02, true), None);
        // A comfortable bitrate needs no advice.
        assert_eq!(codec_hint(VideoCodec::H264, 0.12, false), None);
    }

    #[test]
    fn the_timeline_shows_the_whole_clip_unless_zoomed() {
        assert_eq!(timeline_window(1.0, 0.0, 20.0), (0.0, 20.0));
        // A stray view_start cannot scroll past the end.
        assert_eq!(timeline_window(1.0, 50.0, 20.0), (0.0, 20.0));
        // Zooming in keeps the window inside the clip.
        let (start, span) = timeline_window(4.0, 100.0, 20.0);
        assert_eq!(span, 5.0);
        assert_eq!(start, 15.0);
    }

    #[test]
    fn zooming_keeps_the_anchor_where_it_is_on_screen() {
        // Zooming around the 60 % mark of the visible window keeps it there.
        let (zoom, start) = zoomed_window(2.0, 0.0, 20.0, 2.0, 6.0);
        assert_eq!(zoom, 4.0);
        let (start_after, span_after) = timeline_window(zoom, start, 20.0);
        assert!(((6.0 - start_after) / span_after - 0.6).abs() < 1e-9);

        // An anchor outside the window stays clamped to the near edge.
        let (zoom, start) = zoomed_window(2.0, 0.0, 20.0, 2.0, 17.0);
        let (start_after, span_after) = timeline_window(zoom, start, 20.0);
        assert!(((17.0 - start_after) / span_after - 1.0).abs() < 1e-9);

        // Zooming out all the way lands back on the whole clip.
        let (zoom, start) = zoomed_window(1.0, 0.0, 20.0, 0.1, 12.0);
        assert_eq!(zoom, 1.0);
        assert_eq!(timeline_window(zoom, start, 20.0), (0.0, 20.0));
    }

    #[test]
    fn frame_steps_land_on_frame_boundaries_and_never_drift() {
        let fps: f64 = 59.94;
        let mut position = 0.0;
        for step in 1..=300 {
            position = frame_step_position(position, fps, 1, 20.0);
            let index = (position * fps).round() as i64;
            assert_eq!(index, step, "step {step} landed on frame {index}");
        }
        // Both ends clamp instead of running off the clip.
        assert_eq!(frame_step_position(19.99, fps, 30, 20.0), 20.0);
        assert_eq!(frame_step_position(0.0, fps, -1, 20.0), 0.0);
    }

    #[test]
    fn the_ruler_picks_steps_that_fit_and_labels_them_uniquely() {
        // Wide view: seconds. Zoomed in: fractions of a second.
        assert_eq!(tick_step(50.0), 2.0);
        assert_eq!(tick_step(200.0), 0.5);
        assert_eq!(tick_step(5000.0), 0.05);
        assert_eq!(tick_label(75.0, 1.0), "1:15");
        assert_eq!(tick_label(0.5, 0.5), "0:00.5");
        assert_eq!(tick_label(0.05, 0.05), "0:00.05");
    }

    #[test]
    fn trim_preview_always_starts_at_in_and_ends_at_out() {
        let trim = Playback::Trim.range(10.0, 3.0, 7.0);
        assert_eq!(trim, (3.0, 7.0));
        for position in [1.0, 4.0, 8.0] {
            assert_eq!(Playback::Trim.start_at(position, trim), 3.0);
        }
    }
}
