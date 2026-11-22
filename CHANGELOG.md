# Changelog

The Python/Tkinter prototype that came before this is gone from the tree, so the log starts
with the native Rust version. Changes are grouped by release instead of by commit.

## 0.1.0 — 2026-11-22

First tagged build of the Rust app.

### Added

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

### Changed

- Cuts that only need a container change are remuxed instead of re-encoded. A remux that cannot
  hit the requested cut is upgraded to an exact re-encode.
- HDR (PQ/HLG) is tonemapped to SDR and tagged as bt709 instead of being passed through with
  washed-out colours.
- 5.1 audio is downmixed to stereo at the planned bitrate.
- Rotation metadata is applied to the preview, the crop box and the bitrate plan.
- The video bitrate is capped at a sane bits-per-pixel value so very large limits do not ask
  the hardware encoder for an impossible stream.

### Fixed

- A cut that missed the nearest keyframe could produce a file with extra seconds at the end.
- Failed exports left FFmpeg processes running.
- Non-ASCII paths could panic the stderr tail reader when the character limit landed inside a
  multi-byte character.
