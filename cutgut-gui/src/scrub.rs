//! Scrub cache: the clip is decoded once, in parallel chunks, into memory.
//!
//! Every frame the editor used to ask for started a fresh `ffmpeg` process. On
//! Windows that alone costs about 0.4 s before the first pixel, and a 4K HEVC
//! source adds seeking on top, which is why dragging the playhead felt like the
//! app was thinking. Decoding the clip once — scaled to roughly preview size —
//! into a bounded cache lets the picture follow the mouse, while the exact frame
//! for the moment the user actually stops on is still fetched at full size.
//!
//! The pass is split across several decoders: each one seeks once and walks its
//! own quarter of the clip, which turns a serial ten-second walk into a couple
//! of seconds. Hardware decoding is tried first and the whole thing is repeated
//! in software if the driver refuses the codec.
//!
//! The same frames are box-filtered into one atlas texture for the timeline, so
//! the strip is drawn from real footage at any zoom level without spawning a
//! second set of decoders.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use cutgut::plan::VideoInfo;
use eframe::egui::{Color32, ColorImage, Context};

use crate::process::Process;

/// Widest a cached frame gets, before the budget argues. This is close to the
/// width the preview is drawn at, so scrubbing does not look like 360p.
const CACHE_WIDTH_MAX: u32 = 1280;
/// Next widths to try when the budget is tight. Wide first.
const CACHE_WIDTHS: [u32; 15] = [
    CACHE_WIDTH_MAX,
    1152,
    1088,
    1024,
    960,
    896,
    832,
    768,
    704,
    640,
    576,
    512,
    448,
    384,
    320,
];
/// Roughly how much memory the cache may hold, in bytes of RGBA.
const CACHE_BUDGET_BYTES: f64 = 256_000_000.0;
/// A long clip gets fewer frames per second rather than a smaller cache.
const MIN_FPS: f64 = 2.0;
const MAX_FPS: f64 = 8.0;
/// Frames we would like to hold, before the budget and the clip length argue.
const WANTED_FRAMES: f64 = 160.0;
const HARD_FRAME_CAP: f64 = 240.0;
/// Decoders filling the cache at the same time.
const WORKERS: usize = 4;
/// Timeline tiles: one texture, sixteen columns.
const ATLAS_COLS: usize = 16;
/// Tile width. Tiles are stretched when the timeline is zoomed in, so a 64 px
/// tile turned into a smear; still cheap to hold at 192 px.
const ATLAS_CELL_W: usize = 192;
/// The atlas may not grow past this.
const ATLAS_BUDGET_BYTES: f64 = 48_000_000.0;

pub struct CachedFrame {
    pub time: f64,
    pub image: Arc<ColorImage>,
}

struct Shared {
    /// One slot per expected frame; parallel decoders fill different ranges.
    slots: Vec<Option<CachedFrame>>,
    /// Seconds between cached frames.
    interval: f64,
    /// Latest time decoded so far, and how many cached frames that is.
    highest: f64,
    filled: usize,
    /// Every cached frame shrunk into one tile: the timeline paints from this.
    atlas: ColorImage,
    atlas_dirty: bool,
    /// Frames the clip should produce, so the UI can show how far along it is.
    expected: usize,
    /// Chunk decoders still running.
    workers_left: usize,
    finished: bool,
    /// Atlas tile size.
    cell: [usize; 2],
}

/// Frame rate and width for a clip: bounded memory first, detail second.
fn cache_shape(duration: f64, aspect: f64) -> (f64, usize, usize) {
    let seconds = duration.max(0.1);
    let mut fps = (WANTED_FRAMES / seconds).clamp(MIN_FPS, MAX_FPS);
    if fps * seconds > HARD_FRAME_CAP {
        fps = HARD_FRAME_CAP / seconds;
    }
    let frames = (fps * seconds).ceil().max(1.0);
    let per_frame = CACHE_BUDGET_BYTES / frames;

    // Widest frame the budget allows: sharpness is what a scrub is judged on.
    let mut width = CACHE_WIDTHS[CACHE_WIDTHS.len() - 1];
    for candidate in CACHE_WIDTHS {
        let height = even_height(candidate, aspect);
        if f64::from(candidate * height * 4) <= per_frame {
            width = candidate;
            break;
        }
    }
    (fps, width as usize, even_height(width, aspect) as usize)
}

fn even_height(width: u32, aspect: f64) -> u32 {
    let height = (f64::from(width) / aspect.max(0.1) / 2.0).round() as u32 * 2;
    height.clamp(2, 4320)
}

/// Tile width for the timeline atlas, shrunk when a long clip needs many tiles.
fn atlas_cell(aspect: f64, expected: usize) -> [usize; 2] {
    let mut width = ATLAS_CELL_W;
    while width > 48 {
        let height = even_height(width as u32, aspect) as usize;
        if (width * height * 4 * expected.max(1)) as f64 <= ATLAS_BUDGET_BYTES {
            break;
        }
        width -= 32;
    }
    [width, even_height(width as u32, aspect) as usize]
}

pub struct ScrubCache {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    /// Every decoder this cache has started, so `clear` can kill all of them.
    registry: Arc<Mutex<Vec<Process>>>,
    worker: Option<JoinHandle<()>>,
    /// Seconds between cached frames, and where they sit.
    pub interval: f64,
    pub size: [usize; 2],
    atlas_texture: Option<eframe::egui::TextureHandle>,
    /// When the atlas was uploaded last: a fresh tile arrives every frame while
    /// the clip decodes, and pushing a megabyte-sized texture at screen rate is
    /// not worth it.
    atlas_uploaded: Option<std::time::Instant>,
}

impl Default for ScrubCache {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared {
                slots: Vec::new(),
                interval: 1.0,
                highest: 0.0,
                filled: 0,
                atlas: ColorImage::new([1, 1], vec![Color32::TRANSPARENT]),
                atlas_dirty: false,
                expected: 0,
                workers_left: 0,
                finished: true,
                cell: [ATLAS_CELL_W, 108],
            })),
            stop: Arc::new(AtomicBool::new(true)),
            registry: Arc::new(Mutex::new(Vec::new())),
            worker: None,
            interval: 1.0,
            size: [1, 1],
            atlas_texture: None,
            atlas_uploaded: None,
        }
    }
}

impl ScrubCache {
    pub fn clear(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for process in self
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            process.cancel();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.registry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let mut shared = self.lock();
        shared.slots.clear();
        shared.expected = 0;
        shared.filled = 0;
        shared.workers_left = 0;
        shared.finished = true;
        shared.atlas_dirty = false;
        shared.highest = 0.0;
        drop(shared);
        self.atlas_texture = None;
        self.atlas_uploaded = None;
    }

    /// Starts the pass over `path`. Cheap to call for a clip that is already open.
    pub fn open(&mut self, ctx: &Context, path: &str, info: &VideoInfo) {
        self.clear();
        let aspect = {
            let (width, height) = info.display_size();
            f64::from(width.max(1)) / f64::from(height.max(1))
        };
        let duration = info.duration.max(0.1);
        let (fps, width, height) = cache_shape(duration, aspect);
        let expected = ((fps * duration).ceil() as usize).clamp(1, 4096);
        let cell = atlas_cell(aspect, expected);
        let rows = expected.div_ceil(ATLAS_COLS).max(1);
        let interval = 1.0 / fps;

        {
            let mut shared = self.lock();
            *shared = Shared {
                slots: (0..expected).map(|_| None).collect(),
                interval,
                highest: 0.0,
                filled: 0,
                atlas: ColorImage::new(
                    [ATLAS_COLS * cell[0], rows * cell[1]],
                    vec![Color32::TRANSPARENT; ATLAS_COLS * cell[0] * rows * cell[1]],
                ),
                atlas_dirty: true,
                expected,
                workers_left: 0,
                finished: false,
                cell,
            };
        }

        // One decoder per ~20 frames: a short clip should not pay for four
        // processes it cannot keep busy.
        let chunks = WORKERS.min(expected.div_ceil(20)).max(1);
        let ranges = chunk_ranges(duration, chunks);
        self.lock().workers_left = ranges.len();

        self.interval = interval;
        self.size = [width, height];
        self.atlas_texture = None;
        self.atlas_uploaded = None;
        self.stop = Arc::new(AtomicBool::new(false));

        let ctx = ctx.clone();
        let path = path.to_owned();
        let shared = self.shared.clone();
        let stop = self.stop.clone();
        let registry = self.registry.clone();
        self.worker = Some(std::thread::spawn(move || {
            let job = Job {
                chunk: Chunk {
                    ctx,
                    path,
                    fps,
                    size: [width, height],
                    interval,
                    expected,
                    shared,
                    stop,
                },
                ranges,
                registry,
            };
            // Hardware decoding roughly halves the pass on AV1/HEVC captures,
            // but a driver can refuse a codec it claims to support. If not a
            // single frame arrived, the whole pass is repeated in software.
            let decoded = job.fill(true);
            if decoded == 0 && !job.chunk.stop.load(Ordering::Relaxed) {
                job.reset_slots();
                job.fill(false);
            }
            job.finish();
        }));
    }

    /// The cached frame closest to `time`, or nothing while the decoder has not
    /// reached that part yet (guessing would show the wrong picture).
    pub fn frame_at(&self, time: f64) -> Option<(usize, Arc<ColorImage>)> {
        let shared = self.lock();
        let index = nearest(&shared, time)?;
        let frame = shared.slots[index].as_ref()?;
        // While the pass runs, the nearest frame can be a second away from the
        // requested moment in a part nothing has decoded yet.
        if !shared.finished && (frame.time - time).abs() > shared.interval * 2.0 {
            return None;
        }
        Some((index, frame.image.clone()))
    }

    /// Index of the cached frame closest to `time`, but only when it really is
    /// that close. The timeline uses it
    /// so a tile waits for its own frame instead of borrowing a distant one and
    /// then swapping it twice while the clip decodes — that swap is the flicker
    /// seen right after a clip opens.
    pub fn nearest_index_within(&self, time: f64, tolerance: f64) -> Option<usize> {
        let shared = self.lock();
        let index = nearest(&shared, time)?;
        let frame = shared.slots[index].as_ref()?;
        ((frame.time - time).abs() <= tolerance).then_some(index)
    }

    pub fn last_time(&self) -> Option<f64> {
        let shared = self.lock();
        (shared.filled > 0).then_some(shared.highest)
    }

    pub fn finished(&self) -> bool {
        self.lock().finished
    }

    /// Cached frames per second of clip, for the progress readout.
    pub fn fill_fraction(&self) -> f32 {
        let shared = self.lock();
        if shared.expected == 0 {
            return 1.0;
        }
        shared.filled as f32 / shared.expected as f32
    }

    /// Tile size in the atlas, and how many fit per row.
    pub fn atlas_layout(&self) -> ([usize; 2], usize) {
        let shared = self.lock();
        (shared.cell, ATLAS_COLS)
    }

    /// Uploads the tile atlas when new frames landed, and hands back the texture.
    pub fn atlas(&mut self, ctx: &Context) -> Option<&eframe::egui::TextureHandle> {
        const UPLOAD_EVERY: std::time::Duration = std::time::Duration::from_millis(110);
        let ready = self
            .atlas_uploaded
            .is_none_or(|uploaded| uploaded.elapsed() >= UPLOAD_EVERY);
        let image = {
            let mut shared = self.lock();
            if shared.atlas_dirty && (ready || shared.finished) {
                shared.atlas_dirty = false;
                Some(shared.atlas.clone())
            } else {
                None
            }
        };
        if image.is_some() {
            self.atlas_uploaded = Some(std::time::Instant::now());
        }
        if let Some(image) = image {
            match &mut self.atlas_texture {
                Some(texture) => texture.set(image, eframe::egui::TextureOptions::LINEAR),
                None => {
                    self.atlas_texture = Some(ctx.load_texture(
                        "scrub-atlas",
                        image,
                        eframe::egui::TextureOptions::LINEAR,
                    ))
                }
            }
        }
        self.atlas_texture.as_ref()
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

impl Drop for ScrubCache {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Splits the clip into equal spans; the last one takes the remainder.
fn chunk_ranges(duration: f64, chunks: usize) -> Vec<(f64, f64)> {
    let chunks = chunks.max(1);
    let span = duration / chunks as f64;
    (0..chunks)
        .map(|index| {
            let start = index as f64 * span;
            let length = if index + 1 == chunks {
                duration - start
            } else {
                span
            };
            (start, length)
        })
        .collect()
}

/// Closest slot to `time`, preferring the earlier neighbour: a frame you can
/// see now beats the right one a moment later.
fn nearest(shared: &Shared, time: f64) -> Option<usize> {
    let last = shared.slots.len().checked_sub(1)?;
    if shared.interval <= 0.0 {
        return None;
    }
    let center = ((time.max(0.0) / shared.interval).round() as usize).min(last) as isize;
    let last = last as isize;
    for step in 0..=last.max(0) {
        let left = center - step;
        if left >= 0 && shared.slots[left as usize].is_some() {
            return Some(left as usize);
        }
        let right = center + step;
        if right <= last && shared.slots[right as usize].is_some() {
            return Some(right as usize);
        }
        if left <= 0 && right >= last {
            break;
        }
    }
    None
}

/// Everything a decoder thread needs. Cheap to clone, one per worker.
#[derive(Clone)]
struct Chunk {
    ctx: Context,
    path: String,
    fps: f64,
    size: [usize; 2],
    interval: f64,
    expected: usize,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
}

impl Chunk {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    /// Reads one span of the clip into the cache; returns how many frames landed.
    fn run(&self, process: &Process, start: f64, length: f64, hardware: bool) -> usize {
        let [width, height] = self.size;
        let mut command = cutgut::ffmpeg::command(&cutgut::ffmpeg::ffmpeg_path());
        if hardware {
            command.args(["-hwaccel", "auto"]);
        }
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-threads",
                "3",
                "-ss",
                &format!("{start:.6}"),
                "-t",
                &format!("{length:.6}"),
                "-i",
                &self.path,
                "-map",
                "0:v:0",
                "-an",
                "-sn",
                "-dn",
                "-filter_threads",
                "1",
                "-vf",
                &format!(
                    "fps={:.5},scale={width}:{height}:flags=fast_bilinear",
                    self.fps
                ),
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "pipe:1",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut buffer = vec![0u8; width * height * 4];
        let mut reader = None;
        if process.spawn(&mut command).is_ok() {
            reader = process.stdout();
        }
        let Some(reader) = reader.as_mut() else {
            return 0;
        };
        let base = (start / self.interval).round() as usize;
        let mut count = 0usize;
        while !self.stop.load(Ordering::Relaxed) {
            if reader.read_exact(&mut buffer).is_err() {
                break;
            }
            let index = base + count;
            if index >= self.expected {
                break;
            }
            let time = index as f64 * self.interval;
            let image = Arc::new(ColorImage::from_rgba_unmultiplied([width, height], &buffer));
            {
                let mut shared = self.lock();
                if shared.slots[index].is_none() {
                    shared.filled += 1;
                    shared.highest = shared.highest.max(time);
                    let cell = shared.cell;
                    paint_cell(&mut shared.atlas, cell, ATLAS_COLS, index, &image);
                    shared.atlas_dirty = true;
                    shared.slots[index] = Some(CachedFrame { time, image });
                }
            }
            count += 1;
            self.ctx.request_repaint();
        }
        process.finish();
        count
    }
}

/// One cache pass: the chunk list, plus the decoders it may need to stop.
struct Job {
    chunk: Chunk,
    ranges: Vec<(f64, f64)>,
    registry: Arc<Mutex<Vec<Process>>>,
}

impl Job {
    /// Runs every chunk in parallel; returns how many frames arrived.
    fn fill(&self, hardware: bool) -> usize {
        let mut workers = Vec::new();
        for &(start, length) in &self.ranges {
            let process = Process::default();
            self.registry
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(process.clone());
            let chunk = self.chunk.clone();
            workers.push(std::thread::spawn(move || {
                chunk.run(&process, start, length, hardware)
            }));
        }
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok())
            .sum()
    }

    fn reset_slots(&self) {
        let mut shared = self.chunk.lock();
        for slot in shared.slots.iter_mut() {
            *slot = None;
        }
        shared.filled = 0;
        shared.highest = 0.0;
    }

    fn finish(&self) {
        for process in self
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
        {
            process.finish();
        }
        let mut shared = self.chunk.lock();
        shared.finished = true;
        shared.workers_left = 0;
        drop(shared);
        self.chunk.ctx.request_repaint();
    }
}

/// Box-filters one frame into its atlas cell: nearest-neighbour would sparkle at
/// a fraction of the size.
fn paint_cell(
    atlas: &mut ColorImage,
    cell: [usize; 2],
    cols: usize,
    index: usize,
    src: &ColorImage,
) {
    let [cw, ch] = cell;
    let [aw, ah] = atlas.size;
    let (col, row) = (index % cols, index / cols);
    let (ox, oy) = (col * cw, row * ch);
    if ox + cw > aw || oy + ch > ah {
        return;
    }
    let [sw, sh] = src.size;
    if sw == 0 || sh == 0 {
        return;
    }
    for y in 0..ch {
        let sy0 = y * sh / ch;
        let sy1 = ((y + 1) * sh).div_ceil(ch).max(sy0 + 1).min(sh);
        for x in 0..cw {
            let sx0 = x * sw / cw;
            let sx1 = ((x + 1) * sw).div_ceil(cw).max(sx0 + 1).min(sw);
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for sy in sy0..sy1 {
                let row = sy * sw;
                for sx in sx0..sx1 {
                    let pixel = src.pixels[row + sx];
                    r += u32::from(pixel.r());
                    g += u32::from(pixel.g());
                    b += u32::from(pixel.b());
                    n += 1;
                }
            }
            let n = n.max(1);
            atlas.pixels[(oy + y) * aw + ox + x] =
                Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared_with(len: usize, interval: f64, present: &[usize]) -> Shared {
        let mut shared = Shared {
            slots: (0..len).map(|_| None).collect(),
            interval,
            highest: 0.0,
            filled: 0,
            atlas: ColorImage::new([1, 1], vec![Color32::TRANSPARENT]),
            atlas_dirty: false,
            expected: len,
            workers_left: 0,
            finished: false,
            cell: [8, 8],
        };
        for &index in present {
            shared.slots[index] = Some(CachedFrame {
                time: index as f64 * interval,
                image: Arc::new(ColorImage::new([1, 1], vec![Color32::BLACK])),
            });
            shared.filled += 1;
        }
        shared
    }

    #[test]
    fn short_clips_get_more_detail_than_long_ones() {
        let (short_fps, short_w, _) = cache_shape(10.0, 16.0 / 9.0);
        let (long_fps, long_w, _) = cache_shape(300.0, 16.0 / 9.0);
        assert_eq!(short_fps, MAX_FPS);
        assert!(short_w > long_w, "{short_w} vs {long_w}");
        assert!(long_fps <= 1.0, "{long_fps} fps for a five minute clip");
    }

    #[test]
    fn the_cache_stays_inside_its_memory_budget() {
        for duration in [5.0, 15.0, 20.0, 45.0, 90.0, 120.0, 240.0, 600.0] {
            let (fps, w, h) = cache_shape(duration, 16.0 / 9.0);
            let bytes = (fps * duration * (w * h * 4) as f64) as usize;
            assert!(
                bytes < 260_000_000,
                "{duration:.0} s caches {} MB",
                bytes / 1_000_000
            );
        }
    }

    /// Scrubbing a minute-long clip used to show 640 px frames, which read as
    /// 360p once stretched to the preview.
    #[test]
    fn a_minute_long_clip_still_scrubs_near_preview_size() {
        let (_, width, height) = cache_shape(60.0, 16.0 / 9.0);
        assert!(width >= 832, "{width}x{height} is too soft to scrub with");
        assert!(width > 640);
    }

    #[test]
    fn portrait_clips_keep_their_shape() {
        let (_, w, h) = cache_shape(12.0, 9.0 / 16.0);
        assert!(h > w, "{w}x{h} for a portrait clip");
        assert_eq!(h % 2, 0, "scalers want even dimensions");
    }

    #[test]
    fn the_atlas_does_not_outgrow_its_budget() {
        let cell = atlas_cell(16.0 / 9.0, 240);
        let bytes = cell[0] * cell[1] * 4 * 240;
        assert!(bytes < 50_000_000, "{bytes} bytes of tiles");
        // A wide tile is what keeps a zoomed-in strip from turning to mush.
        assert!(cell[0] >= 96, "{} px tiles are too soft", cell[0]);
    }

    #[test]
    fn chunks_cover_the_whole_clip_once() {
        let ranges = chunk_ranges(10.0, 4);
        assert_eq!(ranges.len(), 4);
        assert_eq!(ranges[0].0, 0.0);
        let total: f64 = ranges.iter().map(|(_, length)| length).sum();
        assert!((total - 10.0).abs() < 1e-9);
        let end = ranges[3].0 + ranges[3].1;
        assert!((end - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_neighbour_is_found_across_a_hole() {
        // Parallel decoders leave gaps: only frames 0, 1, 5 and 9 are in yet.
        let shared = shared_with(10, 0.5, &[0, 1, 5, 9]);
        assert_eq!(nearest(&shared, 0.0), Some(0));
        assert_eq!(nearest(&shared, 0.5), Some(1));
        // 1.0 s asks for slot 2, which is missing: the earlier one wins.
        assert_eq!(nearest(&shared, 1.0), Some(1));
        assert_eq!(nearest(&shared, 3.0), Some(5));
        assert_eq!(nearest(&shared, 100.0), Some(9));
    }

    #[test]
    fn a_frame_cell_is_filled_from_the_frame() {
        let src = ColorImage::new([8, 8], vec![Color32::from_rgb(10, 20, 30); 64]);
        let mut atlas = ColorImage::new([32, 8], vec![Color32::TRANSPARENT; 256]);
        paint_cell(&mut atlas, [8, 8], 4, 1, &src);
        // Cell 1 covers x = 8..16, the rest stays empty.
        assert_eq!(atlas.pixels[3], Color32::TRANSPARENT);
        assert_eq!(atlas.pixels[8], Color32::from_rgb(10, 20, 30));
        assert_eq!(atlas.pixels[15], Color32::from_rgb(10, 20, 30));
        assert_eq!(atlas.pixels[16], Color32::TRANSPARENT);
    }
}
