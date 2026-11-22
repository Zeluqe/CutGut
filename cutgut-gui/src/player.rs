use std::io::Read;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cutgut::plan::VideoInfo;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

use crate::process::Process;

#[cfg(windows)]
#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

/// Switches the system timer to 1 ms for as long as it is alive.
///
/// Frames are paced with `park_timeout`, and Windows rounds those waits up to the
/// current timer tick — 15.6 ms by default, which is more than a whole frame at
/// 60 fps. Without this the preview judders no matter how fast the decoder is.
/// The resolution is restored when the guard is dropped.
pub struct TimerResolution(());

impl TimerResolution {
    pub fn raise() -> Self {
        #[cfg(windows)]
        unsafe {
            timeBeginPeriod(1);
        }
        Self(())
    }
}

impl Drop for TimerResolution {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            timeEndPeriod(1);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackRate {
    Tenth,
    Quarter,
    Half,
    #[default]
    Normal,
    Double,
    Quadruple,
}

impl PlaybackRate {
    pub const OPTIONS: [Self; 6] = [
        Self::Tenth,
        Self::Quarter,
        Self::Half,
        Self::Normal,
        Self::Double,
        Self::Quadruple,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tenth => "0.1×",
            Self::Quarter => "0.25×",
            Self::Half => "0.5×",
            Self::Normal => "1×",
            Self::Double => "2×",
            Self::Quadruple => "4×",
        }
    }

    /// Clip seconds per wall second — the only thing a speed change has to alter.
    pub(crate) fn scale(self) -> f64 {
        match self {
            Self::Tenth => 0.1,
            Self::Quarter => 0.25,
            Self::Half => 0.5,
            Self::Normal => 1.0,
            Self::Double => 2.0,
            Self::Quadruple => 4.0,
        }
    }

    pub fn audio_filter(self, duration: f64) -> String {
        let tempo = match self {
            Self::Tenth => ",atempo=0.5,atempo=0.5,atempo=0.5,atempo=0.8",
            Self::Quarter => ",atempo=0.5,atempo=0.5",
            Self::Half => ",atempo=0.5",
            Self::Normal => "",
            Self::Double => ",atempo=2",
            Self::Quadruple => ",atempo=2,atempo=2",
        };
        format!("atrim=duration={duration:.6}{tempo}")
    }
}

/// Frames per clip second the preview decoder is asked for.
///
/// The decoder always runs at the clip's own frame rate; pacing decides how long
/// a frame stays on screen, so a speed change never needs a new ffmpeg process.
/// Asking for *fewer* frames while slowing down — which is what the old
/// `fps / scale` did — is what made 0.5× crawl at 15 pictures per second: slow
/// motion needs every frame, it only holds each of them for longer.
pub fn preview_fps(source_fps: f64) -> f64 {
    source_fps.clamp(12.0, 60.0)
}

/// Where the picture stands: the clip time of the frame the UI put on screen and
/// when it did so. A restarted sound decoder lines itself up with this instead of
/// guessing how far the picture has run.
#[derive(Clone, Copy)]
pub struct Cursor {
    pub media: f64,
    pub wall: Instant,
}

/// Lets a decoder warmed up at a new tempo take the sound over from the one that
/// is still playing. It raises `armed` once it has buffers ready and starts
/// playing only when the player raises `go`, so the two streams never overlap.
pub struct Handover {
    pub armed: Arc<AtomicBool>,
    pub go: Arc<AtomicBool>,
}

/// Clip seconds per wall second, read back from the shared word.
fn scale_of(word: &AtomicU64) -> f64 {
    f64::from_bits(word.load(Ordering::Relaxed))
}

/// Wall clock offset (from the preview clock's origin) at which `media` belongs,
/// given the offset of the frame handed over last.
///
/// Anchoring to that offset rather than to `Instant::now()` keeps the picture on
/// the sound's timeline: a frame that arrives late is dropped, not carried
/// forward as a permanent delay.
fn schedule(scheduled: Duration, scheduled_media: f64, media: f64, scale: f64) -> Duration {
    let step = (media - scheduled_media).max(0.0) / scale;
    scheduled + Duration::from_secs_f64(step)
}

pub struct Frame {
    pub time: f64,
    pub image: ColorImage,
}

#[derive(Clone, Copy, Debug)]
pub struct PreviewSize {
    pub width: usize,
    pub height: usize,
}

impl PreviewSize {
    pub fn fit(info: &VideoInfo, max_w: f64, max_h: f64) -> Self {
        // The decoder's output is already rotated, so the preview has to be laid
        // out for the displayed size or portrait clips come out stretched.
        let (width, height) = info.display_size();
        let w = width.max(1) as f64;
        let h = height.max(1) as f64;
        let scale = (max_w / w).min(max_h / h).min(1.0);
        let even = |value: f64| ((value * scale / 2.0).round() as usize).max(1) * 2;
        Self {
            width: even(w),
            height: even(h),
        }
    }

    /// Preview playback. Sized to the area the video is actually drawn in on a
    /// maximised window, so the picture is not upscaled into mush; a smaller
    /// source is never blown up past its own pixels.
    pub fn playback(info: &VideoInfo) -> Self {
        Self::fit(info, 1440.0, 900.0)
    }

    /// The still shown when the playhead rests. Bigger than playback: it is one
    /// frame every so often, and it is what the user inspects a cut on.
    pub fn still(info: &VideoInfo) -> Self {
        Self::fit(info, 1920.0, 1200.0)
    }

    fn bytes(self) -> usize {
        self.width * self.height * 4
    }

    fn image(self, rgba: &[u8]) -> ColorImage {
        ColorImage::from_rgba_unmultiplied([self.width, self.height], rgba)
    }
}

struct Session {
    rx: Receiver<Frame>,
    stop: Arc<AtomicBool>,
    video: Process,
    audio: Process,
    thread: JoinHandle<()>,
    audio_thread: Option<JoinHandle<()>>,
    audio_stop: Option<Arc<AtomicBool>>,
    pending_audio: Option<PendingAudio>,
    clock: Arc<OnceLock<Instant>>,
    cursor: Arc<Mutex<Option<Cursor>>>,
    /// Live playback speed, in clip seconds per wall second.
    rate: Arc<AtomicU64>,
    source: String,
    span: (f64, f64),
    audio_only: Option<(f64, f64, PlaybackRate)>,
}

/// A sound decoder warming up at a new tempo while the old one still plays.
struct PendingAudio {
    armed: Arc<AtomicBool>,
    go: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    decoder: Process,
    thread: JoinHandle<()>,
}

/// Everything one device-owning sound thread needs.
struct AudioJob {
    path: String,
    start: f64,
    end: f64,
    rate: PlaybackRate,
    clock: Arc<OnceLock<Instant>>,
    cursor: Arc<Mutex<Option<Cursor>>>,
    handover: Option<Handover>,
    stop: Arc<AtomicBool>,
    decoder: Process,
}

pub struct Player {
    ctx: egui::Context,
    session: Option<Session>,
    retired: Vec<(JoinHandle<()>, Option<JoinHandle<()>>)>,
    retired_audio: Vec<JoinHandle<()>>,
    position: Option<f64>,
    volume: Arc<AtomicU32>,
}

impl Player {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            session: None,
            retired: Vec::new(),
            retired_audio: Vec::new(),
            position: None,
            volume: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
        }
    }

    pub fn set_volume(&mut self, percent: f32) {
        self.volume.store(
            (percent / 100.0).clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
    }

    pub fn position(&self) -> Option<f64> {
        self.position
    }

    pub fn is_active(&self) -> bool {
        self.session.is_some()
    }

    pub fn start(
        &mut self,
        path: &str,
        info: &VideoInfo,
        start: f64,
        end: f64,
        rate: PlaybackRate,
    ) {
        self.stop();
        let (frames, rx) = mpsc::sync_channel(2);
        let stop = Arc::new(AtomicBool::new(false));
        let video = Process::default();
        let audio = Process::default();
        let clock = Arc::new(OnceLock::new());
        let cursor = Arc::new(Mutex::new(None));
        let speed = Arc::new(AtomicU64::new(rate.scale().to_bits()));
        let size = PreviewSize::playback(info);
        let fps = preview_fps(info.fps);
        let source = path.to_owned();
        let duration = (end - start).max(0.001);

        let audio_stop = Arc::new(AtomicBool::new(false));
        let audio_thread = info.has_audio().then(|| {
            spawn_audio(
                self.ctx.clone(),
                self.volume.clone(),
                AudioJob {
                    path: source.clone(),
                    start,
                    end,
                    rate,
                    clock: clock.clone(),
                    cursor: cursor.clone(),
                    handover: None,
                    stop: audio_stop.clone(),
                    decoder: audio.clone(),
                },
            )
        });
        let has_audio = audio_thread.is_some();

        let ctx = self.ctx.clone();
        let worker_stop = stop.clone();
        let worker_clock = clock.clone();
        let worker_speed = speed.clone();
        let worker_decoder = video.clone();
        let thread = std::thread::spawn(move || {
            run_preview(
                &source,
                start,
                duration,
                fps,
                size,
                worker_decoder,
                frames,
                ctx,
                worker_stop,
                worker_clock,
                worker_speed,
            );
        });

        self.session = Some(Session {
            rx,
            stop,
            video,
            audio,
            thread,
            audio_thread,
            audio_stop: has_audio.then_some(audio_stop),
            pending_audio: None,
            clock,
            cursor,
            rate: speed,
            source: path.to_owned(),
            span: (start, end),
            audio_only: None,
        });
    }

    pub fn start_audio(&mut self, path: &str, start: f64, end: f64, rate: PlaybackRate) {
        self.stop();
        let (_, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let audio = Process::default();
        let clock = Arc::new(OnceLock::from(Instant::now()));
        let cursor = Arc::new(Mutex::new(None));
        let speed = Arc::new(AtomicU64::new(rate.scale().to_bits()));
        let thread = spawn_audio(
            self.ctx.clone(),
            self.volume.clone(),
            AudioJob {
                path: path.to_owned(),
                start,
                end,
                rate,
                clock: clock.clone(),
                cursor: cursor.clone(),
                handover: None,
                stop: stop.clone(),
                decoder: audio.clone(),
            },
        );
        self.position = Some(start);
        self.session = Some(Session {
            rx,
            stop: stop.clone(),
            video: Process::default(),
            audio,
            thread,
            audio_thread: None,
            audio_stop: Some(stop),
            pending_audio: None,
            clock,
            cursor,
            rate: speed,
            source: path.to_owned(),
            span: (start, end),
            audio_only: Some((start, end, rate)),
        });
    }

    /// Applies a new speed to the preview that is already running.
    ///
    /// The picture is only re-paced, so the clip keeps playing without a single
    /// frozen frame. Sound has its tempo baked in by ffmpeg, so a decoder at the
    /// new tempo warms up behind the scenes and takes the device over as soon as
    /// it has something to play.
    pub fn set_rate(&mut self, rate: PlaybackRate) {
        let scale = rate.scale();
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if (scale_of(&session.rate) - scale).abs() < f64::EPSILON {
            return;
        }
        if session.audio_only.is_some() {
            // No picture to re-time, and the playhead follows the wall clock:
            // pick the sound up again wherever it has already got to.
            let path = session.source.clone();
            let end = session.span.1;
            let position = self.position.unwrap_or(session.span.0);
            self.start_audio(&path, position, end, rate);
            return;
        }

        session.rate.store(scale.to_bits(), Ordering::Relaxed);
        // Wake the pacer so the very next frame is already timed at the new speed.
        session.thread.thread().unpark();
        if session.audio_stop.is_none() {
            return;
        }

        let armed = Arc::new(AtomicBool::new(false));
        let go = Arc::new(AtomicBool::new(false));
        let audio_stop = Arc::new(AtomicBool::new(false));
        let decoder = Process::default();
        let job = PendingAudio {
            armed: armed.clone(),
            go: go.clone(),
            stop: audio_stop.clone(),
            decoder: decoder.clone(),
            thread: spawn_audio(
                self.ctx.clone(),
                self.volume.clone(),
                AudioJob {
                    path: session.source.clone(),
                    start: picture_media(session, Instant::now()),
                    end: session.span.1,
                    rate,
                    clock: session.clock.clone(),
                    cursor: session.cursor.clone(),
                    handover: Some(Handover { armed, go }),
                    stop: audio_stop,
                    decoder,
                },
            ),
        };
        // A second change of mind before the first handover: drop the half-warm
        // decoder instead of letting two of them pile up.
        let stale = session.pending_audio.replace(job);
        self.retired_audio.extend(stale.map(|stale| {
            stale.stop.store(true, Ordering::Relaxed);
            stale.decoder.cancel();
            stale.thread
        }));
    }

    pub fn stop(&mut self) {
        if let Some(mut session) = self.session.take() {
            session.stop.store(true, Ordering::Relaxed);
            session.thread.thread().unpark();
            session.video.cancel();
            session.audio.cancel();
            if let Some(stop) = session.audio_stop.take() {
                stop.store(true, Ordering::Relaxed);
            }
            if let Some(pending) = session.pending_audio.take() {
                pending.stop.store(true, Ordering::Relaxed);
                pending.decoder.cancel();
                self.retired_audio.push(pending.thread);
            }
            self.retired.push((session.thread, session.audio_thread));
        }
        self.position = None;
        self.retired.retain(|(video, audio)| {
            !video.is_finished() || audio.as_ref().is_some_and(|thread| !thread.is_finished())
        });
        self.retired_audio.retain(|thread| !thread.is_finished());
    }

    /// Cuts the sound over to the decoder that was warmed up at the new tempo,
    /// once it has its first buffers ready.
    fn swap_audio_if_ready(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let Some(pending) = session.pending_audio.as_ref() else {
            return;
        };
        if !pending.armed.load(Ordering::Relaxed) {
            return;
        }
        let pending = session
            .pending_audio
            .take()
            .expect("pending sound checked above");
        if let Some(stop) = session.audio_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        session.audio.cancel();
        let old_thread = session.audio_thread.take();
        session.audio = pending.decoder;
        session.audio_stop = Some(pending.stop);
        session.audio_thread = Some(pending.thread);
        // The old stream is on its way out; the new one waits for the device to
        // be released and then lines itself up with the picture.
        pending.go.store(true, Ordering::Relaxed);
        if let Some(thread) = old_thread {
            self.retired_audio.push(thread);
        }
    }

    pub fn poll(&mut self) -> Option<Frame> {
        self.swap_audio_if_ready();
        let session = self.session.as_mut()?;
        if let Some((start, end, _)) = session.audio_only {
            let elapsed = session
                .clock
                .get()
                .map_or(0.0, |origin| origin.elapsed().as_secs_f64());
            let position = (start + elapsed * scale_of(&session.rate)).min(end);
            if session.thread.is_finished() {
                self.stop();
            } else {
                self.ctx.request_repaint_after(Duration::from_millis(33));
            }
            self.position = Some(position);
            return None;
        }
        let mut latest = None;
        let ended = loop {
            match session.rx.try_recv() {
                Ok(frame) => latest = Some(frame),
                Err(TryRecvError::Empty) => break false,
                Err(TryRecvError::Disconnected) => break true,
            }
        };
        if let Some(frame) = &latest {
            let shown = Instant::now();
            session.clock.get_or_init(|| shown);
            *session.cursor.lock().unwrap() = Some(Cursor {
                media: frame.time,
                wall: shown,
            });
            self.position = Some(frame.time);
        }
        if ended {
            self.stop();
        }
        latest
    }
}

/// Clip time the picture is showing right now, extrapolated from the frame that
/// is on screen.
fn picture_media(session: &Session, now: Instant) -> f64 {
    let cursor = *session.cursor.lock().unwrap();
    match cursor {
        Some(cursor) => {
            let advance = now.saturating_duration_since(cursor.wall).as_secs_f64();
            (cursor.media + advance * scale_of(&session.rate)).min(session.span.1)
        }
        None => session.span.0,
    }
}

fn spawn_audio(ctx: egui::Context, volume: Arc<AtomicU32>, job: AudioJob) -> JoinHandle<()> {
    std::thread::spawn(move || {
        #[cfg(windows)]
        crate::audio::run(
            job.path,
            job.start,
            job.end,
            job.rate,
            volume,
            job.clock,
            job.cursor,
            job.handover,
            job.stop,
            job.decoder,
        );
        #[cfg(not(windows))]
        {
            let _ = volume;
            play_audio(
                &job.path,
                job.start,
                job.end,
                job.rate,
                job.clock,
                job.cursor,
                job.handover,
                job.stop,
                job.decoder,
            );
        }
        ctx.request_repaint();
    })
}

/// Paces the decoder's frames onto the wall clock.
///
/// The decoder always produces the clip's own frame rate, so the speed is applied
/// here: the shared speed word is read again for every single frame, which
/// re-times the rest of the clip from the picture that is on screen right now.
#[allow(clippy::too_many_arguments)]
fn run_preview(
    source: &str,
    start: f64,
    duration: f64,
    fps: f64,
    size: PreviewSize,
    decoder: Process,
    frames: SyncSender<Frame>,
    ctx: egui::Context,
    stop: Arc<AtomicBool>,
    clock: Arc<OnceLock<Instant>>,
    speed: Arc<AtomicU64>,
) {
    let mut command = decoder_command(source, start, size);
    command.args([
        "-t",
        &format!("{duration:.6}"),
        "-r",
        &format!("{fps:.3}"),
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "pipe:1",
    ]);
    if decoder.spawn(&mut command).is_err() {
        return;
    }
    let Some(mut pipe) = decoder.stdout() else {
        decoder.finish();
        return;
    };

    let mut buffer = vec![0; size.bytes()];
    let mut index = 0usize;
    let mut scheduled = Duration::ZERO;
    let mut scheduled_media = start;

    while !stop.load(Ordering::Relaxed) && pipe.read_exact(&mut buffer).is_ok() {
        let media = start + index as f64 / fps;
        if media - start >= duration {
            break;
        }
        if index == 0 {
            // Hand frame one over at once; the sound starts only after the UI
            // has shown it, and the clock it publishes then is the origin every
            // later frame is scheduled against.
            if !publish(&frames, &ctx, size.image(&buffer), media) {
                break;
            }
            index += 1;
            while clock.get().is_none() && !stop.load(Ordering::Relaxed) {
                std::thread::park_timeout(Duration::from_millis(4));
            }
            continue;
        }

        let (target, late) = loop {
            let scale = scale_of(&speed);
            let target = schedule(scheduled, scheduled_media, media, scale);
            let Some(origin) = clock.get().copied() else {
                std::thread::park_timeout(Duration::from_millis(4));
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                continue;
            };
            let deadline = origin + target;
            let now = Instant::now();
            if deadline > now {
                // A speed change unparks this wait, and the loop then aims the
                // frame at the new speed instead of holding the old deadline.
                std::thread::park_timeout(deadline - now);
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                continue;
            }
            // Half a frame of slack; anything later would visibly trail the
            // sound, so this picture is skipped rather than shown late.
            let tolerance = Duration::from_secs_f64(0.5 / (fps * scale));
            break (target, now - deadline > tolerance);
        };

        scheduled = target;
        scheduled_media = media;
        index += 1;
        if late {
            continue;
        }
        if !publish(&frames, &ctx, size.image(&buffer), media) {
            break;
        }
    }

    // Let the last picture stand for the rest of its own time.
    if !stop.load(Ordering::Relaxed) {
        if let Some(origin) = clock.get().copied() {
            let end = schedule(
                scheduled,
                scheduled_media,
                start + duration,
                scale_of(&speed),
            );
            std::thread::park_timeout((origin + end).saturating_duration_since(Instant::now()));
        }
    }
    decoder.finish();
}

fn publish(frames: &SyncSender<Frame>, ctx: &egui::Context, image: ColorImage, time: f64) -> bool {
    match frames.try_send(Frame { time, image }) {
        Ok(()) => {
            ctx.request_repaint();
            true
        }
        Err(mpsc::TrySendError::Full(_)) => true,
        Err(mpsc::TrySendError::Disconnected(_)) => false,
    }
}

pub fn upload(
    ctx: &egui::Context,
    texture: &mut Option<TextureHandle>,
    name: &str,
    image: ColorImage,
) {
    if let Some(texture) = texture {
        texture.set(image, TextureOptions::LINEAR);
    } else {
        *texture = Some(ctx.load_texture(name, image, TextureOptions::LINEAR));
    }
}

fn decoder_command(path: &str, time: f64, size: PreviewSize) -> std::process::Command {
    let mut command = cutgut::ffmpeg::command(&cutgut::ffmpeg::ffmpeg_path());
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-threads",
            "2",
            "-ss",
            &format!("{time:.6}"),
            "-i",
            path,
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-filter_threads",
            "1",
            "-vf",
            // Bicubic keeps edges readable when a 4K source is scaled down to
            // preview size; the cost is a fraction of the decode itself.
            &format!("scale={}:{}:flags=bicubic", size.width, size.height),
            "-threads",
            "1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

pub fn decode_still(
    path: &str,
    time: f64,
    size: PreviewSize,
    process: &Process,
) -> Option<ColorImage> {
    let mut command = decoder_command(path, time, size);
    command.args([
        "-frames:v",
        "1",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "pipe:1",
    ]);
    process.spawn(&mut command).ok()?;
    let mut buffer = vec![0; size.bytes()];
    let read = process
        .stdout()
        .is_some_and(|mut out| out.read_exact(&mut buffer).is_ok());
    process.finish();
    read.then(|| size.image(&buffer))
}

#[cfg(not(windows))]
#[allow(clippy::too_many_arguments)]
fn play_audio(
    path: &str,
    start: f64,
    end: f64,
    rate: PlaybackRate,
    clock: Arc<OnceLock<Instant>>,
    cursor: Arc<Mutex<Option<Cursor>>>,
    handover: Option<Handover>,
    stop: Arc<AtomicBool>,
    process: Process,
) {
    let from = match &handover {
        Some(handover) => {
            handover.armed.store(true, Ordering::Relaxed);
            while !handover.go.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(2));
            }
            let cursor = *cursor.lock().unwrap();
            cursor.map_or(start, |cursor| {
                cursor.media + cursor.wall.elapsed().as_secs_f64() * rate.scale()
            })
        }
        None => {
            while clock.get().is_none() && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(5));
            }
            let elapsed = clock
                .get()
                .map_or(Duration::ZERO, |origin| origin.elapsed());
            start + elapsed.as_secs_f64() * rate.scale()
        }
    };
    if stop.load(Ordering::Relaxed) {
        return;
    }
    let remaining = (end - from).max(0.05);
    let mut command = cutgut::ffmpeg::command(&cutgut::ffmpeg::ffplay_path());
    command
        .args([
            "-nodisp",
            "-vn",
            "-autoexit",
            "-loglevel",
            "quiet",
            "-ss",
            &format!("{from:.6}"),
            "-t",
            &format!("{remaining:.6}"),
            "-af",
            &rate.audio_filter(remaining),
            path,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if process.spawn(&mut command).is_ok() {
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(10));
        }
        process.finish();
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
        for (video, audio) in self.retired.drain(..) {
            let _ = video.join();
            if let Some(audio) = audio {
                let _ = audio.join();
            }
        }
        for audio in self.retired_audio.drain(..) {
            let _ = audio.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_decodes_the_clip_at_its_own_frame_rate() {
        assert_eq!(preview_fps(30.0), 30.0);
        assert_eq!(preview_fps(60.0), 60.0);
        // Slow motion gets every source frame, so no speed needs fewer pictures
        // per clip second than the clip itself has.
        assert_eq!(preview_fps(24.0), 24.0);
        assert_eq!(preview_fps(0.0), 12.0);
        assert_eq!(preview_fps(240.0), 60.0);
    }

    #[test]
    fn preview_size_follows_the_rotation_metadata() {
        let info = VideoInfo {
            duration: 10.0,
            width: 1920,
            height: 1080,
            fps: 60.0,
            bitrate: 0,
            codec: "h264".into(),
            audio_codec: None,
            rotation: -90,
            pix_fmt: "yuv420p".into(),
            color_transfer: "bt709".into(),
            audio_channels: 0,
        };
        let size = PreviewSize::playback(&info);
        assert!(
            size.height > size.width,
            "a portrait clip must not be previewed as landscape"
        );
    }

    #[test]
    fn speed_stretches_the_spacing_of_frames() {
        let fps = 60.0;
        let normal = schedule(Duration::ZERO, 0.0, 1.0 / fps, 1.0).as_secs_f64();
        assert!((normal - 1.0 / fps).abs() < 1e-6);
        let slow = schedule(Duration::ZERO, 0.0, 1.0 / fps, 0.5).as_secs_f64();
        assert!((slow - 2.0 / fps).abs() < 1e-6);
        let fast = schedule(Duration::ZERO, 0.0, 1.0 / fps, 4.0).as_secs_f64();
        assert!((fast - 1.0 / (4.0 * fps)).abs() < 1e-6);
    }

    #[test]
    fn a_speed_change_continues_from_the_picture_on_screen() {
        let fps = 30.0;
        // Frame 10 is due at 10/30 s; at that instant the speed is halved.
        let up_to = schedule(Duration::ZERO, 0.0, 10.0 / fps, 1.0);
        assert!((up_to.as_secs_f64() - 10.0 / fps).abs() < 1e-6);
        let next = schedule(up_to, 10.0 / fps, 11.0 / fps, 0.5);
        let expected = 10.0 / fps + 2.0 / fps;
        assert!(
            (next.as_secs_f64() - expected).abs() < 1e-6,
            "{} vs {expected}",
            next.as_secs_f64()
        );
    }

    #[test]
    fn audio_tempo_matches_every_preview_speed() {
        let sample_rate = 16_000.0;
        let ffmpeg = cutgut::ffmpeg::ffmpeg_path();
        for rate in PlaybackRate::OPTIONS {
            let filter = rate.audio_filter(2.0);
            let args = [
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=16000:duration=3",
                "-af",
                &filter,
                "-f",
                "s16le",
                "pipe:1",
            ]
            .map(str::to_owned);
            let output = cutgut::ffmpeg::run_process(&ffmpeg, &args).unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let duration = output.stdout.len() as f64 / (sample_rate * 2.0);
            let expected = 2.0 / rate.scale();
            assert!(
                (duration - expected).abs() < 0.06 * expected,
                "{}: {duration:.3}s vs {expected:.3}s",
                rate.label()
            );
        }
    }

    #[cfg(windows)]
    fn next_frame(player: &mut Player, timeout: Duration) -> Option<Frame> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(frame) = player.poll() {
                return Some(frame);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        None
    }

    /// What the user feels: changing the speed must neither stop the picture nor
    /// let the clip advance at the wrong pace.
    #[cfg(windows)]
    #[test]
    fn changing_the_speed_re_times_the_running_picture() {
        let path = "../temp/rust_test/sample.mp4";
        if !std::path::Path::new(path).is_file() {
            eprintln!("sample clip missing, skipping speed test");
            return;
        }
        let info = cutgut::media::probe_video(path).expect("sample information");
        let mut player = Player::new(egui::Context::default());
        player.set_volume(0.0);
        player.start(path, &info, 0.0, info.duration, PlaybackRate::Normal);
        assert!(next_frame(&mut player, Duration::from_secs(5)).is_some());

        for (rate, expected) in [(PlaybackRate::Half, 0.5), (PlaybackRate::Double, 2.0)] {
            player.set_rate(rate);
            // Let the pacer pick the new speed up before the window starts.
            std::thread::sleep(Duration::from_millis(150));

            let from = player.position().expect("running preview");
            let begin = Instant::now();
            let mut previous = begin;
            let mut longest = Duration::ZERO;
            let mut frames = 0;
            while begin.elapsed() < Duration::from_secs(1) {
                std::thread::sleep(Duration::from_millis(2));
                if player.poll().is_some() {
                    let shown = Instant::now();
                    longest = longest.max(shown - previous);
                    previous = shown;
                    frames += 1;
                }
            }
            let wall = begin.elapsed().as_secs_f64();
            let advance = player.position().expect("running preview") - from;
            let pace = advance / wall;
            assert!(
                (pace - expected).abs() < 0.3 * expected,
                "{}: advanced {pace:.2}× in {wall:.2}s",
                rate.label()
            );
            assert!(frames >= 12, "{}: only {frames} frames", rate.label());
            assert!(
                longest < Duration::from_millis(250),
                "{}: the picture stopped for {longest:?}",
                rate.label()
            );
        }
        player.stop();
    }

    #[cfg(windows)]
    #[test]
    fn stop_cancels_both_decoders_without_waiting_on_the_ui() {
        let path = "../temp/rust_test/sample.mp4";
        if !std::path::Path::new(path).is_file() {
            eprintln!("sample clip missing, skipping process integration test");
            return;
        }
        let info = cutgut::media::probe_video(path).expect("sample information");
        let mut player = Player::new(egui::Context::default());
        player.set_volume(0.0);
        for start in [0.0, 2.0, 1.0] {
            player.start(path, &info, start, info.duration, PlaybackRate::Normal);
            let deadline = Instant::now() + Duration::from_secs(4);
            let frame = loop {
                if let Some(frame) = player.poll() {
                    break frame;
                }
                assert!(
                    player.is_active() && Instant::now() < deadline,
                    "first frame"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(frame.time, start);
            let session = player.session.as_ref().unwrap();
            let video = session.video.clone();
            let audio = session.audio.clone();
            assert!(video.id().is_some());
            let deadline = Instant::now() + Duration::from_secs(2);
            while audio.id().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(audio.id().is_some(), "audio decoder started");
            let stop = Instant::now();
            player.stop();
            assert!(stop.elapsed() < Duration::from_millis(150));
            let deadline = Instant::now() + Duration::from_secs(2);
            while (video.id().is_some() || audio.id().is_some()) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                video.id().is_none() && audio.id().is_none(),
                "workers reaped the decoders"
            );
            assert!(!player.is_active());
        }
    }
}
