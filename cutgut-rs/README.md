# CutGut engine (Rust)

This crate is the engine behind CutGut: stream probing, bitrate planning, FFmpeg encoding
(H.264 / HEVC / AV1), two-pass, remux, retries, audio export, thumbnails, filmstrips, gallery
scanning and cleanup. The native UI lives in [`../cutgut-gui/`](../cutgut-gui/).

| Part | Scope |
| --- | --- |
| Engine | probe, plan, hard size limits, FFmpeg encode, 2-pass, remux, retry, audio, thumbnails, gallery, cleanup |
| GUI | native egui window: editor, gallery, settings (in `../cutgut-gui/`) |

## Quick start (Windows)

The GUI is built separately; see the main [README](../README.md#building-from-source). The
engine alone is enough for the CLI:

```bat
cargo build --release
target\release\cutgut.exe --help
```

Requirement: FFmpeg (`ffmpeg.exe`, `ffprobe.exe`) in `PATH` or next to the executable.

## Building

The engine has **no third-party Rust dependencies** and builds offline:

```bat
cargo build --release
target\release\cutgut.exe --help
```

Release size/startup on Windows: **~0.5 MB**, `--help` in **~0.15 s**.

## CLI

```bat
cutgut probe clip.mp4
cutgut engines
cutgut plan clip.mp4 --start 10 --end 40 --target 20 --codec av1
cutgut encode clip.mp4 --out out.mp4 --start 10 --end 40 --target 20 --codec av1 --gpu
cutgut audio clip.mp4 --out out.m4a --format m4a --end 30
cutgut thumb clip.mp4 --out thumb.jpg --at 2
cutgut filmstrip clip.mp4 --out strip.png --count 20
cutgut gallery "%USERPROFILE%\Videos\NVIDIA"
cutgut cleanup in.mp4 out.mp4 --policy trash
```

Common options: `--start`, `--end`, `--target` (MB), `--codec h264|hevc|av1`,
`--speed hq|fast|balanced`, `--cpu` / `--gpu`, `--crop original|16:9|9:16|1:1`,
`--lang pl|en` (default `en`), `--quiet`.

## Modules

- `src/plan.rs` — `VideoInfo`, `CropBox`, `Plan`, `QualityAssessment`
- `src/ffmpeg.rs` — `CancellationToken`, `ProgressUpdate`, `encode_video`, `encode_audio`
- `src/media.rs` — `probe_video`, `generate_thumbnail`, `generate_filmstrip`,
  `scan_gallery`, output naming
- `src/main.rs` — CLI (`probe`, `engines`, `plan`, `encode`, `audio`, `thumb`,
  `filmstrip`, `gallery`, `cleanup`, and the interactive menu when run without arguments)

## Engine behavior

- Hard limit = `target_mb × 1,000,000` bytes, verified after encoding.
- AV1 plans to 94% of the limit (SVT-AV1 / NVENC AV1 VBR can overshoot); other codecs use 96%.
- Remux (`-c copy`) when the clip already fits and does not need cropping.
- Up to three attempts, each starting from the source, with the same codec.
- Hardware encoder detection by a real one-frame encode (NVENC, AMF, CPU), with fallback.

Details: [`../docs/EXPORT_QUALITY_AND_HARD_LIMIT.md`](../docs/EXPORT_QUALITY_AND_HARD_LIMIT.md).
