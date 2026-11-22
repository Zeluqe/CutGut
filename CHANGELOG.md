# Changelog

The Python/Tkinter prototype that came before this is gone from the tree, so the log starts
with the native Rust version.

## How versions are named

Releases use `vMAJOR.MINOR.PATCH-YYYY.MM.DD` (this release is `v0.2.0-2026.11.22`). The app shows
the short `MAJOR.MINOR.PATCH` part, and the date is the day the build was made. Every entry
below is grouped by what actually changed: **New features**, **Fixes**, **GUI**, **Docs** and
**Internal**. A release that only fixes bugs still gets a patch bump; a new feature or a
visible GUI change bumps the minor version.

## 0.2.0 — 2026-11-22

### Fixes

- Frame stepping with `,` / `.` no longer flashes a cached frame that can be up to a fifth of
  a second away. The editor keeps the current picture until the exact frame arrives, and a
  decode that was superseded by the next step is dropped instead of shown.

### Docs

- New English screenshots of the gallery, editor, export panel and settings.
- The technical export document is now in English, with a Polish version next to it. The
  LaTeX formulas that GitHub rendered as `'_' allowed only in math mode` were replaced with
  plain code blocks.
- README explains that CutGut is built around NVIDIA's built-in `Videos\NVIDIA` recordings and
  Discord's free 20 MB limit.
- Engine README translated to English.

### GUI

- English is the default interface language. Existing installs that already saved `lang=pl`
  stay Polish; a fresh install starts in English.

## 0.1.0 — 2026-11-22

First tagged build of the Rust app.

### New features

- Native editor: frame-accurate IN/OUT, timeline with real frames, zoom, frame stepping,
  scrubbing and a 9:16 / 1:1 / 16:9 crop overlay.
- Export panel with H.264, HEVC and AV1, NVENC/AMF/CPU selection, MP4/MKV/M4A/Opus/MP3 output,
  a live quality estimate and a PNG frame snapshot.
- Hard size limit in decimal megabytes. The result is measured after encoding and re-planned
  up to three times if it does not fit.
- Hardware encoder detection by encoding a real frame, with a clear fallback to CPU.
- Gallery grouped by game with cached thumbnails, search, favourites, multi-select, clip info,
  rename/delete/import and a storage breakdown per game.
- CLI (`cutgut probe|engines|plan|encode|audio|thumb|filmstrip|gallery|cleanup`) built on the
  same engine as the GUI.
- Polish and English interface, light and dark themes, settings stored in `%APPDATA%\CutGut`.

### Fixes

- A cut that missed the nearest keyframe could produce a file with extra seconds at the end.
- Failed exports left FFmpeg processes running.
- Non-ASCII paths could panic the stderr tail reader when the character limit landed inside a
  multi-byte character.
- The background playback test no longer fails on Windows when the decoder still holds the
  file open at cleanup time.
- The release no longer tries to upload `CutGut.exe` and `cutgut.exe` as two assets that
  collide case-insensitively. The CLI ships as `cutgut-cli.zip`.

### GUI

- Cuts that only need a container change are remuxed instead of re-encoded. A remux that cannot
  hit the requested cut is upgraded to an exact re-encode.
- HDR (PQ/HLG) is tonemapped to SDR and tagged as bt709 instead of being passed through with
  washed-out colours.
- 5.1 audio is downmixed to stereo at the planned bitrate.
- Rotation metadata is applied to the preview, the crop box and the bitrate plan.
- The video bitrate is capped at a sane bits-per-pixel value so very large limits do not ask
  the hardware encoder for an impossible stream.
