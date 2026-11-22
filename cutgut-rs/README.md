# CutGut — silnik (Rust)

Ten katalog zawiera natywny silnik CutGut: wykrywanie strumieni, planowanie bitrate'u,
kodowanie FFmpeg (H.264 / HEVC / AV1), 2-pass, remux, retry, eksport audio, miniatury,
filmstrip, skanowanie galerii, polityki usuwania źródła — plus CLI.

| Część | Zakres |
|---|---|
| **Silnik** | probe, plan, twarde limity rozmiaru, FFmpeg encode, 2-pass, remux, retry, audio, miniatury, galeria, cleanup |
| **GUI** | natywne okno egui: edytor, galeria, ustawienia (w `../cutgut-gui/`) |

## Szybki start (Windows)

GUI buduje się osobno — instrukcja jest w głównym
[README](../README.md#building-from-source). Sam silnik wystarczy do CLI:

```bat
cargo build --release
target\release\cutgut.exe --help
```

Wymaganie: FFmpeg (`ffmpeg.exe`, `ffprobe.exe`) w `PATH` lub obok pliku exe.

## Budowanie

Silnik ma **zero zależności zewnętrznych** i buduje się offline:

```bat
cargo build --release
target\release\cutgut.exe --help
```

Rozmiar/start (release, Windows): **~0,5 MB**, `--help` w **~0,15 s**.

## Użycie CLI

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

Wspólne opcje: `--start`, `--end`, `--target` (MB), `--codec h264|hevc|av1`,
`--speed hq|fast|balanced`, `--cpu` / `--gpu`, `--crop original|16:9|9:16|1:1`,
`--lang pl|en`, `--quiet`.

## Moduły

- `src/plan.rs` — `VideoInfo`, `CropBox`, `Plan`, `QualityAssessment`
- `src/ffmpeg.rs` — `CancellationToken`, `ProgressUpdate`, `encode_video`, `encode_audio`
- `src/media.rs` — `probe_video`, `generate_thumbnail`, `generate_filmstrip`,
  `scan_gallery`, nazewnictwo plików wyjściowych
- `src/main.rs` — CLI (`probe`, `engines`, `plan`, `encode`, `audio`, `thumb`,
  `filmstrip`, `gallery`, `cleanup`, interaktywne menu bez argumentów)

## Zachowanie silnika

- Twardy limit = `target_mb × 1 000 000` bajtów, weryfikowany po zakodowaniu.
- AV1 planuje z zapasem **94%** limitu (SVT-AV1/NVENC AV1 potrafi przestrzelić VBR),
  pozostałe kodeki 96%.
- Remux (`-c copy`) gdy plik już się mieści i nie wymaga kadrowania.
- Do 3 prób, każda liczona od pliku źródłowego, z zachowaniem wybranego kodeka.
- Wykrywanie enkoderów realnym testem 1 klatki (NVENC, AMF, CPU), z fallbackiem.

Szczegóły: [`../docs/EXPORT_QUALITY_AND_HARD_LIMIT.md`](../docs/EXPORT_QUALITY_AND_HARD_LIMIT.md).
