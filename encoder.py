import os
import sys
import subprocess
import time
import json
import re
import shutil
import urllib.request
import zipfile
from dataclasses import dataclass
from typing import Optional, Callable

CREATE_NO_WINDOW = 0x08000000 if os.name == 'nt' else 0

@dataclass
class VideoInfo:
    duration: float
    width: int
    height: int
    fps: float
    bitrate: int
    codec: str
    audio_codec: Optional[str]

@dataclass
class ProgressUpdate:
    stage: str          # e.g. 'Przebieg 1/2' lub 'Kompresja GPU'
    percent: float      # 0.0 - 100.0
    speed: str          # e.g. '3.2x'
    fps: float
    eta_s: float
    elapsed_s: float

class CancellationToken:
    def __init__(self):
        self.cancelled = False
        self.current_process: Optional[subprocess.Popen] = None

    def cancel(self):
        self.cancelled = True
        if self.current_process and self.current_process.poll() is None:
            try:
                self.current_process.terminate()
                time.sleep(0.2)
                if self.current_process.poll() is None:
                    self.current_process.kill()
            except Exception:
                pass

class EncodingError(Exception):
    pass

def get_base_dir() -> str:
    if getattr(sys, 'frozen', False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.abspath(__file__))

def get_ffmpeg_path() -> str:
    # 1. Sprawdz PATH
    p = shutil.which('ffmpeg')
    if p:
        return p
    # 2. Sprawdz folder aplikacji
    local = os.path.join(get_base_dir(), 'ffmpeg.exe')
    if os.path.exists(local):
        return local
    return 'ffmpeg'

def get_ffprobe_path() -> str:
    p = shutil.which('ffprobe')
    if p:
        return p
    local = os.path.join(get_base_dir(), 'ffprobe.exe')
    if os.path.exists(local):
        return local
    return 'ffprobe'

def ensure_ffmpeg(status_callback: Optional[Callable[[str], None]] = None) -> bool:
    try:
        cmd = [get_ffmpeg_path(), '-version']
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=CREATE_NO_WINDOW)
        if res.returncode == 0:
            return True
    except Exception:
        pass

    local_ffmpeg = os.path.join(get_base_dir(), 'ffmpeg.exe')
    if os.path.exists(local_ffmpeg):
        os.environ['PATH'] = get_base_dir() + os.pathsep + os.environ.get('PATH', '')
        return True

    if status_callback:
        status_callback('Pobieranie silnika FFmpeg...')

    try:
        url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip'
        zip_dest = os.path.join(get_base_dir(), 'ffmpeg_dl_temp.zip')
        urllib.request.urlretrieve(url, zip_dest)
        
        with zipfile.ZipFile(zip_dest, 'r') as z:
            for member in z.namelist():
                if member.endswith('ffmpeg.exe'):
                    with z.open(member) as src, open(local_ffmpeg, 'wb') as dst:
                        dst.write(src.read())
                    break
        try: os.remove(zip_dest)
        except Exception: pass
        
        os.environ['PATH'] = get_base_dir() + os.pathsep + os.environ.get('PATH', '')
        return os.path.exists(local_ffmpeg)
    except Exception as e:
        if status_callback:
            status_callback(f'Blad pobierania FFmpeg: {e}')
        return False

def probe_video(file_path: str) -> VideoInfo:
    if not os.path.exists(file_path):
        raise FileNotFoundError(f'Plik nie istnieje: {file_path}')

    # 1. Proba przez ffprobe (JSON)
    cmd = [
        get_ffprobe_path(), '-v', 'error',
        '-show_entries', 'format=duration,bit_rate:stream=codec_name,codec_type,width,height,r_frame_rate,duration',
        '-of', 'json', file_path
    ]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, check=True, creationflags=CREATE_NO_WINDOW)
        data = json.loads(res.stdout)
        
        duration = float(data.get('format', {}).get('duration', 0.0))
        bitrate = int(data.get('format', {}).get('bit_rate', 0))
        
        video_stream = None
        audio_stream = None
        for s in data.get('streams', []):
            if s.get('codec_type') == 'video' and not video_stream:
                video_stream = s
            elif s.get('codec_type') == 'audio' and not audio_stream:
                audio_stream = s

        if not video_stream:
            raise EncodingError('Brak strumienia wideo w pliku!')

        width = int(video_stream.get('width', 1920))
        height = int(video_stream.get('height', 1080))
        codec = video_stream.get('codec_name', 'h264')
        
        # Parsowanie FPS
        fps = 60.0
        fps_str = video_stream.get('r_frame_rate', '60/1')
        if '/' in fps_str:
            num, den = fps_str.split('/')
            den_f = float(den)
            if den_f > 0:
                fps = float(num) / den_f
        elif fps_str:
            fps = float(fps_str)

        if duration <= 0 and 'duration' in video_stream:
            duration = float(video_stream['duration'])

        audio_codec = audio_stream.get('codec_name') if audio_stream else None

        return VideoInfo(
            duration=duration,
            width=width,
            height=height,
            fps=round(fps, 2),
            bitrate=bitrate,
            codec=codec,
            audio_codec=audio_codec
        )
    except Exception:
        return _fallback_probe(file_path)

def _fallback_probe(file_path: str) -> VideoInfo:
    cmd = [get_ffmpeg_path(), '-i', file_path]
    res = subprocess.run(cmd, capture_output=True, text=True, creationflags=CREATE_NO_WINDOW)
    duration = 60.0
    width, height = 1920, 1080
    fps = 60.0
    codec = 'h264'
    audio_codec = 'aac'

    for line in res.stderr.splitlines():
        if 'Duration:' in line:
            m = re.search(r'Duration:\s*(\d+):(\d+):(\d+\.\d+)', line)
            if m:
                duration = int(m.group(1))*3600 + int(m.group(2))*60 + float(m.group(3))
        if 'Stream #' in line and 'Video:' in line:
            m_res = re.search(r'(\d{3,5})x(\d{3,5})', line)
            if m_res:
                width, height = int(m_res.group(1)), int(m_res.group(2))
            m_fps = re.search(r'(\d+(?:\.\d+)?)\s*fps', line)
            if m_fps:
                fps = float(m_fps.group(1))
            if 'hevc' in line.lower() or 'h265' in line.lower():
                codec = 'hevc'
            elif 'h264' in line.lower():
                codec = 'h264'

    return VideoInfo(
        duration=duration,
        width=width,
        height=height,
        fps=fps,
        bitrate=0,
        codec=codec,
        audio_codec=audio_codec
    )

def check_nvenc_support() -> bool:
    try:
        cmd = [get_ffmpeg_path(), '-y', '-f', 'lavfi', '-i', 'testsrc=duration=0.1:size=320x240:rate=1', '-c:v', 'h264_nvenc', '-f', 'null', '-']
        res = subprocess.run(cmd, capture_output=True, creationflags=CREATE_NO_WINDOW)
        return res.returncode == 0
    except Exception:
        return False

def calculate_target_bytes(target_mb: float) -> int:
    # Dual-Limit Math: bezpieczenstwo dla limitow binarnych (MiB) i dziesietnych (MB)
    # min(0.995 * N * 1024^2, 0.98 * N * 1000^2)
    binary_limit = 0.995 * target_mb * 1024 * 1024
    decimal_limit = 0.98 * target_mb * 1000 * 1000
    return int(min(binary_limit, decimal_limit))

def calculate_plan(video_info: VideoInfo, start_s: float, end_s: float, target_mb: float, is_hevc: bool = False):
    dur_s = max(end_s - start_s, 0.1)
    
    # 1. Oblicz maksymalny dozwolony rozmiar oraz bezpieczny cel bajtowy
    target_bytes = calculate_target_bytes(target_mb)
    
    # 2. Narzut audio i kontenera
    audio_bps = 96000 if is_hevc else 128000
    overhead_bytes = 40000 + int(dur_s * 600)
    
    net_video_bytes = max(target_bytes - int(audio_bps * dur_s / 8) - overhead_bytes, 1000)
    video_kbps = max(int((net_video_bytes * 8 / dur_s) / 1000), 50)
    
    # 3. Inteligentne skalowanie i klatkaz
    filters = []
    if video_kbps < 450:
        if video_info.height > 480:
            filters.append('scale=-2:480')
        if video_info.fps > 30:
            filters.append('fps=30')
    elif video_kbps < 900:
        if video_info.height > 720:
            filters.append('scale=-2:720')
        if video_info.fps > 45:
            filters.append('fps=30')
            
    filter_str = ','.join(filters) if filters else None
    
    return {
        'duration_s': dur_s,
        'target_bytes': target_bytes,
        'video_kbps': video_kbps,
        'audio_kbps': int(audio_bps / 1000),
        'filter_str': filter_str
    }

def encode_video(
    input_path: str,
    output_path: str,
    start_s: float,
    end_s: float,
    target_mb: float = 20.0,
    preset_mode: str = 'NVENC_HQ',  # 'NVENC_HQ', 'NVENC_FAST', 'CPU_BALANCED', 'CPU_FAST', 'CPU_HEVC'
    progress_callback: Optional[Callable[[ProgressUpdate], None]] = None,
    cancel_token: Optional[CancellationToken] = None
) -> str:
    if not os.path.exists(input_path):
        raise FileNotFoundError(f'Nie znaleziono pliku wejsciowego: {input_path}')

    if end_s <= start_s:
        raise ValueError('Czas konca musi byc wiekszy niz czas poczatku!')

    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    video_info = probe_video(input_path)
    is_hevc = (preset_mode == 'CPU_HEVC')
    
    # Auto-fallback do CPU jesli brak NVENC
    if preset_mode in ('NVENC_HQ', 'NVENC_FAST') and not check_nvenc_support():
        preset_mode = 'CPU_BALANCED'

    plan = calculate_plan(video_info, start_s, end_s, target_mb, is_hevc)
    dur_s = plan['duration_s']
    v_kbps = plan['video_kbps']
    a_kbps = plan['audio_kbps']
    filter_str = plan['filter_str']
    
    unique_id = int(time.time() * 1000)
    base_dir = get_base_dir()
    stats_file = os.path.join(base_dir, f'ffmpeg2pass_{unique_id}')

    # Klatkowa dokladnosc: -i przed -ss / -to
    common_args = ['-i', input_path, '-ss', str(start_s), '-to', str(end_s)]
    vf_args = ['-vf', filter_str] if filter_str else []
    ffmpeg_bin = get_ffmpeg_path()
    
    try:
        if preset_mode in ('NVENC_HQ', 'NVENC_FAST'):
            nv_preset = 'p6' if preset_mode == 'NVENC_HQ' else 'p3'
            cmd = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', 'h264_nvenc',
                '-b:v', f'{v_kbps}k',
                '-maxrate', f'{int(v_kbps * 1.25)}k',
                '-bufsize', f'{int(v_kbps * 2.0)}k',
                '-preset', nv_preset,
                '-tune', 'hq',
                '-rc:v', 'vbr',
                '-multipass', 'fullres',
                '-pix_fmt', 'yuv420p',
                '-c:a', 'aac',
                '-b:a', f'{a_kbps}k',
                '-movflags', '+faststart',
                output_path
            ]
            _run_ffmpeg_with_progress(cmd, dur_s, 'Kompresja GPU (NVIDIA NVENC)', progress_callback, cancel_token)

        else:
            codec = 'libx265' if preset_mode == 'CPU_HEVC' else 'libx264'
            cpu_preset = 'veryfast' if preset_mode == 'CPU_FAST' else 'slow'

            # Pass 1
            cmd_pass1 = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', codec,
                '-b:v', f'{v_kbps}k',
                '-preset', cpu_preset,
                '-pass', '1',
                '-passlogfile', stats_file,
                '-an',
                '-f', 'null',
                '-'
            ]
            _run_ffmpeg_with_progress(cmd_pass1, dur_s, 'Analiza wideo (Przebieg 1/2)', progress_callback, cancel_token, stage_weight=0.35, stage_offset=0.0)

            if cancel_token and cancel_token.cancelled:
                raise EncodingError('Anulowano przez uzytkownika.')

            # Pass 2
            cmd_pass2 = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', codec,
                '-b:v', f'{v_kbps}k',
                '-maxrate', f'{int(v_kbps * 1.3)}k',
                '-bufsize', f'{int(v_kbps * 2.0)}k',
                '-preset', cpu_preset,
                '-pass', '2',
                '-passlogfile', stats_file,
                '-pix_fmt', 'yuv420p',
                '-c:a', 'aac',
                '-b:a', f'{a_kbps}k',
                '-movflags', '+faststart',
                output_path
            ]
            _run_ffmpeg_with_progress(cmd_pass2, dur_s, 'Finalna kompresja (Przebieg 2/2)', progress_callback, cancel_token, stage_weight=0.65, stage_offset=35.0)

        # Weryfikacja rozmiaru pliku (Retry loop w razie przekroczenia)
        if os.path.exists(output_path):
            actual_size = os.path.getsize(output_path)
            max_allowed = plan['target_bytes']
            
            if actual_size > max_allowed:
                corrected_kbps = int(v_kbps * (max_allowed / actual_size) * 0.94)
                corrected_kbps = max(corrected_kbps, 40)
                
                cmd_retry = [
                    ffmpeg_bin, '-y', '-hide_banner',
                    '-progress', 'pipe:1'
                ] + common_args + vf_args + [
                    '-c:v', 'libx264',
                    '-b:v', f'{corrected_kbps}k',
                    '-preset', 'veryfast',
                    '-pix_fmt', 'yuv420p',
                    '-c:a', 'aac',
                    '-b:a', f'{a_kbps}k',
                    '-movflags', '+faststart',
                    output_path
                ]
                _run_ffmpeg_with_progress(cmd_retry, dur_s, 'Korekta rozmiaru...', progress_callback, cancel_token)
                
                # Ostateczna weryfikacja po probie korekty
                final_size = os.path.getsize(output_path)
                if final_size > max_allowed:
                    raise EncodingError(
                        f'Plik wyjsciowy ({final_size / 1000000:.2f} MB) przekracza dopuszczalny limit {target_mb} MB '
                        f'(maksimum: {max_allowed / 1000000:.2f} MB) pomimo proby korekty.'
                    )

        return output_path

    finally:
        for f in os.listdir(base_dir):
            if f'ffmpeg2pass_{unique_id}' in f:
                try: os.remove(os.path.join(base_dir, f))
                except Exception: pass

def _run_ffmpeg_with_progress(
    cmd: list,
    total_duration_s: float,
    stage_name: str,
    progress_callback: Optional[Callable[[ProgressUpdate], None]],
    cancel_token: Optional[CancellationToken],
    stage_weight: float = 1.0,
    stage_offset: float = 0.0
):
    start_time = time.time()
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
        universal_newlines=True,
        creationflags=CREATE_NO_WINDOW
    )

    if cancel_token:
        cancel_token.current_process = proc

    out_time_us = 0
    speed_str = '1.0x'
    fps_val = 0.0

    try:
        while True:
            if cancel_token and cancel_token.cancelled:
                proc.terminate()
                raise EncodingError('Operacja anulowana przez uzytkownika.')

            line = proc.stdout.readline()
            if not line and proc.poll() is not None:
                break
            
            line = line.strip()
            if not line:
                continue

            if '=' in line:
                key, _, val = line.partition('=')
                key, val = key.strip(), val.strip()
                
                if key == 'out_time_us':
                    try: out_time_us = int(val)
                    except ValueError: pass
                elif key == 'speed':
                    speed_str = val
                elif key == 'fps':
                    try: fps_val = float(val)
                    except ValueError: pass
                elif key == 'progress' and val == 'continue':
                    cur_time_s = out_time_us / 1000000.0
                    sub_pct = min(max(cur_time_s / max(total_duration_s, 0.1), 0.0), 1.0)
                    overall_pct = stage_offset + (sub_pct * stage_weight * 100.0)
                    
                    elapsed = time.time() - start_time
                    if sub_pct > 0.01 and elapsed > 0.5:
                        total_est = elapsed / sub_pct
                        eta = max(total_est - elapsed, 0.0)
                    else:
                        eta = 0.0

                    if progress_callback:
                        progress_callback(ProgressUpdate(
                            stage=stage_name,
                            percent=min(overall_pct, 100.0),
                            speed=speed_str,
                            fps=fps_val,
                            eta_s=eta,
                            elapsed_s=elapsed
                        ))

        ret = proc.wait()
        if ret != 0:
            err_msg = proc.stderr.read()
            if cancel_token and cancel_token.cancelled:
                raise EncodingError('Operacja anulowana.')
            raise EncodingError(f'Blad FFmpeg (kod {ret}): {err_msg[-300:]}')

    finally:
        if cancel_token and cancel_token.current_process == proc:
            cancel_token.current_process = None
