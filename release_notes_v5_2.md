## ✂️ CutGut 202608230-5-2 (Interaktywny ciemny podgląd kadru 9:16 / 1:1 i usprawnienia GUI)

### 🌟 Ulepszenia i poprawki:
- **🌒 Ciemnoszara winieta wokół kadru**: Obszar poza wybranym kadrem jest przyciemniony półprzezroczystym ciemnym grafitem (`rgba(10,15,29,200)`), dzięki czemu **zaznaczenie 9:16 / 1:1 jest jasne, w 100% czytelne i wyraźne**.
- **🖐️ Płynne przeciąganie myszką i kursory**:
  - Najechanie na kadr zmienia kursor w otwartą dłoń (`OpenHandCursor`), a przeciąganie w zaciśniętą pięść (`ClosedHandCursor`).
  - Kliknięcie poza kadr natychmiast przesuwa środek kadru do klikniętego miejsca.
- **🏷️ Plakietka wymiarów i siatka trójpodziału**:
  - Nad ramką wyświetla się plakietka z formatem i rozdzielczością (np. `📱 9:16 (606×1080)`).
  - Wewnątrz ramki wyświetla się subtelna siatka trójpodziału oraz szmaragdowe narożniki (`#34d399`).
- **🛡️ Bezpieczna hierarchia widgetów**: `CropOverlay` jest teraz bezpośrednim dzieckiem `QVideoWidget` z właściwością `WA_TranslucentBackground`, co gwarantuje stabilne wyświetlanie nad sprzętowym odtwarzaczem wideo na Windows.

### 📦 Pobieranie
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608230-5-2/CutGut.exe)** (Samodzielny plik wykonywalny dla Windows)
