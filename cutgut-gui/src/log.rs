//! Append-only log in `%APPDATA%\CutGut\logs`.
//!
//! Exports hand ffmpeg a long argument list and can fail with a page of stderr;
//! a four-second toast cannot carry that. Every job records what it was asked to
//! do and how it ended, and the error dialog shows the same text plus where the
//! file lives — so a bad export can be diagnosed after the fact.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::settings::config_dir;

/// How many daily files are kept before the oldest ones are dropped.
const KEEP_FILES: usize = 7;

fn log_dir() -> PathBuf {
    config_dir().join("logs")
}

fn log_path() -> PathBuf {
    let day = chrono::Local::now().format("%Y-%m-%d");
    log_dir().join(format!("cutgut-{day}.log"))
}

/// Folder holding the logs; the error dialog offers to open it.
pub fn directory() -> PathBuf {
    log_dir()
}

/// Dropped when the log gets too long, so one runaway job cannot fill a disk.
fn trim_old_files() {
    let Ok(entries) = std::fs::read_dir(log_dir()) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    if files.len() <= KEEP_FILES {
        return;
    }
    files.sort();
    let excess = files.len() - KEEP_FILES;
    for path in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

/// Writes one timestamped line. Never fails the app: a log that cannot be
/// written is not worth interrupting an export for.
pub fn write(category: &str, text: impl AsRef<str>) {
    static PRUNED: OnceLock<()> = OnceLock::new();
    if PRUNED.set(()).is_ok() {
        trim_old_files();
    }
    let dir = log_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let line = format!(
        "{} [{category}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        text.as_ref().replace('\n', "\n    ")
    );
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// ffmpeg build string, filled in once in the background: asking for it costs a
/// process start, which is too much for the UI thread and too useful to skip.
static VERSION: OnceLock<Mutex<String>> = OnceLock::new();

fn version_slot() -> &'static Mutex<String> {
    VERSION.get_or_init(|| Mutex::new(String::new()))
}

pub fn ffmpeg_version() -> String {
    version_slot().lock().unwrap().clone()
}

/// Records the environment once per run and warms the ffmpeg version.
pub fn startup() {
    write(
        "app",
        format!(
            "CutGut {} · {} · settings in {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            config_dir().display()
        ),
    );
    std::thread::spawn(|| {
        let path = cutgut::ffmpeg::ffmpeg_path();
        let version = cutgut::ffmpeg::run_process(
            &path,
            &["-hide_banner".to_string(), "-version".to_string()],
        )
        .ok()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default();
        let line = if version.is_empty() {
            format!("ffmpeg not usable: {path}")
        } else {
            format!("{path} · {version}")
        };
        *version_slot().lock().unwrap() = line.clone();
        write("app", line);
    });
}

/// Header for a bug report: environment plus the tail of today's log.
pub fn diagnostics() -> String {
    let dir = log_dir();
    let text = std::fs::read_to_string(log_path()).unwrap_or_default();
    let tail: Vec<&str> = text.lines().rev().take(60).collect();
    let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
    format!(
        "CutGut {}\n{}\nffmpeg: {}\nlog: {}\n\n{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        ffmpeg_version(),
        dir.display(),
        tail
    )
}
