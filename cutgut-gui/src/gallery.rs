//! Gallery view: chips for games, compact thumbnail cards, and a large
//! frosted-style preview modal with inline playback and file actions.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::TimeZone;
use cutgut::ffmpeg;
use cutgut::media::{self, Clip};
use cutgut::plan::VideoInfo;
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense,
    Stroke, TextureHandle, Vec2,
};

use crate::game_icons::{self, GameIcons};
use crate::i18n::{t, Lang};
use crate::icons::{self, Icon};
use crate::library::Library;
use crate::player::{self, PlaybackRate, Player, PreviewSize};
use crate::still_preview::StillPreview;
use crate::theme::{self, Palette};
use crate::widgets::{self, BtnKind, Volume};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Favorites,
    Imported,
    Output,
    Group(String),
}

/// How the gallery splits its clips into headed sections.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GroupBy {
    /// One section per game / category.
    #[default]
    Game,
    /// One section per gallery folder (or per game folder inside a root).
    Folder,
    /// Videos and images.
    FileType,
    /// One section per recording day.
    Date,
    /// A single section with no heading.
    None,
}

impl GroupBy {
    pub const ALL: [Self; 5] = [
        Self::Game,
        Self::Folder,
        Self::FileType,
        Self::Date,
        Self::None,
    ];

    pub fn from_index(index: u8) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or_default()
    }

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0) as u8
    }

    fn icon(self) -> Icon {
        match self {
            Self::Game => Icon::Gallery,
            Self::Folder => Icon::Folder,
            Self::FileType => Icon::Film,
            Self::Date => Icon::Calendar,
            Self::None => Icon::Sort,
        }
    }

    fn label(self, lang: Lang) -> &'static str {
        match self {
            Self::Game => t(lang, "group_game"),
            Self::Folder => t(lang, "group_folder"),
            Self::FileType => t(lang, "group_type"),
            Self::Date => t(lang, "group_date"),
            Self::None => t(lang, "group_none"),
        }
    }
}

/// Thumbnail density of the grid, kept small so a row still shows a few clips.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CardSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl CardSize {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    pub fn from_index(index: u8) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or_default()
    }

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|size| *size == self).unwrap_or(1) as u8
    }

    /// Target card width; the grid then fits as many columns as possible.
    fn target_width(self) -> f32 {
        match self {
            Self::Small => 150.0,
            Self::Medium => 204.0,
            Self::Large => 268.0,
        }
    }

    fn label(self, lang: Lang) -> &'static str {
        match self {
            Self::Small => t(lang, "card_small"),
            Self::Medium => t(lang, "card_medium"),
            Self::Large => t(lang, "card_large"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
enum SortBy {
    #[default]
    Date,
    Name,
    FileType,
    Size,
    Duration,
}

impl SortBy {
    const ALL: [Self; 5] = [
        Self::Date,
        Self::Name,
        Self::FileType,
        Self::Size,
        Self::Duration,
    ];

    fn index(self) -> u8 {
        Self::ALL.iter().position(|sort| *sort == self).unwrap_or(0) as u8
    }

    /// A small icon that identifies the criterion in the sort menu.
    fn icon(self) -> Option<Icon> {
        Some(match self {
            Self::Date => Icon::Calendar,
            Self::Name => Icon::Text,
            Self::FileType => Icon::Film,
            Self::Size => Icon::Storage,
            Self::Duration => Icon::Clock,
        })
    }

    fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::Date, Lang::Pl) => "Data",
            (Self::Name, Lang::Pl) => "Nazwa",
            (Self::FileType, Lang::Pl) => "Typ pliku",
            (Self::Size, Lang::Pl) => "Rozmiar",
            (Self::Duration, Lang::Pl) => "Czas trwania",
            (Self::Date, Lang::En) => "Date",
            (Self::Name, Lang::En) => "Name",
            (Self::FileType, Lang::En) => "File type",
            (Self::Size, Lang::En) => "File size",
            (Self::Duration, Lang::En) => "Duration",
        }
    }
}

fn sort_clips<K: Ord>(clips: &mut [Clip], descending: bool, key: impl Fn(&Clip) -> Option<K>) {
    clips.sort_by_cached_key(|clip| {
        let value = key(clip);
        let missing = value.is_none();
        let order = if descending {
            (None, value.map(std::cmp::Reverse))
        } else {
            (value, None)
        };
        (missing, order, clip.path.clone())
    });
}
struct ThumbMsg {
    clip: Clip,
    image: Option<egui::ColorImage>,
}

struct DurationMsg {
    clip: Clip,
    duration: Option<f64>,
}

/// What the user did with a gallery card this frame.
enum CardAction {
    None,
    Open,
    /// Dragged far enough to start an OS drag of the file out of the app.
    DragOut,
    /// Ctrl+click: add or remove this clip from the marked set.
    Mark,
    /// Shift+click: mark the range from the anchor to this clip.
    MarkRange,
    /// A row of the right-click menu was chosen.
    Menu(ClipCommand),
}

type Groups = Vec<(String, Vec<Clip>)>;
type GalleryScan = (Groups, HashSet<PathBuf>);

struct ScanUpdate {
    result: GalleryScan,
    finished: bool,
}

type GameCache = BTreeMap<PathBuf, (u64, u64, Option<String>)>;

/// How many thumbnail textures may live on the GPU at once.
///
/// Textures are a few hundred kilobytes each, so scrolling a long gallery used to
/// keep every thumbnail it ever showed resident (~1 GB of VRAM after a few
/// thousand clips). Dropped thumbnails come back from the on-disk JPG cache in a
/// frame or two, which is why the budget can be this small.
const TEXTURE_BUDGET: usize = 200;

/// How long the clip slide-in animation lasts.
const SLIDE_TIME: f32 = 0.26;

/// How long the fullscreen transport bar stays up after the mouse stops moving.
const FULLSCREEN_BAR_IDLE: f64 = 2.6;

/// Height of the fullscreen transport bar.
const FULLSCREEN_BAR_HEIGHT: f32 = 54.0;

/// Probed durations keyed by path, with the (mtime, size) they were probed at.
type DurationCache = BTreeMap<PathBuf, (u64, u64, f64)>;

/// How long the thumbnail takes to grow out of its card.
const ZOOM_TIME: f32 = 0.26;
/// The way back takes a little longer, so it reads as a glide rather than a snap.
const ZOOM_BACK_TIME: f32 = 0.34;

/// A thumbnail morphing between a gallery card and the preview picture.
struct ZoomAnim {
    /// Where the thumbnail starts (the card image) and where it goes.
    from: Rect,
    to: Rect,
    texture: Option<TextureHandle>,
    opened_at: f64,
    /// Set when the preview starts closing; the panel fades while the
    /// thumbnail flies back to `to`.
    closing: Option<f64>,
    /// True once the opening morph has landed: nothing is painted any more,
    /// but the card rect and the texture stay ready for the way back.
    settled: bool,
}

pub struct Gallery {
    pub dir: String,
    /// Every scanned folder; `dir` mirrors the first one.
    dirs: Vec<String>,
    output_dir: PathBuf,
    output_paths: HashSet<PathBuf>,
    groups: Groups,
    game_icons: GameIcons,
    filter: Filter,
    /// Filter to return to when the active game chip is clicked again.
    filter_before_group: Filter,
    sort_by: SortBy,
    descending: bool,
    sort_dirty: bool,
    group_by: GroupBy,
    card_size: CardSize,
    library: Library,
    imported: Vec<Clip>,
    notice: Option<String>,
    backdrop: Option<TextureHandle>,
    search: String,
    filters_open: bool,
    scroll_anchor: f32,
    scroll_height: f32,
    scroll_pending: f32,
    reset_scroll: bool,
    textures: HashMap<String, TextureHandle>,
    /// Insertion order of `textures`, oldest first, used to pick eviction victims.
    texture_order: VecDeque<String>,
    thumb_requests: mpsc::Sender<Clip>,
    thumb_results: Receiver<ThumbMsg>,
    thumb_pending: HashSet<(PathBuf, u64, u64)>,
    thumb_versions: HashMap<String, (u64, u64)>,
    thumb_cache_dir: PathBuf,
    /// Probed durations by path key; a version without a value means probing
    /// already failed for that exact file (so it is not retried).
    durations: HashMap<String, f64>,
    duration_requests: mpsc::Sender<Clip>,
    duration_results: Receiver<DurationMsg>,
    duration_pending: HashSet<(PathBuf, u64, u64)>,
    duration_versions: HashMap<String, (u64, u64)>,
    duration_cache: DurationCache,
    duration_cache_file: PathBuf,
    duration_cache_dirty: bool,
    scanned: bool,
    scan: Option<Receiver<ScanUpdate>>,
    /// Set when a scan is superseded (folder change, refresh, window closing) so
    /// its worker threads stop spawning ffprobe processes for a folder nobody is
    /// looking at any more.
    scan_cancel: Arc<AtomicBool>,
    ctx: egui::Context,
    open_on_scan: bool,
    play_on_probe: bool,
    rename: Option<(String, String)>,
    /// Clips waiting for the delete confirmation.
    delete: Option<Vec<String>>,
    edit_request: Option<String>,
    /// Clip to keep highlighted (e.g. just exported) until it is opened.
    highlight: Option<PathBuf>,
    /// Scroll position (content y) of the highlighted card, once located.
    highlight_y: Option<f32>,
    /// Still need to scroll the grid to the highlighted card.
    highlight_scroll: bool,
    selected: Option<Clip>,
    selected_info: Option<VideoInfo>,
    /// Clips marked for a bulk action (Ctrl+click, Shift+click, Ctrl+A).
    marked: Vec<PathBuf>,
    /// Where a Shift+click range starts.
    mark_anchor: Option<PathBuf>,
    player: Player,
    preview_tex: Option<TextureHandle>,
    playing: bool,
    playback_rate: PlaybackRate,
    preview_press: widgets::PreviewPress,
    /// Speed badge that slides in from the top of the picture while it is held
    /// down (or right after the speed was picked from the menu).
    rate_osd: Option<(PlaybackRate, Instant)>,
    /// Held down: the badge stays until the button comes back up.
    rate_osd_held: bool,
    /// Card-to-screen zoom that plays when a clip is opened or closed.
    zoom: Option<ZoomAnim>,
    /// Live on-screen rect of every visible card, so closing the preview can
    /// send the thumbnail back exactly where it came from.
    card_rects: HashMap<PathBuf, Rect>,
    /// Rect the preview picture occupied in the last frame.
    picture_rect: Option<Rect>,
    /// Media position (seconds) of the frame that is/should be on screen.
    position: f64,
    duration: f64,
    /// True while the seek bar is being dragged.
    seeking: bool,
    /// Resume playback when the current seek finishes.
    seek_resume: bool,
    /// Transient play/pause feedback: `true` = paused (||), `false` = playing (>).
    flash: Option<(bool, Instant)>,
    probe: Option<Receiver<Result<VideoInfo, String>>>,
    still: StillPreview,
    backdrop_rx: Option<Receiver<Option<egui::ColorImage>>>,
    /// Time the preview modal appeared, used for the smooth zoom-in animation.
    modal_opened_at: Option<f64>,
    /// The preview asked for a fullscreen window, so closing it gives the frame back.
    preview_fullscreen: bool,
    /// Middle-button panning of the grid: pointer y and offset when it started.
    pan: Option<(f32, f32)>,
    /// Last known vertical scroll offset of the grid.
    grid_offset: f32,
    /// When the last clip change happened and which way it came from (-1 / +1),
    /// used for the slide-in animation.
    slide_at: Option<f64>,
    slide_dir: f32,
    /// Where the fullscreen transport bar sits, so hovering it keeps it visible.
    fullscreen_bar: Option<Rect>,
    /// Last time the pointer moved in a fullscreen preview.
    fullscreen_bar_shown: Option<f64>,
    volume: Volume,
    /// Volume just changed by the user; the app persists it in the settings file.
    volume_change: Option<f32>,
}

impl Gallery {
    pub fn new(dir: String, ctx: egui::Context) -> Self {
        let thumb_cache_dir = media::get_temp_dir().join("gallery_cache");
        let (thumb_requests, thumb_results) = thumbnail_workers(&thumb_cache_dir, &ctx);
        let (duration_requests, duration_results) = duration_workers(&ctx);
        let duration_cache_file = thumb_cache_dir.join("durations.json");
        let duration_cache: DurationCache = std::fs::read(&duration_cache_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            dirs: vec![dir.clone()],
            dir,
            output_dir: PathBuf::new(),
            output_paths: Default::default(),
            groups: Vec::new(),
            game_icons: GameIcons::new(&ctx),
            filter: Filter::All,
            filter_before_group: Filter::All,
            sort_by: SortBy::Date,
            group_by: GroupBy::default(),
            card_size: CardSize::default(),
            descending: true,
            sort_dirty: true,
            library: Library::load(),
            imported: Vec::new(),
            notice: None,
            backdrop: None,
            search: String::new(),
            filters_open: true,
            scroll_anchor: 0.0,
            scroll_height: 0.0,
            scroll_pending: 0.0,
            reset_scroll: false,
            textures: HashMap::new(),
            texture_order: VecDeque::new(),
            thumb_requests,
            thumb_results,
            thumb_pending: HashSet::new(),
            thumb_versions: HashMap::new(),
            thumb_cache_dir,
            durations: HashMap::new(),
            duration_requests,
            duration_results,
            duration_pending: HashSet::new(),
            duration_versions: HashMap::new(),
            duration_cache,
            duration_cache_file,
            duration_cache_dirty: false,
            scanned: false,
            scan: None,
            scan_cancel: Arc::new(AtomicBool::new(false)),
            ctx: ctx.clone(),
            open_on_scan: false,
            play_on_probe: false,
            rename: None,
            delete: None,
            edit_request: None,
            highlight: None,
            highlight_y: None,
            highlight_scroll: false,
            selected: None,
            selected_info: None,
            marked: Vec::new(),
            mark_anchor: None,
            player: Player::new(ctx.clone()),
            preview_tex: None,
            playing: false,
            playback_rate: PlaybackRate::default(),
            preview_press: widgets::PreviewPress::default(),
            rate_osd: None,
            rate_osd_held: false,
            zoom: None,
            card_rects: HashMap::new(),
            picture_rect: None,
            position: 0.0,
            duration: 0.0,
            seeking: false,
            seek_resume: false,
            flash: None,
            probe: None,
            still: StillPreview::new(ctx),
            backdrop_rx: None,
            modal_opened_at: None,
            preview_fullscreen: false,
            pan: None,
            grid_offset: 0.0,
            slide_at: None,
            slide_dir: 1.0,
            fullscreen_bar: None,
            fullscreen_bar_shown: None,
            volume: Volume::default(),
            volume_change: None,
        }
    }

    /// Applies the persisted preview volume (0–100) to this gallery.
    pub fn set_volume(&mut self, percent: f32) {
        self.volume.set_percent(percent);
        self.player.set_volume(self.volume.percent());
    }

    /// Returns a volume the user just picked, so it can be saved.
    pub fn take_volume_change(&mut self) -> Option<f32> {
        self.volume_change.take()
    }

    pub fn set_dir(&mut self, dir: String) {
        self.set_dirs(vec![dir]);
    }

    /// Every folder the gallery scans; the first one is the primary folder.
    pub fn set_dirs(&mut self, dirs: Vec<String>) {
        let changed = self.dirs != dirs;
        self.dirs = dirs;
        self.dir = self
            .dirs
            .first()
            .cloned()
            .unwrap_or_else(|| media::default_gallery_dir().to_string_lossy().into_owned());
        if changed || (!self.scanned && self.scan.is_none()) {
            self.reset_scroll = true;
            self.reload();
        }
    }

    pub fn set_output_dir(&mut self, dir: &str) {
        let dir = if dir.is_empty() {
            media::get_default_output_dir()
        } else {
            PathBuf::from(dir)
        };
        if self.output_dir != dir {
            self.output_dir = dir;
            if self.scanned || self.scan.is_some() {
                self.reload();
            }
        }
    }

    pub fn source_game(&self, path: &Path) -> Option<String> {
        if let Some(game) = self.library.export_games.get(path) {
            return Some(game.clone());
        }
        if let Some(clip) = self
            .groups
            .iter()
            .flat_map(|(_, clips)| clips)
            .find(|clip| clip.path == path)
        {
            return (clip.group != OUTPUT_GROUP).then(|| clip.group.clone());
        }
        if let Some(folder) = path
            .strip_prefix(&self.dir)
            .ok()
            .and_then(Path::parent)
            .and_then(|relative| relative.iter().next())
        {
            return Some(game_icons::game_name(&folder.to_string_lossy()).to_owned());
        }
        let stem = path.file_stem()?.to_str()?;
        dated_name(stem).map(|(name, _)| game_icons::game_name(name).to_owned())
    }

    pub fn export_finished(&mut self, path: PathBuf, game: Option<String>, lang: Lang) {
        // Keep the fresh export highlighted until the user opens it, and make
        // sure the current filter/search cannot hide it.
        self.highlight = Some(path.clone());
        self.highlight_y = None;
        self.highlight_scroll = true;
        self.filter = Filter::All;
        self.search.clear();
        if let Some(game) = game {
            let mut library = self.library.clone();
            library.export_games.insert(path, game);
            if !self.store_library(library.clone(), lang) {
                self.library = library;
            }
        }
        self.start_scan();
    }

    pub fn open_first(&mut self) {
        if self.scan.is_some() {
            self.open_on_scan = true;
            return;
        }
        let clip = self
            .groups
            .first()
            .and_then(|(_, clips)| clips.first())
            .or_else(|| self.imported.first())
            .cloned();
        if let Some(clip) = clip {
            self.open_clip(&clip);
        }
    }

    pub fn play_selected(&mut self) {
        if self.scan.is_some() || self.probe.is_some() {
            self.play_on_probe = true;
        } else if self.selected_info.is_some() && !self.playing {
            self.toggle_play();
        }
    }

    pub fn take_edit(&mut self) -> Option<String> {
        self.edit_request.take()
    }

    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    pub fn import_dialog(&mut self, lang: Lang) {
        if let Some(paths) = rfd::FileDialog::new()
            .set_title(t(lang, "import_clips"))
            .add_filter(t(lang, "media_files"), media::VIDEO_EXTS)
            .pick_files()
        {
            self.import(paths, lang);
        }
    }

    pub fn import(&mut self, paths: Vec<PathBuf>, lang: Lang) {
        let scanned: std::collections::HashSet<_> = self
            .groups
            .iter()
            .flat_map(|(_, clips)| clips.iter().map(|clip| &clip.path))
            .collect();
        let mut library = self.library.clone();
        let mut added = false;
        for path in paths {
            if let Some(clip) = Clip::from_video_path(&path, "") {
                if !scanned.contains(&clip.path) {
                    added |= library.imported.insert(clip.path);
                }
            } else {
                self.notice = Some(format!(
                    "{} · {}",
                    t(lang, "import_problem"),
                    path.display()
                ));
            }
        }
        if added && self.store_library(library, lang) {
            self.filter = Filter::Imported;
            self.search.clear();
            self.reset_scroll = true;
            self.reload();
        }
    }

    fn store_library(&mut self, library: Library, lang: Lang) -> bool {
        if library.save().is_err() {
            self.notice = Some(t(lang, "library_problem").to_owned());
            return false;
        }
        self.library = library;
        true
    }

    fn toggle_favorite(&mut self, path: &Path, lang: Lang) {
        let mut library = self.library.clone();
        if !library.favorites.remove(path) {
            library.favorites.insert(path.to_path_buf());
        }
        self.store_library(library, lang);
    }

    pub fn marked_count(&self) -> usize {
        self.marked.len()
    }

    /// Drops the marks; returns `true` when there was something to clear, so the
    /// caller can let Escape clear a selection instead of leaving the gallery.
    pub fn clear_marks(&mut self) -> bool {
        let had = !self.marked.is_empty();
        self.marked.clear();
        self.mark_anchor = None;
        had
    }

    fn is_marked(&self, path: &Path) -> bool {
        self.marked.iter().any(|marked| marked == path)
    }

    fn toggle_mark(&mut self, path: &Path) {
        match self.marked.iter().position(|marked| marked == path) {
            Some(index) => {
                self.marked.remove(index);
            }
            None => self.marked.push(path.to_path_buf()),
        }
        self.mark_anchor = Some(path.to_path_buf());
    }

    /// Marks every clip between the anchor and `path`, in display order.
    fn mark_range(&mut self, path: &Path, lang: Lang) {
        let order: Vec<PathBuf> = self
            .sections(lang)
            .iter()
            .flat_map(|section| section.clips.iter().map(|clip| clip.path.clone()))
            .collect();
        let Some(target) = order.iter().position(|candidate| candidate == path) else {
            return;
        };
        let anchor = self
            .mark_anchor
            .as_ref()
            .and_then(|anchor| order.iter().position(|candidate| candidate == anchor))
            .unwrap_or(target);
        let (from, to) = if anchor <= target {
            (anchor, target)
        } else {
            (target, anchor)
        };
        self.marked = order[from..=to].to_vec();
        self.mark_anchor = Some(path.to_path_buf());
    }

    /// Ctrl+A: mark everything the current filter and search show.
    pub fn mark_all(&mut self, lang: Lang) {
        self.marked = self
            .sections(lang)
            .iter()
            .flat_map(|section| section.clips.iter().map(|clip| clip.path.clone()))
            .collect();
        self.mark_anchor = self.marked.first().cloned();
    }

    fn marked_clips(&self) -> Vec<Clip> {
        self.all_clips()
            .filter(|clip| self.is_marked(&clip.path))
            .cloned()
            .collect()
    }

    pub fn has_modal(&self) -> bool {
        self.selected.is_some() || self.rename.is_some() || self.delete.is_some()
    }

    pub fn stop_all(&mut self) {
        self.stop_playback();
    }

    pub fn release_source(&mut self, path: &Path) {
        if self.selected.as_ref().is_some_and(|clip| clip.path == path) {
            self.close_modal();
        }
    }

    pub fn clip_removed(&mut self, path: &Path, lang: Lang) {
        if self.highlight.as_deref() == Some(path) {
            self.highlight = None;
            self.highlight_y = None;
            self.highlight_scroll = false;
        }
        self.release_source(path);
        let mut library = self.library.clone();
        library.replace_path(path, None);
        if !self.store_library(library.clone(), lang) {
            self.library = library;
        }
        self.start_scan();
    }

    /// Whoever closed the preview (button, Escape, view switch), the window frame
    /// comes back once no clip is open any more. Called every frame by the app.
    pub fn sync_window(&mut self, ctx: &egui::Context) {
        if self.preview_fullscreen && self.selected.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            self.preview_fullscreen = false;
        }
    }

    pub fn close_modal(&mut self) {
        if self.preview_press.cancel() {
            self.playback_rate = PlaybackRate::Normal;
        }
        self.stop_playback();
        self.selected = None;
        self.selected_info = None;
        self.preview_tex = None;
        self.backdrop = None;
        self.rename = None;
        self.delete = None;
        self.flash = None;
        self.rate_osd = None;
        self.rate_osd_held = false;
        self.probe = None;
        self.still.clear();
        self.backdrop_rx = None;
        self.modal_opened_at = None;
        self.slide_at = None;
        self.fullscreen_bar = None;
        self.fullscreen_bar_shown = None;
        self.open_on_scan = false;
        self.play_on_probe = false;
        self.zoom = None;
        self.picture_rect = None;
        self.card_rects.clear();
    }

    /// Closing the preview flies the picture back into its card first; when the
    /// card is gone (scrolled away, fullscreen) it just shuts down.
    fn begin_close(&mut self) {
        let card = self
            .selected
            .as_ref()
            .and_then(|clip| self.card_rects.get(&clip.path).copied());
        let now = self.ctx.input(|input| input.time);
        let Some(picture) = self.picture_rect.filter(|_| !self.preview_fullscreen) else {
            self.close_modal();
            return;
        };
        // A clip reached with the arrow keys has no card of its own (it may be
        // scrolled out of view), and its opening morph is gone too — that used to
        // make Escape snap the panel shut. Collapse into the grid instead.
        let target =
            card.unwrap_or_else(|| Rect::from_center_size(picture.center(), picture.size() * 0.45));
        if let Some(zoom) = self.zoom.as_mut() {
            if zoom.closing.is_none() && zoom.texture.is_some() {
                zoom.to = target;
                zoom.closing = Some(now);
                self.ctx.request_repaint();
                return;
            }
        }
        let texture = self
            .preview_tex
            .clone()
            .or_else(|| self.zoom.as_ref().and_then(|zoom| zoom.texture.clone()));
        if let Some(texture) = texture {
            self.zoom = Some(ZoomAnim {
                from: picture,
                to: target,
                texture: Some(texture),
                opened_at: now,
                closing: Some(now),
                settled: true,
            });
            self.ctx.request_repaint();
            return;
        }
        self.close_modal();
    }

    /// Draws the morphing thumbnail above the panel while it travels.
    fn paint_zoom(&mut self, ctx: &egui::Context, fullscreen: bool) {
        if fullscreen {
            return;
        }
        let Some(zoom) = self.zoom.as_ref() else {
            return;
        };
        let Some(texture) = zoom.texture.as_ref() else {
            return;
        };
        let target = self.picture_rect.unwrap_or(zoom.to);
        let now = ctx.input(|input| input.time);
        let (started, from, to, fading_out) = match zoom.closing {
            Some(at) => (at, target, zoom.to, false),
            None => (zoom.opened_at, zoom.from, target, true),
        };
        let duration = if fading_out {
            ZOOM_TIME
        } else {
            ZOOM_BACK_TIME
        };
        let progress = (((now - started) as f32) / duration).clamp(0.0, 1.0);
        // The opening morph has landed: stop painting (and stop asking for
        // repaints) but keep the card rect for the way back.
        if progress >= 1.0 && zoom.closing.is_none() {
            if !zoom.settled {
                if let Some(zoom) = self.zoom.as_mut() {
                    zoom.settled = true;
                }
            }
            return;
        }
        let eased = if fading_out {
            1.0 - (1.0 - progress).powi(3)
        } else {
            // Smoothstep: it leaves the frame gently and lands just as gently.
            progress * progress * (3.0 - 2.0 * progress)
        };
        let rect = Rect::from_min_max(
            from.min + (to.min - from.min) * eased,
            from.max + (to.max - from.max) * eased,
        );
        // The hand-over happens while the thumbnail is still opaque, so the live
        // picture is already in place by the time this fades away.
        let alpha = if fading_out {
            1.0 - ((progress - 0.86) / 0.14).clamp(0.0, 1.0)
        } else {
            // The panel is already gone, so the picture can show up at once.
            (progress / 0.08).clamp(0.0, 1.0)
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("preview_zoom"),
        ));
        theme::rounded_image_tinted(
            &painter,
            rect,
            CornerRadius::same(9),
            texture.id(),
            theme::cover_uv(texture.size_vec2(), rect.size()),
            Color32::WHITE.gamma_multiply(alpha),
        );
        ctx.request_repaint();
    }

    fn stop_playback(&mut self) {
        self.position = self.current_position();
        self.player.stop();
        self.playing = false;
        self.seeking = false;
        self.seek_resume = false;
    }

    /// Position of the frame that is currently on screen.
    fn current_position(&self) -> f64 {
        if self.playing {
            if let Some(position) = self.player.position() {
                return position.clamp(0.0, self.duration.max(0.0));
            }
        }
        self.position
    }

    fn start_playback(&mut self, from: f64) {
        let (Some(clip), Some(info)) = (&self.selected, &self.selected_info) else {
            return;
        };
        self.still.clear();
        self.player.set_volume(self.volume.percent());
        if media::is_gallery_audio(&clip.path) {
            self.player.start_audio(
                &clip.path.to_string_lossy(),
                from,
                self.duration,
                self.playback_rate,
            );
        } else {
            self.player.start(
                &clip.path.to_string_lossy(),
                info,
                from,
                self.duration,
                self.playback_rate,
            );
        }
        self.position = from;
        self.playing = true;
    }

    fn request_frame(&mut self) {
        let (Some(clip), Some(info)) = (&self.selected, &self.selected_info) else {
            return;
        };
        if media::is_gallery_audio(&clip.path) {
            return;
        }
        let last_frame = (info.duration - 1.0 / info.fps.max(1.0)).max(0.0);
        self.still.request(
            &clip.path.to_string_lossy(),
            self.position.min(last_frame),
            PreviewSize::still(info),
        );
    }

    fn poll_probe(&mut self, lang: Lang) {
        let Some(result) = &self.probe else { return };
        match result.try_recv() {
            Ok(Ok(info)) => {
                self.probe = None;
                self.duration = info.duration.max(0.01);
                self.selected_info = Some(info);
                if std::mem::take(&mut self.play_on_probe) {
                    self.toggle_play();
                } else {
                    self.request_frame();
                }
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.probe = None;
                self.notice = Some(t(lang, "media_missing").to_owned());
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    pub fn reload(&mut self) {
        self.close_modal();
        self.thumb_versions
            .retain(|path, _| self.textures.contains_key(path));
        self.start_scan();
    }

    fn start_scan(&mut self) {
        // Stop the previous scan: its threads would otherwise keep probing files
        // of the folder the user already left.
        self.scan_cancel.store(true, Ordering::Relaxed);
        self.scan_cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.scan_cancel);
        let roots: Vec<PathBuf> = self.dirs.iter().map(PathBuf::from).collect();
        let output_dir = self.output_dir.clone();
        let export_games = self.library.export_games.clone();
        let cache_dir = self.thumb_cache_dir.clone();
        let ctx = self.ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.scan = Some(rx);
        std::thread::spawn(move || {
            scan_gallery(
                &roots,
                &output_dir,
                &export_games,
                &cache_dir,
                &cancel,
                |result, finished| {
                    let delivered = tx.send(ScanUpdate { result, finished }).is_ok();
                    if delivered {
                        ctx.request_repaint();
                    }
                    delivered && !cancel.load(Ordering::Relaxed)
                },
            );
        });
    }

    fn poll_scan(&mut self) {
        let Some(scan) = &self.scan else { return };
        let mut update = match scan.try_recv() {
            Ok(update) => update,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.scan = None;
                self.scanned = true;
                return;
            }
        };
        while let Ok(newer) = scan.try_recv() {
            update = newer;
        }
        if update.finished {
            self.scan = None;
        }
        let (groups, output_paths) = update.result;
        self.groups = groups;
        self.sort_dirty = true;
        self.output_paths = output_paths;
        self.game_icons.request_steam(
            self.groups
                .iter()
                .map(|(name, _)| name.as_str())
                .filter(|name| *name != OUTPUT_GROUP),
        );
        if let Filter::Group(group) = &self.filter {
            if update.finished && !self.groups.iter().any(|(name, _)| name == group) {
                self.filter = Filter::All;
            }
        }
        let scanned: std::collections::HashSet<_> = self
            .groups
            .iter()
            .flat_map(|(_, clips)| clips.iter().map(|clip| &clip.path))
            .collect();
        self.imported = self
            .library
            .imported
            .iter()
            .filter(|path| !scanned.contains(*path))
            .filter_map(|path| Clip::from_video_path(path, ""))
            .collect();
        self.imported
            .sort_by_key(|clip| std::cmp::Reverse(clip.mtime));
        self.scanned = true;
        if update.finished && std::mem::take(&mut self.open_on_scan) {
            // `open_first`/`open_clip` start playback on their own now.
            self.open_first();
        }

        let current: HashMap<_, _> = self
            .all_clips()
            .map(|clip| {
                (
                    clip.path.to_string_lossy().into_owned(),
                    (clip.mtime, clip.size),
                )
            })
            .collect();
        self.textures.retain(|path, _| {
            current
                .get(path)
                .is_some_and(|version| Some(version) == self.thumb_versions.get(path))
        });
        self.thumb_versions
            .retain(|path, version| current.get(path) == Some(version));
        self.durations.retain(|path, _| {
            current
                .get(path)
                .is_some_and(|version| Some(version) == self.duration_versions.get(path))
        });
        self.duration_versions
            .retain(|path, version| current.get(path) == Some(version));
        // Drop cache entries for files that are gone or changed, and surface
        // cached durations for the clips that are still here.
        let cache_len = self.duration_cache.len();
        self.duration_cache.retain(|path, (mtime, size, _)| {
            current.get(&path.to_string_lossy().into_owned()) == Some(&(*mtime, *size))
        });
        if self.duration_cache.len() != cache_len {
            self.duration_cache_dirty = true;
        }
        let mut cached = Vec::new();
        for clip in self.all_clips() {
            let key = clip.path.to_string_lossy().into_owned();
            if self.duration_versions.contains_key(&key) {
                continue;
            }
            if let Some((mtime, size, duration)) = self.duration_cache.get(&clip.path) {
                if (*mtime, *size) == (clip.mtime, clip.size) {
                    cached.push((key, (*mtime, *size), *duration));
                }
            }
        }
        for (key, version, duration) in cached {
            self.duration_versions.insert(key.clone(), version);
            self.durations.insert(key, duration);
        }
    }

    fn poll_durations(&mut self, ctx: &egui::Context) {
        for _ in 0..24 {
            let Ok(DurationMsg { clip, duration }) = self.duration_results.try_recv() else {
                break;
            };
            self.duration_pending
                .remove(&(clip.path.clone(), clip.mtime, clip.size));
            if !self.all_clips().any(|current| {
                current.path == clip.path
                    && current.mtime == clip.mtime
                    && current.size == clip.size
            }) {
                continue;
            }
            let key = clip.path.to_string_lossy().into_owned();
            self.duration_versions
                .insert(key.clone(), (clip.mtime, clip.size));
            if let Some(duration) = duration {
                self.durations.insert(key, duration);
                self.duration_cache
                    .insert(clip.path.clone(), (clip.mtime, clip.size, duration));
                self.duration_cache_dirty = true;
                self.sort_dirty |= self.sort_by == SortBy::Duration;
            }
        }
        if self.duration_cache_dirty {
            self.persist_duration_cache();
            ctx.request_repaint();
        }
    }

    fn persist_duration_cache(&mut self) {
        self.duration_cache_dirty = false;
        use std::io::Write as _;
        if std::fs::create_dir_all(&self.thumb_cache_dir).is_err() {
            return;
        }
        if let Ok(mut file) = tempfile::NamedTempFile::new_in(&self.thumb_cache_dir) {
            if serde_json::to_writer(&mut file, &self.duration_cache).is_ok()
                && file.flush().is_ok()
            {
                let _ = file.persist(&self.duration_cache_file);
            }
        }
    }

    fn poll_thumbnails(&mut self, ctx: &egui::Context) {
        for _ in 0..24 {
            let Ok(ThumbMsg { clip, image }) = self.thumb_results.try_recv() else {
                return;
            };
            self.thumb_pending
                .remove(&(clip.path.clone(), clip.mtime, clip.size));
            if !self.all_clips().any(|current| {
                current.path == clip.path
                    && current.mtime == clip.mtime
                    && current.size == clip.size
            }) {
                continue;
            }
            let path = clip.path.to_string_lossy().into_owned();
            self.thumb_versions
                .insert(path.clone(), (clip.mtime, clip.size));
            if let Some(image) = image {
                let texture = ctx.load_texture(&path, image, egui::TextureOptions::LINEAR);
                self.textures.insert(path.clone(), texture);
                self.texture_order.push_back(path);
            }
        }
        ctx.request_repaint();
    }

    /// Keeps the GPU texture set bounded while never dropping a thumbnail that is
    /// on screen this frame. Evicted entries forget their version too, so the
    /// card asks for the thumbnail again and it is reloaded from the JPG cache.
    fn trim_textures(&mut self, visible: &[String]) {
        // Steady state: nothing to evict and the order list has no stale entries.
        if self.texture_order.len() == self.textures.len() && self.textures.len() <= TEXTURE_BUDGET
        {
            return;
        }
        let visible: HashSet<&str> = visible.iter().map(String::as_str).collect();
        let excess = self.textures.len().saturating_sub(TEXTURE_BUDGET);
        let mut evicted = 0;
        let mut kept = VecDeque::with_capacity(self.texture_order.len());
        for key in std::mem::take(&mut self.texture_order) {
            if !self.textures.contains_key(&key) {
                continue;
            }
            if evicted < excess && !visible.contains(key.as_str()) {
                self.textures.remove(&key);
                self.thumb_versions.remove(&key);
                evicted += 1;
            } else {
                kept.push_back(key);
            }
        }
        self.texture_order = kept;
    }

    fn poll_player(&mut self, ctx: &egui::Context) {
        if self.playing {
            if let Some(frame) = self.player.poll() {
                self.position = frame.time;
                player::upload(ctx, &mut self.preview_tex, "gallery_preview", frame.image);
            }
            if let Some(position) = self.player.position() {
                self.position = position;
            }
            if !self.player.is_active() {
                self.playing = false;
                if self.duration - self.position <= 0.12 {
                    self.position = self.duration;
                }
            }
        }
        if let Some(image) = self.still.poll() {
            player::upload(ctx, &mut self.preview_tex, "gallery_preview", image);
        }
        if let Some(rx) = &self.backdrop_rx {
            match rx.try_recv() {
                Ok(image) => {
                    self.backdrop_rx = None;
                    if let Some(image) = image {
                        if self
                            .selected
                            .as_ref()
                            .is_some_and(|clip| media::is_gallery_image(&clip.path))
                        {
                            player::upload(ctx, &mut self.preview_tex, "gallery_image", image);
                            return;
                        }
                        self.backdrop = Some(ctx.load_texture(
                            "blurred_clip_backdrop",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
                Err(TryRecvError::Disconnected) => self.backdrop_rx = None,
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        lang: Lang,
        pause_overlay: bool,
        scroll_locked: bool,
    ) {
        // The preview modal fills exactly this area, leaving the top bar visible.
        let view = ui.max_rect();
        self.poll_scan();
        self.game_icons.poll(ctx);
        self.poll_thumbnails(ctx);
        self.poll_durations(ctx);
        self.poll_probe(lang);
        self.poll_player(ctx);
        let palette = Palette::get(ui);
        let total = self.all_clips().count();
        let favorites = self
            .all_clips()
            .filter(|clip| self.library.favorites.contains(&clip.path))
            .count();

        ui.add_space(4.0);
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(4, 0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // The search field grows with the query; the folder path gives
                    // up the space it needs instead of being pushed out.
                    let search_width =
                        widgets::search_width(ui, &self.search, 224.0, 224.0 + 280.0);
                    let path_width = (ui.available_width()
                        - 395.0
                        - (search_width - 224.0)
                        - ui.spacing().item_spacing.x)
                        .clamp(60.0, 580.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(path_width, widgets::TOOLBAR_HEIGHT),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            // Secondary folders are folded into a "+N" badge so the
                            // primary path keeps the whole row.
                            let extra = self.dirs.len().saturating_sub(1);
                            let shown = if extra > 0 {
                                format!("{}  +{extra}", self.dir)
                            } else {
                                self.dir.clone()
                            };
                            let folders = self.dirs.join(
                                "
",
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(shown).size(11.0).color(palette.muted),
                                )
                                .truncate(),
                            )
                            .on_hover_text(folders);
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_square(
                            ui,
                            Icon::Refresh,
                            t(lang, "refresh"),
                            widgets::TOOLBAR_HEIGHT,
                        )
                        .clicked()
                        {
                            self.reload();
                        }
                        if widgets::icon_square(
                            ui,
                            Icon::Folder,
                            t(lang, "open_folder"),
                            widgets::TOOLBAR_HEIGHT,
                        )
                        .clicked()
                        {
                            if self.filter == Filter::Output {
                                open_folder(&self.output_dir.to_string_lossy());
                            } else {
                                open_folder(&self.dir);
                            }
                        }
                        widgets::icon_toggle(
                            ui,
                            &mut self.filters_open,
                            Icon::Filter,
                            t(lang, "filters"),
                            widgets::TOOLBAR_HEIGHT,
                        );
                        self.sort_menu(ui, lang);
                        self.layout_menu(ui, lang);
                        self.reset_scroll |= widgets::search(
                            ui,
                            &mut self.search,
                            t(lang, "search"),
                            search_width,
                            search_width,
                        )
                        .changed();
                    });
                });

                ui.add_space(4.0);
                let filter_id = ui.make_persistent_id("gallery_filters");
                let mut filters = egui::collapsing_header::CollapsingState::load_with_default_open(
                    ctx, filter_id, true,
                );
                filters.set_open(self.filters_open);
                filters.show_body_unindented(ui, |ui| self.filter_bar(ui, lang, total, favorites));
                if self.marked_count() > 0 {
                    ui.add_space(3.0);
                    self.marked_bar(ui, lang);
                }
            });

        if !self.scanned && total == 0 {
            ui.spinner();
            return;
        }

        let old_scroll_style = ui.spacing().scroll;
        ui.spacing_mut().scroll = egui::style::ScrollStyle {
            bar_width: 5.0,
            bar_inner_margin: 4.0,
            handle_min_length: 28.0,
            foreground_color: true,
            ..egui::style::ScrollStyle::solid()
        };
        let grid_rect = ui.available_rect_before_wrap();
        // Middle-button drag pans the grid, exactly like dragging empty space.
        let middle = ctx.input(|input| {
            (
                input.pointer.middle_down(),
                input.pointer.button_pressed(egui::PointerButton::Middle),
                input.pointer.interact_pos(),
            )
        });
        if self.has_modal() || scroll_locked {
            self.pan = None;
        } else if middle.1 {
            if let Some(pos) = middle.2.filter(|pos| grid_rect.contains(*pos)) {
                self.pan = Some((pos.y, self.grid_offset));
            }
        } else if !middle.0 {
            self.pan = None;
        }
        if self.pan.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("gallery_grid_scroll")
            .scroll_source(if scroll_locked {
                egui::scroll_area::ScrollSource::NONE
            } else {
                egui::scroll_area::ScrollSource::ALL
            })
            .auto_shrink([false, false]);
        if self.reset_scroll {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        if let (Some((start_y, start_offset)), Some(pos)) = (self.pan, middle.2) {
            scroll = scroll.vertical_scroll_offset((start_offset - (pos.y - start_y)).max(0.0));
            ctx.request_repaint();
        }
        // One-shot jump to the highlighted card, once its position was located
        // during a previous frame's layout (the scan may still be running).
        let highlight_applied = self.highlight_scroll && self.highlight_y.is_some();
        if highlight_applied {
            scroll = scroll.vertical_scroll_offset((self.highlight_y.unwrap() - 12.0).max(0.0));
        }

        let mut opened = None;
        let mut open_from: Option<(PathBuf, Rect)> = None;
        let mut card_rects: HashMap<PathBuf, Rect> = HashMap::new();
        let mut drag_out = None;
        let mut mark_toggle: Option<Clip> = None;
        let mut mark_range: Option<Clip> = None;
        let mut menu_command: Option<(ClipCommand, Clip)> = None;
        let mut found_highlight_y = None;
        let mut dismiss_highlight = false;
        let mut visible_without_thumbnail = Vec::new();
        let mut visible_without_duration = Vec::new();
        // Keys of the thumbnails painted this frame; they must survive eviction.
        let mut visible_textures: Vec<String> = Vec::new();
        let can_wheel = !scroll_locked
            && !self.has_modal()
            && !egui::Popup::is_any_open(ctx)
            && ctx.input(|input| {
                !input.modifiers.ctrl
                    && input
                        .pointer
                        .hover_pos()
                        .is_some_and(|p| grid_rect.contains(p))
            });
        if can_wheel && !self.reset_scroll {
            ctx.input_mut(|input| {
                let delta = input.smooth_scroll_delta.y;
                if delta != 0.0 && delta.signum() != self.scroll_pending.signum() {
                    self.scroll_pending = 0.0;
                }
                self.scroll_pending += delta;
                let applied = self.scroll_pending * (1.0 - (-input.stable_dt * 18.0).exp());
                self.scroll_pending -= applied;
                input.smooth_scroll_delta.y = applied;
            });
            if self.scroll_pending.abs() > 0.25 {
                ctx.request_repaint();
            } else {
                self.scroll_pending = 0.0;
            }
        } else {
            self.scroll_pending = 0.0;
        }
        let wheel = ctx.input(|input| {
            if input
                .pointer
                .hover_pos()
                .is_some_and(|p| grid_rect.contains(p))
                && !input.modifiers.ctrl
            {
                input.raw_scroll_delta.y
            } else {
                0.0
            }
        });
        self.sort_gallery();
        let sections = self.sections(lang);
        if self.sort_by == SortBy::Duration && self.duration_pending.len() < 24 {
            let available = 24 - self.duration_pending.len();
            visible_without_duration.extend(
                sections
                    .iter()
                    .flat_map(|section| &section.clips)
                    .copied()
                    .filter(|clip| {
                        !media::is_gallery_image(&clip.path)
                            && !self
                                .duration_versions
                                .contains_key(clip.path.to_string_lossy().as_ref())
                            && !self.duration_pending.contains(&(
                                clip.path.clone(),
                                clip.mtime,
                                clip.size,
                            ))
                    })
                    .take(available)
                    .cloned(),
            );
        }
        let scroll_output = if sections.is_empty() {
            scroll.show(ui, |ui| {
                ui.add_space(5.0);
                ui.vertical_centered(|ui| {
                    let heading = if self.filter == Filter::Favorites && favorites == 0 {
                        "no_favorites"
                    } else if total == 0 {
                        "empty"
                    } else {
                        "no_results"
                    };
                    ui.label(
                        RichText::new(t(lang, heading))
                            .size(18.0)
                            .color(palette.text),
                    );
                    if total == 0 || self.filter == Filter::Imported {
                        ui.add_space(5.0);
                        if widgets::upload_zone(
                            ui,
                            t(lang, "import_clips"),
                            t(lang, "import_hint"),
                            436.0,
                            170.0,
                        )
                        .clicked()
                        {
                            self.import_dialog(lang);
                        }
                    }
                });
            })
        } else {
            scroll.show_viewport(ui, |ui, viewport| {
                const HEADING: f32 = 28.0;
                const HEADING_GAP: f32 = 4.0;
                let gap = 5.0;
                let available = ui.available_width();
                let target = self.card_size.target_width();
                let columns = (((available + gap) / (target + gap)).floor() as usize).clamp(1, 8);
                let card_width =
                    ((available - gap * (columns - 1) as f32) / columns as f32).floor();
                let card_height = card_width * 9.0 / 16.0 + 21.0;
                let row_height = card_height + gap;
                let origin = ui.max_rect().min;
                let mut top = 0.0;
                for section in sections {
                    let rows = section.clips.len().div_ceil(columns);
                    // "No grouping" has no heading, so it starts right at the top.
                    let heading = if section.name.is_empty() {
                        0.0
                    } else {
                        HEADING
                    };
                    let heading_top = top + if heading > 0.0 { 7.0 } else { 2.0 };
                    let body = heading_top + heading + HEADING_GAP;
                    let end = body + rows as f32 * row_height;
                    if let Some(highlight) = &self.highlight {
                        if let Some(index) = section
                            .clips
                            .iter()
                            .position(|clip| &clip.path == highlight)
                        {
                            let row = index / columns;
                            found_highlight_y = Some(body + row as f32 * row_height);
                        }
                    }
                    if viewport.min.y <= end && top <= viewport.max.y {
                        if heading > 0.0 && heading_top + heading >= viewport.min.y {
                            let header = Rect::from_min_size(
                                origin + egui::vec2(0.0, heading_top),
                                egui::vec2(available, HEADING),
                            );
                            section_heading(ui, &section, header, &self.game_icons);
                        }

                        let first =
                            ((viewport.min.y - body) / row_height).floor().max(0.0) as usize;
                        let last = (((viewport.max.y - body) / row_height).ceil().max(0.0)
                            as usize)
                            .min(rows);
                        for row in first..last {
                            let y = body + row as f32 * row_height;
                            let rect = Rect::from_min_size(
                                origin + egui::vec2(0.0, y),
                                egui::vec2(available, card_height),
                            );
                            let mut row_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .id_salt((section.name.as_ref(), section.imported, row))
                                    .max_rect(rect)
                                    .layout(Layout::left_to_right(Align::Min)),
                            );
                            row_ui.spacing_mut().item_spacing.x = gap;
                            let start = row * columns;
                            for &clip in
                                &section.clips[start..(start + columns).min(section.clips.len())]
                            {
                                if !media::is_gallery_audio(&clip.path)
                                    && !self
                                        .thumb_versions
                                        .contains_key(clip.path.to_string_lossy().as_ref())
                                {
                                    visible_without_thumbnail.push(clip.clone());
                                }
                                if !media::is_gallery_image(&clip.path)
                                    && !self
                                        .duration_versions
                                        .contains_key(clip.path.to_string_lossy().as_ref())
                                {
                                    visible_without_duration.push(clip.clone());
                                }
                                visible_textures.push(clip.path.to_string_lossy().into_owned());
                                let mut card_rect = Rect::ZERO;
                                let (action, dismiss) = row_ui
                                    .push_id(&clip.path, |ui| {
                                        card_rect = Rect::from_min_size(
                                            ui.cursor().min,
                                            Vec2::new(card_width, card_height),
                                        );
                                        card_rects.insert(clip.path.clone(), card_rect);
                                        self.card(ui, clip, card_width, card_height, lang)
                                    })
                                    .inner;
                                dismiss_highlight |= dismiss;
                                match action {
                                    CardAction::Open => {
                                        opened = Some(clip.clone());
                                        open_from = Some((clip.path.clone(), card_rect));
                                    }
                                    CardAction::DragOut => drag_out = Some(clip.path.clone()),
                                    CardAction::Mark => mark_toggle = Some(clip.clone()),
                                    CardAction::MarkRange => mark_range = Some(clip.clone()),
                                    CardAction::Menu(command) => {
                                        menu_command = Some((command, clip.clone()));
                                    }
                                    CardAction::None => {}
                                }
                            }
                        }
                    }
                    top = end;
                }
                ui.set_min_height(top);
            })
        };
        ui.spacing_mut().scroll = old_scroll_style;
        for clip in visible_without_thumbnail {
            if self
                .thumb_pending
                .insert((clip.path.clone(), clip.mtime, clip.size))
            {
                let _ = self.thumb_requests.send(clip);
            }
        }
        for clip in visible_without_duration {
            if self
                .duration_pending
                .insert((clip.path.clone(), clip.mtime, clip.size))
            {
                let _ = self.duration_requests.send(clip);
            }
        }
        self.trim_textures(&visible_textures);

        let offset = scroll_output.state.offset.y;
        self.grid_offset = offset;
        let height = scroll_output.inner_rect.height();
        let can_scroll = scroll_output.content_size.y > height + 1.0;
        if highlight_applied {
            // The jump consumed the pending scroll; the highlight itself stays.
            self.scroll_anchor = offset;
            self.highlight_scroll = false;
            self.highlight_y = None;
            self.reset_scroll = false;
        } else {
            if let Some(y) = found_highlight_y {
                if self.highlight_scroll {
                    self.highlight_y = Some(y);
                    ctx.request_repaint();
                }
            }
        }
        if highlight_applied {
            // Already handled above; skip the reset/filter-visibility logic.
        } else if self.reset_scroll {
            self.filters_open = true;
            self.scroll_anchor = 0.0;
            self.reset_scroll = false;
            ctx.request_repaint();
        } else if !scroll_locked && !self.has_modal() && can_scroll {
            if (height - self.scroll_height).abs() > 0.5 {
                // Expanding the viewport can clamp its offset near the last row.
                self.scroll_anchor = offset;
            }
            self.update_filter_visibility(ctx, offset, wheel);
        }
        self.scroll_height = height;
        if dismiss_highlight {
            self.highlight = None;
            self.highlight_y = None;
            self.highlight_scroll = false;
        }
        if let Some(clip) = mark_toggle {
            self.toggle_mark(&clip.path);
        }
        if let Some(clip) = mark_range {
            self.mark_range(&clip.path, lang);
        }
        if let Some(command) = menu_command {
            self.run_clip_command(command.0, &command.1, lang);
        }
        self.card_rects = card_rects;
        if let Some(clip) = opened {
            // Opening a clip is an explicit "just this one" gesture.
            self.marked.clear();
            let from = open_from
                .filter(|(path, _)| *path == clip.path)
                .map(|(_, rect)| {
                    Rect::from_min_max(rect.left_top() + egui::vec2(0.0, 21.0), rect.right_bottom())
                });
            if let Some(from) = from {
                let texture = self
                    .textures
                    .get(clip.path.to_string_lossy().as_ref())
                    .cloned();
                let now = self.ctx.input(|input| input.time);
                self.zoom = Some(ZoomAnim {
                    from,
                    to: from,
                    texture,
                    opened_at: now,
                    closing: None,
                    settled: false,
                });
            }
            self.open_clip(&clip);
        }
        if let Some(path) = drag_out {
            crate::drag_out::drag_file_out(&path);
        }
        if self.selected.is_some() {
            self.modal(ctx, lang, pause_overlay, scroll_locked, view);
        }
        self.rename_delete_modals(ctx, lang);
    }

    fn update_filter_visibility(&mut self, ctx: &egui::Context, offset: f32, wheel: f32) {
        let delta = offset - self.scroll_anchor;
        let target = if wheel > 1.0 || delta < -8.0 || (delta < -0.5 && offset <= 1.0) {
            Some(true)
        } else if wheel < -1.0 || delta > 18.0 {
            Some(false)
        } else {
            None
        };
        if let Some(open) = target {
            self.scroll_anchor = offset;
            if self.filters_open != open {
                self.filters_open = open;
                ctx.request_repaint();
            }
        }
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui, lang: Lang, total: usize, favorites: usize) {
        let palette = Palette::get(ui);
        let mut filter = None;
        // Chips wrap onto another line instead of hiding behind a scrollbar, so
        // every category stays one click away.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
            let digits = total.to_string().len();
            let response = widgets::filter_tab(
                ui,
                t(lang, "all"),
                total,
                digits,
                self.filter == Filter::All,
                None,
            );
            if response.clicked() {
                filter = Some(Filter::All);
            }
            usage_menu(
                &response,
                t(lang, "all"),
                || (self.category_stats(&Filter::All), self.total_bytes()),
                lang,
            );

            // A pink heart marks favourites at a glance.
            let heart = |p: &egui::Painter, rect| {
                let pink = if palette.dark {
                    theme::PINK
                } else {
                    theme::PINK_LIGHT
                };
                icons::heart(p, rect, pink, 1.0);
            };
            let response = widgets::filter_tab(
                ui,
                t(lang, "fav_chip"),
                favorites,
                digits,
                self.filter == Filter::Favorites,
                Some(&heart),
            );
            if response.clicked() {
                filter = Some(Filter::Favorites);
            }
            usage_menu(
                &response,
                t(lang, "favorites"),
                || (self.category_stats(&Filter::Favorites), self.total_bytes()),
                lang,
            );

            let output =
                |p: &egui::Painter, rect| icons::paint(p, Icon::Folder, rect, palette.muted, 1.4);
            let output_count = self.output_paths.len();
            let response = widgets::filter_tab(
                ui,
                t(lang, "cutgut_output"),
                output_count,
                output_count.to_string().len(),
                self.filter == Filter::Output,
                Some(&output),
            );
            if response.clicked() {
                filter = Some(Filter::Output);
            }
            usage_menu(
                &response,
                t(lang, "cutgut_output"),
                || (self.category_stats(&Filter::Output), self.total_bytes()),
                lang,
            );

            if !self.imported.is_empty() || self.filter == Filter::Imported {
                let upload = |p: &egui::Painter, rect| {
                    icons::paint(p, Icon::Upload, rect, palette.muted, 1.4)
                };
                let count = self.imported.len();
                let response = widgets::filter_tab(
                    ui,
                    t(lang, "imported"),
                    count,
                    count.to_string().len(),
                    self.filter == Filter::Imported,
                    Some(&upload),
                );
                if response.clicked() {
                    filter = Some(Filter::Imported);
                }
                usage_menu(
                    &response,
                    t(lang, "imported"),
                    || (self.category_stats(&Filter::Imported), self.total_bytes()),
                    lang,
                );
            }
            for (name, clips) in &self.groups {
                if name == OUTPUT_GROUP {
                    continue;
                }
                let selected = matches!(&self.filter, Filter::Group(group) if group == name);
                let icon = |p: &egui::Painter, rect| self.game_icons.paint(p, name, rect);
                let count = clips.len();
                let response = widgets::filter_tab(
                    ui,
                    &truncate_middle(name, 24),
                    count,
                    count.to_string().len(),
                    selected,
                    Some(&icon),
                );
                if response.clicked() {
                    filter = Some(self.chip_filter(name, selected));
                }
                usage_menu(
                    &response,
                    name,
                    || {
                        (
                            self.category_stats(&Filter::Group(name.clone())),
                            self.total_bytes(),
                        )
                    },
                    lang,
                );
            }
        });
        if let Some(filter) = filter {
            if self.filter != filter {
                // Remember where to go back to when a game chip is clicked twice.
                if !matches!(self.filter, Filter::Group(_)) {
                    self.filter_before_group = self.filter.clone();
                }
                self.filter = filter;
                self.reset_scroll = true;
            }
        }
    }

    /// Bar shown while clips are marked: how many, and what can be done with them.
    fn marked_bar(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let palette = Palette::get(ui);
        let count = self.marked_count();
        let mut clear = false;
        let mut favorite = false;
        let mut copy = false;
        let mut reveal = false;
        let mut delete: Option<Vec<String>> = None;
        egui::Frame::new()
            .fill(
                palette
                    .accent_tint
                    .gamma_multiply(if palette.dark { 0.5 } else { 0.75 }),
            )
            .corner_radius(7)
            .inner_margin(egui::Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (slot, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    icons::paint(ui.painter(), Icon::Check, slot, palette.accent, 1.8);
                    ui.label(
                        RichText::new(format!("{count} {}", t(lang, "marked_clips")))
                            .size(11.5)
                            .strong(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_square(ui, Icon::Close, t(lang, "clear_selection"), 28.0)
                            .clicked()
                        {
                            clear = true;
                        }
                        if widgets::icon_square(ui, Icon::Trash, t(lang, "delete"), 28.0).clicked()
                        {
                            delete = Some(
                                self.marked
                                    .iter()
                                    .map(|path| path.to_string_lossy().into_owned())
                                    .collect(),
                            );
                        }
                        if widgets::icon_square(ui, Icon::Folder, t(lang, "open_folder"), 28.0)
                            .clicked()
                        {
                            reveal = true;
                        }
                        if widgets::icon_square(ui, Icon::Copy, t(lang, "copy_path"), 28.0)
                            .clicked()
                        {
                            copy = true;
                        }
                        let all_favorite = self
                            .marked_clips()
                            .iter()
                            .all(|clip| self.library.favorites.contains(&clip.path));
                        let heart = if all_favorite {
                            Icon::Heart
                        } else {
                            Icon::HeartOutline
                        };
                        if widgets::icon_square(
                            ui,
                            heart,
                            t(
                                lang,
                                if all_favorite {
                                    "remove_favorite"
                                } else {
                                    "add_favorite"
                                },
                            ),
                            28.0,
                        )
                        .clicked()
                        {
                            favorite = true;
                        }
                    });
                });
            });
        if clear {
            self.clear_marks();
        }
        if favorite {
            let clips = self.marked_clips();
            let all_favorite = clips
                .iter()
                .all(|clip| self.library.favorites.contains(&clip.path));
            let mut library = self.library.clone();
            for clip in &clips {
                if all_favorite {
                    library.favorites.remove(&clip.path);
                } else {
                    library.favorites.insert(clip.path.clone());
                }
            }
            if !self.store_library(library.clone(), lang) {
                self.library = library;
            }
        }
        if copy {
            let text = self
                .marked
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("\n");
            ui.ctx().copy_text(text);
            self.notice = Some(t(lang, "path_copied").to_owned());
        }
        if reveal {
            let mut folders: Vec<String> = Vec::new();
            for clip in self.marked_clips().iter().take(3) {
                if let Some(parent) = clip.path.parent() {
                    let parent = parent.to_string_lossy().into_owned();
                    if !folders.contains(&parent) {
                        folders.push(parent);
                    }
                }
            }
            for folder in folders {
                open_folder(&folder);
            }
        }
        if let Some(paths) = delete {
            self.stop_playback();
            self.delete = Some(paths);
        }
    }

    fn sort_menu(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let button = widgets::icon_square(
            ui,
            Icon::Sort,
            if lang == Lang::Pl {
                "Sortuj klipy"
            } else {
                "Sort clips"
            },
            widgets::TOOLBAR_HEIGHT,
        );
        let previous = (self.sort_by, self.descending);
        egui::Popup::menu(&button).show(|ui| {
            widgets::animate_menu(ui);
            ui.set_min_width(186.0);
            ui.set_max_width(206.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for sort in SortBy::ALL {
                let selected = self.sort_by == sort;
                if widgets::menu_row(ui, sort.icon(), sort.label(lang), selected).clicked()
                    && !selected
                {
                    self.sort_by = sort;
                    self.descending =
                        matches!(sort, SortBy::Date | SortBy::Size | SortBy::Duration);
                }
            }
            ui.separator();
            let (ascending, descending) = if lang == Lang::Pl {
                ("Rosnąco", "Malejąco")
            } else {
                ("Ascending", "Descending")
            };
            for (value, icon, label) in [
                (false, Icon::Ascending, ascending),
                (true, Icon::Descending, descending),
            ] {
                let selected = self.descending == value;
                if widgets::menu_row(ui, Some(icon), label, selected).clicked() && !selected {
                    self.descending = value;
                }
            }
        });
        if previous != (self.sort_by, self.descending) {
            self.reset_scroll = true;
            self.sort_dirty = true;
        }
    }

    /// Grouping mode and thumbnail density, kept out of the sort menu so both
    /// stay short enough to scan in one glance.
    fn layout_menu(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let button = widgets::icon_square(
            ui,
            Icon::Gallery,
            if lang == Lang::Pl {
                "Grupy i układ"
            } else {
                "Groups and layout"
            },
            widgets::TOOLBAR_HEIGHT,
        );
        let previous = (self.group_by, self.card_size);
        egui::Popup::menu(&button).show(|ui| {
            widgets::animate_menu(ui);
            ui.set_min_width(186.0);
            ui.set_max_width(212.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(t(lang, "group_by"))
                    .size(10.5)
                    .color(Palette::get(ui).muted),
            );
            for mode in GroupBy::ALL {
                let selected = self.group_by == mode;
                if widgets::menu_row(ui, Some(mode.icon()), mode.label(lang), selected).clicked()
                    && !selected
                {
                    self.group_by = mode;
                    self.reset_scroll = true;
                }
            }
            ui.separator();
            ui.label(
                RichText::new(t(lang, "card_size"))
                    .size(10.5)
                    .color(Palette::get(ui).muted),
            );
            for size in CardSize::ALL {
                let selected = self.card_size == size;
                if widgets::menu_row(ui, None, size.label(lang), selected).clicked() && !selected {
                    self.card_size = size;
                    self.reset_scroll = true;
                }
            }
        });
        if previous != (self.group_by, self.card_size) {
            self.sort_dirty = true;
        }
    }

    pub fn group_by(&self) -> GroupBy {
        self.group_by
    }

    pub fn card_size(&self) -> CardSize {
        self.card_size
    }

    /// Sort criterion and direction as plain numbers, for the settings file.
    pub fn sort_state(&self) -> (u8, bool) {
        (self.sort_by.index(), self.descending)
    }

    pub fn set_sort(&mut self, sort: u8, descending: bool) {
        let sort = SortBy::ALL.get(sort as usize).copied().unwrap_or_default();
        if self.sort_by != sort || self.descending != descending {
            self.sort_by = sort;
            self.descending = descending;
            self.sort_dirty = true;
            self.reset_scroll = true;
        }
    }

    pub fn set_layout(&mut self, group_by: GroupBy, card_size: CardSize) {
        if self.group_by != group_by || self.card_size != card_size {
            self.group_by = group_by;
            self.card_size = card_size;
            self.reset_scroll = true;
        }
    }

    fn sort_gallery(&mut self) {
        if !std::mem::take(&mut self.sort_dirty) {
            return;
        }
        for clips in self
            .groups
            .iter_mut()
            .map(|(_, clips)| clips)
            .chain([&mut self.imported])
        {
            let descending = self.descending;
            match self.sort_by {
                SortBy::Date => sort_clips(clips, descending, |clip| {
                    recording_datetime(clip)
                        .map(|date| date.and_utc().timestamp())
                        .or_else(|| {
                            chrono::Local
                                .timestamp_opt(clip.mtime as i64, 0)
                                .single()
                                .map(|date| date.naive_local().and_utc().timestamp())
                        })
                }),
                SortBy::Name => {
                    sort_clips(clips, descending, |clip| Some(clip.name().to_lowercase()))
                }
                SortBy::FileType => sort_clips(clips, descending, |clip| {
                    Some((
                        clip.path
                            .extension()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_ascii_lowercase(),
                        clip.name().to_lowercase(),
                    ))
                }),
                SortBy::Size => sort_clips(clips, descending, |clip| Some(clip.size)),
                SortBy::Duration => sort_clips(clips, descending, |clip| {
                    self.durations
                        .get(clip.path.to_string_lossy().as_ref())
                        .map(|seconds| (seconds * 1_000_000.0) as u64)
                }),
            }
        }
    }
    fn sections(&self, lang: Lang) -> Vec<Section<'_>> {
        use std::borrow::Cow;
        let query = self.search.trim().to_lowercase();
        let favorites_only = self.filter == Filter::Favorites;
        let output_only = self.filter == Filter::Output;
        let keep = |clip: &Clip, imported: bool| -> bool {
            let allowed = match &self.filter {
                Filter::All | Filter::Favorites | Filter::Output => true,
                Filter::Imported => imported,
                Filter::Group(active) => !imported && &clip.group == active,
            };
            allowed
                && (!favorites_only || self.library.favorites.contains(&clip.path))
                && (!output_only || self.output_paths.contains(&clip.path))
                && (query.is_empty() || clip.name().to_lowercase().contains(&query))
        };
        // Game order first, so the default grouping keeps the scanner's order.
        let mut entries: Vec<(bool, &Clip)> = Vec::new();
        for (_, clips) in &self.groups {
            entries.extend(
                clips
                    .iter()
                    .filter(|clip| keep(clip, false))
                    .map(|c| (false, c)),
            );
        }
        entries.extend(
            self.imported
                .iter()
                .filter(|clip| keep(clip, true))
                .map(|c| (true, c)),
        );
        if entries.is_empty() {
            return Vec::new();
        }
        if self.group_by == GroupBy::None {
            return vec![Section {
                name: Cow::Borrowed(""),
                clips: entries.into_iter().map(|(_, clip)| clip).collect(),
                imported: false,
                icon: None,
            }];
        }

        // name, is-imported, heading icon, clips
        let mut buckets: Vec<Bucket<'_, '_>> = Vec::new();
        for (imported, clip) in entries {
            let is_image = media::is_gallery_image(&clip.path);
            let (name, is_imported, icon) = match self.group_by {
                GroupBy::None => (Cow::Borrowed(""), false, None),
                GroupBy::Game => {
                    if imported {
                        (Cow::Borrowed(t(lang, "imported")), true, None)
                    } else if clip.group == OUTPUT_GROUP {
                        (
                            Cow::Borrowed(t(lang, "cutgut_output")),
                            false,
                            Some(Icon::Film),
                        )
                    } else {
                        (Cow::Borrowed(clip.group.as_str()), false, None)
                    }
                }
                GroupBy::Folder => (
                    Cow::Owned(self.folder_label(clip, lang)),
                    imported,
                    Some(if imported { Icon::Upload } else { Icon::Folder }),
                ),
                GroupBy::FileType => (
                    Cow::Borrowed(if is_image {
                        t(lang, "media_images")
                    } else {
                        t(lang, "media_videos")
                    }),
                    false,
                    Some(if is_image { Icon::Image } else { Icon::Film }),
                ),
                GroupBy::Date => (Cow::Owned(date_label(clip)), false, Some(Icon::Calendar)),
            };
            match buckets.iter_mut().find(|(known, _, _, _)| *known == name) {
                Some((_, _, _, clips)) => clips.push(clip),
                None => buckets.push((name, is_imported, icon, vec![clip])),
            }
        }
        // Dates read newest first, names alphabetically; games keep scan order.
        match self.group_by {
            GroupBy::Date => buckets.sort_by(|a, b| b.0.cmp(&a.0)),
            GroupBy::Folder | GroupBy::FileType => buckets.sort_by(|a, b| a.0.cmp(&b.0)),
            _ => {}
        }
        buckets
            .into_iter()
            .map(|(name, imported, icon, clips)| Section {
                name,
                clips,
                imported,
                icon,
            })
            .collect()
    }

    fn card(
        &self,
        ui: &mut egui::Ui,
        clip: &Clip,
        width: f32,
        height: f32,
        lang: Lang,
    ) -> (CardAction, bool) {
        let palette = Palette::get(ui);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
        let title = clip.name();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &title)
        });
        // Right-click: clip details plus the disk usage of its category.
        let category = if clip.group.is_empty() {
            t(lang, "imported").to_owned()
        } else if clip.group == OUTPUT_GROUP {
            t(lang, "cutgut_output").to_owned()
        } else {
            clip.group.clone()
        };
        let menu = clip_menu(&response, clip, &category, self, lang);
        let hovered = ui.ctx().animate_bool(
            response.id.with("card_hover"),
            response.hovered() || response.has_focus(),
        );
        let highlighted = self.highlight.as_ref() == Some(&clip.path);
        let dismiss_highlight = highlighted && response.hovered();
        let highlight_anim = ui.ctx().animate_bool_with_time(
            response.id.with("card_highlight"),
            highlighted && !dismiss_highlight,
            0.24,
        );
        let marked = self.is_marked(&clip.path);
        let painter = ui.painter().with_clip_rect(rect);
        let base = icons::lerp_color(palette.panel, palette.raised, hovered * 0.75);
        let base = icons::lerp_color(base, palette.accent_tint, highlight_anim * 0.55);
        let base = icons::lerp_color(base, palette.accent_tint, if marked { 0.55 } else { 0.0 });
        painter.rect_filled(rect, 6, base);
        let recorded = recording_date(clip);
        let date = recorded
            .clone()
            .unwrap_or_else(|| format_clip_date(clip.mtime));
        // Top strip: date on the left, duration in the middle, size on the
        // right (the duration sits between date and size, without separators).
        let font = FontId::proportional(11.0);
        let date_galley = painter.layout_no_wrap(date, font.clone(), palette.muted);
        let size = format!("{:.1} MB", clip.size as f64 / 1_000_000.0);
        let size_galley = painter.layout_no_wrap(size, font.clone(), palette.muted);
        let date_end = rect.left() + 5.0 + date_galley.size().x;
        let size_start = rect.right() - 5.0 - size_galley.size().x;
        let mid_y = rect.top() + 10.5;
        painter.galley(
            egui::pos2(rect.left() + 5.0, mid_y - date_galley.size().y * 0.5),
            date_galley,
            palette.muted,
        );
        painter.galley(
            egui::pos2(size_start, mid_y - size_galley.size().y * 0.5),
            size_galley,
            palette.muted,
        );
        if media::is_gallery_image(&clip.path) {
            let centre = (date_end + size_start) * 0.5;
            icons::paint(
                &painter,
                Icon::Image,
                Rect::from_center_size(egui::pos2(centre, mid_y), Vec2::splat(14.0)),
                palette.muted,
                1.15,
            );
        } else if let Some(duration) = self.durations.get(clip.path.to_string_lossy().as_ref()) {
            let duration_galley =
                painter.layout_no_wrap(format_duration(*duration), font, palette.muted);
            let centre = (date_end + size_start) * 0.5;
            painter.galley(
                egui::pos2(
                    centre - duration_galley.size().x * 0.5,
                    mid_y - duration_galley.size().y * 0.5,
                ),
                duration_galley,
                palette.muted,
            );
        }
        let image_rect =
            Rect::from_min_max(rect.left_top() + egui::vec2(0.0, 21.0), rect.right_bottom());
        let radius = CornerRadius {
            nw: 0,
            ne: 0,
            sw: 6,
            se: 6,
        };
        painter.rect_filled(image_rect, radius, Color32::from_rgb(30, 33, 36));
        let key = clip.path.to_string_lossy();
        if let Some(texture) = self.textures.get(key.as_ref()) {
            theme::rounded_image(
                &painter,
                image_rect,
                radius,
                texture.id(),
                theme::cover_uv(texture.size_vec2(), image_rect.size()),
            );
        } else {
            let icon_rect = Rect::from_center_size(image_rect.center(), Vec2::splat(29.0));
            icons::paint(
                &painter,
                if media::is_gallery_audio(&clip.path) {
                    Icon::Audio
                } else {
                    Icon::Film
                },
                icon_rect,
                Color32::from_gray(140),
                1.5,
            );
        }

        if recorded.is_none() {
            let label = Rect::from_min_size(image_rect.min, egui::vec2(width, 20.0));
            painter.rect_filled(label, 0, Color32::from_black_alpha(175));
            let stem = clip.path.file_stem().unwrap_or_default().to_string_lossy();
            let mut job = egui::text::LayoutJob::simple_singleline(
                stem.into_owned(),
                FontId::proportional(11.5),
                Color32::WHITE,
            );
            job.wrap.max_width = width - 10.0;
            job.wrap.max_rows = 1;
            let galley = painter.layout_job(job);
            painter.galley(
                label.min + egui::vec2(5.0, (20.0 - galley.size().y) * 0.5),
                galley,
                Color32::WHITE,
            );
        }

        if highlight_anim > 0.01 {
            let glow = ui.painter();
            let accent = if palette.dark {
                Color32::from_rgb(255, 209, 97)
            } else {
                Color32::from_rgb(144, 77, 0)
            };
            glow.rect_stroke(
                rect.expand(2.0),
                8,
                Stroke::new(2.0_f32, accent.gamma_multiply(0.34 * highlight_anim)),
                egui::StrokeKind::Inside,
            );
            glow.rect_stroke(
                rect,
                6,
                Stroke::new(3.0_f32, accent.gamma_multiply(highlight_anim)),
                egui::StrokeKind::Inside,
            );
        }
        if marked {
            let accent = if palette.dark {
                Color32::from_rgb(255, 209, 97)
            } else {
                Color32::from_rgb(168, 96, 0)
            };
            painter.rect_stroke(
                rect,
                6,
                Stroke::new(2.0_f32, accent),
                egui::StrokeKind::Inside,
            );
            // A tick in the corner says "included in the bulk action".
            let badge = Rect::from_center_size(
                egui::pos2(rect.right() - 15.0, image_rect.top() + 15.0),
                Vec2::splat(19.0),
            );
            painter.circle_filled(badge.center(), 9.5, accent);
            icons::paint(
                &painter,
                Icon::Check,
                Rect::from_center_size(badge.center(), Vec2::splat(12.0)),
                Color32::from_gray(24),
                1.9,
            );
        }
        let dragged_out = crate::drag_out::started(&response);
        let clicked = response.clicked();
        let modifiers = ui.input(|input| input.modifiers);
        response.on_hover_cursor(egui::CursorIcon::PointingHand);
        let action = if let Some(command) = menu {
            CardAction::Menu(command)
        } else if dragged_out {
            CardAction::DragOut
        } else if clicked {
            if modifiers.ctrl || modifiers.command {
                CardAction::Mark
            } else if modifiers.shift {
                CardAction::MarkRange
            } else {
                CardAction::Open
            }
        } else {
            CardAction::None
        };
        (action, dismiss_highlight)
    }

    fn open_clip(&mut self, clip: &Clip) {
        if self.preview_press.cancel() {
            self.playback_rate = PlaybackRate::Normal;
        }
        // Show the fullscreen controls again for every freshly opened clip.
        self.fullscreen_bar = None;
        // Opening the highlighted clip ends its highlight.
        if self.highlight.as_deref() == Some(clip.path.as_path()) {
            self.highlight = None;
            self.highlight_y = None;
            self.highlight_scroll = false;
        }
        self.stop_playback();
        self.still.clear();
        self.preview_tex = None;
        self.backdrop = None;
        self.position = 0.0;
        self.duration = 0.0;
        self.flash = None;
        self.selected_info = None;
        self.modal_opened_at = None;
        self.picture_rect = None;
        self.selected = Some(clip.clone());
        if media::is_gallery_image(&clip.path) {
            self.probe = None;
            self.play_on_probe = false;
            let source = clip.path.clone();
            let ctx = self.ctx.clone();
            let (tx, rx) = mpsc::channel();
            self.backdrop_rx = Some(rx);
            let max_side = ctx.input(|input| input.max_texture_side).min(4096) as u32;
            std::thread::spawn(move || {
                let _ = tx.send(load_gallery_image(&source, max_side));
                ctx.request_repaint();
            });
            return;
        }
        // Opening a clip starts playing right away, keeping the user's volume.
        self.play_on_probe = true;

        let source = clip.path.clone();
        let ctx = self.ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.probe = Some(rx);
        std::thread::spawn(move || {
            let result = if media::is_gallery_audio(&source) {
                media::probe_duration(&source)
                    .map(|duration| VideoInfo {
                        duration,
                        width: 0,
                        height: 0,
                        fps: 0.0,
                        bitrate: 0,
                        codec: String::new(),
                        audio_codec: None,
                        rotation: 0,
                        pix_fmt: String::new(),
                        color_transfer: String::new(),
                        audio_channels: 2,
                    })
                    .ok_or_else(|| "Could not read audio duration".to_owned())
            } else {
                media::probe_video(&source.to_string_lossy())
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        let cache = thumb_cache_path(&self.thumb_cache_dir, clip);
        let ctx = self.ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.backdrop_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(blurred_backdrop(&cache));
            ctx.request_repaint();
        });
    }

    fn toggle_play(&mut self) {
        if self.selected_info.is_none() {
            if self.probe.is_some() {
                self.play_on_probe = !self.play_on_probe;
            }
            return;
        }
        if self.playing {
            // Paused: brief (||) feedback.
            self.flash = Some((true, Instant::now()));
            self.stop_playback();
            return;
        }
        let from = if self.position >= self.duration - 0.1 {
            0.0
        } else {
            self.position
        };
        self.start_playback(from);
        // Resumed: brief (>) feedback.
        self.flash = Some((false, Instant::now()));
    }

    fn neighbors(&self, path: &Path, lang: Lang) -> (Option<Clip>, Option<Clip>) {
        let mut clips = self
            .sections(lang)
            .into_iter()
            .flat_map(|section| section.clips);
        let mut previous: Option<&Clip> = None;
        while let Some(clip) = clips.next() {
            if clip.path == path {
                return (previous.cloned(), clips.next().cloned());
            }
            previous = Some(clip);
        }
        (None, None)
    }

    fn modal(
        &mut self,
        ctx: &egui::Context,
        lang: Lang,
        pause_overlay: bool,
        blocked: bool,
        view: Rect,
    ) {
        let Some(clip) = self.selected.clone() else {
            return;
        };
        // Once the fly-back animation is over the panel can go.
        if let Some(at) = self.zoom.as_ref().and_then(|zoom| zoom.closing) {
            if self.ctx.input(|input| input.time) - at >= f64::from(ZOOM_BACK_TIME) {
                self.close_modal();
                return;
            }
        }
        let is_image = media::is_gallery_image(&clip.path);
        let is_audio = media::is_gallery_audio(&clip.path);
        let (mut previous, mut next) = self.neighbors(&clip.path, lang);
        let can_navigate = !blocked && self.rename.is_none() && self.delete.is_none();
        let mut navigate = None;
        let mut slide_dir = 0.0;
        if can_navigate && !ctx.wants_keyboard_input() && !egui::Popup::is_any_open(ctx) {
            let (left, right) = ctx.input_mut(|input| {
                (
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
                )
            });
            if left {
                navigate = previous.take();
                slide_dir = -1.0;
            } else if right {
                navigate = next.take();
                slide_dir = 1.0;
            }
            // Frame stepping lives here too: a cut is easier to judge on the big
            // preview than by opening the editor for one frame.
            let (back, forward, to_start, to_end) = ctx.input(|input| {
                let shift = input.modifiers.shift;
                (
                    input.key_pressed(egui::Key::Comma)
                        || input.key_pressed(egui::Key::OpenBracket)
                        || (shift && input.key_pressed(egui::Key::ArrowLeft)),
                    input.key_pressed(egui::Key::Period)
                        || input.key_pressed(egui::Key::CloseBracket)
                        || (shift && input.key_pressed(egui::Key::ArrowRight)),
                    input.key_pressed(egui::Key::Home),
                    input.key_pressed(egui::Key::End),
                )
            });
            if back {
                self.step_frame(-1);
            } else if forward {
                self.step_frame(1);
            } else if to_start {
                self.step_frame(i32::MIN / 2);
            } else if to_end {
                self.step_frame(i32::MAX / 2);
            }
        }
        // Fullscreen: the window itself goes fullscreen and the preview uses all
        // of it; there is no top bar, no outer gap and no rounded panel.
        let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
        let view = if fullscreen { ctx.content_rect() } else { view };
        // F / F11 toggle the window fullscreen, like in any video player.
        let hotkey = !ctx.wants_keyboard_input()
            && !egui::Popup::is_any_open(ctx)
            && ctx.input(|input| {
                input.key_pressed(egui::Key::F) || input.key_pressed(egui::Key::F11)
            });
        let key = clip.path.to_string_lossy().to_string();
        // The panel takes part in the zoom: it fades in while the thumbnail
        // grows out of its card, and fades away while the picture flies back.
        let now = ctx.input(|input| input.time);
        let zoom_fade = self.zoom.as_ref().map_or(1.0, |zoom| {
            match zoom.closing {
                // On the way out the panel holds its ground and only leaves at
                // the very end, when the thumbnail is already back in its card.
                Some(at) => {
                    // Everything except the returning picture gets out of the way
                    // in the first third of the animation.
                    let gone = (((now - at) as f32 / ZOOM_BACK_TIME) / 0.32).clamp(0.0, 1.0);
                    1.0 - gone * gone * (3.0 - 2.0 * gone)
                }
                None => {
                    let elapsed = ((now - zoom.opened_at) as f32 / ZOOM_TIME).clamp(0.0, 1.0);
                    1.0 - (1.0 - elapsed).powi(3)
                }
            }
        });
        let backdrop_alpha = ctx.animate_bool(
            egui::Id::new("backdrop_fade").with(&key),
            self.backdrop.is_some(),
        ) * zoom_fade;
        // The blur covers the gallery only, so the top bar stays sharp and usable.
        if let Some(texture) = &self.backdrop {
            egui::Area::new(egui::Id::new("preview_backdrop"))
                .order(egui::Order::Middle)
                .interactable(false)
                .fixed_pos(view.min)
                .show(ctx, |ui| {
                    ui.set_min_size(view.size());
                    ui.painter().image(
                        texture.id(),
                        view,
                        theme::cover_uv(texture.size_vec2(), view.size()),
                        Color32::WHITE.gamma_multiply(backdrop_alpha),
                    );
                });
        }
        // The preview fills the gallery area and only dims the blurred backdrop by
        // 20%, instead of being a small centred card.
        let outer_gap = if fullscreen { 0.0 } else { 8.0 };
        const HEADER_ROW: f32 = 48.0;
        /// Width of the black bar the picture keeps free on each side for the
        /// previous/next arrows, so they never cover the video itself.
        const ARROW_RESERVE: f32 = 50.0;
        const CONTROLS_ROW: f32 = 38.0;
        const ACTION_ROW: f32 = 50.0;
        const DIALOG_MARGIN: f32 = 5.0;
        let view = view.shrink(outer_gap);
        let dialog_margin = if fullscreen { 0.0 } else { DIALOG_MARGIN };
        let inner_height = (view.height() - 2.0 * dialog_margin).max(240.0);
        let mut close = false;
        let style = ctx.style();
        let frame = theme::dialog(&style);
        // The preview panel must be solid: a translucent fill lets the blurred
        // gallery show through the header strip and its rounded corners.
        // Its shadow is dropped as well - the area clips it, which leaves a
        // translucent rectangle hanging off the rounded corners.
        let frame = frame
            .fill(theme::opaque_window_fill(&style))
            .shadow(egui::epaint::Shadow::NONE);
        let frame = if fullscreen {
            // A fullscreen preview is the video and nothing else: no window
            // margin and no rounded corners around it.
            frame.inner_margin(0).corner_radius(0).stroke(Stroke::NONE)
        } else {
            frame
        };
        egui::Area::new(egui::Id::new("gallery_preview"))
            .order(egui::Order::Foreground)
            .fixed_pos(view.min)
            .show(ctx, |ui| {
                ui.set_min_size(view.size());
                ui.set_max_size(view.size());
                if zoom_fade < 1.0 {
                    ui.multiply_opacity(zoom_fade);
                }
                // 20% dim over the blurred screenshot behind the panel.
                ui.painter()
                    .rect_filled(view, 0, Color32::from_black_alpha(51));
                // Consumes clicks that would otherwise reach the gallery grid
                // underneath; whether they close the preview is decided below,
                // once the panel rect is known.
                let backdrop =
                    ui.interact(view, egui::Id::new("preview_backdrop_hit"), Sense::click());
                let panel = frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(inner_height);
                    // Size everything from the real content width so the picture is
                    // never inset inside the panel.
                    let width = ui.available_width();
                    let preview_height = if is_audio {
                        90.0
                    } else if fullscreen {
                        // Fullscreen: the picture owns the whole panel. Header,
                        // transport and actions float on top of it instead.
                        inner_height
                    } else {
                        (inner_height - HEADER_ROW - CONTROLS_ROW - ACTION_ROW - 42.0).max(150.0)
                    };
                    let palette = Palette::get(ui);
                    if !fullscreen {
                        // The buttons own the full height of the header row, so they
                        // read as targets rather than icons.
                        const HEADER_BUTTON: f32 = 44.0;
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.vertical(|ui| {
                                // Title and details sit close together: the gap is
                                // mostly wasted space above the picture.
                                ui.spacing_mut().item_spacing.y *= 0.3;
                                let reserved =
                                    3.0 * HEADER_BUTTON + 2.0 * ui.spacing().item_spacing.x + 18.0;
                                ui.set_max_width((width - reserved).max(80.0));
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(clip.name()).size(19.0).strong(),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(&key);
                                let date = recording_date(&clip)
                                    .unwrap_or_else(|| format_clip_date(clip.mtime));
                                let group = match clip.group.as_str() {
                                    "" => t(lang, "imported"),
                                    OUTPUT_GROUP => t(lang, "cutgut_output"),
                                    game => game,
                                };
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    ui.label(
                                        theme::mono(format!(
                                            "{date}  ·  {:.1} MB  ·",
                                            clip.size as f64 / 1_000_000.0
                                        ))
                                        .size(11.0)
                                        .color(palette.muted),
                                    );
                                    // The game's own icon, right before its name.
                                    if !matches!(clip.group.as_str(), "" | OUTPUT_GROUP) {
                                        let (rect, _) = ui
                                            .allocate_exact_size(Vec2::splat(13.0), Sense::hover());
                                        self.game_icons.paint(ui.painter(), &clip.group, rect);
                                    }
                                    ui.label(
                                        RichText::new(group)
                                            .size(11.0)
                                            .family(egui::FontFamily::Monospace)
                                            .color(palette.muted),
                                    );
                                });
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                close = self.preview_buttons(
                                    ui,
                                    lang,
                                    &clip,
                                    fullscreen,
                                    hotkey,
                                    HEADER_BUTTON,
                                );
                            });
                        });
                        ui.add_space(2.0);
                    }
                    let (preview_rect, preview_response) = ui.allocate_exact_size(
                        Vec2::new(width, preview_height),
                        Sense::click_and_drag(),
                    );
                    // Smooth "zoom in" of the picture: 0.90 -> 1.0 scale + fade.
                    let now = ctx.input(|input| input.time);
                    let opened = *self.modal_opened_at.get_or_insert(now);
                    let progress = (((now - opened) as f32) / 0.22).clamp(0.0, 1.0);
                    let eased = 1.0 - (1.0 - progress).powi(3);
                    if progress < 1.0 {
                        ctx.request_repaint();
                    }
                    let painter = ui.painter_at(preview_rect);
                    let corner = if fullscreen { 0 } else { 10 };
                    painter.rect_filled(preview_rect, corner, Color32::from_rgb(13, 15, 17));
                    // Reserve a gutter on both sides so the navigation arrows never
                    // sit on top of the picture. Only clips with a neighbour need it.
                    let gutter = if (previous.is_some() || next.is_some()) && !is_audio {
                        ARROW_RESERVE
                    } else {
                        0.0
                    };
                    let picture_area = preview_rect.shrink2(Vec2::new(gutter, 0.0));
                    // The fitted picture, used to place the navigation arrows inside
                    // the black bars beside it.
                    let mut picture_rect: Option<Rect> = None;
                    let texture = self
                        .preview_tex
                        .as_ref()
                        .or_else(|| self.textures.get(&key));
                    if is_audio {
                        icons::paint(
                            &painter,
                            Icon::Audio,
                            Rect::from_center_size(preview_rect.center(), Vec2::splat(32.0)),
                            Palette::get(ui).accent,
                            2.0,
                        );
                    } else if let Some(texture) = texture {
                        // A neighbour rides in from its side, fading as it lands.
                        let slide = self.slide_at.map_or(1.0, |start| {
                            (((now - start) as f32) / SLIDE_TIME).clamp(0.0, 1.0)
                        });
                        let slide_eased = 1.0 - (1.0 - slide).powi(3);
                        if slide < 1.0 {
                            ctx.request_repaint();
                        }
                        let slide_dx =
                            (1.0 - slide_eased) * picture_area.width() * 0.5 * self.slide_dir;
                        let size = texture.size_vec2();
                        let fit =
                            (picture_area.width() / size.x).min(picture_area.height() / size.y);
                        let zoom = if slide < 1.0 {
                            1.0
                        } else {
                            0.90 + 0.10 * eased
                        };
                        let image_rect =
                            Rect::from_center_size(picture_area.center(), size * fit * zoom);
                        let fade = eased * slide_eased;
                        let tint = Color32::from_white_alpha((255.0 * (0.55 + 0.45 * fade)) as u8);
                        picture_rect = Some(image_rect);
                        let image_rect = image_rect.translate(Vec2::new(slide_dx, 0.0));
                        theme::rounded_image_tinted(
                            &painter,
                            image_rect,
                            CornerRadius::same(if fullscreen { 0 } else { 9 }),
                            texture.id(),
                            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            tint,
                        );
                        self.picture_rect = Some(image_rect);
                    } else {
                        painter.text(
                            preview_rect.center(),
                            Align2::CENTER_CENTER,
                            t(
                                lang,
                                if self.probe.is_some() || self.still.pending {
                                    "loading"
                                } else {
                                    "media_missing"
                                },
                            ),
                            FontId::proportional(13.0),
                            Color32::from_gray(177),
                        );
                    }

                    if pause_overlay && !is_audio {
                        self.flash_overlay(ctx, &painter, preview_rect);
                    }
                    let play_label = t(lang, if self.playing { "pause" } else { "play" });
                    preview_response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            preview_response.enabled(),
                            play_label,
                        )
                    });
                    use widgets::PreviewAction;
                    // Big overlay arrows: the quickest way to flip through a gallery.
                    // They live in the letterbox bars when the aspect leaves any.
                    let arrow = Self::preview_nav_arrows(
                        ui,
                        preview_rect,
                        picture_rect.unwrap_or(preview_rect),
                        (can_navigate && previous.is_some()).then(|| t(lang, "previous_clip")),
                        (can_navigate && next.is_some()).then(|| t(lang, "next_clip")),
                    );
                    if arrow < 0 {
                        navigate = previous.take();
                        slide_dir = -1.0;
                    } else if arrow > 0 {
                        navigate = next.take();
                        slide_dir = 1.0;
                    }
                    let image = (!is_image && !is_audio && self.selected_info.is_some())
                        .then_some(preview_rect);
                    if arrow == 0 {
                        match self
                            .preview_press
                            .update(&preview_response, image, can_navigate)
                        {
                            PreviewAction::Drag => crate::drag_out::drag_file_out(&clip.path),
                            PreviewAction::Click if !is_image => self.toggle_play(),
                            PreviewAction::Hold(rate) => {
                                self.change_preview_rate(rate);
                                self.rate_osd = Some((rate, Instant::now()));
                                self.rate_osd_held = true;
                                if !self.playing {
                                    self.toggle_play();
                                }
                            }
                            PreviewAction::Release => {
                                self.rate_osd_held = false;
                                let shown = self.rate_osd.map(|(rate, _)| rate);
                                self.change_preview_rate(PlaybackRate::Normal);
                                if let Some(rate) = shown {
                                    self.rate_osd = Some((rate, Instant::now()));
                                }
                            }
                            _ => {}
                        }
                    }
                    if fullscreen {
                        // In fullscreen the header row is gone, so its buttons float
                        // over the top right corner of the video instead.
                        // The three buttons fill a 38 px tall capsule in the corner.
                        const FLOAT_BUTTON: f32 = 38.0;
                        let gap = ui.spacing().item_spacing.x;
                        let width = 3.0 * FLOAT_BUTTON + 2.0 * gap;
                        let row = Rect::from_min_size(
                            Pos2::new(
                                preview_rect.right() - width - 12.0,
                                preview_rect.top() + 12.0,
                            ),
                            Vec2::new(width, FLOAT_BUTTON),
                        );
                        ui.painter().rect_filled(
                            row.expand2(Vec2::new(6.0, 4.0)),
                            CornerRadius::same(10),
                            Color32::from_black_alpha(96),
                        );
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(row)
                                .layout(Layout::right_to_left(Align::Center)),
                            |ui| {
                                close |= self.preview_buttons(
                                    ui,
                                    lang,
                                    &clip,
                                    true,
                                    hotkey,
                                    FLOAT_BUTTON,
                                );
                            },
                        );
                    }
                    self.paint_rate_osd(ui, &painter, preview_rect);
                    self.paint_zoom(ctx, fullscreen);
                    preview_response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if fullscreen {
                        self.fullscreen_controls(ui, ctx, lang, preview_rect, is_image);
                    } else {
                        if !is_image {
                            self.controls_row(ui, lang, width);
                        }
                        ui.separator();
                        ui.add_space(2.0);
                        // One row of equal buttons that spans the whole panel, so
                        // the actions line up with the transport above them.
                        let gap = ui.spacing().item_spacing.x;
                        let button = ((width - 3.0 * gap) / 4.0).max(88.0);
                        // A still image has no "Edit" button, so the remaining
                        // three stay centred instead of hugging the left edge.
                        let count = if is_image || is_audio { 3.0 } else { 4.0 };
                        let inset =
                            ((width - (count * button + (count - 1.0) * gap)) * 0.5).max(0.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(inset);
                            if !is_image
                                && !is_audio
                                && widgets::icon_button(
                                    ui,
                                    Icon::Scissors,
                                    t(lang, "edit"),
                                    BtnKind::Primary,
                                    button,
                                )
                                .clicked()
                            {
                                // Straight into the editor: the fly-back animation
                                // would delay the switch for no reason here.
                                self.edit_request = Some(key.clone());
                                self.close_modal();
                            }
                            if widgets::icon_button(
                                ui,
                                Icon::Pencil,
                                t(lang, "rename"),
                                BtnKind::Secondary,
                                button,
                            )
                            .clicked()
                            {
                                let stem = Path::new(&key)
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned();
                                // Renaming must not interrupt playback.
                                self.rename = Some((key.clone(), stem));
                            }
                            if widgets::icon_button(
                                ui,
                                Icon::Folder,
                                t(lang, "open_folder"),
                                BtnKind::Secondary,
                                button,
                            )
                            .clicked()
                            {
                                reveal_path(&key);
                            }
                            if widgets::icon_button(
                                ui,
                                Icon::Trash,
                                t(lang, "delete"),
                                BtnKind::Danger,
                                button,
                            )
                            .clicked()
                            {
                                self.stop_playback();
                                self.delete = Some(vec![key.clone()]);
                            }
                        });
                        // Keep the action row off the panel edge.
                        ui.add_space(8.0);
                    }
                });
                // A click on the dimmed margin around the panel closes the preview;
                // clicks on the panel itself (padding included) belong to it.
                let pointer = ctx
                    .input(|input| input.pointer.interact_pos())
                    .unwrap_or(egui::Pos2::new(-1.0, -1.0));
                if backdrop.clicked() && !panel.response.rect.contains(pointer) {
                    close = true;
                }
            });
        let escape = ctx.input(|input| input.key_pressed(egui::Key::Escape));
        if escape && fullscreen && self.rename.is_none() && self.delete.is_none() {
            // First Escape gives the window frame back, the second closes the clip.
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            self.preview_fullscreen = false;
        } else if self.rename.is_none() && self.delete.is_none() && (close || escape) {
            self.begin_close();
        } else if let Some(clip) = navigate {
            // The next clip rides in from the right, the previous one from the left.
            self.slide_at = Some(ctx.input(|input| input.time));
            self.slide_dir = slide_dir;
            // A neighbour slides in; there is no card to grow out of.
            self.zoom = None;
            self.open_clip(&clip);
        }
    }

    /// Transport controls floating over the bottom of a fullscreen preview.
    ///
    /// They fade away while the pointer rests so the video stays unobstructed, and
    /// come back on any mouse movement, on a click or while the bar is hovered.
    fn fullscreen_controls(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        lang: Lang,
        area: Rect,
        is_image: bool,
    ) {
        if is_image {
            return;
        }
        let now = ctx.input(|input| input.time);
        let active =
            ctx.input(|input| input.pointer.delta() != Vec2::ZERO || input.pointer.any_click());
        if active || self.fullscreen_bar.is_none() {
            self.fullscreen_bar_shown = Some(now);
        }
        let last = *self.fullscreen_bar_shown.get_or_insert(now);
        let hovered = ctx
            .pointer_hover_pos()
            .is_some_and(|pos| self.fullscreen_bar.is_some_and(|bar| bar.contains(pos)));
        let visible = now - last < FULLSCREEN_BAR_IDLE || hovered || self.seeking;
        if visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
        let bar = Rect::from_min_max(
            Pos2::new(area.left(), area.bottom() - FULLSCREEN_BAR_HEIGHT),
            area.max,
        );
        self.fullscreen_bar = Some(bar);
        if !visible {
            return;
        }
        // A scrim keeps the controls readable over bright frames.
        let painter = ui.painter_at(bar);
        painter.rect_filled(bar, 0, Color32::from_black_alpha(132));
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(bar.left(), bar.top()),
                Vec2::new(bar.width(), 1.0),
            ),
            0,
            Color32::from_white_alpha(14),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bar.shrink2(Vec2::new(14.0, 7.0)))
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                self.controls_row(ui, lang, bar.width() - 60.0);
            },
        );
    }

    /// Close, fullscreen and favourite buttons of the preview header.
    ///
    /// In fullscreen they float over the top right corner of the video instead of
    /// sitting in a header row. Returns `true` when the close button was clicked.
    fn preview_buttons(
        &mut self,
        ui: &mut egui::Ui,
        lang: Lang,
        clip: &Clip,
        fullscreen: bool,
        hotkey: bool,
        size: f32,
    ) -> bool {
        let close = widgets::icon_square(ui, Icon::Close, t(lang, "close"), size).clicked();
        let label = t(
            lang,
            if fullscreen {
                "exit_fullscreen"
            } else {
                "fullscreen"
            },
        );
        let icon = if fullscreen {
            Icon::FullscreenExit
        } else {
            Icon::Fullscreen
        };
        if widgets::icon_square(ui, icon, label, size).clicked() || hotkey {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
            self.preview_fullscreen = !fullscreen;
        }
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
        if widgets::favorite(
            ui,
            rect,
            response,
            self.library.favorites.contains(&clip.path),
            lang,
            false,
        )
        .clicked()
        {
            self.toggle_favorite(&clip.path, lang);
        }
        close
    }

    /// Draws the previous/next clip arrows over the preview.
    ///
    /// Each arrow sits in the middle of the black bar beside the picture; when the
    /// clip is full-bleed (no bars) it is inset over the picture instead. Hover grows
    /// and lightens the circle with a short animation rather than snapping.
    /// Returns `-1` for "previous", `1` for "next" and `0` when nothing was clicked.
    fn preview_nav_arrows(
        ui: &mut egui::Ui,
        area: Rect,
        picture: Rect,
        previous: Option<&str>,
        next: Option<&str>,
    ) -> i8 {
        // Each black bar beside the picture is one big button: the chevron is just
        // a hint in the middle of it, so clicking anywhere in the bar navigates.
        let bar = (picture.left() - area.left())
            .min(area.right() - picture.right())
            .max(0.0);
        const BAR_MIN: f32 = 18.0;
        if bar < BAR_MIN {
            return 0;
        }
        let chevron = (bar * 0.50).clamp(14.0, 34.0);
        let painter = ui.painter_at(area);

        let mut clicked = 0;
        for (strip, icon, hint, delta, id) in [
            (
                Rect::from_min_max(area.left_top(), Pos2::new(picture.left(), area.bottom())),
                Icon::Previous,
                previous,
                -1,
                "preview_previous",
            ),
            (
                Rect::from_min_max(Pos2::new(picture.right(), area.top()), area.right_bottom()),
                Icon::Next,
                next,
                1,
                "preview_next",
            ),
        ] {
            let response = ui.interact(strip, egui::Id::new(id), Sense::click());
            let enabled = hint.is_some();
            let hovered = enabled && response.hovered();
            let glow = ui
                .ctx()
                .animate_bool_with_time(response.id.with("hover"), hovered, 0.12);
            if glow > 0.0 {
                // A faint sheen across the whole bar, so the target is obvious.
                painter.rect_filled(strip, 0, Color32::from_white_alpha((16.0 * glow) as u8));
            }
            let alpha = if enabled { 182.0 + 73.0 * glow } else { 62.0 };
            let center = Pos2::new(strip.center().x, area.center().y);
            // The chevron grows a touch and brightens under the pointer.
            let size = chevron * (1.0 + 0.10 * glow);
            icons::paint(
                &painter,
                icon,
                Rect::from_center_size(center, Vec2::splat(size)),
                Color32::from_white_alpha(alpha as u8),
                2.2,
            );
            if let Some(hint) = hint {
                let hit = response.clicked();
                response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(hint.to_owned());
                if hit {
                    clicked = delta;
                }
            }
        }
        clicked
    }

    /// Short feedback badge in the middle of the picture: (||) right after a
    /// pause, (>) right after a resume, nothing at all while it is playing.
    fn flash_overlay(&mut self, ctx: &egui::Context, painter: &egui::Painter, area: Rect) {
        let Some((paused, created)) = self.flash else {
            return;
        };
        const HOLD: f32 = 0.40;
        const FADE: f32 = 0.32;
        let elapsed = created.elapsed().as_secs_f32();
        if elapsed > HOLD + FADE {
            self.flash = None;
            return;
        }
        let alpha = if elapsed <= HOLD {
            1.0
        } else {
            1.0 - (elapsed - HOLD) / FADE
        };
        let center = area.center();
        let radius = (area.height() * 0.085).clamp(26.0, 36.0);
        painter.circle_filled(
            center,
            radius,
            Color32::from_black_alpha((150.0 * alpha) as u8),
        );
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(1.2_f32, Color32::from_white_alpha((85.0 * alpha) as u8)),
        );
        icons::paint(
            painter,
            if paused { Icon::Pause } else { Icon::Play },
            Rect::from_center_size(center, Vec2::splat(radius * 0.72)),
            Color32::from_white_alpha((238.0 * alpha) as u8),
            2.0,
        );
        ctx.request_repaint();
    }

    fn change_preview_rate(&mut self, rate: PlaybackRate) {
        // A running preview is only re-paced: restarting the decoders here is
        // what used to freeze the picture (and the sound) for half a second.
        if std::mem::replace(&mut self.playback_rate, rate) == rate {
            return;
        }
        if self.playing {
            self.player.set_rate(rate);
        }
        if rate == PlaybackRate::Normal {
            if !self.rate_osd_held {
                self.rate_osd = None;
            }
        } else {
            self.rate_osd = Some((rate, Instant::now()));
        }
    }

    /// Frame-accurate step in the gallery preview, so a cut can be inspected
    /// without opening the editor.
    fn step_frame(&mut self, delta: i32) {
        let Some(info) = self.selected_info.clone() else {
            return;
        };
        let fps = info.fps;
        if fps <= 0.0 {
            return;
        }
        if self.playing {
            self.stop_playback();
        }
        let step = 1.0 / fps;
        let last = (self.duration.max(step) - step).max(0.0) / step;
        let frame = (self.position / step).round() + f64::from(delta);
        self.position = frame.clamp(0.0, last.round()).max(0.0) * step;
        self.flash = None;
        self.request_frame();
        self.ctx.request_repaint();
    }

    /// Speed badge: slides down from the top of the picture, stays while the
    /// picture is held down, then fades. Half transparent so footage reads.
    fn paint_rate_osd(&mut self, ui: &egui::Ui, painter: &egui::Painter, area: Rect) {
        let Some((rate, since)) = self.rate_osd else {
            return;
        };
        const SLIDE: f32 = 0.15;
        const HOLD: f32 = 0.85;
        const FADE: f32 = 0.35;
        let elapsed = since.elapsed().as_secs_f32();
        if !self.rate_osd_held && elapsed > HOLD + FADE {
            self.rate_osd = None;
            return;
        }
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
        let size = galley.size() + Vec2::new(34.0, 14.0);
        let center = egui::pos2(
            area.center().x,
            area.top() + 10.0 + size.y * 0.5 - (1.0 - slide) * (size.y + 14.0),
        );
        let pill = Rect::from_center_size(center, size);
        if Rect::intersect(pill, area).height() > 1.0 {
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
    fn controls_row(&mut self, ui: &mut egui::Ui, lang: Lang, width: f32) {
        let palette = Palette::get(ui);
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            widgets::borderless_controls(ui);
            ui.spacing_mut().item_spacing.x = 9.0;
            let play_label = t(lang, if self.playing { "pause" } else { "play" });
            if widgets::icon_square(
                ui,
                if self.playing {
                    Icon::Pause
                } else {
                    Icon::Play
                },
                play_label,
                32.0,
            )
            .clicked()
            {
                self.toggle_play();
            }
            self.seek_bar(ui, (width - 400.0).max(90.0));
            widgets::playback_time(
                ui,
                format_time(self.position),
                format_time(self.duration),
                palette.muted,
            );
            let mut rate = self.playback_rate;
            if widgets::playback_speed(ui, &mut rate, lang) {
                self.change_preview_rate(rate);
            }
            if let Some(volume) = self.volume.show(ui, lang, 78.0) {
                self.player.set_volume(volume);
                self.volume_change = Some(volume);
            }
        });
        ui.add_space(3.0);
    }

    fn seek_bar(&mut self, ui: &mut egui::Ui, width: f32) {
        let palette = Palette::get(ui);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::click_and_drag());
        let hover = ui.ctx().animate_bool(
            response.id.with("seek_hover"),
            response.hovered() || self.seeking,
        );
        let painter = ui.painter();

        let duration = self.duration.max(0.1);
        let fraction = (self.position / duration).clamp(0.0, 1.0) as f32;
        let track =
            Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 5.0 + 1.5 * hover));
        painter.rect_filled(track, CornerRadius::same(4), palette.line);
        let filled = Rect::from_min_size(
            track.min,
            Vec2::new(track.width() * fraction, track.height()),
        );
        if fraction > 0.0 {
            painter.rect_filled(filled, CornerRadius::same(4), palette.accent);
        }
        let knob = Pos2::new(track.left() + track.width() * fraction, track.center().y);
        let knob_size = 9.0 + 3.0 * hover;
        painter.rect_filled(
            Rect::from_center_size(knob, Vec2::splat(knob_size)),
            CornerRadius::same(2),
            palette.accent,
        );

        let time_at = |x: f32| -> f64 {
            ((x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0) as f64 * duration
        };

        if response.clicked() || response.drag_started() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.seeking = true;
                self.seek_resume = self.playing;
                if self.playing {
                    self.position = self.current_position();
                    self.player.stop();
                    self.playing = false;
                }
                self.position = time_at(pointer.x);
                // Show the frame under the cursor while the user is still dragging.
                self.request_frame();
            }
        }
        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let target = time_at(pointer.x);
                // Only ask for a new still when the position actually moved a
                // frame's worth; the decoder coalesces the rest on its own.
                if (target - self.position).abs() >= 1.0 / 60.0 {
                    self.position = target;
                    self.request_frame();
                }
            }
        }
        if (response.drag_stopped() || response.clicked()) && self.seeking {
            self.seeking = false;
            let target = self.position;
            if self.seek_resume && target < self.duration - 0.015 {
                self.start_playback(target);
            } else {
                self.request_frame();
            }
            self.seek_resume = false;
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    fn rename_delete_modals(&mut self, ctx: &egui::Context, lang: Lang) {
        if let Some((_, buffer)) = self.rename.as_mut() {
            let mut confirm = false;
            let modal = egui::Modal::new(egui::Id::new("rename"))
                .frame(theme::dialog(&ctx.style()))
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    widgets::heading(ui, Icon::Pencil, t(lang, "rename"));
                    ui.add_space(4.0);
                    ui.label(RichText::new(t(lang, "new_name")).color(Palette::get(ui).muted));
                    let entry = ui.add(
                        egui::TextEdit::singleline(buffer)
                            .margin(egui::Margin::symmetric(9, 7))
                            .desired_width(ui.available_width()),
                    );
                    confirm =
                        entry.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    ui.add_space(10.0);
                    let gap = ui.spacing().item_spacing.x;
                    let button = (ui.available_width() - gap) * 0.5;
                    ui.horizontal(|ui| {
                        confirm |= widgets::icon_button(
                            ui,
                            Icon::Pencil,
                            t(lang, "save"),
                            BtnKind::Primary,
                            button,
                        )
                        .clicked();
                        if widgets::icon_button(
                            ui,
                            Icon::Close,
                            t(lang, "cancel"),
                            BtnKind::Secondary,
                            button,
                        )
                        .clicked()
                        {
                            ui.close();
                        }
                    });
                });
            if confirm {
                if let Some((path, name)) = self.rename.clone() {
                    match self.perform_rename(&path, &name, lang) {
                        Ok(()) => self.rename = None,
                        Err(key) => self.notice = Some(t(lang, key).to_owned()),
                    }
                }
            } else if modal.should_close() {
                self.rename = None;
            }
        }

        if let Some(paths) = self.delete.clone() {
            let mut confirm = false;
            let modal = egui::Modal::new(egui::Id::new("delete"))
                .frame(theme::dialog(&ctx.style()))
                .show(ctx, |ui| {
                    ui.set_width(392.0);
                    let palette = Palette::get(ui);
                    let title = if paths.len() > 1 {
                        t(lang, "confirm_delete_many")
                    } else {
                        t(lang, "confirm_delete")
                    };
                    widgets::heading(ui, Icon::Trash, title);
                    ui.add_space(4.0);
                    // The files themselves get their own card, so it is obvious
                    // what the dialog is about even when several clips are selected.
                    let clips: Vec<Clip> = self
                        .all_clips()
                        .filter(|clip| paths.iter().any(|path| clip.path == Path::new(path)))
                        .cloned()
                        .collect();
                    egui::Frame::new()
                        .fill(palette.well)
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(11, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            for (index, path) in paths.iter().take(4).enumerate() {
                                if index > 0 {
                                    ui.add_space(3.0);
                                }
                                let name = Path::new(path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned();
                                ui.add(egui::Label::new(RichText::new(name).strong()).truncate());
                            }
                            if paths.len() > 4 {
                                ui.add_space(3.0);
                                ui.label(
                                    RichText::new(format!("+{}", paths.len() - 4))
                                        .size(11.0)
                                        .color(palette.muted),
                                );
                            }
                            let total: u64 = clips.iter().map(|clip| clip.size).sum();
                            ui.add_space(3.0);
                            ui.label(
                                theme::mono(format!(
                                    "{}  ·  {:.1} MB",
                                    if paths.len() > 1 {
                                        format!("{} {}", paths.len(), t(lang, "clips_short"))
                                    } else {
                                        clips.first().and_then(recording_date).unwrap_or_default()
                                    },
                                    total as f64 / 1_000_000.0
                                ))
                                .size(11.0)
                                .color(palette.muted),
                            );
                        });
                    ui.add_space(9.0);
                    ui.label(
                        RichText::new(t(lang, "delete_warning"))
                            .size(12.0)
                            .color(palette.muted),
                    );
                    ui.add_space(11.0);
                    let gap = ui.spacing().item_spacing.x;
                    let button = (ui.available_width() - gap) * 0.5;
                    ui.horizontal(|ui| {
                        confirm = widgets::icon_button(
                            ui,
                            Icon::Trash,
                            t(lang, "yes"),
                            BtnKind::Danger,
                            button,
                        )
                        .clicked();
                        if widgets::icon_button(
                            ui,
                            Icon::Check,
                            t(lang, "keep_clip"),
                            BtnKind::Secondary,
                            button,
                        )
                        .clicked()
                        {
                            ui.close();
                        }
                    });
                });
            if confirm {
                self.delete = None;
                // If the open clip is among them, the preview should not vanish:
                // it slides to the neighbour the user would have seen next.
                let land = self
                    .selected
                    .as_ref()
                    .filter(|clip| paths.iter().any(|path| Path::new(path) == clip.path))
                    .map(|clip| {
                        let (previous, next) = self.neighbors(&clip.path, lang);
                        previous.or(next)
                    });
                // Release the decoder first: Windows cannot move a file that is
                // still open by FFmpeg.
                let was_playing = self.playing;
                let resume_at = self.current_position();
                self.stop_playback();
                let mut library = self.library.clone();
                let mut failed = false;
                for path in &paths {
                    if ffmpeg::recycle_path(path) {
                        library.replace_path(Path::new(path), None);
                    } else {
                        failed = true;
                    }
                }
                self.store_library(library, lang);
                self.marked
                    .retain(|marked| !paths.iter().any(|path| marked == Path::new(path)));
                if failed {
                    if was_playing {
                        self.start_playback(resume_at);
                    }
                    self.notice = Some(t(lang, "delete_failed").to_owned());
                }
                self.reload();
                if !failed {
                    if let Some(Some(clip)) = land {
                        // The neighbour rides in from the left, like a step back.
                        self.slide_at = Some(self.ctx.input(|input| input.time));
                        self.slide_dir = -1.0;
                        self.open_clip(&clip);
                    }
                }
            } else if modal.should_close() {
                self.delete = None;
            }
        }
    }

    fn all_clips(&self) -> impl Iterator<Item = &Clip> {
        self.groups
            .iter()
            .flat_map(|(_, clips)| clips)
            .chain(&self.imported)
    }

    /// Disk usage per category (game / imported), heaviest first.
    pub fn usage(&self, lang: Lang) -> Vec<CategoryUsage> {
        let mut usage: Vec<CategoryUsage> = self
            .groups
            .iter()
            .map(|(name, clips)| CategoryUsage {
                name: if name == OUTPUT_GROUP {
                    t(lang, "cutgut_output").to_owned()
                } else {
                    name.clone()
                },
                size: clips.iter().map(|clip| clip.size).sum(),
                count: clips.len(),
                color: self
                    .game_icons
                    .color(name)
                    .unwrap_or_else(|| category_color(name)),
                game: Some(name.clone()),
            })
            .collect();
        if !self.imported.is_empty() {
            let label = t(lang, "imported").to_owned();
            usage.push(CategoryUsage {
                name: label.clone(),
                size: self.imported.iter().map(|clip| clip.size).sum(),
                count: self.imported.len(),
                color: category_color(&label),
                game: None,
            });
        }
        usage.sort_by(|a, b| b.size.cmp(&a.size));
        usage
    }

    /// Total number of clips, their weight and the heaviest single clip.
    fn category_stats(&self, filter: &Filter) -> (usize, u64, u64) {
        let clips: Vec<&Clip> = match filter {
            Filter::All => self.all_clips().collect(),
            Filter::Favorites => self
                .all_clips()
                .filter(|clip| self.library.favorites.contains(&clip.path))
                .collect(),
            Filter::Output => self
                .all_clips()
                .filter(|clip| self.output_paths.contains(&clip.path))
                .collect(),
            Filter::Imported => self.imported.iter().collect(),
            Filter::Group(name) => self
                .groups
                .iter()
                .find(|(group, _)| group == name)
                .map(|(_, clips)| clips.iter().collect())
                .unwrap_or_default(),
        };
        let total = clips.iter().map(|clip| clip.size).sum();
        let largest = clips.iter().map(|clip| clip.size).max().unwrap_or(0);
        (clips.len(), total, largest)
    }

    fn total_bytes(&self) -> u64 {
        self.all_clips().map(|clip| clip.size).sum()
    }

    /// Paints a game/category icon (used by the storage bar legend).
    pub fn paint_game_icon(&self, painter: &egui::Painter, name: &str, rect: Rect) {
        if name == OUTPUT_GROUP {
            icons::paint(
                painter,
                Icon::Folder,
                rect,
                self.ctx.style().visuals.text_color(),
                1.4,
            );
        } else {
            self.game_icons.paint(painter, name, rect);
        }
    }

    /// `YYYY-MM-DD` of the recording behind `path`, falling back to today.
    ///
    /// Exports inherit this date so a clip cut from a 2025 recording is still
    /// dated 2025 instead of the day the cut happened.
    pub fn recording_day(&self, path: &Path) -> String {
        if let Some(clip) = self.all_clips().find(|clip| clip.path.as_path() == path) {
            if let Some(date) = recording_datetime(clip) {
                return date.format("%Y-%m-%d").to_string();
            }
            if let Some(date) = chrono::Local
                .timestamp_opt(clip.mtime as i64, 0)
                .single()
                .map(|date| date.naive_local())
            {
                return date.format("%Y-%m-%d").to_string();
            }
        }
        media::today_string()
    }

    /// Filter to switch to when a game chip is clicked.
    ///
    /// The chips behave like a toggle: clicking the active one returns to whatever
    /// was selected before (All, Favorites, ...) instead of trapping the user in
    /// the category.
    fn chip_filter(&self, name: &str, selected: bool) -> Filter {
        if selected {
            self.filter_before_group.clone()
        } else {
            Filter::Group(name.to_owned())
        }
    }

    pub fn poll_usage(&mut self, dirs: Vec<String>, ctx: &egui::Context) {
        self.set_dirs(dirs);
        self.poll_scan();
        self.game_icons.poll(ctx);
    }

    fn perform_rename(
        &mut self,
        old: &str,
        new_name: &str,
        lang: Lang,
    ) -> Result<(), &'static str> {
        let old_path = Path::new(old);
        let name = new_name.trim();
        // Reject names Windows can never create (`* ? " < > |`, `CON`, `NUL`,
        // trailing dot...) before touching the filesystem: retrying those would
        // just freeze the UI for nothing.
        if media::check_file_name(name).is_err() {
            return Err("invalid_name");
        }
        let requested = Path::new(name);
        let rename = Path::new(name);
        let explicit_extension = media::is_video_file(name)
            || media::is_gallery_audio(rename)
            || media::is_gallery_image(rename);
        let (stem, extension) = if explicit_extension {
            (
                requested
                    .file_stem()
                    .unwrap_or(requested.as_os_str())
                    .to_string_lossy(),
                requested.extension(),
            )
        } else {
            (name.into(), old_path.extension())
        };
        let suffix = extension
            .map(|ext| format!(".{}", ext.to_string_lossy()))
            .unwrap_or_default();
        let mut target = old_path.with_file_name(format!("{stem}{suffix}"));
        if target == old_path {
            return Ok(());
        }
        let mut counter = 2;
        while target.exists() {
            target = old_path.with_file_name(format!("{stem} ({counter}){suffix}"));
            counter += 1;
        }

        // Windows refuses to rename a file that FFmpeg still has open, so the
        // decoder is released for the few milliseconds the rename needs and
        // playback resumes at the same position right afterwards. Nothing else
        // about the open clip is torn down.
        let resume_at = self.current_position();
        let was_playing = self.playing;
        if was_playing {
            self.player.stop();
            self.playing = false;
        }
        let mut renamed = false;
        for _ in 0..RENAME_RETRIES {
            if std::fs::rename(old_path, &target).is_ok() {
                renamed = true;
                break;
            }
            std::thread::sleep(RENAME_RETRY_DELAY);
        }
        if !renamed {
            if was_playing {
                self.start_playback(resume_at);
            }
            return Err("rename_failed");
        }

        let mut library = self.library.clone();
        library.replace_path(old_path, Some(&target));
        if !self.store_library(library.clone(), lang) {
            // Keep the in-memory gallery pointed at the renamed file if the preference write failed.
            self.library = library;
        }
        // Update the open view in place instead of reloading the whole gallery:
        // the preview modal, the playing clip and its thumbnail stay intact.
        self.apply_renamed_path(old_path, &target);
        if was_playing {
            self.start_playback(resume_at);
        } else {
            self.request_frame();
        }
        Ok(())
    }

    /// Retargets the in-memory gallery (groups, imported clips, textures and the
    /// open clip) after a file was renamed on disk.
    fn apply_renamed_path(&mut self, old: &Path, new: &Path) {
        self.sort_dirty = true;
        let target = new.to_path_buf();
        let mut retarget = |clip: &mut Clip| {
            if clip.path == old {
                clip.path = target.clone();
            }
        };
        for (_, clips) in &mut self.groups {
            clips.iter_mut().for_each(&mut retarget);
        }
        self.imported.iter_mut().for_each(&mut retarget);
        if let Some(clip) = self.selected.as_mut() {
            retarget(clip);
        }
        if let Some(highlight) = self.highlight.as_mut() {
            if highlight == old {
                *highlight = target.clone();
            }
        }
        let old_key = old.to_string_lossy().to_string();
        let new_key = new.to_string_lossy().to_string();
        if self.output_paths.remove(old) {
            self.output_paths.insert(new.to_path_buf());
        }
        if let Some(version) = self.thumb_versions.remove(&old_key) {
            self.thumb_versions.insert(new_key.clone(), version);
        }
        if let Some(texture) = self.textures.remove(&old_key) {
            self.textures.insert(new_key.clone(), texture);
            // Keep the eviction order pointing at the texture under its new name.
            if let Some(entry) = self.texture_order.iter_mut().find(|key| **key == old_key) {
                *entry = new_key.clone();
            } else {
                self.texture_order.push_back(new_key.clone());
            }
        }
        if let Some(duration) = self.durations.remove(&old_key) {
            self.durations.insert(new_key.clone(), duration);
        }
        if let Some(version) = self.duration_versions.remove(&old_key) {
            self.duration_versions.insert(new_key.clone(), version);
        }
        if let Some(entry) = self.duration_cache.remove(old) {
            self.duration_cache.insert(new.to_path_buf(), entry);
            self.duration_cache_dirty = true;
        }
        if self.scan.is_some() {
            self.start_scan();
        }
    }
}

type Bucket<'a, 'b> = (std::borrow::Cow<'a, str>, bool, Option<Icon>, Vec<&'b Clip>);

struct Section<'a> {
    name: std::borrow::Cow<'a, str>,
    clips: Vec<&'a Clip>,
    imported: bool,
    /// Generic heading icon; `None` means "use the game icon of `name`".
    icon: Option<Icon>,
}

const OUTPUT_GROUP: &str = "\0output";

/// Windows briefly refuses to rename a file that a decoder still holds open, so
/// the rename is retried — but only for a bounded time (~250 ms), because this
/// runs on the UI thread.
const RENAME_RETRIES: usize = 10;
const RENAME_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(25);

/// One gallery category (game / imported) with the space it occupies.
pub struct CategoryUsage {
    pub name: String,
    pub size: u64,
    pub count: usize,
    pub color: Color32,
    /// Game name for the icon; `None` for synthetic categories.
    pub game: Option<String>,
}

const CATEGORY_COLORS: [Color32; 12] = [
    Color32::from_rgb(96, 205, 255),
    Color32::from_rgb(122, 219, 158),
    Color32::from_rgb(255, 178, 92),
    Color32::from_rgb(232, 140, 200),
    Color32::from_rgb(174, 152, 255),
    Color32::from_rgb(255, 122, 122),
    Color32::from_rgb(140, 212, 232),
    Color32::from_rgb(202, 206, 122),
    Color32::from_rgb(255, 150, 80),
    Color32::from_rgb(132, 192, 122),
    Color32::from_rgb(196, 164, 124),
    Color32::from_rgb(152, 164, 196),
];

/// Stable, theme-independent colour for a category/name.
pub fn category_color(name: &str) -> Color32 {
    let hash = name.bytes().fold(7usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(usize::from(byte))
    });
    CATEGORY_COLORS[hash % CATEGORY_COLORS.len()]
}

fn format_bytes(size: u64) -> String {
    let mb = size as f64 / 1_000_000.0;
    if mb >= 1_000.0 {
        format!("{:.2} GB", mb / 1_000.0)
    } else {
        format!("{mb:.1} MB")
    }
}

fn usage_row(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.5));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::mono(value).size(11.0).color(color));
        });
    });
}

/// Right-click menu with the disk usage of one category (and of the clicked clip).
fn usage_menu(
    response: &egui::Response,
    title: &str,
    usage: impl FnOnce() -> ((usize, u64, u64), u64),
    lang: Lang,
) {
    egui::Popup::context_menu(response).width(212.0).show(|ui| {
        let (stats, all_bytes) = usage();
        let palette = Palette::get(ui);
        ui.set_width(202.0);
        ui.spacing_mut().item_spacing.y = 3.0;
        widgets::animate_menu(ui);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
            ui.painter().rect_filled(rect, 2, category_color(title));
            ui.add_space(2.0);
            ui.label(RichText::new(title).size(12.0).strong());
        });
        ui.separator();
        usage_row(
            ui,
            t(lang, "clips_count"),
            stats.0.to_string(),
            palette.muted,
        );
        usage_row(
            ui,
            t(lang, "total_size"),
            format_bytes(stats.1),
            palette.text,
        );
        usage_row(
            ui,
            t(lang, "largest_clip"),
            format_bytes(stats.2),
            palette.text,
        );
        let share = if all_bytes > 0 {
            stats.1 as f64 / all_bytes as f64 * 100.0
        } else {
            0.0
        };
        usage_row(
            ui,
            t(lang, "share"),
            format!("{share:.1} %"),
            palette.accent,
        );
    });
}

/// Right-click on a card: everything about that one clip, nothing else.
/// Actions offered by the right-click menu of a card.
#[derive(Clone, Copy, PartialEq)]
enum ClipCommand {
    Preview,
    Edit,
    ShowInFolder,
    CopyPath,
    CopyFile,
    ToggleFavorite,
    Delete,
}

fn clip_menu(
    response: &egui::Response,
    clip: &Clip,
    category: &str,
    gallery: &Gallery,
    lang: Lang,
) -> Option<ClipCommand> {
    let mut chosen = None;
    egui::Popup::context_menu(response).width(244.0).show(|ui| {
        chosen = clip_menu_body(ui, clip, category, gallery, lang);
    });
    chosen
}

/// Compact clip sheet: file name, date and size, then the action rows.
fn clip_menu_body(
    ui: &mut egui::Ui,
    clip: &Clip,
    category: &str,
    gallery: &Gallery,
    lang: Lang,
) -> Option<ClipCommand> {
    let palette = Palette::get(ui);
    let mut chosen = None;
    // Everything in the menu shares one left edge, and that edge sits close to
    // the popup border so the sheet stays compact.
    const INSET: f32 = 6.0;
    ui.set_width(228.0);
    ui.spacing_mut().item_spacing.y = 2.0;
    widgets::animate_menu(ui);
    let mono = |size: f32, color: Color32| egui::TextFormat {
        font_id: egui::FontId::monospace(size),
        color,
        ..Default::default()
    };
    let caption = |size: f32| mono(size, palette.muted.gamma_multiply(0.9));
    // A label/value pair painted in two tones, so the values pop while the
    // captions stay quiet.
    let field = |job: &mut egui::text::LayoutJob, name: &str, value: &str| {
        job.append(&format!("{name}: "), 0.0, caption(9.5));
        job.append(value, 0.0, mono(10.0, palette.text));
    };
    let pipe = |job: &mut egui::text::LayoutJob| {
        job.append("   |   ", 0.0, mono(9.5, palette.muted.gamma_multiply(0.6)));
    };
    ui.horizontal(|ui| {
        ui.add_space(INSET);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            // The extension is gone: the format has its own row below.
            let stem = clip
                .path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| clip.name());
            ui.add(egui::Label::new(RichText::new(stem).size(12.5).strong()).truncate());
            let recorded = recording_date(clip).unwrap_or_else(|| format_clip_date(clip.mtime));
            let mut job = egui::text::LayoutJob::default();
            field(&mut job, t(lang, "date_label"), &recorded);
            pipe(&mut job);
            field(&mut job, t(lang, "size_label"), &format_bytes(clip.size));
            ui.label(job);
            let key = clip.path.to_string_lossy().into_owned();
            let length = gallery
                .durations
                .get(key.as_str())
                .map(|seconds| format_duration(*seconds))
                .unwrap_or_else(|| "—".to_owned());
            let mut job = egui::text::LayoutJob::default();
            field(&mut job, t(lang, "duration_label"), &length);
            pipe(&mut job);
            field(&mut job, t(lang, "format_label"), &clip_kind(clip, lang));
            ui.label(job);
            let folder = clip
                .path
                .parent()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            ui.add(
                egui::Label::new(
                    RichText::new(folder.as_str())
                        .size(9.5)
                        .color(palette.muted.gamma_multiply(0.8)),
                )
                .truncate(),
            )
            .on_hover_text(folder);
            // The game sits closest to the actions, as the last piece of context.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (slot, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
                gallery.paint_game_icon(ui.painter(), category, slot);
                ui.label(
                    RichText::new(category)
                        .size(10.0)
                        .family(egui::FontFamily::Monospace)
                        .color(palette.muted),
                );
            });
        });
    });
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(1.0);
    let favorite = gallery.library.favorites.contains(&clip.path);
    let pink = if palette.dark {
        crate::theme::PINK
    } else {
        crate::theme::PINK_LIGHT
    };
    let never = None;
    for (command, icon, label, tint) in [
        (
            ClipCommand::Preview,
            Icon::Play,
            t(lang, "preview_clip"),
            Some(palette.good),
        ),
        (
            ClipCommand::Edit,
            Icon::Scissors,
            t(lang, "edit_clip"),
            Some(crate::theme::GOLD),
        ),
        (
            ClipCommand::ShowInFolder,
            Icon::Folder,
            t(lang, "open_folder"),
            never,
        ),
        (
            ClipCommand::CopyPath,
            Icon::Text,
            t(lang, "copy_path"),
            never,
        ),
        (
            ClipCommand::CopyFile,
            Icon::Copy,
            t(lang, "copy_file"),
            never,
        ),
        (
            ClipCommand::ToggleFavorite,
            // A filled heart means "click to remove", an empty one "click to add".
            if favorite {
                Icon::Heart
            } else {
                Icon::HeartOutline
            },
            t(
                lang,
                if favorite {
                    "remove_favorite"
                } else {
                    "add_favorite"
                },
            ),
            Some(pink),
        ),
        (
            ClipCommand::Delete,
            Icon::Trash,
            t(lang, "delete"),
            Some(palette.danger),
        ),
    ] {
        if widgets::menu_row_sized(ui, Some(icon), label, false, tint, 32.0, 5.0).clicked() {
            chosen = Some(command);
            ui.close();
        }
    }
    chosen
}

/// Runs a right-click menu command on the clip it was opened for.
impl Gallery {
    fn run_clip_command(&mut self, command: ClipCommand, clip: &Clip, lang: Lang) {
        match command {
            ClipCommand::Preview => {
                self.marked.clear();
                self.open_clip(clip);
            }
            ClipCommand::Edit => {
                self.stop_playback();
                self.edit_request = Some(clip.path.to_string_lossy().into_owned());
            }
            ClipCommand::ShowInFolder => open_folder(&clip.path.to_string_lossy()),
            ClipCommand::CopyPath => {
                self.ctx.copy_text(clip.path.to_string_lossy().into_owned());
                self.notice = Some(t(lang, "path_copied").to_owned());
            }
            ClipCommand::CopyFile => {
                if crate::drag_out::copy_file_to_clipboard(&clip.path) {
                    self.notice = Some(t(lang, "file_copied").to_owned());
                } else {
                    self.notice = Some(t(lang, "copy_failed").to_owned());
                }
            }
            ClipCommand::ToggleFavorite => self.toggle_favorite(&clip.path, lang),
            ClipCommand::Delete => {
                self.stop_playback();
                self.delete = Some(vec![clip.path.to_string_lossy().into_owned()]);
            }
        }
    }
}

/// Short human description of the file, e.g. "MP4 (Video)" or "PNG (Image)".
fn clip_kind(clip: &Clip, lang: Lang) -> String {
    let extension = clip
        .path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_uppercase();
    let kind = if media::is_gallery_image(&clip.path) {
        t(lang, "kind_image")
    } else {
        t(lang, "kind_video")
    };
    if extension.is_empty() {
        kind.to_owned()
    } else {
        format!("{extension} ({kind})")
    }
}

fn dated_name(stem: &str) -> Option<(&str, chrono::NaiveDateTime)> {
    let stem = stem.strip_suffix(".DVR").unwrap_or(stem);
    let (prefix, time) = stem.rsplit_once(" - ")?;
    let (name, date) = prefix.rsplit_once(' ')?;
    let mut clock = time.split('.');
    let hours = clock.next()?;
    let minutes = clock.next()?;
    let seconds = clock.next()?;
    if let Some(frames) = clock.next() {
        if frames.len() != 2 || !frames.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
    }
    if clock.next().is_some() {
        return None;
    }
    let timestamp = chrono::NaiveDateTime::parse_from_str(
        &format!("{date} {hours}:{minutes}:{seconds}"),
        "%Y.%m.%d %H:%M:%S",
    )
    .ok()?;
    Some((name, timestamp))
}

fn original_export_name(stem: &str) -> &str {
    if let Some((base, _)) = stem.rsplit_once(" - CG ") {
        return base;
    }
    if let Some(legacy) = stem.strip_prefix("CutGut_") {
        if let Some((base, _)) = legacy.rsplit_once(".DVR_") {
            return base;
        }
    }
    stem
}

fn recording_date(clip: &Clip) -> Option<String> {
    recording_datetime(clip).map(|date| date.format("%Y-%m-%d %H:%M").to_string())
}

fn recording_datetime(clip: &Clip) -> Option<chrono::NaiveDateTime> {
    let stem = clip.path.file_stem()?.to_str()?;
    let (name, date) = dated_name(original_export_name(stem))?;
    let folder = clip.path.parent()?.file_name()?.to_str()?;
    if game_icons::game_name(name).eq_ignore_ascii_case(&clip.group)
        || name.eq_ignore_ascii_case(folder)
    {
        Some(date)
    } else {
        None
    }
}

impl Drop for Gallery {
    fn drop(&mut self) {
        // Closing the window must stop the scan workers instead of letting them
        // finish probing a gallery nobody will see.
        self.scan_cancel.store(true, Ordering::Relaxed);
    }
}

fn scan_gallery(
    roots: &[PathBuf],
    output: &Path,
    export_games: &BTreeMap<PathBuf, String>,
    cache_dir: &Path,
    cancel: &AtomicBool,
    mut publish: impl FnMut(GalleryScan, bool) -> bool,
) {
    // Several folders merge into one library; categories are keyed by game name,
    // so clips from different roots simply land in the same group.
    let mut groups: Vec<(String, Vec<Clip>)> = Vec::new();
    for root in roots {
        for (name, clips) in media::scan_gallery_media(root) {
            match groups.iter_mut().find(|(known, _)| *known == name) {
                Some((_, existing)) => existing.extend(clips),
                None => groups.push((name, clips)),
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return;
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return;
    }
    let exports = if output.as_os_str().is_empty() {
        Vec::new()
    } else {
        media::scan_gallery_media(output)
    };
    let mut hasher = DefaultHasher::new();
    output.hash(&mut hasher);
    let cache_file = cache_dir.join(format!("games-{:016x}.json", hasher.finish()));
    let mut cache: GameCache = std::fs::read(&cache_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let missing: Vec<_> = exports
        .iter()
        .flat_map(|(_, clips)| clips)
        .filter(|clip| {
            !media::is_gallery_image(&clip.path)
                && !cache
                    .get(&clip.path)
                    .is_some_and(|(mtime, size, _)| (*mtime, *size) == (clip.mtime, clip.size))
        })
        .collect();

    let first = scan_with_output(
        groups.clone(),
        exports.clone(),
        output,
        export_games,
        &cache,
    );
    if !publish(first, missing.is_empty()) || missing.is_empty() {
        return;
    }
    let mut found = Vec::new();
    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        let workers = 4;
        for i in 0..workers {
            let tx = tx.clone();
            let missing = &missing;
            scope.spawn(move || {
                for clip in missing.iter().skip(i).step_by(workers) {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(game) = media::probe_cutgut_game(&clip.path) {
                        if tx
                            .send((clip.path.clone(), (clip.mtime, clip.size, game)))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            });
        }
        drop(tx);
        found.extend(rx);
    });
    cache.extend(found);
    // A superseded scan must not write its (now irrelevant) cache file.
    if cancel.load(Ordering::Relaxed) {
        return;
    }
    cache.retain(|path, _| {
        exports
            .iter()
            .flat_map(|(_, clips)| clips)
            .any(|clip| &clip.path == path)
    });
    use std::io::Write as _;
    if std::fs::create_dir_all(cache_dir).is_ok() {
        if let Ok(mut file) = tempfile::NamedTempFile::new_in(cache_dir) {
            if serde_json::to_writer(&mut file, &cache).is_ok() && file.flush().is_ok() {
                let _ = file.persist(cache_file);
            }
        }
    }
    publish(
        scan_with_output(groups, exports, output, export_games, &cache),
        true,
    );
}

fn scan_with_output(
    mut groups: Groups,
    exports: Groups,
    output: &Path,
    export_games: &BTreeMap<PathBuf, String>,
    cache: &GameCache,
) -> GalleryScan {
    let mut paths = HashSet::new();
    let mut grouped_exports = Vec::new();
    let root_folder = output.file_name();
    for (_, clips) in exports {
        for clip in clips {
            if !paths.insert(clip.path.clone()) {
                continue;
            }
            let stem = clip
                .path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let original = original_export_name(stem);
            let game = cache
                .get(&clip.path)
                .filter(|(mtime, size, _)| (*mtime, *size) == (clip.mtime, clip.size))
                .and_then(|(_, _, game)| game.clone())
                .or_else(|| export_games.get(&clip.path).cloned())
                .or_else(|| {
                    dated_name(original).map(|(name, _)| game_icons::game_name(name).to_owned())
                })
                .or_else(|| {
                    (Some(std::ffi::OsStr::new(&clip.group)) != root_folder)
                        .then(|| clip.group.clone())
                })
                .unwrap_or_else(|| OUTPUT_GROUP.to_owned());
            grouped_exports.push((game, vec![clip]));
        }
    }
    for (_, clips) in &mut groups {
        clips.retain(|clip| !paths.contains(&clip.path));
    }
    groups.retain(|(_, clips)| !clips.is_empty());
    groups.extend(grouped_exports);
    (group_games(groups), paths)
}

fn group_games(folders: Vec<(String, Vec<Clip>)>) -> Vec<(String, Vec<Clip>)> {
    let mut groups: BTreeMap<String, Vec<Clip>> = BTreeMap::new();
    for (folder, clips) in folders {
        let name = game_icons::game_name(&folder);
        let game = groups.entry(name.to_owned()).or_default();
        game.extend(clips.into_iter().map(|mut clip| {
            if clip.group != name {
                clip.group = name.to_owned();
            }
            clip
        }));
    }
    for clips in groups.values_mut() {
        clips.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
    }
    let unknown_exports = groups.remove(OUTPUT_GROUP);
    groups
        .into_iter()
        .chain(unknown_exports.map(|clips| (OUTPUT_GROUP.to_owned(), clips)))
        .collect()
}

impl Gallery {
    /// Section name for the "by folder" grouping: the game folder inside a root,
    /// the root's own name for clips that sit directly in it.
    fn folder_label(&self, clip: &Clip, lang: Lang) -> String {
        if clip.group == OUTPUT_GROUP {
            return t(lang, "cutgut_output").to_owned();
        }
        let mut roots: Vec<&str> = self.dirs.iter().map(String::as_str).collect();
        if roots.is_empty() {
            roots.push(self.dir.as_str());
        }
        for root in roots {
            if let Ok(relative) = clip.path.strip_prefix(root) {
                let mut parts = relative.components();
                if let (Some(first), Some(_)) = (parts.next(), parts.next()) {
                    return first.as_os_str().to_string_lossy().into_owned();
                }
                return Path::new(root)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| root.to_owned());
            }
        }
        clip.path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Recording day of a clip, used as a section name when grouping by date.
fn date_label(clip: &Clip) -> String {
    recording_datetime(clip)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .or_else(|| {
            chrono::Local
                .timestamp_opt(clip.mtime as i64, 0)
                .single()
                .map(|date| date.format("%Y-%m-%d").to_string())
        })
        .unwrap_or_default()
}

fn section_heading(ui: &egui::Ui, section: &Section<'_>, rect: Rect, game_icons: &GameIcons) {
    let palette = Palette::get(ui);
    let painter = ui.painter();
    let icon_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Vec2::splat(24.0),
    );
    match section.icon {
        Some(icon) => icons::paint(painter, icon, icon_rect, palette.accent, 1.6),
        None if section.imported => {
            icons::paint(painter, Icon::Upload, icon_rect, palette.accent, 1.6)
        }
        None => game_icons.paint(painter, section.name.as_ref(), icon_rect),
    }
    let text = painter.layout_no_wrap(
        section.name.to_string(),
        FontId::proportional(15.0),
        palette.text,
    );
    let left = icon_rect.right() + 5.0;
    let right = left + text.size().x;
    painter.galley(
        egui::pos2(left, rect.center().y - text.size().y * 0.5),
        text,
        palette.text,
    );
    let count = painter.layout_no_wrap(
        section.clips.len().to_string(),
        FontId::monospace(10.5),
        palette.muted,
    );
    painter.galley(
        egui::pos2(right + 5.0, rect.center().y - count.size().y * 0.5),
        count,
        palette.muted,
    );
}

fn blurred_backdrop(path: &Path) -> Option<egui::ColorImage> {
    let image = image::open(path).ok()?.thumbnail(144, 90).to_rgba8();
    let blurred = image::imageops::blur(&image, 9.0);
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [blurred.width() as usize, blurred.height() as usize],
        blurred.as_raw(),
    ))
}

fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Gallery duration badge: `30s`, `1m 30s`, or `1h 2m 3s` over an hour.
/// Gallery duration badge: `30s`, `1m 30s`, or `1h 2m 3s` over an hour.
/// Zero trailing units are omitted, so exactly a minute is just `1m`.
fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    if total < 60 {
        format!("{total}s")
    } else if total < 3600 {
        let (minutes, secs) = (total / 60, total % 60);
        if secs == 0 {
            format!("{minutes}m")
        } else {
            format!("{minutes}m {secs}s")
        }
    } else {
        let (hours, minutes, secs) = (total / 3600, (total % 3600) / 60, total % 60);
        if minutes == 0 && secs == 0 {
            format!("{hours}h")
        } else if secs == 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h {minutes}m {secs}s")
        }
    }
}

fn format_clip_date(epoch_secs: u64) -> String {
    chrono::Local
        .timestamp_opt(epoch_secs as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| media::format_date_time(epoch_secs))
}

fn thumbnail_workers(
    cache: &Path,
    ctx: &egui::Context,
) -> (mpsc::Sender<Clip>, Receiver<ThumbMsg>) {
    let (requests, pending) = mpsc::channel::<Clip>();
    let (results, received) = mpsc::channel();
    let pending = Arc::new(Mutex::new(pending));
    for _ in 0..3 {
        let pending = pending.clone();
        let results = results.clone();
        let cache = cache.to_path_buf();
        let ctx = ctx.clone();
        std::thread::spawn(move || loop {
            let Ok(clip) = pending.lock().unwrap().recv() else {
                break;
            };
            let path = thumb_cache_path(&cache, &clip);
            let image = if media::is_gallery_image(&clip.path) {
                load_gallery_image(&clip.path, 320)
            } else {
                read_thumbnail(&path).or_else(|| {
                    std::fs::create_dir_all(&cache).ok()?;
                    let scratch = tempfile::tempdir_in(&cache).ok()?;
                    let thumbnail = scratch.path().join("thumbnail.jpg");
                    let source = clip.path.to_string_lossy();
                    let target = thumbnail.to_string_lossy();
                    media::generate_thumbnail(&source, &target, Some(0.8))
                        .or_else(|_| media::generate_thumbnail(&source, &target, Some(0.0)))
                        .ok()?;
                    let image = read_thumbnail(&thumbnail)?;
                    let _ = std::fs::rename(thumbnail, path);
                    Some(image)
                })
            };
            if results.send(ThumbMsg { clip, image }).is_err() {
                break;
            }
            ctx.request_repaint();
        });
    }
    (requests, received)
}

fn duration_workers(ctx: &egui::Context) -> (mpsc::Sender<Clip>, Receiver<DurationMsg>) {
    let (requests, pending) = mpsc::channel::<Clip>();
    let (results, received) = mpsc::channel();
    let pending = Arc::new(Mutex::new(pending));
    for _ in 0..2 {
        let pending = pending.clone();
        let results = results.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || loop {
            let Ok(clip) = pending.lock().unwrap().recv() else {
                break;
            };
            let duration = media::probe_duration(&clip.path);
            if results.send(DurationMsg { clip, duration }).is_err() {
                break;
            }
            ctx.request_repaint();
        });
    }
    (requests, received)
}

fn load_gallery_image(path: &Path, max_side: u32) -> Option<egui::ColorImage> {
    let mut image = image::open(path).ok()?;
    if image.width().max(image.height()) > max_side {
        image = image.thumbnail(max_side, max_side);
    }
    let rgba = image.into_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

fn read_thumbnail(path: &Path) -> Option<egui::ColorImage> {
    let image = image::open(path).ok()?.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

fn truncate_middle(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let head = max / 2 - 1;
    let tail = max - head - 1;
    let start: String = chars.iter().take(head).collect();
    let end: String = chars.iter().skip(chars.len() - tail).collect();
    format!("{start}…{end}")
}

fn thumb_cache_path(cache_dir: &Path, clip: &Clip) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    clip.path.to_string_lossy().hash(&mut hasher);
    clip.mtime.hash(&mut hasher);
    let key = format!("{:016x}", hasher.finish());
    cache_dir.join(format!("{key}.jpg"))
}

fn open_folder(path: &str) {
    if Path::new(path).is_dir() {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
}

fn reveal_path(path: &str) {
    let _ = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(path)
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_navigation_follows_the_filtered_gallery_order() {
        let mut gallery = Gallery::new(String::new(), egui::Context::default());
        let clip = |path: &str| Clip {
            path: PathBuf::from(path),
            group: "Game".into(),
            size: 0,
            mtime: 0,
        };
        gallery.groups.push((
            "Game".into(),
            vec![clip("one.mp4"), clip("two.mp4"), clip("three.mp4")],
        ));
        gallery
            .groups
            .push(("Other".into(), vec![clip("four.mp4")]));
        let selected = Path::new("three.mp4");
        let (previous, next) = gallery.neighbors(selected, Lang::En);
        assert_eq!(previous.unwrap().path, Path::new("two.mp4"));
        assert_eq!(next.unwrap().path, Path::new("four.mp4"));
        gallery.filter = Filter::Favorites;
        gallery.library.favorites = [PathBuf::from("one.mp4"), selected.to_owned()].into();
        let (previous, next) = gallery.neighbors(selected, Lang::En);
        assert_eq!(previous.unwrap().path, Path::new("one.mp4"));
        assert!(next.is_none());
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(0.0), "0s");
        assert_eq!(format_duration(42.4), "42s");
        assert_eq!(format_duration(42.6), "43s");
        assert_eq!(format_duration(60.0), "1m");
        assert_eq!(format_duration(90.0), "1m 30s");
        assert_eq!(format_duration(605.0), "10m 5s");
        assert_eq!(format_duration(3599.6), "1h");
        assert_eq!(format_duration(3660.0), "1h 1m");
        assert_eq!(format_duration(3723.0), "1h 2m 3s");
    }

    #[test]
    fn exported_clip_is_highlighted_until_opened() {
        let mut gallery = Gallery::new(String::new(), egui::Context::default());
        gallery.filter = Filter::Favorites;
        gallery.search = "nothing".into();
        let path = PathBuf::from("export.mp4");
        gallery.export_finished(path.clone(), None, Lang::En);
        assert_eq!(gallery.highlight.as_deref(), Some(path.as_path()));
        assert!(gallery.highlight_scroll);
        assert!(gallery.filter == Filter::All);
        assert!(gallery.search.is_empty());
        // Opening a different clip keeps the highlight...
        gallery.open_clip(&Clip {
            path: PathBuf::from("other.mp4"),
            group: String::new(),
            size: 0,
            mtime: 0,
        });
        assert_eq!(gallery.highlight.as_deref(), Some(path.as_path()));
        // ...opening the highlighted clip clears it.
        gallery.open_clip(&Clip {
            path: path.clone(),
            group: String::new(),
            size: 0,
            mtime: 0,
        });
        assert!(gallery.highlight.is_none());
        assert!(!gallery.highlight_scroll);
    }

    #[test]
    fn renamed_exports_keep_the_game_stored_in_the_video() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("source.mp4");
        let output = root.path().join("outputs");
        let args = [
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=24",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-threads",
            "1",
            input.to_str().unwrap(),
        ]
        .map(str::to_owned);
        let generated = ffmpeg::run_process(&ffmpeg::ffmpeg_path(), &args).unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );

        for extension in ["mp4", "mkv"] {
            let path = output.join(format!("export.{extension}"));
            let crop = cutgut::plan::CropBox {
                x: 0,
                y: 0,
                w: 144,
                h: 80,
                ratio_type: "custom".into(),
            };
            ffmpeg::encode_video_with_game(
                input.to_str().unwrap(),
                path.to_str().unwrap(),
                0.0,
                1.0,
                20.0,
                "auto",
                Some(cutgut::plan::VideoCodec::H264),
                "fast",
                Some(false),
                (extension == "mkv").then_some(&crop),
                Some("Valorant"),
                &mut |_| {},
                &ffmpeg::CancellationToken::new(),
            )
            .unwrap();
            std::fs::rename(path, output.join(format!("Mój najlepszy klip.{extension}"))).unwrap();
        }

        let cache = root.path().join("cache");
        let mut groups = Vec::new();
        let mut updates = Vec::new();
        for _ in 0..2 {
            scan_gallery(
                std::slice::from_ref(&root.path().to_path_buf()),
                &output,
                &BTreeMap::new(),
                &cache,
                &AtomicBool::new(false),
                |(result, _), finished| {
                    groups = result;
                    updates.push(finished);
                    true
                },
            );
        }
        assert_eq!(updates, [false, true, true]);
        let valorant = groups.iter().find(|(game, _)| game == "Valorant").unwrap();
        assert_eq!(valorant.1.len(), 2);
    }

    #[test]
    fn clicking_the_active_game_chip_returns_to_the_previous_filter() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut gallery = Gallery::new(root.path().to_string_lossy().into_owned(), ctx);
        gallery.filter_before_group = Filter::Favorites;
        gallery.filter = Filter::Group("Valorant".to_owned());

        assert_eq!(gallery.chip_filter("Valorant", true), Filter::Favorites);
        assert_eq!(
            gallery.chip_filter("Overwatch 2", false),
            Filter::Group("Overwatch 2".to_owned())
        );
    }

    #[test]
    fn texture_budget_evicts_unused_thumbnails_but_keeps_visible_ones() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut gallery = Gallery::new(root.path().to_string_lossy().into_owned(), ctx.clone());
        let mut keys = Vec::new();
        for index in 0..TEXTURE_BUDGET + 40 {
            let key = format!("clip-{index}.mp4");
            gallery.textures.insert(
                key.clone(),
                ctx.load_texture(
                    key.clone(),
                    egui::ColorImage::filled([1, 1], Color32::WHITE),
                    egui::TextureOptions::LINEAR,
                ),
            );
            gallery.thumb_versions.insert(key.clone(), (0, 0));
            gallery.texture_order.push_back(key.clone());
            keys.push(key);
        }

        let visible = vec![keys[0].clone()];
        gallery.trim_textures(&visible);
        assert_eq!(gallery.textures.len(), TEXTURE_BUDGET);
        assert!(
            gallery.textures.contains_key(&keys[0]),
            "a thumbnail on screen must never be evicted"
        );
        assert!(
            !gallery.textures.contains_key(&keys[1]),
            "the oldest unused thumbnail should be released"
        );
        assert!(
            !gallery.thumb_versions.contains_key(&keys[1]),
            "an evicted texture must be requestable again"
        );
        assert_eq!(gallery.texture_order.len(), gallery.textures.len());

        gallery.trim_textures(&visible);
        assert_eq!(gallery.textures.len(), TEXTURE_BUDGET);
    }

    #[test]
    fn a_cancelled_scan_publishes_nothing() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("clip.mp4"), []).unwrap();
        let cancelled = AtomicBool::new(true);
        let mut published = 0;
        scan_gallery(
            std::slice::from_ref(&root.path().to_path_buf()),
            &PathBuf::new(),
            &BTreeMap::new(),
            root.path(),
            &cancelled,
            |_, _| {
                published += 1;
                true
            },
        );
        assert_eq!(published, 0, "a superseded scan must stay silent");
    }

    #[test]
    fn refreshing_keeps_unchanged_thumbnails() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("clip.mp4");
        std::fs::write(&path, []).unwrap();
        let clip = Clip::from_video_path(&path, "Test").unwrap();
        let key = path.to_string_lossy().into_owned();
        let ctx = egui::Context::default();
        let mut gallery = Gallery::new(root.path().to_string_lossy().into_owned(), ctx.clone());
        gallery.textures.insert(
            key.clone(),
            ctx.load_texture(
                "thumbnail",
                egui::ColorImage::filled([1, 1], Color32::WHITE),
                egui::TextureOptions::LINEAR,
            ),
        );
        gallery
            .thumb_versions
            .insert(key.clone(), (clip.mtime, clip.size));
        let texture = gallery.textures[&key].id();
        gallery.reload();
        assert_eq!(gallery.textures[&key].id(), texture);
        for _ in 0..100 {
            gallery.poll_scan();
            if gallery.scan.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(gallery.scanned);
        gallery.close_modal();
        assert_eq!(gallery.textures[&key].id(), texture);
    }

    #[test]
    fn output_filter_keeps_custom_exports_with_their_source_game() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("outputs");
        std::fs::create_dir(&output).unwrap();
        let custom = output.join("My highlight.mp4");
        let dated =
            output.join("Counter-strike 2 2026.11.06 - 18.43.09.03.DVR - CG 2026-11-12.mp4");
        let unknown = output.join("older clip.mp4");
        let legacy = output
            .join("CutGut_Counter-strike 2 2026.11.03 - 21.03.43.08.DVR_16_9_20261104_212854.mp4");
        for path in [&custom, &dated, &legacy, &unknown] {
            std::fs::write(path, []).unwrap();
        }
        let games = BTreeMap::from([(custom.clone(), "Counter-Strike 2".into())]);
        let (groups, paths) = scan_with_output(
            media::scan_gallery(root.path()),
            media::scan_gallery(&output),
            &output,
            &games,
            &GameCache::new(),
        );
        assert_eq!(paths.len(), 4);
        let clips = &groups
            .iter()
            .find(|(game, _)| game == "Counter-Strike 2")
            .unwrap()
            .1;
        assert_eq!(clips.len(), 3);
        assert_eq!(
            recording_date(clips.iter().find(|clip| clip.path == custom).unwrap()),
            None
        );
        assert_eq!(
            recording_date(clips.iter().find(|clip| clip.path == dated).unwrap()).as_deref(),
            Some("2026-11-06 18:43")
        );
        assert_eq!(
            groups
                .iter()
                .find(|(game, _)| game == OUTPUT_GROUP)
                .unwrap()
                .1[0]
                .path,
            unknown
        );
    }

    #[test]
    fn minecraft_aliases_form_one_section_in_clip_date_order() {
        let mut folders = Vec::new();
        for (index, name) in [
            "Minecraft",
            "java-runtime-epilson",
            "Base Profile",
            "Counter-strike 2",
        ]
        .into_iter()
        .enumerate()
        {
            folders.push((
                name.into(),
                vec![Clip {
                    path: PathBuf::from(format!("{name}.mp4")),
                    group: name.into(),
                    size: 0,
                    mtime: index as u64,
                }],
            ));
        }
        let groups = group_games(folders);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "Counter-Strike 2");
        assert_eq!(groups[1].0, "Minecraft");
        let minecraft = &groups[1].1;
        assert!(minecraft.iter().all(|clip| clip.group == "Minecraft"));
        assert_eq!(
            minecraft.iter().map(|clip| clip.mtime).collect::<Vec<_>>(),
            [2, 1, 0]
        );
    }

    #[test]
    fn scrolling_gallery_grid_hides_and_restores_filters() {
        let ctx = egui::Context::default();
        let mut gallery = Gallery::new(String::new(), ctx.clone());
        gallery.scanned = true;
        gallery.groups.push((
            "Minecraft".into(),
            (0..40)
                .map(|id| Clip {
                    path: PathBuf::from(format!("{id}.mp4")),
                    group: "Minecraft".into(),
                    size: 0,
                    mtime: id,
                })
                .collect(),
        ));
        let mut frame = |wheel: f32, scroll_locked: bool| {
            let mut input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 650.0))),
                ..Default::default()
            };
            input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(450.0, 350.0)));
            if wheel != 0.0 {
                input.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, wheel),
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    gallery.ui(ui, ctx, Lang::Pl, false, scroll_locked)
                });
            });
            gallery.filters_open
        };
        assert!(frame(0.0, false));
        assert!(frame(-100.0, true));
        assert!(!frame(-100.0, false));
        assert!(frame(100.0, false));
    }
}

#[cfg(test)]
mod usage_tests {
    use super::*;

    fn clip(path: &str, size: u64) -> Clip {
        Clip {
            path: PathBuf::from(path),
            group: String::new(),
            size,
            mtime: 0,
        }
    }

    #[test]
    fn category_stats_and_usage_agree() {
        let mut gallery = Gallery::new(String::new(), egui::Context::default());
        gallery.groups = vec![
            (
                "Game A".to_owned(),
                vec![clip("C:/a1.mp4", 100), clip("C:/a2.mp4", 300)],
            ),
            ("Game B".to_owned(), vec![clip("C:/b1.mp4", 600)]),
        ];
        gallery.imported = vec![clip("C:/i.mp4", 1_000)];

        assert_eq!(gallery.total_bytes(), 2_000);
        assert_eq!(
            gallery.category_stats(&Filter::Group("Game A".to_owned())),
            (2, 400, 300)
        );
        assert_eq!(gallery.category_stats(&Filter::All), (4, 2_000, 1_000));

        let usage = gallery.usage(Lang::En);
        assert_eq!(usage.len(), 3);
        assert_eq!(usage[0].name, "Imported");
        assert_eq!(usage[0].size, 1_000);
        assert_eq!(usage[0].count, 1);
        assert!(usage[0].game.is_none());
        let game = &usage[1];
        assert_eq!(game.name, "Game B");
        assert_eq!(game.game.as_deref(), Some("Game B"));
        assert_eq!(game.color, category_color("Game B"));
    }
}
