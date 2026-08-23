## ✂️ CutGut 202608230-5-3 (Natychmiastowy i 100% niezawodny podgląd kadru 9:16 przez QVideoSink)

### 🌟 Co rozwiązano w tym wydaniu:
- **🎯 100% pewny i widoczny podgląd kadru (QVideoSink)**:
  - Zastąpiono hardware'owy widget WMF / DirectX natywnym potokiem `QVideoSink` + `VideoCanvasWidget`.
  - Dzięki temu klatka wideo oraz ciemnoszara winieta i jasna ramka 9:16 są rysowane **w jednym i tym samym potoku graficznym `QPainter`**, co całkowicie eliminuje problem znikania nakładki pod warstwą sprzętową Windows.
- **🌒 Wyraźna winieta 9:16 / 1:1**:
  - Całość ekranu poza kadrem 9:16 jest ciemnoszara/grafitowa (`rgba(10, 15, 29, 210)`).
  - Wybrany kadr jest w 100% jasny, naturalny i czysty.
  - Płynne przeciąganie kursorem dłoni i przyciski `[ ◀ Lewo ]`, `[ 🎯 Środek ]`, `[ Prawo ▶ ]`.

### 📦 Pobieranie
- **[CutGut.exe](https://github.com/Zeluqe/CutGut/releases/download/202608230-5-3/CutGut.exe)** (Samodzielny plik wykonywalny dla Windows)
