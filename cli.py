import argparse
import sys
import os
import time

if hasattr(sys.stdout, 'reconfigure'):
    try: sys.stdout.reconfigure(encoding='utf-8')
    except Exception: pass
if hasattr(sys.stderr, 'reconfigure'):
    try: sys.stderr.reconfigure(encoding='utf-8')
    except Exception: pass

import encoder

def parse_target_mb(val: str) -> float:
    v = val.lower().replace('mb', '').replace('m', '').strip()
    try:
        return float(v)
    except ValueError:
        return 20.0

def main():
    parser = argparse.ArgumentParser(
        description='CutGut CLI - Smart video trimming and 2-pass compression tool (Discord 20MB/10MB/50MB).'
    )
    parser.add_argument('input', help='Sciezka do pliku wideo')
    parser.add_argument('-s', '--start', type=float, default=0.0, help='Czas poczatku w sekundach (domyslnie 0.0)')
    parser.add_argument('-e', '--end', type=float, default=None, help='Czas konca w sekundach (domyslnie calosc)')
    parser.add_argument('-t', '--target', default='20mb', help='Docelowy limit rozmiaru: 20mb (domyslnie), 10mb, 50mb, 500mb lub liczba MB')
    parser.add_argument(
        '--encoder',
        choices=['nvenc', 'nvenc_fast', 'amf', 'amf_fast', 'cpu', 'cpu_fast', 'hevc'],
        default=None,
        help='Wybierz enkoder: nvenc (NVIDIA GPU HQ), nvenc_fast, amf (AMD GPU HQ), amf_fast, cpu (libx264 balanced), cpu_fast, hevc (libx265)'
    )
    parser.add_argument('-o', '--output', default=None, help='Sciezka do pliku wyjsciowego')

    args = parser.parse_args()

    if not os.path.exists(args.input):
        print(f'Blad: Plik nie istnieje: {args.input}', file=sys.stderr)
        sys.exit(1)

    print(f'Analiza pliku: {args.input}...')
    try:
        info = encoder.probe_video(args.input)
    except Exception as e:
        print(f'Blad odczytu wideo: {e}', file=sys.stderr)
        sys.exit(1)

    start_s = max(args.start, 0.0)
    end_s = args.end if args.end is not None else info.duration
    
    if end_s <= start_s:
        print(f'Blad: Czas konca ({end_s}s) musi byc wiekszy niz poczatek ({start_s}s)!', file=sys.stderr)
        sys.exit(1)

    target_mb = parse_target_mb(args.target)
    
    # Mapowanie wyboru enkodera
    enc_map = {
        'nvenc': 'NVENC_HQ',
        'nvenc_fast': 'NVENC_FAST',
        'amf': 'AMF_HQ',
        'amf_fast': 'AMF_FAST',
        'cpu': 'CPU_BALANCED',
        'cpu_fast': 'CPU_FAST',
        'hevc': 'CPU_HEVC'
    }
    
    if args.encoder:
        preset_mode = enc_map.get(args.encoder, 'CPU_BALANCED')
    else:
        preset_mode = encoder.get_best_available_encoder()

    if not args.output:
        out_dir = os.path.join(encoder.get_base_dir(), 'outputs')
        os.makedirs(out_dir, exist_ok=True)
        args.output = os.path.join(out_dir, f'CutGut_{int(time.time())}.mp4')

    print(f'Parametry: Zakres: {start_s:.2f}s - {end_s:.2f}s (Dlugosc: {end_s-start_s:.2f}s)')
    print(f'Limit: {target_mb:.1f} MB | Enkoder: {preset_mode}')
    print(f'Zapis do: {args.output}')

    def on_progress(p: encoder.ProgressUpdate):
        bar_len = 30
        filled = int(bar_len * p.percent / 100.0)
        bar = '=' * filled + '-' * (bar_len - filled)
        sys.stdout.write(f'\r[{p.stage}] [{bar}] {p.percent:5.1f}% | Speed: {p.speed:>5} | ETA: {p.eta_s:4.1f}s ')
        sys.stdout.flush()

    try:
        res = encoder.encode_video(
            input_path=args.input,
            output_path=args.output,
            start_s=start_s,
            end_s=end_s,
            target_mb=target_mb,
            preset_mode=preset_mode,
            progress_callback=on_progress
        )
        print('\n')
        final_size = os.path.getsize(res)
        print(f'Sukces! Gotowy plik: {res}')
        print(f'Rozmiar: {final_size} bajtow ({final_size / (1024*1024):.2f} MiB / {final_size / 1000000:.2f} MB)')
    except Exception as e:
        print(f'\nBlad kompresji: {e}', file=sys.stderr)
        sys.exit(1)

if __name__ == '__main__':
    main()
