//! Command line texts (PL/EN).
//!
//! The CLI mirrors the GUI language: `--lang pl` prints Polish, `--lang en`
//! prints English. Every user-visible line of the CLI goes through [`t`].

use crate::i18n;

const USAGE_PL: &str = r#"CutGut (silnik Rust) — planowanie i kodowanie FFmpeg

Użycie:
  cutgut <komenda> [argumenty]

Komendy:
  probe <plik>                       Informacje o pliku wideo
  engines                            Wykryj dostępne enkodery (NVENC/AMF/CPU)
  plan <plik> [opcje]                Pokaż plan eksportu (bitrate, jakość, remux)
  encode <plik> --out <plik> [opcje] Eksportuj wideo z twardym limitem rozmiaru
  audio <plik> --out <plik> [opcje]  Eksportuj tylko audio (m4a/opus/mp3)
  thumb <plik> --out <png> [--at s]  Zapisz miniaturę klatki
  filmstrip <plik> --out <png>       Wygeneruj pasek miniaturek
  gallery [folder]                   Wypisz klipy pogrupowane po grach
  cleanup <wej> <wyj> --policy P     Polityka usuwania źródła (never/trash/delete)

Opcje wspólne:
  --start <s>      Początek (domyślnie 0)
  --end <s>        Koniec (domyślnie długość pliku)
  --target <MB>    Limit rozmiaru w MB (domyślnie 20)
  --codec <c>      h264 | hevc | av1 (domyślnie h264)
  --speed <s>      hq | fast | balanced (domyślnie hq)
  --cpu / --gpu    Wymuś enkoder programowy / sprzętowy
  --crop <ratio>   original | 16:9 | 9:16 | 1:1
  --lang <pl|en>   Język komunikatów (domyślnie pl)
  --quiet          Bez paska postępu

Przykłady:
  cutgut plan gameplay.mp4 --start 10 --end 40 --target 20 --codec av1
  cutgut encode gameplay.mp4 --out out.mp4 --start 10 --end 40 --target 20 --codec av1 --gpu
  cutgut audio gameplay.mp4 --out out.m4a --format m4a
  cutgut gallery "%USERPROFILE%\Videos\NVIDIA"
"#;

const USAGE_EN: &str = r#"CutGut (Rust engine) — FFmpeg planning and encoding

Usage:
  cutgut <command> [arguments]

Commands:
  probe <file>                        Show video file information
  engines                             Detect available encoders (NVENC/AMF/CPU)
  plan <file> [options]               Show the export plan (bitrate, quality, remux)
  encode <file> --out <file> [options] Export video with a strict size limit
  audio <file> --out <file> [options]  Export audio only (m4a/opus/mp3)
  thumb <file> --out <png> [--at s]   Save a frame thumbnail
  filmstrip <file> --out <png>        Generate a filmstrip
  gallery [folder]                    List clips grouped by game
  cleanup <in> <out> --policy P       Source cleanup policy (never/trash/delete)

Common options:
  --start <s>      Start (default 0)
  --end <s>        End (default: full file length)
  --target <MB>    Size limit in MB (default 20)
  --codec <c>      h264 | hevc | av1 (default h264)
  --speed <s>      hq | fast | balanced (default hq)
  --cpu / --gpu    Force software / hardware encoder
  --crop <ratio>   original | 16:9 | 9:16 | 1:1
  --lang <pl|en>   Message language (default pl)
  --quiet          No progress bar

Examples:
  cutgut plan gameplay.mp4 --start 10 --end 40 --target 20 --codec av1
  cutgut encode gameplay.mp4 --out out.mp4 --start 10 --end 40 --target 20 --codec av1 --gpu
  cutgut audio gameplay.mp4 --out out.m4a --format m4a
  cutgut gallery "%USERPROFILE%\Videos\NVIDIA"
"#;

/// Translates a CLI key into the active language.
pub fn t(key: &str) -> &'static str {
    let pl = i18n::is_pl();
    match key {
        "usage" => {
            if pl {
                USAGE_PL
            } else {
                USAGE_EN
            }
        }
        "error" => {
            if pl {
                "Błąd"
            } else {
                "Error"
            }
        }
        "unknown_command" => {
            if pl {
                "Nieznana komenda"
            } else {
                "Unknown command"
            }
        }
        "menu_title" => {
            if pl {
                "CutGut — silnik Rust (menu)"
            } else {
                "CutGut — Rust engine (menu)"
            }
        }
        "menu_gallery" => {
            if pl {
                "1) Galeria klipów (Videos\\NVIDIA)"
            } else {
                "1) Clip gallery (Videos\\NVIDIA)"
            }
        }
        "menu_plan" => {
            if pl {
                "2) Pokaż plan eksportu dla pliku"
            } else {
                "2) Show the export plan for a file"
            }
        }
        "menu_encode" => {
            if pl {
                "3) Eksportuj klip (twardy limit rozmiaru)"
            } else {
                "3) Export a clip (strict size limit)"
            }
        }
        "menu_audio" => {
            if pl {
                "4) Eksportuj tylko audio"
            } else {
                "4) Export audio only"
            }
        }
        "menu_still" => {
            if pl {
                "5) Miniatura / filmstrip"
            } else {
                "5) Thumbnail / filmstrip"
            }
        }
        "menu_engines" => {
            if pl {
                "6) Wykryj enkodery (NVENC / AMF / CPU)"
            } else {
                "6) Detect encoders (NVENC / AMF / CPU)"
            }
        }
        "menu_exit" => {
            if pl {
                "0) Wyjście"
            } else {
                "0) Exit"
            }
        }
        "choice" => {
            if pl {
                "Wybór"
            } else {
                "Choice"
            }
        }
        "unknown_choice" => {
            if pl {
                "Nieznany wybór"
            } else {
                "Unknown choice"
            }
        }
        "no_file" => {
            if pl {
                "Nie podano pliku."
            } else {
                "No file was given."
            }
        }
        "prompt_gallery_dir" => {
            if pl {
                "Folder galerii (Enter = domyślny)"
            } else {
                "Gallery folder (Enter = default)"
            }
        }
        "prompt_video" => {
            if pl {
                "Plik wideo"
            } else {
                "Video file"
            }
        }
        "prompt_output" => {
            if pl {
                "Plik wyjściowy"
            } else {
                "Output file"
            }
        }
        "prompt_output_next" => {
            if pl {
                "Plik wyjściowy (Enter = obok źródła)"
            } else {
                "Output file (Enter = next to the source)"
            }
        }
        "prompt_format" => {
            if pl {
                "Format (m4a/opus/mp3)"
            } else {
                "Format (m4a/opus/mp3)"
            }
        }
        "prompt_what" => {
            if pl {
                "Co wygenerować? (t = miniatura, f = filmstrip)"
            } else {
                "What to generate? (t = thumbnail, f = filmstrip)"
            }
        }
        "prompt_output_image" => {
            if pl {
                "Plik wyjściowy (jpg/png)"
            } else {
                "Output file (jpg/png)"
            }
        }
        "prompt_start" => {
            if pl {
                "Początek w sekundach"
            } else {
                "Start in seconds"
            }
        }
        "prompt_end" => {
            if pl {
                "Koniec w sekundach (Enter = do końca)"
            } else {
                "End in seconds (Enter = to the end)"
            }
        }
        "prompt_target" => {
            if pl {
                "Limit rozmiaru w MB"
            } else {
                "Size limit in MB"
            }
        }
        "prompt_codec" => {
            if pl {
                "Kodek (h264/hevc/av1)"
            } else {
                "Codec (h264/hevc/av1)"
            }
        }
        "prompt_speed" => {
            if pl {
                "Szybkość (hq/fast)"
            } else {
                "Speed (hq/fast)"
            }
        }
        "prompt_cpu" => {
            if pl {
                "Wymusić CPU? (t/N)"
            } else {
                "Force CPU? (t/N)"
            }
        }
        "need_path_probe" => {
            if pl {
                "Podaj ścieżkę pliku: cutgut probe <plik>"
            } else {
                "Provide a file path: cutgut probe <file>"
            }
        }
        "need_path_plan" => {
            if pl {
                "Podaj ścieżkę pliku: cutgut plan <plik>"
            } else {
                "Provide a file path: cutgut plan <file>"
            }
        }
        "need_input_encode" => {
            if pl {
                "Podaj plik wejściowy: cutgut encode <plik> --out <plik>"
            } else {
                "Provide an input file: cutgut encode <file> --out <file>"
            }
        }
        "need_input_audio" => {
            if pl {
                "Podaj plik wejściowy: cutgut audio <plik> --out <plik>"
            } else {
                "Provide an input file: cutgut audio <file> --out <file>"
            }
        }
        "need_input_thumb" => {
            if pl {
                "Podaj plik: cutgut thumb <plik> --out <png>"
            } else {
                "Provide a file: cutgut thumb <file> --out <png>"
            }
        }
        "need_input_filmstrip" => {
            if pl {
                "Podaj plik: cutgut filmstrip <plik> --out <png>"
            } else {
                "Provide a file: cutgut filmstrip <file> --out <png>"
            }
        }
        "missing_out" => {
            if pl {
                "Brak --out <plik wyjściowy>"
            } else {
                "Missing --out <output file>"
            }
        }
        "missing_out_png" => {
            if pl {
                "Brak --out <png>"
            } else {
                "Missing --out <png>"
            }
        }
        "unknown_codec" => {
            if pl {
                "Nieznany kodek"
            } else {
                "Unknown codec"
            }
        }
        "label_file" => {
            if pl {
                "Plik"
            } else {
                "File"
            }
        }
        "label_image" => {
            if pl {
                "Obraz"
            } else {
                "Image"
            }
        }
        "label_duration" => {
            if pl {
                "Czas"
            } else {
                "Duration"
            }
        }
        "label_codec" => {
            if pl {
                "Kodek"
            } else {
                "Codec"
            }
        }
        "label_audio" => {
            if pl {
                "Audio"
            } else {
                "Audio"
            }
        }
        "label_bitrate" => {
            if pl {
                "Bitrate"
            } else {
                "Bitrate"
            }
        }
        "none" => {
            if pl {
                "brak"
            } else {
                "none"
            }
        }
        "detect_encoders" => {
            if pl {
                "Wykrywanie enkoderów (test 1 klatki)..."
            } else {
                "Detecting encoders (1 frame test)..."
            }
        }
        "nvenc_flags" => {
            if pl {
                "Flagi jakości NVENC:"
            } else {
                "NVENC quality flags:"
            }
        }
        "label_encoder" => {
            if pl {
                "Enkoder"
            } else {
                "Encoder"
            }
        }
        "label_clip_duration" => {
            if pl {
                "Czas klipu"
            } else {
                "Clip duration"
            }
        }
        "label_hard_limit" => {
            if pl {
                "Limit twardy"
            } else {
                "Hard limit"
            }
        }
        "label_budget" => {
            if pl {
                "Cel budżetu"
            } else {
                "Budget target"
            }
        }
        "label_video" => {
            if pl {
                "Wideo"
            } else {
                "Video"
            }
        }
        "label_remux" => {
            if pl {
                "Remux"
            } else {
                "Remux"
            }
        }
        "label_hdr" => {
            if pl {
                "HDR"
            } else {
                "HDR"
            }
        }
        "hdr_tonemapped" => {
            if pl {
                "tak (mapowanie na SDR / bt709)"
            } else {
                "yes (mapped to SDR / bt709)"
            }
        }
        "hdr_untouched" => {
            if pl {
                "tak, ale brak zscale w ffmpeg — kolory mogą być wyprane"
            } else {
                "yes, but this ffmpeg has no zscale — colours may be washed out"
            }
        }
        "hdr_none" => {
            if pl {
                "nie"
            } else {
                "no"
            }
        }
        "limit_capped" => {
            if pl {
                "Limit większy, niż klip potrzebuje — plik będzie mniejszy"
            } else {
                "Limit larger than the clip needs — the file will be smaller"
            }
        }
        "label_quality" => {
            if pl {
                "Jakość"
            } else {
                "Quality"
            }
        }
        "label_tip" => {
            if pl {
                "Tip"
            } else {
                "Tip"
            }
        }
        "yes" => {
            if pl {
                "tak"
            } else {
                "yes"
            }
        }
        "no" => {
            if pl {
                "nie"
            } else {
                "no"
            }
        }
        "label_input" => {
            if pl {
                "Wejście"
            } else {
                "Input"
            }
        }
        "label_output" => {
            if pl {
                "Wyjście"
            } else {
                "Output"
            }
        }
        "label_limit" => {
            if pl {
                "Limit"
            } else {
                "Limit"
            }
        }
        "done" => {
            if pl {
                "Gotowe"
            } else {
                "Done"
            }
        }
        "thumbnail" => {
            if pl {
                "Miniatura"
            } else {
                "Thumbnail"
            }
        }
        "filmstrip" => {
            if pl {
                "Filmstrip"
            } else {
                "Filmstrip"
            }
        }
        "folder_missing" => {
            if pl {
                "Folder nie istnieje"
            } else {
                "Folder does not exist"
            }
        }
        "no_clips" => {
            if pl {
                "Brak klipów w"
            } else {
                "No clips in"
            }
        }
        "gallery" => {
            if pl {
                "Galeria"
            } else {
                "Gallery"
            }
        }
        "clips_word" => {
            if pl {
                "klipów"
            } else {
                "clips"
            }
        }
        "cleanup_usage" => {
            if pl {
                "Użycie: cutgut cleanup <wejście> <wyjście> --policy never|trash|delete"
            } else {
                "Usage: cutgut cleanup <input> <output> --policy never|trash|delete"
            }
        }
        _ => Box::leak(key.to_string().into_boxed_str()),
    }
}
