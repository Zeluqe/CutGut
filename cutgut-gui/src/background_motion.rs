use std::{
    io::{self, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

use cutgut::ffmpeg;
use eframe::egui;

use crate::process::Process;

const WIDTH: usize = 960;
const HEIGHT: usize = 540;
/// Built-in loops used to be uploaded as full 1920x1080 RGBA frames (8.3 MB per
/// frame into the UI thread). 720p cuts the per-frame transfer by more than half
/// and is indistinguishable behind the background veil.
const FULL_WIDTH: usize = 1280;
const FULL_HEIGHT: usize = 720;
const FIT: &str = "scale=960:540:force_original_aspect_ratio=increase,crop=960:540,setsar=1";
const CUSTOM_FPS: u32 = 30;
const FRAME_BUFFER: usize = 8;

pub fn open_built_in(bytes: &[u8]) -> io::Result<tempfile::NamedTempFile> {
    let mut file = tempfile::Builder::new().suffix(".webm").tempfile()?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(file)
}

/// Cached custom backgrounds are stored with a `.motion` extension, so the file
/// name cannot be trusted to tell a GIF apart from a video. Sniff the GIF magic
/// bytes instead, otherwise `-ignore_loop 1` would silently be skipped.
fn is_gif(path: &Path) -> bool {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
    {
        return true;
    }
    let mut header = [0u8; 3];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .is_ok_and(|()| &header == b"GIF")
}

fn input_command(path: &Path) -> Command {
    let mut command = ffmpeg::command(&ffmpeg::ffmpeg_path());
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-threads",
        "4",
        "-filter_threads",
        "2",
    ]);
    if is_gif(path) {
        command.args(["-ignore_loop", "1"]);
    }
    command
}

pub fn poster(path: &Path) -> io::Result<Vec<u8>> {
    let output = input_command(path)
        .arg("-i")
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-an",
            "-vf",
            FIT,
            "-frames:v",
            "1",
            "-c:v",
            "mjpeg",
            "-q:v",
            "2",
            "-f",
            "image2pipe",
            "pipe:1",
        ])
        .output()?;
    if output.status.success() && !output.stdout.is_empty() {
        Ok(output.stdout)
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

pub struct MotionBackground {
    process: Process,
    frames: Option<Receiver<egui::ColorImage>>,
    thread: Option<std::thread::JoinHandle<()>>,
    frame_period: Duration,
    next_frame: Option<Instant>,
}

impl MotionBackground {
    pub fn start(ctx: &egui::Context, path: &Path) -> Self {
        Self::start_filtered(
            ctx,
            path,
            [WIDTH, HEIGHT],
            Some(&format!("{FIT},fps={CUSTOM_FPS}")),
            CUSTOM_FPS,
        )
    }

    pub fn start_built_in(ctx: &egui::Context, path: &Path) -> Self {
        Self::start_filtered(
            ctx,
            path,
            [FULL_WIDTH, FULL_HEIGHT],
            Some(&format!("scale={FULL_WIDTH}:{FULL_HEIGHT},setsar=1")),
            24,
        )
    }

    fn start_filtered(
        ctx: &egui::Context,
        path: &Path,
        size: [usize; 2],
        filter: Option<&str>,
        fps: u32,
    ) -> Self {
        let process = Process::default();
        let decoder = process.clone();
        let (tx, frames) = mpsc::sync_channel(FRAME_BUFFER);
        let ctx = ctx.clone();
        let path = path.to_path_buf();
        let filter = filter.map(str::to_owned);
        let thread = std::thread::spawn(move || {
            let mut command = input_command(&path);
            command
                .args(["-stream_loop", "-1", "-i"])
                .arg(path)
                .args(["-map", "0:v:0", "-an"]);
            if let Some(filter) = filter {
                command.arg("-vf").arg(filter);
            }
            command
                .args([
                    "-fps_mode",
                    "passthrough",
                    "-pix_fmt",
                    "rgba",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            if decoder.spawn(&mut command).is_ok() {
                if let Some(mut pipe) = decoder.stdout() {
                    let mut pixels = vec![0; size[0] * size[1] * 4];
                    let mut first = true;
                    while pipe.read_exact(&mut pixels).is_ok() {
                        let frame = egui::ColorImage::from_rgba_premultiplied(size, &pixels);
                        if tx.send(frame).is_err() {
                            break;
                        }
                        if std::mem::take(&mut first) {
                            ctx.request_repaint();
                        }
                    }
                }
                decoder.finish();
            }
        });
        Self {
            process,
            frames: Some(frames),
            thread: Some(thread),
            frame_period: Duration::from_secs_f64(1.0 / f64::from(fps)),
            next_frame: None,
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context) -> Option<egui::ColorImage> {
        let now = Instant::now();
        let deadline = self.next_frame.unwrap_or(now);
        if deadline > now {
            repaint_at(ctx, deadline - now);
            return None;
        }
        let frames = self.frames.as_ref()?;
        let mut frame = match frames.try_recv() {
            Ok(frame) => frame,
            Err(TryRecvError::Empty) => {
                repaint_at(ctx, Duration::from_millis(5));
                return None;
            }
            Err(TryRecvError::Disconnected) => return None,
        };
        let behind = (now - deadline).as_nanos() / self.frame_period.as_nanos();
        for _ in 0..behind.min(2) {
            if let Ok(next) = frames.try_recv() {
                frame = next;
            } else {
                break;
            }
        }
        let next = if behind > 2 {
            now + self.frame_period
        } else {
            deadline + self.frame_period * (behind as u32 + 1)
        };
        self.next_frame = Some(next);
        repaint_at(ctx, next.saturating_duration_since(now));
        Some(frame)
    }

    pub fn interpolate(&self, ctx: &egui::Context) -> f32 {
        let Some(deadline) = self.next_frame else {
            return 0.0;
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if !remaining.is_zero() {
            repaint_at(ctx, remaining.min(Duration::from_secs_f64(1.0 / 60.0)));
        }
        (1.0 - remaining.as_secs_f32() / self.frame_period.as_secs_f32()).clamp(0.0, 1.0)
    }
}

fn repaint_at(ctx: &egui::Context, delay: Duration) {
    // Polling is deadline based; padding the request by a whole predicted frame
    // made every wake-up land after the deadline and pushed the frame pacing off.
    ctx.request_repaint_after(delay);
}

impl Drop for MotionBackground {
    fn drop(&mut self) {
        drop(self.frames.take());
        self.process.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn built_in_loop_plays_without_an_external_asset() {
        let bytes = crate::background_loops::preset_loop("orchid-dusk").unwrap();
        let file = open_built_in(bytes).unwrap();
        let playback = MotionBackground::start_built_in(&egui::Context::default(), file.path());
        let first = playback
            .frames
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(first.size, [FULL_WIDTH, FULL_HEIGHT]);
        for _ in 0..5 {
            let next = playback
                .frames
                .as_ref()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            if next.pixels != first.pixels {
                return;
            }
        }
        panic!("the built-in background did not animate");
    }

    #[test]
    fn gifs_and_videos_produce_posters_and_loop_from_local_copies() {
        let directory = tempfile::tempdir().unwrap();
        for extension in ["gif", "mp4"] {
            let file = directory.path().join(format!("fixture.{extension}"));
            let output = ffmpeg::command(&ffmpeg::ffmpeg_path())
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=96x54:rate=8",
                    "-t",
                    "0.5",
                    "-filter_threads",
                    "1",
                ])
                .arg(&file)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{extension}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!poster(&file).unwrap().is_empty());
            let local = file.with_extension("motion");
            std::fs::rename(file, &local).unwrap();
            let playback = MotionBackground::start(&egui::Context::default(), &local);
            // Two dozen 30 fps frames span 0.8 s, so the half-second source repeats.
            for _ in 0..24 {
                let frame = playback
                    .frames
                    .as_ref()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(4))
                    .unwrap_or_else(|e| panic!("{extension}: {e}"));
                assert_eq!(frame.size, [WIDTH, HEIGHT]);
            }
            drop(playback);
            std::fs::remove_file(local).unwrap();
        }
    }
}
