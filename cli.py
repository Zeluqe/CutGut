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

__version__ = "202608230-3-0"

def parse_target_mb(val: str) -> float:
    v = val.lower().replace('mb', '').replace('m', '').strip()
    try:
        return float(v)
    except ValueError:
        return 20.0

def main():
    parser = argparse.ArgumentParser(
        description=f'CutGut CLI v{__version__} - Smart video trimming and 2-pass compression tool (Discord 20MB/10MB/50MB).'
    )
    parser.add_argument('-v', '--version', action='version', version=f'%(prog)s {__version__}')
    parser.add_argument('inputs', nargs='+', help='Sciezka lub sciezki do plikow wideo')
    parser.add_argument('-s', '--start', type=float, default=0.0, help='Czas poczatku w sekundach (domyslnie 0.0)')
    parser.add_argument('-e', '--end', type=float, default=None, help='Czas konca w sekundach (domyslnie calosc)')
    parser.add_argument('-t', '--target', default='20mb', help='Docelowy limit rozmiaru: 20mb (domyslnie), 10mb, 50mb, 500mb lub liczba MB')
    parser.add_argument(
        '--encoder',
        choices=['nvenc', 'nvenc_fast', 'amf', 'amf_fast', 'cpu', 'cpu_fast', 'hevc'],
        default=None,
        help='Wybierz enkoder: nvenc (NVIDIA GPU HQ), nvenc_fast, amf (AMD GPU HQ), amf_fast, cpu (libx264 balanced), cpu_fast, hevc (libx265)'
    )
    parser.add_argument('-o', '--output', default=None, help='Sciezka do pliku wyjsciowego (dla pojedynczego pliku)')
    parser.add_argument('--output-dir', default=None, help='Docelowy folder dla wygenerowanych plikow wideo')
    parser.add_argument(
        '--cleanup',
        choices=['never', 'ask', 'trash', 'delete'],
        default='never',
        help='Polityka kasowania pliku zrodlowego: never (domyslnie), ask, trash (do Kosza), delete (trwale)'
    )
    parser.add_argument(
        '--explain-plan',
        action='store_true',
        help='Wyswietl szczegolowa analize jakosci planu (bppf, ocena, porady) przed kodowaniem'
    )
    parser.add_argument(
        '--preview-at',
        type=float,
        default=None,
        help='Wygeneruj 6-sekundowa probke jakosci wokol podanego czasu (w sekundach) bez pelnego kodowania'
    )

    args = parser.parse_args()

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

    target_mb = parse_target_mb(args.target)
    out_dir = args.output_dir if args.output_dir else encoder.get_default_output_dir()
    os.makedirs(out_dir, exist_ok=True)

    # Obsługa generowania samej próbki jakości (--preview-at)
    if args.preview_at is not None:
        for inp in args.inputs:
            if not os.path.exists(inp): continue
            info = encoder.probe_video(inp)
            plan = encoder.calculate_plan(info, max(args.start, 0.0), args.end or info.duration, target_mb, (preset_mode == 'CPU_HEVC'), inp)
            print(f"Tworzenie probki jakosci dla {os.path.basename(inp)} w punkcie {args.preview_at:.2f}s...")
            sample = encoder.create_quality_preview(inp, args.preview_at, plan, preset_mode)
            print(f"  -> Gotowa probka: {sample}\n")
        return

    # Budowanie kolejki EncodeJob
    jobs: list[encoder.EncodeJob] = []
    for idx, inp in enumerate(args.inputs):
        if not os.path.exists(inp):
            print(f'Pominieto (brak pliku): {inp}', file=sys.stderr)
            continue
            
        try:
            info = encoder.probe_video(inp)
        except Exception as e:
            print(f'Blad odczytu {inp}: {e}', file=sys.stderr)
            continue

        start_s = max(args.start, 0.0)
        end_s = args.end if args.end is not None else info.duration
        
        if end_s <= start_s:
            print(f'Pominieto {inp}: czas konca ({end_s}s) <= poczatek ({start_s}s)', file=sys.stderr)
            continue

        if args.output and len(args.inputs) == 1:
            out_file = args.output
        else:
            base_name = os.path.splitext(os.path.basename(inp))[0]
            out_file = encoder.generate_output_filepath(output_dir=out_dir, base_name=base_name)

        job = encoder.EncodeJob(
            job_id=str(idx + 1),
            input_path=inp,
            output_path=out_file,
            start_s=start_s,
            end_s=end_s,
            target_mb=target_mb,
            preset_mode=preset_mode,
            cleanup_policy=args.cleanup
        )
        jobs.append(job)

    if not jobs:
        print('Brak prawidlowych zadan do przetworzenia.', file=sys.stderr)
        sys.exit(1)

    print(f'=== CutGut CLI v{__version__} | Kolejka zadan: {len(jobs)} ===')
    print(f'Limit: {target_mb:.1f} MB | Enkoder: {preset_mode} | Czyszczenie: {args.cleanup}\n')

    if args.explain_plan:
        print('--- Analiza jakosci i planu eksportu ---')
        for j in jobs:
            info = encoder.probe_video(j.input_path)
            plan = encoder.calculate_plan(info, j.start_s, j.end_s, j.target_mb, (j.preset_mode == 'CPU_HEVC'), j.input_path)
            q = plan['quality']
            print(f"Plik: {os.path.basename(j.input_path)}")
            print(f"  Zakres: {j.start_s:.2f}s - {j.end_s:.2f}s ({plan['duration_s']:.2f}s)")
            print(f"  Wyjscie: {plan['out_width']}x{plan['out_height']} @ {plan['out_fps']:.0f} FPS | Bitrate: ~{plan['video_kbps']} kbps (bppf: {q.bppf})")
            print(f"  Ocena jakosci: [{q.label}] - {q.description}")
            if q.tip:
                print(f"  Wskazowka: {q.tip}")
            print()

    def on_progress(p: encoder.ProgressUpdate):
        bar_len = 25
        filled = int(bar_len * p.percent / 100.0)
        bar = '=' * filled + '-' * (bar_len - filled)
        sys.stdout.write(f'\r  [{p.stage[:20]}] [{bar}] {p.percent:5.1f}% | {p.speed:>5} | ETA: {p.eta_s:4.1f}s ')
        sys.stdout.flush()

    for idx, job in enumerate(jobs, 1):
        print(f'[{idx}/{len(jobs)}] Przetwarzanie: {os.path.basename(job.input_path)} ({job.start_s:.2f}s - {job.end_s:.2f}s)')
        try:
            res = encoder.encode_video(
                input_path=job.input_path,
                output_path=job.output_path,
                start_s=job.start_s,
                end_s=job.end_s,
                target_mb=job.target_mb,
                preset_mode=job.preset_mode,
                progress_callback=on_progress
            )
            print('\n')
            final_size = os.path.getsize(res)
            job.status = 'finished'
            job.result_size = final_size
            print(f'  -> Gotowe: {res} ({final_size / 1000000:.2f} MB / {final_size / (1024*1024):.2f} MiB)')

            # Obsługa polityki czyszczenia
            if job.cleanup_policy != 'never' and os.path.exists(res) and final_size > 0:
                if job.cleanup_policy == 'ask':
                    if sys.stdin.isatty():
                        ans = input(f'  -> Czy usunac plik zrodlowy {os.path.basename(job.input_path)} do Kosza? [t/N]: ')
                        if ans.lower() in ('t', 'y', 'tak', 'yes'):
                            ok, msg = encoder.cleanup_source_file(job.input_path, res, encoder.SourceCleanupPolicy.TRASH)
                            print(f'  -> {msg}')
                else:
                    pol = encoder.SourceCleanupPolicy(job.cleanup_policy)
                    ok, msg = encoder.cleanup_source_file(job.input_path, res, pol)
                    print(f'  -> {msg}')
            print()
        except Exception as e:
            print(f'\n  -> Blad: {e}\n', file=sys.stderr)
            job.status = 'error'
            job.error_message = str(e)

    print('=== Podsumowanie kolejki ===')
    success_count = sum(1 for j in jobs if j.status == 'finished')
    print(f'Ukonczono pomyslnie: {success_count}/{len(jobs)}')

if __name__ == '__main__':
    main()
