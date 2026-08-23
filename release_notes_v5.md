## ✂️ CutGut 202608230-5-0 (Fala 5: Format & Social Export)

### 🌟 Nowości w wydaniu:
- **📱 Presety kadru i proporcji obrazu (`CropOverlay` / `CropBox`)**:
  - **Pionowy 9:16**: Błyskawiczna konwersja poziomych gameplayów (np. Valorant, CS2) do formatu YouTube Shorts, TikTok oraz Instagram Reels.
  - **Kwadrat 1:1**: Kadr 1:1 pod memy, Discord i posty społecznościowe.
  - **Poziomy 16:9 & Oryginalny**: Klasyczny pełny format panoramiczny.
- **🖐️ Interaktywny podgląd i przesuwanie kadru myszką**:
  - Półprzezroczysta nakładka na wideo z siatką trójpodziału i uchwytami.
  - Możliwość swobodnego przesuwania wycinka w lewo/prawo myszką na żywym podglądzie.
  - Przyciski szybkiego wyrównania: `[ ◀ Lewo ]`, `[ 🎯 Środek ]`, `[ Prawo ▶ ]`.
- **🎯 Gotowe profile eksportu społecznościowego**:
  - `🎯 Discord Clip (20 MB)` — 16:9 pod darmowy limit Discorda
  - `💎 Discord Nitro (50 MB)` — 16:9 wysoki bitrate
  - `📱 Shorts / TikTok HQ (50 MB)` — 9:16 maksymalna ostrość pionowa
  - `📱 Shorts Small (20 MB)` — 9:16 pod limit 20 MB
  - `🎬 TikTok / Reels (50 MB)` — 9:16 optymalizacja pionowa
  - `⬛ Square Meme (20 MB)` — 1:1 format kwadratowy
  - `⚙️ Własny format...` — pełna ręczna kontrola
- **📸 Zapis stop-klatki jako PNG (`extract_frame_png`)**:
  - Przycisk `[ 📸 Klatka PNG ]` zapisuje bieżącą klatkę wideo w pełnej jakości PNG (z uwzględnieniem wybranego kadru) do folderu wynikowego (idealne na miniaturkę lub mem).
- **💻 CLI (`cli.py`)**:
  - Dodano flagi `--crop 9:16|1:1|16:9|w:h:x:y`, `--crop-align left|center|right`, `--preset discord|nitro|shorts_hq|shorts_20mb|tiktok|square`, `--screenshot-at <sekundy>`.

### 📦 Pobieranie
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608230-5-0/CutGut.exe)** (Samodzielny plik wykonywalny dla Windows)
