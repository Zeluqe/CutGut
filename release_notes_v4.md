## ✂️ CutGut 202608230-4-0 (Fala 4)

### 🌟 Nowości w wydaniu:
- **👥 Osobne okno porównania jakości A/B (`QualityComparisonDialog` / `comparison_dialog.py`)**:
  - Nieblokujące, niezależne okno odtwarzające jednocześnie przycięty oryginał (po lewej) oraz skompresowany wynik / próbkę jakości (po prawej).
  - Wspólny pasek czasu, wspólny Play/Pause (ze spacją i kliknięciem w wideo) oraz automatyczna korekcja driftu co 250ms.
  - Precyzyjne sterowanie klatka po klatce (`◀ -1s`, `◀ -1 fr`, `+1 fr ▶`, `+1s ▶`).
  - **Bezpieczne usuwanie oryginału (Deferred Cleanup)**: Plik źródłowy nigdy nie jest usuwany ani przenoszony do Kosza w trakcie trwania porównania — polityka czyszczenia uruchamia się dopiero po zamknięciu okna A/B.
- **🚀 Aktualizacja w programie (1-Click In-App Auto-Update / `update_service.py`)**:
  - Sprawdzanie wydań GitHub Releases w tle przy starcie oraz na żądanie w Ustawieniach (`⚙`).
  - Dialog nowej wersji z changelogiem i pobieraniem w tle.
  - Bezpieczny mechanizm podmiany pliku wykonywalnego z kopią zapasową (`CutGut.exe.bak`) i natychmiastowym restartem nowej wersji.
  - Weryfikacja integralności pobranego pliku.
- **🏷️ Nowa semantyka wersji `DATA-EXTRA-FIXY`**:
  - **DATA** (`YYYYMMDDN`): Data wydania / numer zrzutu.
  - **EXTRA**: Duże pakiety funkcji (np. 4 = A/B Dialog + Auto-updater).
  - **FIXY**: Poprawki błędów oraz małe korekty UX, tekstów, parametrów i wyglądu.
- **💻 Model `ExportResult`**: Ujednolicona struktura wyników eksportu w silniku `encoder.py`.

### 📦 Pobieranie
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608230-4-0/CutGut.exe)** (Samodzielny plik wykonywalny dla Windows)
