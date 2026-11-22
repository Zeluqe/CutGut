//! Tiny PL/EN translation table shared by the library and the CLI.
//!
//! The library runs one language per process, so the active language is kept
//! in a global cell. Front ends call [`set_lang`] once at startup (and again
//! whenever the user switches language) and every message produced by the
//! encoder, the media helpers and the planner is then localized.

use std::sync::atomic::{AtomicU8, Ordering};

const PL: u8 = 0;
const EN: u8 = 1;

static LANG: AtomicU8 = AtomicU8::new(EN);

/// Switches the active language. Unknown codes fall back to English.
pub fn set_lang(code: &str) {
    let next = if code.trim().eq_ignore_ascii_case("pl") {
        PL
    } else {
        EN
    };
    LANG.store(next, Ordering::Relaxed);
}

/// Returns the active language code (`"pl"` or `"en"`).
pub fn lang_code() -> &'static str {
    if LANG.load(Ordering::Relaxed) == PL {
        "pl"
    } else {
        "en"
    }
}

/// True when Polish is the active language.
pub fn is_pl() -> bool {
    LANG.load(Ordering::Relaxed) == PL
}

/// Resolves a language code without touching the global state.
pub fn is_pl_code(code: &str) -> bool {
    code.trim().eq_ignore_ascii_case("pl")
}

/// Translates `key` into the active language.
///
/// Unknown keys are returned unchanged, which keeps technical identifiers
/// (codec names, encoder names, file names) readable in both languages.
pub fn t(key: &str) -> &'static str {
    lookup(is_pl(), key)
}

fn lookup(pl: bool, key: &str) -> &'static str {
    match key {
        // -- process / ffmpeg ------------------------------------------------
        "err_run_program" => {
            if pl {
                "Nie można uruchomić"
            } else {
                "Cannot run"
            }
        }
        "err_run_ffmpeg" => {
            if pl {
                "Nie można uruchomić FFmpeg"
            } else {
                "Cannot launch FFmpeg"
            }
        }
        "err_no_stderr" => {
            if pl {
                "Brak strumienia stderr FFmpeg"
            } else {
                "FFmpeg stderr stream is missing"
            }
        }
        "err_no_stdout" => {
            if pl {
                "Brak strumienia stdout FFmpeg"
            } else {
                "FFmpeg stdout stream is missing"
            }
        }
        "err_wait_ffmpeg" => {
            if pl {
                "Błąd oczekiwania na FFmpeg"
            } else {
                "Error while waiting for FFmpeg"
            }
        }
        "err_cancelled" => {
            if pl {
                "Operacja anulowana przez użytkownika."
            } else {
                "The operation was cancelled by the user."
            }
        }
        "err_timeout" => {
            if pl {
                "Przekroczono czas oczekiwania na FFmpeg"
            } else {
                "Timed out waiting for FFmpeg"
            }
        }
        "err_ffmpeg" => {
            if pl {
                "Błąd FFmpeg:"
            } else {
                "FFmpeg error:"
            }
        }
        "err_replace_output" => {
            if pl {
                "Nie można podmienić pliku wyjściowego"
            } else {
                "Cannot replace the output file"
            }
        }
        "err_ffprobe" => {
            if pl {
                "ffprobe:"
            } else {
                "ffprobe:"
            }
        }
        "err_probe_failed" => {
            if pl {
                "Nie można odczytać informacji o pliku"
            } else {
                "Cannot read the file's media information"
            }
        }
        "err_no_duration" => {
            if pl {
                "Nie można ustalić długości nagrania"
            } else {
                "Cannot determine the clip length"
            }
        }
        // -- encoding stages -------------------------------------------------
        "stage_remux" => {
            if pl {
                "Błyskawiczny Remux (Kopiowanie strumieni)"
            } else {
                "Instant Remux (stream copy)"
            }
        }
        "stage_done" => {
            if pl {
                "Ukończono"
            } else {
                "Done"
            }
        }
        "stage_size_fix" => {
            if pl {
                "Korekta rozmiaru (próba"
            } else {
                "Size correction (attempt"
            }
        }
        "stage_compress" => {
            if pl {
                "Kompresja"
            } else {
                "Compression"
            }
        }
        "stage_pass" => {
            if pl {
                "Przebieg"
            } else {
                "Pass"
            }
        }
        "stage_audio_export" => {
            if pl {
                "Eksport audio ("
            } else {
                "Audio export ("
            }
        }
        "speed_direct" => {
            if pl {
                "Bezpośrednio"
            } else {
                "Direct"
            }
        }
        // -- encoding errors -------------------------------------------------
        "err_encode_not_created" => {
            if pl {
                "Błąd kodowania: plik wyjściowy nie został utworzony."
            } else {
                "Encoding error: the output file was not created."
            }
        }
        "err_encode_corrupt" => {
            if pl {
                "Błąd kodowania: wygenerowany plik jest uszkodzony lub pusty."
            } else {
                "Encoding error: the generated file is corrupted or empty."
            }
        }
        "err_output_too_large" => {
            if pl {
                "Plik wyjściowy ({} MB) przekracza dopuszczalny limit {} MB (maksimum: {} MB) po {} próbach z kodekiem {}. {}"
            } else {
                "The output file ({} MB) exceeds the allowed limit of {} MB (maximum: {} MB) after {} attempts with the {} codec. {}"
            }
        }
        "err_audio_too_large" => {
            if pl {
                "Plik audio ({} MB) przekracza limit {} MB. {}"
            } else {
                "The audio file ({} MB) exceeds the {} MB limit. {}"
            }
        }
        "err_audio_too_large_advice" => {
            if pl {
                "Skróć fragment lub wybierz wyższy limit."
            } else {
                "Shorten the clip or choose a higher limit."
            }
        }
        "err_encode_too_large" => {
            if pl {
                "Zalecane: skróć klip o kilka sekund lub wybierz wyższy limit (np. 50 MB)."
            } else {
                "Recommended: shorten the clip by a few seconds or choose a higher limit (e.g. 50 MB)."
            }
        }
        "err_encode_failed" => {
            if pl {
                "Nie udało się ukończyć kodowania."
            } else {
                "Encoding could not be completed."
            }
        }
        "err_input_not_found" => {
            if pl {
                "Nie znaleziono pliku wejściowego"
            } else {
                "Input file not found"
            }
        }
        "err_file_not_found" => {
            if pl {
                "Nie znaleziono pliku"
            } else {
                "File not found"
            }
        }
        "err_file_missing" => {
            if pl {
                "Plik nie istnieje"
            } else {
                "The file does not exist"
            }
        }
        "err_end_before_start" => {
            if pl {
                "Czas końca musi być większy niż czas początku!"
            } else {
                "The end time must be greater than the start time!"
            }
        }
        "err_output_is_source" => {
            if pl {
                "Ścieżka pliku wyjściowego wskazuje na plik źródłowy! Nadpisanie źródła jest zablokowane."
            } else {
                "The output path points to the source file! Overwriting the source is blocked."
            }
        }
        "err_no_audio_track" => {
            if pl {
                "Plik źródłowy nie zawiera ścieżki audio."
            } else {
                "The source file has no audio track."
            }
        }
        "err_audio_not_created" => {
            if pl {
                "Błąd eksportu audio: plik wyjściowy nie został utworzony."
            } else {
                "Audio export error: the output file was not created."
            }
        }
        "err_audio_empty" => {
            if pl {
                "Błąd eksportu audio: plik wyjściowy jest pusty."
            } else {
                "Audio export error: the output file is empty."
            }
        }
        "err_thumbnail" => {
            if pl {
                "Błąd ekstrakcji klatki PNG:"
            } else {
                "PNG frame extraction error:"
            }
        }
        "err_thumbnail_failed" => {
            if pl {
                "Nie udało się wygenerować miniatury."
            } else {
                "Could not generate the thumbnail."
            }
        }
        "err_filmstrip_failed" => {
            if pl {
                "Nie udało się wygenerować filmstripu."
            } else {
                "Could not generate the filmstrip."
            }
        }
        // -- cleanup ---------------------------------------------------------
        "cleanup_kept" => {
            if pl {
                "Oryginał zachowany (polityka: nigdy nie usuwaj)."
            } else {
                "Original kept (policy: never delete)."
            }
        }
        "err_no_path" => {
            if pl {
                "Brak podanej ścieżki pliku."
            } else {
                "No file path was given."
            }
        }
        "err_same_path" => {
            if pl {
                "Ścieżka źródłowa jest identyczna ze ścieżką wyjściową — operacja zablokowana."
            } else {
                "The source path matches the output path — the operation is blocked."
            }
        }
        "err_source_missing" => {
            if pl {
                "Plik źródłowy nie istnieje lub został już usunięty."
            } else {
                "The source file does not exist or was already deleted."
            }
        }
        "err_output_missing" => {
            if pl {
                "Plik wyjściowy nie istnieje — anulowano usuwanie źródła."
            } else {
                "The output file does not exist — source deletion was cancelled."
            }
        }
        "cleanup_trashed" => {
            if pl {
                "Plik źródłowy został przeniesiony do Kosza."
            } else {
                "The source file was moved to the Recycle Bin."
            }
        }
        "err_trash_failed" => {
            if pl {
                "Nie udało się przenieść pliku do Kosza (plik jest zablokowany?)."
            } else {
                "Could not move the file to the Recycle Bin (is the file locked?)."
            }
        }
        "cleanup_deleted" => {
            if pl {
                "Plik źródłowy został trwale usunięty."
            } else {
                "The source file was permanently deleted."
            }
        }
        "err_delete_failed" => {
            if pl {
                "Błąd usuwania pliku:"
            } else {
                "File deletion error:"
            }
        }
        // -- planner ---------------------------------------------------------
        "plan_target_too_small" => {
            if pl {
                "Obliczony bitrate wideo (< 30 kbps) jest nierealny. Skróć klip lub wybierz wyższy limit (np. 50 MB)."
            } else {
                "The computed video bitrate (< 30 kbps) is unrealistic. Shorten the clip or choose a higher limit (e.g. 50 MB)."
            }
        }
        // -- quality assessment ----------------------------------------------
        "q_remux_label" => {
            if pl {
                "Bł‚skawiczny Remux (Oryginalna jakość)"
            } else {
                "Instant Remux (Original Quality)"
            }
        }
        "q_remux_desc" => {
            if pl {
                "Plik mieści się w limicie bez rekompresji. Zachowano 100% jakości źródłowej."
            } else {
                "File fits the limit without re-encoding. 100% original quality preserved."
            }
        }
        "q_remux_tip" => {
            if pl {
                "Jakość jest maksymalna — brak konieczności ponownego kodowania."
            } else {
                "Quality is maximum — no re-encoding required."
            }
        }
        "q_great_label" => {
            if pl {
                "Świetna ostrość"
            } else {
                "Great Clarity"
            }
        }
        "q_great_tip" => {
            if pl {
                "Bitrate jest optymalny — nie musisz nic zmieniać."
            } else {
                "Bitrate is optimal — no changes needed."
            }
        }
        "q_good_label" => {
            if pl {
                "Dobra ostrość"
            } else {
                "Good Clarity"
            }
        }
        "q_good_tip" => {
            if pl {
                "Jakość jest w zupełności wystarczająca do dzielenia się klipem."
            } else {
                "Quality is good and ideal for sharing."
            }
        }
        "q_ok_label" => {
            if pl {
                "OK do wysłania"
            } else {
                "OK for Sharing"
            }
        }
        "q_ok_tip_long" => {
            if pl {
                "Skróć klip o ~{} s lub wybierz 50 MB, aby podnieść ostrość."
            } else {
                "Shorten the clip by ~{}s or choose 50 MB to increase sharpness."
            }
        }
        "q_ok_tip_short" => {
            if pl {
                "Przełącz na 720p lub kodek AV1/HEVC dla czystszego obrazu."
            } else {
                "Switch downscale to 720p or use AV1/HEVC for a cleaner picture."
            }
        }
        "q_low_label" => {
            if pl {
                "Niska ostrość"
            } else {
                "Low Clarity"
            }
        }
        "q_low_tip" => {
            if pl {
                "Zalecane: skróć klip o ~{} s, wybierz 50 MB lub użyj AV1."
            } else {
                "Recommended: shorten clip by ~{}s, select 50 MB or use AV1."
            }
        }
        "q_very_low_label" => {
            if pl {
                "Bardzo niska ostrość"
            } else {
                "Very Low Clarity"
            }
        }
        "q_very_low_tip" => {
            if pl {
                "Koniecznie skróć klip (o ~{} s) albo wybierz większy limit."
            } else {
                "Strongly advised: shorten the clip (by ~{}s) or increase the size limit."
            }
        }
        "q_description" => {
            if pl {
                "Szacowana gęstość bitów: {} bppf (kodek: {})."
            } else {
                "Estimated bit density: {} bppf (codec: {})."
            }
        }
        "plan_target_too_small_title" => {
            if pl {
                "Docelowy limit rozmiaru"
            } else {
                "The target size limit"
            }
        }
        "plan_target_too_small_clip" => {
            if pl {
                "jest zbyt mały dla klipu o długości"
            } else {
                "is too small for a clip of"
            }
        }
        "err_unknown_policy" => {
            if pl {
                "Nieznana polityka czyszczenia"
            } else {
                "Unknown cleanup policy"
            }
        }
        _ => Box::leak(key.to_string().into_boxed_str()),
    }
}

/// Translates `key` for an explicit language code (used by the planner).
pub fn t_for(code: &str, key: &str) -> &'static str {
    lookup(is_pl_code(code), key)
}

/// Fills a translated template: every `{}` placeholder is replaced by the
/// matching argument, in order.
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut index = 0;
    while let Some(position) = rest.find("{}") {
        out.push_str(&rest[..position]);
        match args.get(index) {
            Some(value) => out.push_str(value),
            None => out.push_str("{}"),
        }
        index += 1;
        rest = &rest[position + 2..];
    }
    out.push_str(rest);
    out
}
