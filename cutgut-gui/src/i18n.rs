//! Tiny PL/EN translation table for the GUI.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Pl,
    En,
}

impl Lang {
    pub const ALL: [Self; 2] = [Self::Pl, Self::En];

    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Pl => "Polski",
            Lang::En => "English",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Pl => "pl",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Self {
        if code.trim().eq_ignore_ascii_case("pl") {
            Lang::Pl
        } else {
            Lang::En
        }
    }
}

pub fn t<'a>(lang: Lang, key: &'a str) -> &'a str {
    let pl = lang == Lang::Pl;
    match key {
        "playback_speed" => {
            if pl {
                "Prędkość odtwarzania"
            } else {
                "Playback speed"
            }
        }
        "original_cleanup" => {
            if pl {
                "Oryginał po eksporcie przyciętego klipu"
            } else {
                "Original after exporting a trimmed clip"
            }
        }
        "cleanup_ask" => {
            if pl {
                "Pytaj"
            } else {
                "Ask"
            }
        }
        "cleanup_automatic" => {
            if pl {
                "Automatycznie do Kosza"
            } else {
                "Recycle automatically"
            }
        }
        "cleanup_off" => {
            if pl {
                "Wyłączone"
            } else {
                "Off"
            }
        }
        "original_cleanup_title" => {
            if pl {
                "Przenieść oryginał do Kosza?"
            } else {
                "Move original to Recycle Bin?"
            }
        }
        "original_cleanup_prompt" => {
            if pl {
                "Przycięty klip został zapisany. Możesz teraz przenieść oryginał do Kosza."
            } else {
                "The trimmed clip has been saved. You can now move the original to the Recycle Bin."
            }
        }
        "keep_original" => {
            if pl {
                "Zachowaj oryginał"
            } else {
                "Keep original"
            }
        }
        "recycle_original" => {
            if pl {
                "Przenieś do Kosza"
            } else {
                "Move to Recycle Bin"
            }
        }
        "original_recycled" => {
            if pl {
                "Klip zapisany. Oryginał przeniesiono do Kosza."
            } else {
                "Clip saved. Original moved to the Recycle Bin."
            }
        }
        "original_cleanup_failed" => {
            if pl {
                "Klip zapisany. Nie udało się przenieść oryginału do Kosza."
            } else {
                "Clip saved. Could not move the original to the Recycle Bin."
            }
        }
        "previous_clip" => {
            if pl {
                "Poprzedni klip"
            } else {
                "Previous clip"
            }
        }
        "next_clip" => {
            if pl {
                "Następny klip"
            } else {
                "Next clip"
            }
        }
        "fullscreen" => {
            if pl {
                "Pełny ekran (F)"
            } else {
                "Fullscreen (F)"
            }
        }
        "exit_fullscreen" => {
            if pl {
                "Wyjdź z pełnego ekranu (F)"
            } else {
                "Exit fullscreen (F)"
            }
        }
        "kb_gallery_navigation" => {
            if pl {
                "Poprzedni / następny klip w podglądzie galerii"
            } else {
                "Previous / next clip in gallery preview"
            }
        }
        "app_title" => "CutGut",
        "tagline" => {
            if pl {
                "Inteligentne przycinanie i kompresja"
            } else {
                "Smart trimming & compression"
            }
        }
        "open" => {
            if pl {
                "Otwórz film"
            } else {
                "Open video"
            }
        }
        "change" => {
            if pl {
                "Zmień film"
            } else {
                "Change video"
            }
        }
        "no_file" => {
            if pl {
                "Nie wybrano filmu"
            } else {
                "No video selected"
            }
        }
        "gallery" => {
            if pl {
                "Galeria"
            } else {
                "Gallery"
            }
        }
        "settings" => {
            if pl {
                "Ustawienia"
            } else {
                "Settings"
            }
        }
        "play" => {
            if pl {
                "Odtwórz"
            } else {
                "Play"
            }
        }
        "pause" => {
            if pl {
                "Pauza"
            } else {
                "Pause"
            }
        }
        "in" => "IN",
        "out" => "OUT",
        "loop" => {
            if pl {
                "Pętla"
            } else {
                "Loop"
            }
        }
        "snapshot_clipboard" => match lang {
            Lang::Pl => "Zrzut w schowku",
            Lang::En => "Screenshot copied",
        },
        "snapshot" => {
            if pl {
                "Klatka PNG"
            } else {
                "PNG frame"
            }
        }
        "timeline" => {
            if pl {
                "Oś czasu"
            } else {
                "Timeline"
            }
        }
        "export" => {
            if pl {
                "Eksport"
            } else {
                "Export"
            }
        }
        "quality" => {
            if pl {
                "Jakość"
            } else {
                "Quality"
            }
        }
        "profile" => {
            if pl {
                "Profil"
            } else {
                "Profile"
            }
        }
        "framing" => {
            if pl {
                "Kadr"
            } else {
                "Framing"
            }
        }
        "limit" => {
            if pl {
                "Limit"
            } else {
                "Limit"
            }
        }
        "codec" => {
            if pl {
                "Kodek"
            } else {
                "Codec"
            }
        }
        "engine" => {
            if pl {
                "Silnik"
            } else {
                "Engine"
            }
        }
        "format" => {
            if pl {
                "Format"
            } else {
                "Format"
            }
        }
        "folder" => {
            if pl {
                "Folder"
            } else {
                "Folder"
            }
        }
        "custom" => {
            if pl {
                "Własny"
            } else {
                "Custom"
            }
        }
        "export_btn" => {
            if pl {
                "Eksportuj"
            } else {
                "Export"
            }
        }
        "cancel" => {
            if pl {
                "Anuluj"
            } else {
                "Cancel"
            }
        }
        "select_video" => {
            if pl {
                "Wybierz film, aby rozpocząć"
            } else {
                "Select a video to begin"
            }
        }
        "select_clip" => {
            if pl {
                "Kliknij miniaturę, aby zobaczyć podgląd"
            } else {
                "Click a thumbnail to preview"
            }
        }
        "drop_hint" => {
            if pl {
                "Kliknij „Otwórz film” lub przeciągnij plik tutaj"
            } else {
                "Click “Open video” or drag a file here"
            }
        }
        "ready" => {
            if pl {
                "Gotowy"
            } else {
                "Ready"
            }
        }
        "analyzing" => {
            if pl {
                "Analizowanie…"
            } else {
                "Analyzing…"
            }
        }
        "exporting" => {
            if pl {
                "Eksportowanie…"
            } else {
                "Exporting…"
            }
        }
        "done" => {
            if pl {
                "Gotowe"
            } else {
                "Done"
            }
        }
        "error" => {
            if pl {
                "Błąd"
            } else {
                "Error"
            }
        }
        "original" => {
            if pl {
                "Oryginalny"
            } else {
                "Original"
            }
        }
        "portrait" => {
            if pl {
                "Pionowy 9:16"
            } else {
                "Vertical 9:16"
            }
        }
        "square" => {
            if pl {
                "Kwadrat 1:1"
            } else {
                "Square 1:1"
            }
        }
        "landscape" => {
            if pl {
                "Poziomy 16:9"
            } else {
                "Landscape 16:9"
            }
        }
        "gpu_hq" => {
            if pl {
                "GPU · wysoka jakość"
            } else {
                "GPU · high quality"
            }
        }
        "gpu_fast" => {
            if pl {
                "GPU · szybki"
            } else {
                "GPU · fast"
            }
        }
        "cpu_hq" => {
            if pl {
                "CPU · 2-pass"
            } else {
                "CPU · 2-pass"
            }
        }
        "cpu_fast" => {
            if pl {
                "CPU · szybki"
            } else {
                "CPU · fast"
            }
        }
        "settings_title" => {
            if pl {
                "Ustawienia"
            } else {
                "Settings"
            }
        }
        "language" => {
            if pl {
                "Język"
            } else {
                "Language"
            }
        }
        "output_dir" => {
            if pl {
                "Folder wynikowy"
            } else {
                "Output folder"
            }
        }
        "choose" => {
            if pl {
                "Wybierz…"
            } else {
                "Choose…"
            }
        }
        "pause_overlay" => {
            if pl {
                "Krótka ikona pauzy/odtwarzania na podglądzie"
            } else {
                "Brief play/pause icon on preview"
            }
        }
        "volume" => {
            if pl {
                "Głośność"
            } else {
                "Volume"
            }
        }
        "mute" => {
            if pl {
                "Wycisz"
            } else {
                "Mute"
            }
        }
        "unmute" => {
            if pl {
                "Włącz dźwięk"
            } else {
                "Unmute"
            }
        }
        "gallery_startup" => {
            if pl {
                "Otwieraj galerię przy starcie (gdy jest folder NVIDIA)"
            } else {
                "Open gallery on startup (when the NVIDIA folder exists)"
            }
        }
        "gallery_dir" => {
            if pl {
                "Foldery galerii"
            } else {
                "Gallery folders"
            }
        }
        "add_folder" => {
            if pl {
                "Dodaj folder"
            } else {
                "Add folder"
            }
        }
        "remove_folder" => {
            if pl {
                "Usuń folder z galerii"
            } else {
                "Remove folder from gallery"
            }
        }
        "close" => {
            if pl {
                "Zamknij"
            } else {
                "Close"
            }
        }
        "save" => {
            if pl {
                "Zapisz"
            } else {
                "Save"
            }
        }
        "all" => {
            if pl {
                "Wszystkie"
            } else {
                "All"
            }
        }
        "search" => {
            if pl {
                "Szukaj klipu…"
            } else {
                "Search clips…"
            }
        }
        "back" => {
            if pl {
                "Wróć"
            } else {
                "Back"
            }
        }
        "edit" => {
            if pl {
                "Edytuj w CutGut"
            } else {
                "Edit in CutGut"
            }
        }
        "rename" => {
            if pl {
                "Zmień nazwę"
            } else {
                "Rename"
            }
        }
        "delete" => {
            if pl {
                "Usuń"
            } else {
                "Delete"
            }
        }
        "open_folder" => {
            if pl {
                "Otwórz folder"
            } else {
                "Open folder"
            }
        }
        "refresh" => {
            if pl {
                "Odśwież"
            } else {
                "Refresh"
            }
        }
        "choose_folder" => {
            if pl {
                "Zmień folder"
            } else {
                "Change folder"
            }
        }
        "empty" => {
            if pl {
                "Brak klipów w tym folderze"
            } else {
                "No clips in this folder"
            }
        }
        "clips" => {
            if pl {
                "klipów"
            } else {
                "clips"
            }
        }
        "confirm_delete" => {
            if pl {
                "Usunąć ten klip?"
            } else {
                "Delete this clip?"
            }
        }
        "delete_warning" => {
            if pl {
                "Klip trafi do Kosza. Tej operacji nie można cofnąć z aplikacji."
            } else {
                "The clip will go to the Recycle Bin. This cannot be undone from the app."
            }
        }
        "yes" => {
            if pl {
                "Tak, usuń"
            } else {
                "Yes, delete"
            }
        }
        "no" => {
            if pl {
                "Anuluj"
            } else {
                "Cancel"
            }
        }
        "keep_clip" => {
            if pl {
                "Oszczędź tego klipa"
            } else {
                "Save this clip"
            }
        }
        "new_name" => {
            if pl {
                "Nowa nazwa pliku"
            } else {
                "New file name"
            }
        }
        "open_output" => {
            if pl {
                "Otwórz folder wyników"
            } else {
                "Open output folder"
            }
        }
        "loading" => {
            if pl {
                "Wczytywanie…"
            } else {
                "Loading…"
            }
        }
        "no_audio" => {
            if pl {
                "bez audio"
            } else {
                "no audio"
            }
        }
        "fps" => "FPS",
        "editor" => {
            if pl {
                "Edytor"
            } else {
                "Editor"
            }
        }
        "preview" => {
            if pl {
                "Podgląd"
            } else {
                "Preview"
            }
        }
        "light_theme" => {
            if pl {
                "Włącz jasny motyw"
            } else {
                "Switch to light theme"
            }
        }
        "dark_theme" => {
            if pl {
                "Włącz ciemny motyw"
            } else {
                "Switch to dark theme"
            }
        }
        "favorites" => {
            if pl {
                "Ulubione"
            } else {
                "Favorites"
            }
        }
        "add_favorite" => {
            if pl {
                "Dodaj do ulubionych"
            } else {
                "Add to favorites"
            }
        }
        "remove_favorite" => {
            if pl {
                "Usuń z ulubionych"
            } else {
                "Remove from favorites"
            }
        }
        "import_clips" => {
            if pl {
                "Dodaj klipy"
            } else {
                "Add clips"
            }
        }
        "import_hint" => {
            if pl {
                "Wybierz pliki lub przeciągnij je tutaj"
            } else {
                "Choose files or drop them here"
            }
        }
        "cutgut_output" => {
            if pl {
                "Edytowane klipy"
            } else {
                "Edited clips"
            }
        }
        "fav_chip" => "Fav",
        "imported" => {
            if pl {
                "Dodane ręcznie"
            } else {
                "Imported"
            }
        }
        "import_problem" => {
            if pl {
                "Nie można dodać pliku"
            } else {
                "Could not add file"
            }
        }
        "library_problem" => {
            if pl {
                "Nie można zapisać zmian w galerii"
            } else {
                "Could not save gallery changes"
            }
        }
        "no_results" => {
            if pl {
                "Brak pasujących klipów"
            } else {
                "No matching clips"
            }
        }
        "no_favorites" => {
            if pl {
                "Nie masz jeszcze ulubionych klipów"
            } else {
                "No favorite clips yet"
            }
        }
        "media_missing" => {
            if pl {
                "Nie można wczytać podglądu"
            } else {
                "Could not load preview"
            }
        }
        "open_hint" => {
            if pl {
                "Wybierz plik lub przeciągnij go tutaj"
            } else {
                "Choose a file or drop it here"
            }
        }
        "delete_failed" => {
            if pl {
                "Nie można przenieść klipu do Kosza"
            } else {
                "Could not move the clip to the Recycle Bin"
            }
        }
        "rename_failed" => {
            if pl {
                "Nie można zmienić nazwy klipu"
            } else {
                "Could not rename the clip"
            }
        }
        "invalid_name" => {
            if pl {
                "Nazwa zawiera znaki niedozwolone w Windows (< > : \" / \\ | ? *) lub jest nazwą zarezerwowaną"
            } else {
                "The name contains characters Windows does not allow (< > : \" / \\ | ? *) or is a reserved name"
            }
        }
        "media_files" => {
            if pl {
                "Filmy"
            } else {
                "Videos"
            }
        }
        "theme" => {
            if pl {
                "Motyw"
            } else {
                "Theme"
            }
        }
        "shortcuts" => {
            if pl {
                "Spacja: odtwarzanie · I/O: punkty cięcia · strzałki: jedna sekunda · Ctrl+O: otwórz · G: galeria · F: pełny ekran · S: klatka PNG"
            } else {
                "Space: playback · I/O: trim points · arrow keys: one second · Ctrl+O: open · G: gallery · F: fullscreen · S: PNG frame"
            }
        }
        "play_full" => {
            if pl {
                "Odtwórz cały film"
            } else {
                "Play full video"
            }
        }
        "play_trim" => {
            if pl {
                "Obejrzyj przycięte"
            } else {
                "Preview trim"
            }
        }
        "pause_trim" => {
            if pl {
                "Wstrzymaj przycięte"
            } else {
                "Pause trim"
            }
        }
        "filters" => {
            if pl {
                "Filtry galerii"
            } else {
                "Gallery filters"
            }
        }
        "mark_in" => {
            if pl {
                "Ustaw początek cięcia (I)"
            } else {
                "Set trim start (I)"
            }
        }
        "mark_out" => {
            if pl {
                "Ustaw koniec cięcia (O)"
            } else {
                "Set trim end (O)"
            }
        }
        "keybinds" => {
            if pl {
                "Skróty klawiszowe"
            } else {
                "Keybinds"
            }
        }
        "clips_count" => {
            if pl {
                "Liczba klipów"
            } else {
                "Clips"
            }
        }
        "clips_short" => {
            if pl {
                "klipów"
            } else {
                "clips"
            }
        }
        "total_size" => {
            if pl {
                "Łączny rozmiar"
            } else {
                "Total size"
            }
        }
        "largest_clip" => {
            if pl {
                "Największy klip"
            } else {
                "Largest clip"
            }
        }
        "marked_clips" => {
            if pl {
                "zaznaczone"
            } else {
                "selected"
            }
        }
        "clear_selection" => {
            if pl {
                "Wyczyść zaznaczenie"
            } else {
                "Clear selection"
            }
        }
        "preview_clip" => {
            if pl {
                "Podgląd"
            } else {
                "Preview"
            }
        }
        "edit_clip" => {
            if pl {
                "Edytuj w CutGut"
            } else {
                "Edit in CutGut"
            }
        }
        "copy_path" => {
            if pl {
                "Kopiuj ścieżkę"
            } else {
                "Copy path"
            }
        }
        "copy_file" => {
            if pl {
                "Kopiuj plik"
            } else {
                "Copy file"
            }
        }
        "path_copied" => {
            if pl {
                "Ścieżka skopiowana"
            } else {
                "Path copied"
            }
        }
        "file_copied" => {
            if pl {
                "Plik skopiowany do schowka"
            } else {
                "File copied to the clipboard"
            }
        }
        "copy_failed" => {
            if pl {
                "Nie udało się skopiować pliku"
            } else {
                "Could not copy the file"
            }
        }
        "confirm_delete_many" => {
            if pl {
                "Usunąć te klipy?"
            } else {
                "Delete these clips?"
            }
        }
        "group_by" => {
            if pl {
                "Grupowanie"
            } else {
                "Group by"
            }
        }
        "card_size" => {
            if pl {
                "Wielkość kafelków"
            } else {
                "Tile size"
            }
        }
        "group_game" => {
            if pl {
                "Gra"
            } else {
                "Game"
            }
        }
        "group_folder" => "Folder",
        "group_type" => {
            if pl {
                "Typ pliku"
            } else {
                "File type"
            }
        }
        "group_date" => {
            if pl {
                "Data nagrania"
            } else {
                "Recording date"
            }
        }
        "group_none" => {
            if pl {
                "Bez grupowania"
            } else {
                "No grouping"
            }
        }
        "card_small" => {
            if pl {
                "Małe"
            } else {
                "Small"
            }
        }
        "card_medium" => {
            if pl {
                "Średnie"
            } else {
                "Medium"
            }
        }
        "card_large" => {
            if pl {
                "Duże"
            } else {
                "Large"
            }
        }
        "media_videos" => {
            if pl {
                "Nagrania"
            } else {
                "Videos"
            }
        }
        "media_images" => {
            if pl {
                "Zdjęcia"
            } else {
                "Images"
            }
        }
        "clip_length" => {
            if pl {
                "Długość"
            } else {
                "Length"
            }
        }
        "clip_type" => "Format",
        "file_size" => {
            if pl {
                "Rozmiar"
            } else {
                "Size"
            }
        }
        "duration_label" => {
            if pl {
                "Czas trwania"
            } else {
                "Duration"
            }
        }
        "format_label" => "Format",
        "date_label" => {
            if pl {
                "Data"
            } else {
                "Date"
            }
        }
        "size_label" => {
            if pl {
                "Rozmiar"
            } else {
                "Size"
            }
        }
        "kind_video" => {
            if pl {
                "Wideo"
            } else {
                "Video"
            }
        }
        "kind_image" => {
            if pl {
                "Obraz"
            } else {
                "Image"
            }
        }
        "media_video" => {
            if pl {
                "wideo"
            } else {
                "video"
            }
        }
        "media_image" => {
            if pl {
                "obraz"
            } else {
                "image"
            }
        }
        "share" => {
            if pl {
                "Udział w bibliotece"
            } else {
                "Share of library"
            }
        }
        "storage" => {
            if pl {
                "Miejsce zajmowane przez nagrania"
            } else {
                "Space used by recordings"
            }
        }
        "kb_play" => {
            if pl {
                "Odtwarzanie / pauza"
            } else {
                "Play / pause"
            }
        }
        "kb_trim" => {
            if pl {
                "Punkt IN / OUT cięcia"
            } else {
                "Set trim IN / OUT"
            }
        }
        "kb_frame_step" => {
            if pl {
                "Klatka w tył / w przód"
            } else {
                "Step one frame back / forward"
            }
        }
        "kb_jump" => {
            if pl {
                "Początek / koniec klipu"
            } else {
                "Start / end of the clip"
            }
        }
        "loading_frames" => {
            if pl {
                "wczytuję klatki {}%"
            } else {
                "loading frames {}%"
            }
        }
        "video_unloaded" => {
            if pl {
                "Film wyładowany z edytora"
            } else {
                "Video unloaded"
            }
        }
        "unload" => {
            if pl {
                "Wyładuj film z edytora"
            } else {
                "Unload video"
            }
        }
        "kb_frame_back" => {
            if pl {
                "Klatka w tył"
            } else {
                "Previous frame"
            }
        }
        "kb_frame_forward" => {
            if pl {
                "Klatka w przód"
            } else {
                "Next frame"
            }
        }
        "kb_zoom_in" => {
            if pl {
                "Przybliż oś czasu"
            } else {
                "Zoom in on the timeline"
            }
        }
        "kb_zoom_out" => {
            if pl {
                "Oddal oś czasu"
            } else {
                "Zoom out of the timeline"
            }
        }
        "kb_zoom" => {
            if pl {
                "Przybliżenie osi czasu"
            } else {
                "Zoom the timeline"
            }
        }
        "zoom_reset" => {
            if pl {
                "Przywróć cały klip"
            } else {
                "Fit the whole clip"
            }
        }
        "kb_step" => {
            if pl {
                "Skok o jedną sekundę"
            } else {
                "Step one second"
            }
        }
        "kb_open" => {
            if pl {
                "Otwórz film"
            } else {
                "Open video"
            }
        }
        "kb_gallery_select" => {
            if pl {
                "Zaznaczanie klipów"
            } else {
                "Select clips"
            }
        }
        "kb_select_all" => {
            if pl {
                "Zaznacz wszystko"
            } else {
                "Select all"
            }
        }
        "kb_gallery" => {
            if pl {
                "Galeria / powrót do edytora"
            } else {
                "Gallery / back to editor"
            }
        }
        "kb_fullscreen" => {
            if pl {
                "Pełny ekran"
            } else {
                "Fullscreen"
            }
        }
        "kb_snapshot" => {
            if pl {
                "Zrzut klatki PNG"
            } else {
                "PNG frame snapshot"
            }
        }
        "kb_import" => {
            if pl {
                "Import klipów w galerii"
            } else {
                "Import clips in the gallery"
            }
        }
        "kb_close" => {
            if pl {
                "Zamknij okno / anuluj"
            } else {
                "Close dialog / cancel"
            }
        }
        "export_failed" => {
            if pl {
                "Eksport nie doszedł do końca"
            } else {
                "The export did not finish"
            }
        }
        "export_failed_hint" => {
            if pl {
                "Poniżej raport ffmpeg i ostatnie wpisy z logu. Plik z logiem zostaje na dysku."
            } else {
                "Below is ffmpeg's report and the tail of the log. The log file stays on disk."
            }
        }
        "copy_report" => {
            if pl {
                "Kopiuj raport"
            } else {
                "Copy report"
            }
        }
        "report_copied" => {
            if pl {
                "Raport skopiowany do schowka"
            } else {
                "Report copied to the clipboard"
            }
        }
        "open_logs" => {
            if pl {
                "Otwórz folder logów"
            } else {
                "Open log folder"
            }
        }
        "codec_hint_h264" => {
            if pl {
                "Przy tym limicie AV1 wygląda najlepiej, HEVC też jest dużo lepszy od H.264"
            } else {
                "At this limit AV1 looks best; HEVC is far ahead of H.264 too"
            }
        }
        "codec_hint_hevc" => {
            if pl {
                "Przy tym limicie AV1 wygląda jeszcze lepiej niż HEVC"
            } else {
                "At this limit AV1 looks even better than HEVC"
            }
        }
        "hdr_tonemapped" => {
            if pl {
                "HDR → SDR: kolory mapowane do bt709"
            } else {
                "HDR → SDR: colours mapped to bt709"
            }
        }
        "hdr_untouched" => {
            if pl {
                "HDR: ta wersja ffmpeg nie ma zscale — kolory mogą wyjść wyprane"
            } else {
                "HDR: this ffmpeg build lacks zscale — colours may come out washed out"
            }
        }
        "limit_capped" => {
            if pl {
                "Limit jest większy, niż ten klip potrzebuje — plik wyjdzie mniejszy"
            } else {
                "The limit is larger than this clip needs — the file will come out smaller"
            }
        }
        _ => key,
    }
}
