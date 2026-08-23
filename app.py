import os

# Konfiguracja folderów
BASE_DIR = os.path.abspath(".")
TEMP_DIR = os.path.join(BASE_DIR, "temp")
OUTPUT_DIR = os.path.join(BASE_DIR, "outputs")

for d in [TEMP_DIR, OUTPUT_DIR]:
    if not os.path.exists(d):
        os.makedirs(d)

os.environ["GRADIO_TEMP_DIR"] = TEMP_DIR

import subprocess
import gradio as gr
import time
import shutil

def get_video_duration(file_path):
    try:
        cmd = [
            'ffprobe', '-v', 'error', '-show_entries', 'format=duration',
            '-of', 'default=noprint_wrappers=1:nokey=1', file_path
        ]
        result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        return float(result.stdout.strip())
    except:
        return 0

def handle_upload(file_obj):
    if not file_obj:
        return None, 0, 100, "Czekam na plik..."
    
    path = file_obj.name if hasattr(file_obj, 'name') else file_obj
    preview_path = os.path.join(OUTPUT_DIR, "preview.mp4")
    
    try:
        shutil.copy(path, preview_path)
    except:
        preview_path = path
        
    duration = get_video_duration(preview_path)
    return preview_path, 0, duration, f"Wgrano wideo: {duration:.2f} sekundy"

def compress_video(input_file, start_t, end_t, target_size_limit="Do 10 MB"):
    if not input_file:
        return None
    
    path = input_file.name if hasattr(input_file, 'name') else input_file
    clean_input = os.path.join(TEMP_DIR, "work_input.mp4")
    shutil.copy(path, clean_input)
    
    duration = end_t - start_t
    if duration <= 0:
        return None
    
    # Celujemy w ~9.95 MB lub ~19.95 MB (w Eksploratorze Windows)
    if "20" in str(target_size_limit):
        target_size_bytes = 19.95 * 1024 * 1024
    else:
        target_size_bytes = 9.95 * 1024 * 1024

    audio_bitrate_bps = 128000
    audio_bytes = (audio_bitrate_bps * duration) / 8
    
    # Narzut kontenera MP4 i nagłówków strumienia (~30 KB + 500 B/sek)
    overhead_bytes = 30000 + (duration * 500)
    video_bytes = max(target_size_bytes - audio_bytes - overhead_bytes, 1000)
    
    video_bitrate_bps = (video_bytes * 8) / duration
    video_bitrate_kbps = max(int(video_bitrate_bps / 1000), 50)
    
    unique_id = int(time.time())
    output_path = os.path.join(OUTPUT_DIR, f"cutgut_{unique_id}.mp4")
    pass_log_file = os.path.join(TEMP_DIR, f"ffmpeg2pass-{unique_id}")
    
    dev_null = 'NUL' if os.name == 'nt' else '/dev/null'
    common_args = ['-ss', str(start_t), '-t', str(duration), '-i', clean_input]
    
    pass1_cmd = ['ffmpeg', '-y'] + common_args + [
        '-c:v', 'libx264', '-b:v', f'{video_bitrate_kbps}k',
        '-pass', '1', '-passlogfile', pass_log_file,
        '-an', '-f', 'mp4', dev_null
    ]
    
    pass2_cmd = ['ffmpeg', '-y'] + common_args + [
        '-c:v', 'libx264', '-b:v', f'{video_bitrate_kbps}k',
        '-pass', '2', '-passlogfile', pass_log_file,
        '-c:a', 'aac', '-b:a', '128k',
        output_path
    ]
    
    try:
        subprocess.run(pass1_cmd, check=True, capture_output=True)
        subprocess.run(pass2_cmd, check=True, capture_output=True)
        return output_path
    except:
        return None
    finally:
        for f in os.listdir(TEMP_DIR):
            if "ffmpeg2pass" in f:
                try: os.remove(os.path.join(TEMP_DIR, f))
                except: pass

with gr.Blocks(title="CutGut v2", css="#status_info { font-weight: bold; color: orange; }") as demo:
    gr.Markdown("# ✂️ CutGut - Panel Edytorski")
    
    with gr.Row():
        with gr.Column(scale=1):
            uploader = gr.File(label="1. Wgraj wideo", file_types=[".mp4"])
            
            with gr.Group():
                gr.Markdown("### 2. Wybierz zakres i limit")
                status_info = gr.Markdown("Czekam na plik...", elem_id="status_info")
                
                with gr.Row():
                    start_input = gr.Number(value=0, label="Początek (sek)")
                    end_input = gr.Number(value=10, label="Koniec (sek)")
                
                size_selector = gr.Radio(
                    choices=["Do 10 MB", "Do 20 MB"], 
                    value="Do 10 MB", 
                    label="Limit rozmiaru pliku"
                )
                
                gr.Markdown("*Wskazówka: Obejrzyj wideo obok i wpisz czas powyżej.*")
            
            compress_btn = gr.Button("🚀 3. Przytnij i Kompresuj", variant="primary")
            
        with gr.Column(scale=2):
            preview_video = gr.Video(label="Podgląd wideo (Sprawdź tutaj czas startu i końca)")
            output_video = gr.Video(label="Wynik końcowy")
    
    uploader.change(
        fn=handle_upload, 
        inputs=[uploader], 
        outputs=[preview_video, start_input, end_input, status_info]
    )
    
    compress_btn.click(
        fn=compress_video, 
        inputs=[uploader, start_input, end_input, size_selector], 
        outputs=[output_video]
    )

if __name__ == "__main__":
    demo.launch(
        theme=gr.themes.Soft(primary_hue="orange"),
        allowed_paths=[BASE_DIR]
    )
