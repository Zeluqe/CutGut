# CutGut — Jakość Eksportu i Gwarancja Twardego Limitu Rozmiaru

Dokumentacja techniczna mechanizmu kodowania wideo, budżetowania bajtowego, obsługi nowoczesnych kodeków (AV1, HEVC, H.264), wykrywania enkoderów sprzętowych, atomowego zapisu oraz pętli korekty rozmiaru.

---

## 1. Wybór Kodeka i Wymagania Sprzętowe

Aplikacja CutGut obsługuje trzy główne standardy kompresji wideo:

| Kodek | Opis i Przeznaczenie | Enkoder GPU (NVIDIA) | Enkoder GPU (AMD) | Enkoder CPU (Fallback) | Kompatybilność Odtwarzania |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **H.264 / AVC** | Maksymalna kompatybilność ze wszystkimi urządzeniami | `h264_nvenc` | `h264_amf` | `libx264` (2-pass / 1-pass) | **Pełna (100%)**: Discord, przeglądarki, starsze telefony i konsole |
| **HEVC / H.265** | Wysoka wydajność kompresji (~30–40% lepsza jakość niż H.264) | `hevc_nvenc` | `hevc_amf` | `libx265` (2-pass / 1-pass) | **Częściowa**: Wymaga wsparcia dekodera HEVC w systemie/przeglądarce odbiorcy |
| **AV1** | Następna generacja, najwyższa ostrość i wydajność przy małych limitach | `av1_nvenc` (RTX 40xx+) | `av1_amf` (RDNA 3+) | `libsvtav1` | **Zależna od odbiorcy**: Wymaga nowszego sprzętu lub przeglądarek Chrome/Edge/Firefox |

### Wykrywanie Dostępności i Jawny Fallback
- Dostępność każdego enkodera nie jest ustalana jedynie na podstawie listy tekstowej `ffmpeg -encoders`, lecz poprzez **rzeczywiste kodowanie testowe 1 klatki** (`color=c=black:s=256x256:d=0.04`) do sinka `null`.
- Wymiar min. 256x256 zapobiega fałszywym odrzuceniom ze strony architektury NVENC (która odrzuca klatki mniejsze niż 64–128 px).
- Wyniki testów są **cache'owane w pamięci** (`OnceLock<Mutex<HashMap<..>>>` w `cutgut-rs/src/ffmpeg.rs`), co eliminuje wielokrotne uruchamianie podprocesów.
- W przypadku żądania kodowania sprzętowego AV1 na karcie bez wsparcia NVENC AV1 (np. RTX 20/30 series), następuje **jawny fallback** do enkodera CPU (`libsvtav1`), o czym użytkownik jest natychmiast informowany w logach oraz GUI, a plan kompresji zostaje przeliczony pod faktycznie używany silnik.

---

## 2. Matematyka Budżetu Bajtowego i Twardy Limit

### Definicja MB
Wszystkie limity rozmiaru w CutGut bazują na ścisłej definicji dziesiętnej:
$$\text{Twardy Limit (Hard Limit)} = \text{target\_mb} \times 1\,000\,000\text{ bajtów}$$
Przykłady:
- **20 MB (Discord Free)** = dokładnie $20\,000\,000$ bajtów.
- **10 MB (Legacy / Small)** = dokładnie $10\,000\,000$ bajtów.
- **50 MB (Nitro Basic)** = dokładnie $50\,000\,000$ bajtów.
- **500 MB (Nitro Full)** = dokładnie $500\,000\,000$ bajtów.

### Bezpieczny Cel Planowania (Planning Budget)
Aby wyjściowy plik nie przekroczył twardego limitu przez wariancję multipleksowania kontenera MP4, początkowy plan bitrate wideo celuje w:
$$\text{Cel Budżetowy} = \text{target\_mb} \times 1\,000\,000 \times 0.960\text{ bajtów}$$
Pozostawia to bezpieczny margines 4.0% (~800 KB dla 20 MB) na nagłówki kontenera MP4, tabelę indeksów (`moov atom`) i ewentualne fluktuacje klatek kluczowych — dzięki temu pliki AV1/HEVC nie lądują dokładnie na granicy limitu.

### Uwzględnianie Audio i Kontenera
1. **Badanie obecności audio**: Jeśli źródło nie zawiera ścieżki dźwiękowej (`VideoInfo::audio_codec == None`), budżet audio wynosi $0\text{ bps}$. Całość dostępnych bajtów trafia do strumienia wideo (`-map 0:v:0 -an`).
2. **Narzut kontenera**:
   $$\text{Overhead} = 30\,000 + (\text{duration\_s} \times 350)\text{ bajtów}$$
3. **Bitrate wideo**:
   $$\text{net\_video\_bytes} = \text{target\_bytes} - \text{audio\_bytes} - \text{overhead\_bytes}$$
   $$\text{video\_kbps} = \max\left(\left\lfloor \frac{\text{net\_video\_bytes} \times 8}{\text{duration\_s} \times 1000} \right\rfloor, 30\right)$$
4. **Weryfikacja nierealnego limitu**: Jeśli $\text{net\_video\_bytes} \le 0$ lub obliczony bitrate wyniósłby poniżej 30 kbps, operacja zostaje wstrzymana z czytelnym błędem `EncodingError`, proponując skrócenie klipu lub podniesienie limitu MB. Nigdy nie jest zgłaszany sukces dla pliku ponad limitem.
5. **Sufit bits-per-pixel**: Limit rozmiaru bywa absurdalnie duży w stosunku do klipu: 4 s nagrania 360p pod limitem 500 MB to ok. 1 Gb/s, a takiego strumienia NVENC i AMF **nie otwierają** (`InitializeEncoder failed: invalid param`), więc eksport kończył się błędem mimo poprawnego planu. Bitrate jest więc ograniczany do `kbps_max = w * h * fps * 0.5 / 1000` (pół bita na piksel to już jakość wizualnie bezstratna dla H.264/HEVC). Plan ustawia wtedy flagę `bitrate_capped`, a GUI i CLI mówią wprost: „Limit jest większy, niż ten klip potrzebuje - plik wyjdzie mniejszy".

---

## 2b. Co realnie daje najlepszą jakość przy tym samym limicie (pomiary VMAF)

Test: 12 s klipu 1080p60 z gry, budżet ok. 8,9 MB (≈5,6 Mb/s), VMAF liczony wobec źródła, wszystko przez własny pipeline CutGuta.

| Ścieżka | Rozmiar | VMAF | Czas (12 s klipu) |
|---|---|---|---|
| AV1 CPU (SVT-AV1 preset 5, 2-pass) | 8,52 MB | **95,37** | 122 s |
| AV1 GPU (`av1_nvenc`, `-tune uhq -preset p7`) | 8,39 MB | **95,23** | ~20 s |
| HEVC GPU (`hevc_nvenc`, `-tune uhq -preset p7 -b_ref_mode middle`) | 8,41 MB | 94,36 | ~15 s |
| HEVC GPU (poprzednie flagi: `p6`/`hq`) | 8,10 MB | 93,63 | ~7 s |
| H.264 CPU (`libx264`, 2-pass slow) | 8,69 MB | 93,35 | 13,5 s |
| H.264 GPU (`h264_nvenc`, `p6`) | 8,15 MB | 88,88 | ~6 s |

Wnioski, które trafiły do kodu:
- **`-tune uhq` + `-preset p7` + `-b_ref_mode middle`** dla NVENC: +0,7 VMAF przy tym samym rozmiarze (HEVC) i +0,9 (H.264). Flagi są *sondeowane* jednoklatkowym testem, więc starszy sterownik po prostu wraca do poprzedniego zestawu zamiast wywalić eksport.
- **AV1 jest najlepszy na MB** (+0,9 VMAF nad HEVC, +6 nad sprzętowym H.264). Jeśli limit jest ciasny (bppf < 0,055), karta planu podpowiada zmianę kodeka.
- **Sprzętowy H.264 przy ciasnym limicie to najgorszy wybór** (88,9 vs 94,4 HEVC) — dlatego podpowiedź pojawia się właśnie tam.

Pomysły sprawdzone i **odrzucone** (zmierzone jako gorsze):
- **Skalowanie w dół do 720p** przy 4,6 Mb/s: VMAF 86,2 — tracimy więcej na rozdzielczości, niż zyskujemy na bitach. Progi w planie już to uwzględniają.
- **Połowa klatek (30 fps)**: VMAF 56,6 (VMAF porównuje klatka w klatkę) i realna utrata płynności. Nie robić.
- **Denoise przed enkodowaniem** (`hqdn3d`): 92,9 — wygładza tekstury i psuje ostrość gry.

Pomysł sprawdzony i **działający, ale świadomie nie włączony domyślnie**:
- **HEVC 10-bit (`-profile:v main10 -pix_fmt p010le -highbitdepth 1`)**: 94,24 VMAF przy 5,34 Mb/s vs 93,63 przy 5,53 Mb/s dla 8-bit — czyli ok. +0,6 VMAF i mniejszy plik, bo 10 bitów zabija banding na gradientach (niebo, dym, cienie). Koszt: mniejsza zgodność odtwarzaczy (QuickTime/iOS, część platform social). Kandydat na opcję „maksymalna jakość", nie na domyślne ustawienie.

---

## 3. Profile Jakości i Rate Control dla NVENC

Dla enkoderów NVIDIA NVENC (`h264_nvenc`, `hevc_nvenc`, `av1_nvenc`) wdrożono zaawansowane parametry sterowania strumieniem VBR:
- **Preset**: `p6` dla trybu wysokiej jakości (HQ) oraz `p3` dla trybu szybkiego (Fast).
- **Tuning**: `-tune hq`.
- **Multipass**: `-multipass fullres` (sprzętowa analiza makrobloków w pełnej rozdzielczości podczas jednej sesji kodowania na układzie NVENC — nie mylić z dwuprzebiegową pełną analizą pliku CPU).
- **Lookahead**: `-rc-lookahead 32` dla przewidywania trudnych scen (dym, cząsteczki, ogień).
- **Adaptive Quantization**: `-spatial-aq 1` oraz `-temporal-aq 1` (weryfikowane dynamicznie dla danego sterownika za pomocą `get_nvenc_quality_options()`).
- **Elastyczny sufit VBR**: `-maxrate {int(v_kbps * 1.5)}k -bufsize {int(v_kbps * 2.0)}k`, co pozwala enkoderowi przydzielić więcej bitów trudniejszym klatkom bez sztucznego zaciskania do CBR.

---

## 4. Bezpieczny Zapis i Pętla Korekty Rozmiaru

### Blokada Nadpisania Źródła
Przed rozpoczęciem operacji sprawdzana jest tożsamość ścieżki wejściowej i wyjściowej (`same_file()` → `std::fs::canonicalize`). Jeśli wyjście wskazuje na źródło (w tym przez dowiązanie symboliczne, twarde lub alias), eksport jest natychmiast blokowany z czytelnym błędem.

### Atomowa Podmiana (`MoveFileExW`)
- Plik tymczasowy tworzony jest bezpośrednio w **katalogu docelowym** (`.cg_tmp_{tag}_{nazwa}`), co gwarantuje znajdowanie się na tym samym wolumenie dyskowym.
- Poprzedni plik docelowy **nie jest usuwany przed zakończeniem kodowania**.
- Dopiero po pomyślnej weryfikacji rozmiaru ($\le \text{hard\_limit\_bytes}$) wykonywana jest atomowa podmiana przez `MoveFileExW(.., MOVEFILE_REPLACE_EXISTING)`.
- W razie błędu kodowania, anulowania lub problemów z uprawnieniami pliku, istniejący plik docelowy użytkownika pozostaje w 100% nienaruszony.
- Sprzątane są wyłącznie pliki tymczasowe należące do danego zadania (izolacja procesów).

### Korekta Rozmiaru z Zachowaniem Ustawień Jakości
Plik jest akceptowany dopiero, gdy jego rzeczywisty rozmiar jest $\le 99.5\%$ twardego limitu (ostatnia, 3. próba toleruje pełny limit). Dzięki temu wynik nigdy nie „stoi” na granicy 20 MB.
Gdy plik po pierwszym podejściu przekroczy bezpieczny próg:
1. Plik tymczasowy zostaje usunięty.
2. Narzut audio i kontenera MP4 ma stałą wagę, dlatego korygowany jest wyłącznie budżet wideo:
   $$\text{measured\_video} = \max(\text{actual\_size} - \text{audio\_bytes} - \text{overhead\_bytes}, 1000)$$
   $$\text{target\_video} = \max(\text{hard\_limit\_bytes} \times \text{safety} - \text{audio\_bytes} - \text{overhead\_bytes}, 1000)$$
   $$\text{corrected\_vkbps} = \max\left(\left\lfloor \text{current\_vkbps} \times \min\left(\frac{\text{target\_video}}{\text{measured\_video}}, 0.95\right) \right\rfloor, 25\right)$$
3. Kodowanie ponawiane jest **Z ORYGINALNEGO ŹRÓDŁA**, z zachowaniem **dokładnie tego samego kodeka, presetu, opcji VBR (w tym multipass, AQ i lookahead)**.
4. Pętla wykonuje **maksymalnie 3 próby**.
5. Jeśli po 3 próbach rozmiar nadal przekracza limit, operacja kończy się błędem (`Err(String)` z czytelnym opisem). Plik ponad limitem nigdy nie jest oznaczany jako udany eksport.

---

## 5. Bezstratny Remux (Stream Copy)

Gdy klip mieści się bezpośrednio w limicie rozmiaru:
- Remux jest dozwolony tylko wtedy, gdy kadr nie został przycięty (`crop_box is None` lub `original`), a kodek źródłowy odpowiada żądanemu kodekowi docelowemu.
- Jeśli użytkownik wybrał AV1, a źródło to H.264, remux jest zablokowany, aby zrealizować kompresję do żądanego formatu.
- Rozmiar remuksowanego pliku jest weryfikowany po zakończeniu: jeśli przekracza limit, plik tymczasowy jest usuwany i CutGut płynnie przechodzi do pełnego kodowania bez naruszania pliku docelowego.
- **Cięcie remuxem jest weryfikowane.** `-c copy` potrafi zaczynać tylko od klatki kluczowej, więc przy cięciu pomiędzy keyframe'ami plik wychodził z „doklejonym" fragmentem przed żądanym czasem (zmierzone: cięcie 6-10 s dawało pierwszą klatkę na -1.835 s i 5.9 s materiału przy deklarowanych 4.077 s). Po remuxie `media::content_span()` czyta PTS pierwszej paczki i długość strumienia wideo, a `ffmpeg::trim_is_exact()` porównuje je z żądaniem (tolerancja ok. 2 klatki, min. 0.12 s). Jeśli cięcie nie trafiło, remux jest odrzucany i eksport idzie dokładną rekompresją — użytkownik dostaje wybrane sekundy. Kopiowanie całego klipu (`in = 0`, `out = duration`) tej weryfikacji nie potrzebuje.

---

## 6. Podgląd Zakresu (Preview trim)

W GUI przycisk **Preview trim** odtwarza wyłącznie zakres IN–OUT z pliku źródłowego
(bez ponownego kodowania), więc to, co widzisz w podglądzie, odpowiada dokładnie temu,
co zostanie wyeksportowane. Plan jakości (bitrate, bppf, przewidywany rozmiar) jest
przeliczany na żywo przy każdej zmianie profilu, limitu, kodeka, silnika lub zakresu.

---

## 7. Jak Uruchomić Eksport AV1

### W GUI (`CutGut.exe`):
1. Uruchom `CutGut.exe` (lub `Uruchom_CutGut.bat`).
2. Otwórz plik wideo (**Open video** / **Change video** lub przeciągnij i upuść).
3. W panelu **Export**:
   - **Codec** → **AV1**.
   - **Engine** → **GPU – high quality** (karta RTX 40xx+) lub **CPU – high quality** (`libsvtav1`).
   - **Limit** → np. 20 MB (Discord Free).
4. Kliknij **Export**.

### W CLI (`cutgut.exe`):
```bat
:: Eksport klipu z kodekiem AV1 i limitem 20 MB
cutgut encode "film.mp4" --out out.mp4 --start 10 --end 30 --target 20 --codec av1 --speed hq

:: Analiza planu bitrate i gęstości bitowej AV1 przed kodowaniem
cutgut plan "film.mp4" --target 20 --codec av1

:: Wymuszenie enkodera CPU (libsvtav1)
cutgut encode "film.mp4" --out out.mp4 --target 20 --codec av1 --cpu
```
