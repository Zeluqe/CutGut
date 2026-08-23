import os
import sys
import subprocess
import time
import json
import re
import shutil
import urllib.request
import zipfile
import functools
import threading
from enum import Enum
from dataclasses import dataclass
from typing import Optional, Callable

__version__ = "202608230-2-0"

CREATE_NO_WINDOW = 0x08000000 if os.name == 'nt' else 0

class SourceCleanupPolicy(str, Enum):
    NEVER = "never"
    ASK = "ask"
    TRASH = "trash"
    DELETE_PERMANENTLY = "delete_permanently"

@dataclass
class QualityAssessment:
    rating: str          # "great", "good", "ok", "low", "very_low"
    label: str           # e.g. "Świetna", "Dobra", "OK do wysłania", "Niska jakość", "Bardzo niska"
    color: str           # "#22c55e", "#84cc16", "#eab308", "#f97316", "#ef4444"
    bppf: float
    description: str
    tip: str

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
class EncodeJob:
    job_id: str
    input_path: str
    output_path: str
    start_s: float
    end_s: float
    target_mb: float
    preset_mode: str
    status: str = "pending"  # pending, running, finished, error, cancelled
    progress_pct: float = 0.0
    result_size: int = 0
    error_message: str = ""
    cleanup_policy: str = "never"

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

@functools.lru_cache(maxsize=1)
def check_nvenc_support() -> bool:
    try:
        cmd = [get_ffmpeg_path(), '-y', '-f', 'lavfi', '-i', 'testsrc=duration=0.1:size=320x240:rate=1', '-c:v', 'h264_nvenc', '-f', 'null', '-']
        res = subprocess.run(cmd, capture_output=True, creationflags=CREATE_NO_WINDOW)
        return res.returncode == 0
    except Exception:
        return False

@functools.lru_cache(maxsize=1)
def check_amf_support() -> bool:
    try:
        cmd = [get_ffmpeg_path(), '-y', '-f', 'lavfi', '-i', 'testsrc=duration=0.1:size=320x240:rate=1', '-c:v', 'h264_amf', '-f', 'null', '-']
        res = subprocess.run(cmd, capture_output=True, creationflags=CREATE_NO_WINDOW)
        return res.returncode == 0
    except Exception:
        return False

def get_best_available_encoder() -> str:
    if check_nvenc_support():
        return 'NVENC_HQ'
    if check_amf_support():
        return 'AMF_HQ'
    return 'CPU_BALANCED'

def calculate_target_bytes(target_mb: float) -> int:
    # Dual-Limit Math: bezpieczenstwo dla limitow binarnych (MiB) i dziesietnych (MB)
    # min(0.995 * N * 1024^2, 0.98 * N * 1000^2)
    binary_limit = 0.995 * target_mb * 1024 * 1024
    decimal_limit = 0.98 * target_mb * 1000 * 1000
    return int(min(binary_limit, decimal_limit))

def can_stream_copy(video_info: VideoInfo, input_path: str, start_s: float, end_s: float, target_mb: float) -> bool:
    target_bytes = calculate_target_bytes(target_mb)
    if not input_path or not os.path.exists(input_path):
        return False
        
    total_size = os.path.getsize(input_path)
    dur_s = max(end_s - start_s, 0.1)
    
    # 1. Caly plik
    if start_s <= 0.05 and end_s >= (video_info.duration - 0.05):
        return total_size <= target_bytes
        
    # 2. Wycinek pliku (estymata wagi z marginesem 5%)
    if video_info.duration > 0 and total_size > 0:
        est_bytes = int((dur_s / max(video_info.duration, 0.1)) * total_size * 1.05)
        return est_bytes <= target_bytes
    elif video_info.bitrate > 0:
        est_bytes = int((dur_s * video_info.bitrate / 8.0) * 1.05)
        return est_bytes <= target_bytes
        
    return False

def assess_quality(
    video_kbps: int,
    width: int,
    height: int,
    fps: float,
    is_remux: bool = False,
    dur_s: float = 60.0,
    target_mb: float = 20.0,
    is_hevc: bool = False,
    lang: str = 'pl'
) -> QualityAssessment:
    if is_remux:
        if lang == 'pl':
            return QualityAssessment(
                rating="great",
                label="Błyskawiczny Remux (Oryginalna jakość)",
                color="#22c55e",
                bppf=0.15,
                description="Plik mieści się w limicie bez kompresji. Zachowano 100% jakości źródłowej.",
                tip="Jakość jest maksymalna — brak konieczności ponownego kodowania."
            )
        else:
            return QualityAssessment(
                rating="great",
                label="Instant Remux (Original Quality)",
                color="#22c55e",
                bppf=0.15,
                description="File fits target size limit directly. 100% original quality preserved.",
                tip="Quality is maximum — no re-encoding required."
            )

    eff_w = max(width, 320)
    eff_h = max(height, 240)
    eff_fps = max(fps, 1.0)
    
    # Bits per pixel frame: (kbps * 1000) / (w * h * fps)
    # HEVC jest o ~35% wydajniejszy od AVC/H.264
    bppf = (video_kbps * 1000.0) / (eff_w * eff_h * eff_fps)
    bppf_score = bppf * (1.35 if is_hevc else 1.0)
    
    if bppf_score >= 0.075:
        rating = "great"
        color = "#22c55e"  # zielony
        if lang == 'pl':
            label = "Świetna jakość"
            desc = "Obraz pozostanie bardzo ostry; doskonałe zachowanie drobnych detali i dynamicznego ruchu."
            tip = "Jakość jest świetna — nie musisz nic zmieniać."
        else:
            label = "Great Quality"
            desc = "Image will stay crisp with great retention of details and motion."
            tip = "Quality is great — no changes needed."
            
    elif bppf_score >= 0.038:
        rating = "good"
        color = "#84cc16"  # jasnozielony
        if lang == 'pl':
            label = "Dobra jakość"
            desc = "Obraz wygląda bardzo dobrze; minimalne straty mogą wystąpić jedynie przy gwałtownym ruchu."
            tip = "Jakość jest dobra i w zupełności wystarczająca do dzielenia się klipem."
        else:
            label = "Good Quality"
            desc = "Video looks clean; slight softness may only appear during fast motion."
            tip = "Quality is good and ideal for video sharing."
            
    elif bppf_score >= 0.018:
        rating = "ok"
        color = "#eab308"  # żółty
        if lang == 'pl':
            label = "OK do wysłania"
            desc = "Na telefonie/Discordzie klip będzie czytelny; szybki ruch i efekty mogą być lekko zmiękczone."
            if dur_s > 30:
                shorten_s = max(int(dur_s * 0.3), 5)
                tip = f"Skróć klip o ~{shorten_s} s lub wybierz 50 MB (Nitro), aby podnieść ostrość."
            else:
                tip = "Przełącz na 720p, aby obraz wyglądał czyściej przy tym samym limicie."
        else:
            label = "OK for Sharing"
            desc = "Looks fine on Discord/mobile; fast motion or particle effects may appear slightly soft."
            if dur_s > 30:
                shorten_s = max(int(dur_s * 0.3), 5)
                tip = f"Shorten clip by ~{shorten_s}s or choose 50 MB (Nitro) to increase sharpness."
            else:
                tip = "Switch downscale to 720p for a cleaner picture at the same size limit."
                
    elif bppf_score >= 0.009:
        rating = "low"
        color = "#f97316"  # pomarańczowy
        if lang == 'pl':
            label = "Niska jakość"
            desc = "Silna kompresja — obraz może być zauważalnie rozmyty lub pikselować w dynamicznych scenach."
            shorten_s = max(int(dur_s * 0.4), 5)
            tip = f"Zalecane: skróć klip o ~{shorten_s} s lub wybierz wyższy limit (50 MB)."
        else:
            label = "Low Quality"
            desc = "Heavy compression — image will be noticeably soft or show macroblocks in fast scenes."
            shorten_s = max(int(dur_s * 0.4), 5)
            tip = f"Recommended: shorten clip by ~{shorten_s}s or select a higher limit (50 MB)."
            
    else:
        rating = "very_low"
        color = "#ef4444"  # czerwony
        if lang == 'pl':
            label = "Bardzo niska jakość"
            desc = "Klip jest za długi dla tego limitu — obraz będzie mocno zniekształcony."
            shorten_s = max(int(dur_s * 0.6), 5)
            tip = f"Koniecznie skróć klip (o ~{shorten_s} s) albo wybierz większy limit rozmiaru."
        else:
            label = "Very Low Quality"
            desc = "Clip is too long for this limit — video will have severe visual compression artifacts."
            shorten_s = max(int(dur_s * 0.6), 5)
            tip = f"Strongly advised: shorten clip (by ~{shorten_s}s) or increase target size."
            
    return QualityAssessment(
        rating=rating,
        label=label,
        color=color,
        bppf=round(bppf, 4),
        description=desc,
        tip=tip
    )

def cleanup_source_file(input_path: str, output_path: str, policy: SourceCleanupPolicy) -> tuple[bool, str]:
    if policy == SourceCleanupPolicy.NEVER:
        return False, "Oryginał zachowany (polityka: Nigdy nie usuwaj)."

    if not input_path or not output_path:
        return False, "Brak podanej ścieżki pliku."

    if os.path.abspath(input_path).lower() == os.path.abspath(output_path).lower():
        return False, "Ścieżka źródłowa jest identyczna ze ścieżką wyjściową — operacja zablokowana."
        
    if not os.path.exists(input_path):
        return False, "Plik źródłowy nie istnieje."
        
    if not os.path.exists(output_path):
        return False, "Plik wyjściowy nie istnieje — anulowano usuwanie źródła."

    if policy == SourceCleanupPolicy.TRASH:
        try:
            if os.name == 'nt':
                # Bezpieczne przeniesienie do systemowego Kosza w Windows
                ps_cmd = [
                    'powershell', '-NoProfile', '-NonInteractive', '-Command',
                    f"[Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile('{input_path}', 'OnlyErrorDialogs', 'SendToRecycleBin')"
                ]
                res = subprocess.run(ps_cmd, capture_output=True, creationflags=CREATE_NO_WINDOW)
                if res.returncode == 0 and not os.path.exists(input_path):
                    return True, "Plik źródłowy został bezpiecznie przeniesiony do Kosza."
            # Fallback dla send2trash
            try:
                import send2trash
                send2trash.send2trash(input_path)
                return True, "Plik źródłowy został bezpiecznie przeniesiony do Kosza."
            except Exception:
                pass
            return False, "Nie udało się przenieść do Kosza."
        except Exception as e:
            return False, f"Nie udało się przenieść do Kosza: {e}"

    elif policy == SourceCleanupPolicy.DELETE_PERMANENTLY:
        try:
            os.remove(input_path)
            return True, "Plik źródłowy został trwale usunięty."
        except Exception as e:
            return False, f"Błąd usuwania pliku: {e}"

    return False, "Oczekiwanie na potwierdzenie użytkownika."

def calculate_plan(
    video_info: VideoInfo,
    start_s: float,
    end_s: float,
    target_mb: float,
    is_hevc: bool = False,
    input_path: str = '',
    lang: str = 'pl'
):
    dur_s = max(end_s - start_s, 0.1)
    
    # 1. Oblicz maksymalny dozwolony rozmiar oraz bezpieczny cel bajtowy
    target_bytes = calculate_target_bytes(target_mb)
    
    # 2. Sprawdz czy mozliwy jest bezstratny i natychmiastowy Remux (Stream Copy)
    is_remux = can_stream_copy(video_info, input_path, start_s, end_s, target_mb) if input_path else False
    
    # 3. Narzut audio i kontenera
    audio_bps = 96000 if is_hevc else 128000
    overhead_bytes = 40000 + int(dur_s * 600)
    
    net_video_bytes = max(target_bytes - int(audio_bps * dur_s / 8) - overhead_bytes, 1000)
    video_kbps = max(int((net_video_bytes * 8 / dur_s) / 1000), 50)
    
    # 4. Inteligentne skalowanie i klatkaz
    filters = []
    out_height = video_info.height
    out_width = video_info.width
    out_fps = video_info.fps
    
    if video_kbps < 450:
        if video_info.height > 480:
            filters.append('scale=-2:480')
            out_height = 480
            out_width = int(video_info.width * 480 / video_info.height) if video_info.height > 0 else 854
            out_width = out_width if out_width % 2 == 0 else out_width + 1
        if video_info.fps > 30:
            filters.append('fps=30')
            out_fps = 30.0
    elif video_kbps < 900:
        if video_info.height > 720:
            filters.append('scale=-2:720')
            out_height = 720
            out_width = int(video_info.width * 720 / video_info.height) if video_info.height > 0 else 1280
            out_width = out_width if out_width % 2 == 0 else out_width + 1
        if video_info.fps > 45:
            filters.append('fps=30')
            out_fps = 30.0
            
    filter_str = ','.join(filters) if filters else None
    
    # 5. Ocena jakości bitrate (bppf)
    quality = assess_quality(
        video_kbps=video_kbps,
        width=out_width,
        height=out_height,
        fps=out_fps,
        is_remux=is_remux,
        dur_s=dur_s,
        target_mb=target_mb,
        is_hevc=is_hevc,
        lang=lang
    )
    
    return {
        'duration_s': dur_s,
        'target_bytes': target_bytes,
        'video_kbps': video_kbps,
        'audio_kbps': int(audio_bps / 1000),
        'filter_str': filter_str,
        'is_remux': is_remux,
        'out_width': out_width,
        'out_height': out_height,
        'out_fps': out_fps,
        'quality': quality
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
    
    # Auto-fallback sprzetowy (NVENC -> AMF -> CPU)
    if preset_mode in ('NVENC_HQ', 'NVENC_FAST') and not check_nvenc_support():
        preset_mode = 'AMF_HQ' if check_amf_support() else 'CPU_BALANCED'
    elif preset_mode in ('AMF_HQ', 'AMF_FAST') and not check_amf_support():
        preset_mode = 'NVENC_HQ' if check_nvenc_support() else 'CPU_BALANCED'

    plan = calculate_plan(video_info, start_s, end_s, target_mb, is_hevc, input_path)
    dur_s = plan['duration_s']
    v_kbps = plan['video_kbps']
    a_kbps = plan['audio_kbps']
    filter_str = plan['filter_str']
    max_allowed = plan['target_bytes']
    ffmpeg_bin = get_ffmpeg_path()
    
    # 0. Szybka sciezka Remux (Stream Copy), jesli sie miesci
    if plan['is_remux']:
        try:
            if progress_callback:
                progress_callback(ProgressUpdate(
                    stage='Błyskawiczny Remux (Kopiowanie)',
                    percent=30.0,
                    speed='Direct',
                    fps=0.0,
                    eta_s=0.5,
                    elapsed_s=0.1
                ))
            
            cmd_remux = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-ss', str(start_s),
                '-to', str(end_s),
                '-i', input_path,
                '-c', 'copy',
                '-movflags', '+faststart',
                output_path
            ]
            res = subprocess.run(cmd_remux, capture_output=True, creationflags=CREATE_NO_WINDOW)
            
            if res.returncode == 0 and os.path.exists(output_path):
                remux_size = os.path.getsize(output_path)
                if 1000 < remux_size <= max_allowed:
                    if progress_callback:
                        progress_callback(ProgressUpdate(
                            stage='Ukończono',
                            percent=100.0,
                            speed='Direct',
                            fps=0.0,
                            eta_s=0.0,
                            elapsed_s=0.2
                        ))
                    return output_path
            # Jesli remux przekroczyl limit lub zawiodl - usun niepelny plik i przejdz do kompresji
            if os.path.exists(output_path):
                try: os.remove(output_path)
                except Exception: pass
        except Exception:
            if os.path.exists(output_path):
                try: os.remove(output_path)
                except Exception: pass

    unique_id = int(time.time() * 1000)
    base_dir = get_base_dir()
    stats_file = os.path.join(base_dir, f'ffmpeg2pass_{unique_id}')
    stats_file_hevc = stats_file.replace('\\', '/')

    # Klatkowa dokladnosc: -i przed -ss / -to
    common_args = ['-i', input_path, '-ss', str(start_s), '-to', str(end_s)]
    vf_args = ['-vf', filter_str] if filter_str else []
    
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

        elif preset_mode in ('AMF_HQ', 'AMF_FAST'):
            quality_mode = 'quality' if preset_mode == 'AMF_HQ' else 'speed'
            cmd = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', 'h264_amf',
                '-usage', 'transcoding',
                '-quality', quality_mode,
                '-rc', 'vbr_peak',
                '-b:v', f'{v_kbps}k',
                '-maxrate', f'{int(v_kbps * 1.25)}k',
                '-bufsize', f'{int(v_kbps * 2.0)}k',
                '-pix_fmt', 'yuv420p',
                '-c:a', 'aac',
                '-b:a', f'{a_kbps}k',
                '-movflags', '+faststart',
                output_path
            ]
            _run_ffmpeg_with_progress(cmd, dur_s, 'Kompresja GPU (AMD AMF)', progress_callback, cancel_token)

        elif preset_mode == 'CPU_HEVC':
            # Dedykowany 2-pass x265 przez -x265-params
            # Pass 1
            cmd_pass1 = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', 'libx265',
                '-b:v', f'{v_kbps}k',
                '-preset', 'slow',
                '-x265-params', f'pass=1:stats={stats_file_hevc}',
                '-an',
                '-f', 'null',
                '-'
            ]
            _run_ffmpeg_with_progress(cmd_pass1, dur_s, 'Analiza HEVC (Przebieg 1/2)', progress_callback, cancel_token, stage_weight=0.35, stage_offset=0.0)

            if cancel_token and cancel_token.cancelled:
                raise EncodingError('Anulowano przez uzytkownika.')

            # Pass 2
            cmd_pass2 = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', 'libx265',
                '-b:v', f'{v_kbps}k',
                '-preset', 'slow',
                '-x265-params', f'pass=2:stats={stats_file_hevc}',
                '-pix_fmt', 'yuv420p',
                '-c:a', 'aac',
                '-b:a', f'{a_kbps}k',
                '-movflags', '+faststart',
                output_path
            ]
            _run_ffmpeg_with_progress(cmd_pass2, dur_s, 'Finalna kompresja HEVC (Przebieg 2/2)', progress_callback, cancel_token, stage_weight=0.65, stage_offset=35.0)

        else:
            # CPU H.264 2-pass
            cpu_preset = 'veryfast' if preset_mode == 'CPU_FAST' else 'slow'

            # Pass 1
            cmd_pass1 = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', 'libx264',
                '-b:v', f'{v_kbps}k',
                '-preset', cpu_preset,
                '-pass', '1',
                '-passlogfile', stats_file,
                '-an',
                '-f', 'null',
                '-'
            ]
            _run_ffmpeg_with_progress(cmd_pass1, dur_s, 'Analiza H.264 (Przebieg 1/2)', progress_callback, cancel_token, stage_weight=0.35, stage_offset=0.0)

            if cancel_token and cancel_token.cancelled:
                raise EncodingError('Anulowano przez uzytkownika.')

            # Pass 2
            cmd_pass2 = [
                ffmpeg_bin, '-y', '-hide_banner',
                '-progress', 'pipe:1'
            ] + common_args + vf_args + [
                '-c:v', 'libx264',
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
            _run_ffmpeg_with_progress(cmd_pass2, dur_s, 'Finalna kompresja H.264 (Przebieg 2/2)', progress_callback, cancel_token, stage_weight=0.65, stage_offset=35.0)

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

    stderr_lines = []
    def drain_stderr():
        try:
            for l in proc.stderr:
                if len(stderr_lines) < 100:
                    stderr_lines.append(l)
                else:
                    stderr_lines.pop(0)
                    stderr_lines.append(l)
        except Exception:
            pass

    t_err = threading.Thread(target=drain_stderr, daemon=True)
    t_err.start()

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
        t_err.join(timeout=0.5)

        if ret != 0:
            err_msg = "".join(stderr_lines)
            if cancel_token and cancel_token.cancelled:
                raise EncodingError('Operacja anulowana.')
            raise EncodingError(f'Blad FFmpeg (kod {ret}): {err_msg[-300:]}')

    finally:
        if cancel_token and cancel_token.current_process == proc:
            cancel_token.current_process = None
