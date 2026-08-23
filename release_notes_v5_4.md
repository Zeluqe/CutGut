## ✂️ CutGut 202608240-5-4 (Poprawka błędu kompresji i inicjalizacji kolejki z kadrowaniem)

### 🐛 Co naprawiono w tym wydaniu:
- **Naprawiono błąd podczas klikania 'PRZYTNIJ I KOMPRESUJ'**: Do definicji modeli danych `EncodeJob` oraz `ExportResult` w `encoder.py` dodano brakujące pole `crop_box: Optional[CropBox] = None`. Zapobiega to crashowi `TypeError: unexpected keyword argument 'crop_box'`.
- **Pełne kadrowanie 9:16 i profile społecznościowe**: Eksport pionowy 9:16 (Shorts/TikTok), kwadratowy 1:1 oraz poziomy 16:9 działają płynnie i stabilnie w kolejce zadań oraz bezpośredniej kompresji.

### 📦 Pobieranie
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608240-5-4/CutGut.exe)** (Samodzielny plik wykonywalny dla Windows)
