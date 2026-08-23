# ✂️ CutGut

> **Inteligentne narzędzie do precyzyjnego przycinania i kompresji wideo (do limitów 10 MB i 20 MB np. na Discord, Messenger, e-mail).**

CutGut automatycznie oblicza optymalny 2-przebiegowy (*2-pass*) bitrate dla kodeków **H.264** oraz **H.265**, gwarantując maksymalną możliwą jakość obrazu przy jednoczesnym pewnym zmieszczeniu się tuż pod limitem wagowym (np. **~9,95 MB** lub **~19,95 MB** w Eksploratorze Windows).

---

## ✨ Kluczowe funkcje

- 🎯 **Precyzyjne limity rozmiaru**:
  - **Limit do 10 MB** (Domyślny) – celuje w ~9,95 MB (idealne na darmowy Discord).
  - **Limit do 20 MB** – celuje w ~19,95 MB dla dłuższego wideo w wyższej jakości.
- 🚀 **Trzy tryby kodowania**:
  - **Szybki (H.264 Fast)** – błyskawiczny eksport przy zachowaniu limitu.
  - **Zbalansowany (H.264 Slow)** – wysoka ostrość i płynność ruchu.
  - **Kinowy (H.265 Ultra)** – najwyższa wydajność kompresji HEVC przy niskim bitrate.
- 🎬 **Wygodny podgląd wideo**:
  - Oś czasu z suwakiem i kontrolkami odtwarzania.
  - Wygodne ustawianie punktów **Start** i **Koniec** przycięcia.
  - Obsługa metody *Drag & Drop* (przeciągnij i upuść plik wideo do okna).
- 🧹 **Automatyczne czyszczenie**:
  - Usuwanie plików tymczasowych i logów analizy po zakończeniu pracy.
  - Opcjonalne pytanie o usunięcie ciężkiego pliku źródłowego.

---

## 🖥️ Dostępne wersje aplikacji

1. **Desktop Pro (gui.py)** – Pełna wersja okienkowa w technologii **PyQt6** ze zintegrowanym odtwarzaczem wideo.
2. **Desktop Lite (gui_mini.py)** – Ultra-lekka wersja okienkowa (poniżej 12 MB) z auto-pobieraniem FFmpeg.
3. **Web Panel (pp.py)** – Panel przeglądarkowy oparty o **Gradio** do edycji w przeglądarce.

---

## 🚀 Szybki start (Wymagania i instalacja)

### Wymagania:
- Python 3.10+
- [FFmpeg](https://ffmpeg.org/) zainstalowany w systemie i dodany do PATH (lub w folderze programu).

### Instalacja bibliotek:
`ash
git clone https://github.com/Zeluqe/CutGut.git
cd CutGut
python -m venv venv
venv\Scripts\activate  # Na Windows
pip install -r requirements.txt
`

### Uruchomienie:
- **Wersja Desktop GUI**: Uruchom plik Uruchom_CutGut.bat lub wpisz python gui.py
- **Wersja Web**: python app.py

---

## 📜 Licencja
Projekt udostępniony na licencji MIT.
