import sys
import os
import subprocess
import time
import threading
import urllib.request
import zipfile
import tkinter as tk
from tkinter import ttk, filedialog, messagebox

# Sciezki
if getattr(sys, 'frozen', False):
    BASE_EXEC_DIR = os.path.dirname(sys.executable)
else:
    BASE_EXEC_DIR = os.path.dirname(os.path.abspath(__file__))

os.environ['PATH'] = BASE_EXEC_DIR + os.pathsep + os.environ.get('PATH', '')

CREATE_NO_WINDOW = 0x08000000

class CutGutMini:
    def __init__(self, root):
        self.root = root
        self.root.title('CutGut Portable')
        self.root.geometry('650x480')
        self.root.minsize(600, 440)
        self.root.configure(bg='#0f172a')

        self.input_file = ''
        self.duration_s = 0.0

        # Styles
        self.style = ttk.Style()
        self.style.theme_use('clam')
        self.style.configure('TFrame', background='#1e293b')
        self.style.configure('TLabel', background='#1e293b', foreground='#f8fafc', font=('Segoe UI', 10))
        self.style.configure('Header.TLabel', background='#0f172a', foreground='#60a5fa', font=('Segoe UI', 15, 'bold'))
        self.style.configure('Accent.TButton', font=('Segoe UI', 11, 'bold'), background='#2563eb', foreground='white')
        self.style.map('Accent.TButton', background=[('active', '#1d4ed8')])
        self.style.configure('Action.TButton', font=('Segoe UI', 10, 'bold'), background='#3b82f6', foreground='white')
        self.style.map('Action.TButton', background=[('active', '#2563eb')])

        self.setup_ui()
        threading.Thread(target=self.ensure_ffmpeg, daemon=True).start()

    def setup_ui(self):
        # Header
        lbl_title = ttk.Label(self.root, text='✂️ CutGut - Kompresor Wideo do 10MB / 20MB', style='Header.TLabel')
        lbl_title.pack(pady=12)

        # File Select Frame
        f_file = ttk.Frame(self.root, padding=12)
        f_file.pack(fill='x', padx=15, pady=6)

        btn_file = ttk.Button(f_file, text='📁 WYBIERZ FILM', style='Action.TButton', command=self.choose_file)
        btn_file.pack(side='left', padx=5)

        self.lbl_file = ttk.Label(f_file, text='Nie wybrano pliku (Wybierz MP4/MKV/AVI/MOV)', foreground='#94a3b8')
        self.lbl_file.pack(side='left', padx=10)

        # Range Frame
        f_range = ttk.Frame(self.root, padding=12)
        f_range.pack(fill='x', padx=15, pady=6)

        lbl_r = ttk.Label(f_range, text='📍 Zakres przycinania (w sekundach):', font=('Segoe UI', 10, 'bold'))
        lbl_r.pack(anchor='w', pady=(0, 8))

        f_inputs = ttk.Frame(f_range)
        f_inputs.pack(fill='x')

        ttk.Label(f_inputs, text='Start (s):').pack(side='left', padx=5)
        self.ent_start = tk.Entry(f_inputs, width=8, bg='#0f172a', fg='white', insertbackground='white', font=('Segoe UI', 10))
        self.ent_start.insert(0, '0.0')
        self.ent_start.pack(side='left', padx=5)
        self.ent_start.bind('<KeyRelease>', lambda e: self.update_dur())

        ttk.Label(f_inputs, text='Koniec (s):').pack(side='left', padx=(15, 5))
        self.ent_end = tk.Entry(f_inputs, width=8, bg='#0f172a', fg='white', insertbackground='white', font=('Segoe UI', 10))
        self.ent_end.insert(0, '10.0')
        self.ent_end.pack(side='left', padx=5)
        self.ent_end.bind('<KeyRelease>', lambda e: self.update_dur())

        self.lbl_dur = ttk.Label(f_inputs, text='Długość: 10.0s', foreground='#60a5fa', font=('Segoe UI', 10, 'bold'))
        self.lbl_dur.pack(side='right', padx=10)

        # Options Frame
        f_opts = ttk.Frame(self.root, padding=12)
        f_opts.pack(fill='x', padx=15, pady=6)

        ttk.Label(f_opts, text='Tryb jakości:').grid(row=0, column=0, padx=5, pady=5, sticky='w')
        self.combo_mode = ttk.Combobox(f_opts, values=['🚀 Szybki (H.264 Fast)', '⚖️ Zbalansowany (H.264 Slow)', '💎 Kinowy (H.265 Ultra)'], state='readonly', width=26)
        self.combo_mode.current(0)
        self.combo_mode.grid(row=0, column=1, padx=10, pady=5, sticky='w')

        ttk.Label(f_opts, text='Limit:').grid(row=0, column=2, padx=(15, 5), pady=5, sticky='w')
        self.combo_size = ttk.Combobox(f_opts, values=['🎯 Limit: Do 10 MB', '🚀 Limit: Do 20 MB'], state='readonly', width=18)
        self.combo_size.current(0)
        self.combo_size.grid(row=0, column=3, padx=5, pady=5, sticky='w')

        # Button
        self.btn_comp = tk.Button(
            self.root, text='🚀 PRZYTNIJ I KOMPRESUJ', font=('Segoe UI', 12, 'bold'),
            bg='#2563eb', fg='white', activebackground='#1d4ed8', activeforeground='white',
            relief='flat', height=2, state='disabled', command=self.start_compression
        )
        self.btn_comp.pack(fill='x', padx=15, pady=15)

        # Progress & Status
        self.prog = ttk.Progressbar(self.root, length=100, mode='determinate')
        self.prog.pack(fill='x', padx=15, pady=5)

        self.lbl_status = ttk.Label(self.root, text='Gotowy. Wybierz film z dysku.', background='#0f172a', foreground='#94a3b8', font=('Segoe UI', 9))
        self.lbl_status.pack(pady=5)

    def ensure_ffmpeg(self):
        try:
            subprocess.run(['ffmpeg', '-version'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=CREATE_NO_WINDOW)
            return
        except:
            pass

        ffmpeg_local = os.path.join(BASE_EXEC_DIR, 'ffmpeg.exe')
        if os.path.exists(ffmpeg_local): return

        self.lbl_status.config(text='Pobieranie silnika FFmpeg (jednorazowo)...', foreground='#f59e0b')
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
            self.lbl_status.config(text='Silnik FFmpeg gotowy!', foreground='#10b981')
        except Exception as e:
            self.lbl_status.config(text=f'Wymagany FFmpeg w systemie: {e}', foreground='#ef4444')

    def choose_file(self):
        f = filedialog.askopenfilename(title='Wybierz wideo', filetypes=[('Wideo', '*.mp4 *.mkv *.avi *.mov *.webm *.flv *.ts')])
        if f:
            self.input_file = f
            self.lbl_file.config(text=os.path.basename(f), foreground='#f8fafc')
            dur = self.get_duration(f)
            self.duration_s = dur
            self.ent_start.delete(0, 'end'); self.ent_start.insert(0, '0.0')
            self.ent_end.delete(0, 'end'); self.ent_end.insert(0, f'{min(dur, 30.0):.1f}')
            self.update_dur()
            self.btn_comp.config(state='normal')
            self.lbl_status.config(text=f'Wczytano: {os.path.basename(f)} ({dur:.1f}s)', foreground='#60a5fa')

    def update_dur(self):
        try:
            s = float(self.ent_start.get())
            e = float(self.ent_end.get())
            self.lbl_dur.config(text=f'Długość: {max(e-s, 0):.1f}s')
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
        self.btn_comp.config(state='disabled')
        threading.Thread(target=self.run_ffmpeg, daemon=True).start()

    def run_ffmpeg(self):
        try:
            start_s = float(self.ent_start.get())
            end_s = float(self.ent_end.get())
            dur_s = end_s - start_s
            if dur_s <= 0:
                self.lbl_status.config(text='Błąd: Nieprawidłowy zakres czasu!', foreground='#ef4444')
                self.btn_comp.config(state='normal')
                return

            mode_idx = self.combo_mode.current()
            target_mb = 19.95 if self.combo_size.current() == 1 else 9.95

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

            self.lbl_status.config(text='Etap 1/2: Analiza wideo...', foreground='#60a5fa')
            self.prog['value'] = 30
            subprocess.run(['ffmpeg', '-y'] + common + p1, check=True, creationflags=CREATE_NO_WINDOW)

            self.lbl_status.config(text='Etap 2/2: Finalna kompresja...', foreground='#60a5fa')
            self.prog['value'] = 75
            subprocess.run(['ffmpeg', '-y'] + common + p2, check=True, creationflags=CREATE_NO_WINDOW)

            for f in os.listdir(BASE_EXEC_DIR):
                if f'stats_{unique_id}' in f:
                    try: os.remove(os.path.join(BASE_EXEC_DIR, f))
                    except: pass

            self.prog['value'] = 100
            sb = os.path.getsize(out_file)
            mib = sb / (1024 * 1024)
            self.lbl_status.config(text=f'Sukces! Rozmiar: {mib:.2f} MB w Eksploratorze.', foreground='#10b981')
            os.startfile(out_dir)

        except Exception as e:
            self.lbl_status.config(text=f'Błąd: {str(e)}', foreground='#ef4444')
        finally:
            self.btn_comp.config(state='normal')

if __name__ == '__main__':
    root = tk.Tk()
    app = CutGutMini(root)
    root.mainloop()
