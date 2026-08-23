import sys
import os
import subprocess
import time
import threading
import urllib.request
import zipfile
import tkinter as tk
from tkinter import filedialog, messagebox
import customtkinter as ctk

# Sciezki
if getattr(sys, 'frozen', False):
    BASE_EXEC_DIR = os.path.dirname(sys.executable)
else:
    BASE_EXEC_DIR = os.path.dirname(os.path.abspath(__file__))

os.environ['PATH'] = BASE_EXEC_DIR + os.pathsep + os.environ.get('PATH', '')

CREATE_NO_WINDOW = 0x08000000

ctk.set_appearance_mode('dark')
ctk.set_default_color_theme('blue')

class CutGutLite(ctk.CTk):
    def __init__(self):
        super().__init__()

        self.title('CutGut Lite (Ultra Portable)')
        self.geometry('720x540')
        self.minsize(680, 500)

        self.input_file = ''
        self.duration_s = 0.0

        # UI Setup
        self.setup_ui()

        # Check FFmpeg in background
        threading.Thread(target=self.ensure_ffmpeg, daemon=True).start()

    def setup_ui(self):
        # Header
        self.header_frame = ctk.CTkFrame(self, fg_color='#0f172a', corner_radius=10)
        self.header_frame.pack(fill='x', padx=15, pady=10)

        self.title_lbl = ctk.CTkLabel(
            self.header_frame, 
            text='CutGut - Kompresor Wideo (do 10MB / 20MB)', 
            font=ctk.CTkFont(size=18, weight='bold'),
            text_color='#60a5fa'
        )
        self.title_lbl.pack(pady=10)

        # File Selection Frame
        self.file_frame = ctk.CTkFrame(self, fg_color='#1e293b', corner_radius=10)
        self.file_frame.pack(fill='x', padx=15, pady=5)

        self.file_btn = ctk.CTkButton(
            self.file_frame, 
            text='WYBIERZ FILM', 
            font=ctk.CTkFont(weight='bold'),
            command=self.choose_file,
            fg_color='#3b82f6',
            hover_color='#2563eb',
            height=38
        )
        self.file_btn.pack(side='left', padx=15, pady=12)

        self.file_lbl = ctk.CTkLabel(
            self.file_frame, 
            text='Nie wybrano pliku (MP4, MKV, AVI, MOV)', 
            font=ctk.CTkFont(size=13),
            text_color='#94a3b8'
        )
        self.file_lbl.pack(side='left', padx=10, pady=12)

        # Range Frame
        self.range_frame = ctk.CTkFrame(self, fg_color='#1e293b', corner_radius=10)
        self.range_frame.pack(fill='x', padx=15, pady=5)

        self.range_title = ctk.CTkLabel(
            self.range_frame, 
            text='ZAKRES PRZYCINANIA (w sekundach):', 
            font=ctk.CTkFont(size=13, weight='bold'),
            text_color='#f8fafc'
        )
        self.range_title.pack(pady=(10, 5))

        self.slider_frame = ctk.CTkFrame(self.range_frame, fg_color='transparent')
        self.slider_frame.pack(fill='x', padx=20, pady=5)

        self.lbl_start = ctk.CTkLabel(self.slider_frame, text='Poczatek (s):', font=ctk.CTkFont(weight='bold'))
        self.lbl_start.pack(side='left', padx=5)

        self.entry_start = ctk.CTkEntry(self.slider_frame, width=80)
        self.entry_start.insert(0, '0.0')
        self.entry_start.pack(side='left', padx=5)

        self.lbl_end = ctk.CTkLabel(self.slider_frame, text='Koniec (s):', font=ctk.CTkFont(weight='bold'))
        self.lbl_end.pack(side='left', padx=(20, 5))

        self.entry_end = ctk.CTkEntry(self.slider_frame, width=80)
        self.entry_end.insert(0, '10.0')
        self.entry_end.pack(side='left', padx=5)

        self.lbl_dur_info = ctk.CTkLabel(
            self.slider_frame, 
            text='Dlugosc: 10.0s', 
            font=ctk.CTkFont(size=13, weight='bold'),
            text_color='#60a5fa'
        )
        self.lbl_dur_info.pack(side='right', padx=10)

        # Options Frame (Mode & Size)
        self.opts_frame = ctk.CTkFrame(self, fg_color='#1e293b', corner_radius=10)
        self.opts_frame.pack(fill='x', padx=15, pady=5)

        self.lbl_mode = ctk.CTkLabel(self.opts_frame, text='Tryb:', font=ctk.CTkFont(weight='bold'))
        self.lbl_mode.grid(row=0, column=0, padx=15, pady=10, sticky='w')

        self.mode_combo = ctk.CTkComboBox(
            self.opts_frame, 
            values=['Szybki (H.264 Fast)', 'Zbalansowany (H.264 Slow)', 'Kinowy (H.265 Ultra)'],
            width=210
        )
        self.mode_combo.set('Szybki (H.264 Fast)')
        self.mode_combo.grid(row=0, column=1, padx=10, pady=10, sticky='w')

        self.lbl_size = ctk.CTkLabel(self.opts_frame, text='Limit:', font=ctk.CTkFont(weight='bold'))
        self.lbl_size.grid(row=0, column=2, padx=(20, 10), pady=10, sticky='w')

        self.size_combo = ctk.CTkComboBox(
            self.opts_frame, 
            values=['Limit: Do 10 MB', 'Limit: Do 20 MB'],
            width=170
        )
        self.size_combo.set('Limit: Do 10 MB')
        self.size_combo.grid(row=0, column=3, padx=10, pady=10, sticky='w')

        # Compress Button
        self.btn_compress = ctk.CTkButton(
            self, 
            text='PRZYTNIJ I KOMPRESUJ', 
            font=ctk.CTkFont(size=15, weight='bold'),
            command=self.start_compression,
            fg_color='#2563eb',
            hover_color='#1d4ed8',
            height=42,
            state='disabled'
        )
        self.btn_compress.pack(fill='x', padx=15, pady=12)

        # Progress and Status
        self.progress_bar = ctk.CTkProgressBar(self)
        self.progress_bar.set(0)
        self.progress_bar.pack(fill='x', padx=15, pady=5)

        self.status_lbl = ctk.CTkLabel(
            self, 
            text='Gotowy. Wybierz film do przyciecia.', 
            font=ctk.CTkFont(size=13),
            text_color='#94a3b8'
        )
        self.status_lbl.pack(pady=5)

    def ensure_ffmpeg(self):
        try:
            subprocess.run(['ffmpeg', '-version'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=CREATE_NO_WINDOW)
            return
        except:
            pass

        ffmpeg_local = os.path.join(BASE_EXEC_DIR, 'ffmpeg.exe')
        if os.path.exists(ffmpeg_local):
            return

        self.status_lbl.configure(text='Pobieranie silnika FFmpeg (jednorazowo)...', text_color='#f59e0b')
        try:
            url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip'
            zip_dest = os.path.join(BASE_EXEC_DIR, 'ffmpeg_temp.zip')
            urllib.request.urlretrieve(url, zip_dest)
            
            with zipfile.ZipFile(zip_dest, 'r') as z:
                for member in z.namelist():
                    if member.endswith('ffmpeg.exe'):
                        with z.open(member) as source, open(ffmpeg_local, 'wb') as target:
                            target.write(source.read())
                        break
            try: os.remove(zip_dest)
            except: pass
            self.status_lbl.configure(text='Silnik FFmpeg gotowy!', text_color='#10b981')
        except Exception as e:
            self.status_lbl.configure(text=f'Uwaga: Wymagany FFmpeg w PATH ({e})', text_color='#ef4444')

    def choose_file(self):
        f = filedialog.askopenfilename(
            title='Wybierz wideo', 
            filetypes=[('Pliki Wideo', '*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts')]
        )
        if f:
            self.input_file = f
            self.file_lbl.configure(text=os.path.basename(f), text_color='#f8fafc')
            dur = self.get_duration(f)
            self.duration_s = dur
            self.entry_start.delete(0, 'end')
            self.entry_start.insert(0, '0.0')
            self.entry_end.delete(0, 'end')
            self.entry_end.insert(0, f'{min(dur, 30.0):.1f}')
            self.update_dur_lbl()
            self.btn_compress.configure(state='normal')
            self.status_lbl.configure(text=f'Wczytano wideo (dlugosc: {dur:.1f}s)', text_color='#60a5fa')

    def update_dur_lbl(self):
        try:
            s = float(self.entry_start.get())
            e = float(self.entry_end.get())
            d = max(e - s, 0)
            self.lbl_dur_info.configure(text=f'Dlugosc: {d:.1f}s')
        except:
            pass

    def get_duration(self, filepath):
        try:
            cmd = ['ffprobe', '-v', 'error', '-show_entries', 'format=duration', '-of', 'default=noprint_wrappers=1:nokey=1', filepath]
            res = subprocess.run(cmd, capture_output=True, text=True, creationflags=CREATE_NO_WINDOW)
            return float(res.stdout.strip())
        except:
            try:
                cmd = ['ffmpeg', '-i', filepath]
                res = subprocess.run(cmd, capture_output=True, text=True, creationflags=CREATE_NO_WINDOW)
                for line in res.stderr.split('\n'):
                    if 'Duration:' in line:
                        t_str = line.split('Duration:')[1].split(',')[0].strip()
                        h, m, s = t_str.split(':')
                        return float(h)*3600 + float(m)*60 + float(s)
            except:
                pass
        return 60.0

    def start_compression(self):
        self.btn_compress.configure(state='disabled')
        threading.Thread(target=self.run_ffmpeg, daemon=True).start()

    def run_ffmpeg(self):
        try:
            start_s = float(self.entry_start.get())
            end_s = float(self.entry_end.get())
            dur_s = end_s - start_s
            if dur_s <= 0:
                self.status_lbl.configure(text='Blad: Czas konca musi byc wiekszy niz poczatku!', text_color='#ef4444')
                self.btn_compress.configure(state='normal')
                return

            mode_idx = 0 if 'Szybki' in self.mode_combo.get() else (1 if 'Zbalansowany' in self.mode_combo.get() else 2)
            is_20mb = '20' in self.size_combo.get()
            target_mb = 19.95 if is_20mb else 9.95

            out_dir = os.path.join(BASE_EXEC_DIR, 'outputs')
            os.makedirs(out_dir, exist_ok=True)
            unique_id = int(time.time())

            target_bytes = target_mb * 1024 * 1024
            a_br = 96000 if mode_idx == 2 else 128000
            overhead_bytes = 30000 + (dur_s * 500)
            net_video_bytes = max(target_bytes - (a_br * dur_s / 8) - overhead_bytes, 1000)
            v_br = max(int((net_video_bytes * 8 / dur_s) / 1000), 50)

            stats = os.path.join(BASE_EXEC_DIR, f'stats_{unique_id}')
            out_file = os.path.join(out_dir, f'CutGut_{unique_id}.mp4')
            common = ['-ss', str(start_s), '-t', str(dur_s), '-i', self.input_file]

            if mode_idx == 0:
                codec, preset = 'libx264', 'veryfast'
                p1 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '1', '-passlogfile', stats, '-an', '-f', 'mp4', 'NUL']
                p2 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '2', '-passlogfile', stats, '-c:a', 'aac', '-b:a', '128k', out_file]
            elif mode_idx == 1:
                codec, preset = 'libx264', 'slow'
                p1 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '1', '-passlogfile', stats, '-an', '-f', 'mp4', 'NUL']
                p2 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-pass', '2', '-passlogfile', stats, '-c:a', 'aac', '-b:a', '128k', out_file]
            else:
                codec, preset = 'libx265', 'slow'
                p1 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-x265-params', f'pass=1:stats={stats}', '-an', '-f', 'mp4', 'NUL']
                p2 = ['-c:v', codec, '-b:v', f'{v_br}k', '-preset', preset, '-x265-params', f'pass=2:stats={stats}', '-c:a', 'aac', '-b:a', '96k', out_file]

            self.status_lbl.configure(text='Etap 1/2: Analiza wideo...', text_color='#60a5fa')
            self.progress_bar.set(0.3)
            subprocess.run(['ffmpeg', '-y'] + common + p1, check=True, creationflags=CREATE_NO_WINDOW)

            self.status_lbl.configure(text='Etap 2/2: Finalna kompresja...', text_color='#60a5fa')
            self.progress_bar.set(0.75)
            subprocess.run(['ffmpeg', '-y'] + common + p2, check=True, creationflags=CREATE_NO_WINDOW)

            for f in os.listdir(BASE_EXEC_DIR):
                if f'stats_{unique_id}' in f:
                    try: os.remove(os.path.join(BASE_EXEC_DIR, f))
                    except: pass

            self.progress_bar.set(1.0)
            sb = os.path.getsize(out_file)
            mib = sb / (1024 * 1024)
            self.status_lbl.configure(text=f'Sukces! Rozmiar: {mib:.2f} MB w Eksploratorze.', text_color='#10b981')

            os.startfile(out_dir)

        except Exception as e:
            self.status_lbl.configure(text=f'Blad: {str(e)}', text_color='#ef4444')
        finally:
            self.btn_compress.configure(state='normal')

if __name__ == '__main__':
    app = CutGutLite()
    app.mainloop()
