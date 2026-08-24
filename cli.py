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

__version__ = "202608240-6-7"

def parse_target_mb(val: str) -> float:
    v = val.lower().replace('mb', '').replace('m', '').strip()
    try:
        return float(v)
    except ValueError:
        return 20.0

def parse_crop_arg(crop_str: str, info: encoder.VideoInfo, align: str = "center") -> Optional[encoder.CropBox]:
    if not crop_str or crop_str.lower() in ('original', 'none', 'auto'):
        return encoder.CropBox(0, 0, info.width, info.height, "original")
        
    c = crop_str.lower().strip()
    if c in ('9:16', 'shorts', 'tiktok', 'reels'):
        return encoder.calculate_default_crop(info.width, info.height, "9:16", align)
    elif c in ('1:1', 'square', 'kwadrat'):
        return encoder.calculate_default_crop(info.width, info.height, "1:1", align)
    elif c in ('16:9', 'landscape', 'poziom'):
        return encoder.calculate_default_crop(info.width, info.height, "16:9", align)
    elif ':' in c:
        parts = [int(p) for p in c.split(':')]
        if len(parts) == 4:
            return encoder.CropBox(x=parts[2], y=parts[3], w=parts[0], h=parts[1], ratio_type="custom")
            
    return encoder.CropBox(0, 0, info.width, info.height, "original")

def main():
    parser = argparse.ArgumentParser(
        description=f'CutGut CLI v{__version__} - Format & Social Export | Smart video trimming & compression tool.'
    )
    parser.add_argument('-v', '--version', action='version', version=f'%(prog)s {__version__}')
    parser.add_argument('inputs', nargs='+', help='Sciezka lub sciezki do plikow wideo')
    parser.add_argument('-s', '--start', type=float, default=0.0, help='Czas poczatku w sekundach (domyslnie 0.0)')
    parser.add_argument('-e', '--end', type=float, default=None, help='Czas konca w sekundach (domyslnie calosc)')
    parser.add_argument('-t', '--target', default='20mb', help='Docelowy limit rozmiaru: 20mb (domyslnie), 10mb, 50mb, 500mb lub liczba MB')
    parser.add_argument(
        '--preset',
        choices=['discord', 'nitro', 'shorts_hq', 'shorts_20mb', 'tiktok', 'square'],
        default=None,
        help='Szybki profil spolecznosciowy (ustawia kadr i limit MB): discord (16:9/20MB), nitro (16:9/50MB), shorts_hq (9:16/50MB), shorts_20mb (9:16/20MB), square (1:1/20MB)'
    )
    parser.add_argument(
        '--crop',
        default=None,
        help='Kadrowanie wideo: original, 9:16 (Shorts), 1:1 (Kwadrat), 16:9 lub w:h:x:y'
    )
    parser.add_argument(
        '--crop-align',
        choices=['center', 'left', 'right'],
        default='center',
        help='Pozycja kadru: center (domyslnie), left, right'
    )
    parser.add_argument(
        '--screenshot-at',
        type=float,
        default=None,
        help='Zapisz stop-klatke w podanym momencie (w sekundach) jako bezstratny plik PNG'
    )
    parser.add_argument(
        '--encoder',
        choices=['nvenc', 'nvenc_fast', 'amf', 'amf_fast', 'cpu', 'cpu_fast', 'hevc'],
        default=None,
        help='Wybierz enkoder: nvenc (NVIDIA GPU HQ), nvenc_fast, amf (AMD GPU HQ), amf_fast, cpu (libx264 balanced), cpu_fast, hevc (libx265)'
    )
    parser.add_argument('-o', '--output', default=None, help='Sciezka do pliku wyjsciowego (dla pojedynczego pliku)')
    parser.add_argument('--output-dir', default=None, help='Docelowy folder dla wygenerowanych plikow wideo / PNG')
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
    crop_arg = args.crop

    # Obsługa gotowych presetów społecznościowych
    if args.preset == 'discord':
        target_mb = 20.0
        crop_arg = crop_arg or '16:9'
    elif args.preset == 'nitro':
        target_mb = 50.0
        crop_arg = crop_arg or '16:9'
    elif args.preset == 'shorts_hq':
        target_mb = 50.0
        crop_arg = crop_arg or '9:16'
    elif args.preset == 'shorts_20mb':
        target_mb = 20.0
        crop_arg = crop_arg or '9:16'
    elif args.preset == 'tiktok':
        target_mb = 50.0
        crop_arg = crop_arg or '9:16'
    elif args.preset == 'square':
        target_mb = 20.0
        crop_arg = crop_arg or '1:1'

    out_dir = args.output_dir if args.output_dir else encoder.get_default_output_dir()
    os.makedirs(out_dir, exist_ok=True)

    # 1. Obsługa screenshotów PNG (--screenshot-at)
    if args.screenshot_at is not None:
        for inp in args.inputs:
            if not os.path.exists(inp): continue
            info = encoder.probe_video(inp)
            crop_box = parse_crop_arg(crop_arg, info, args.crop_align)
            base_name = os.path.splitext(os.path.basename(inp))[0]
            png_path = os.path.join(out_dir, f"CutGut_{base_name}_{int(args.screenshot_at*1000)}ms.png")
            print(f"Zapisywanie klatki PNG z {os.path.basename(inp)} w punkcie {args.screenshot_at:.2f}s...")
            saved = encoder.extract_frame_png(inp, args.screenshot_at, png_path, crop_box)
            print(f"  -> Zapisano klatke: {saved}\\n")
        return

    # 2. Obsługa generowania samej próbki jakości (--preview-at)
    if args.preview_at is not None:
        for inp in args.inputs:
            if not os.path.exists(inp): continue
            info = encoder.probe_video(inp)
            crop_box = parse_crop_arg(crop_arg, info, args.crop_align)
            plan = encoder.calculate_plan(info, max(args.start, 0.0), args.end or info.duration, target_mb, (preset_mode == 'CPU_HEVC'), inp, crop_box=crop_box)
            print(f"Tworzenie probki jakosci dla {os.path.basename(inp)} w punkcie {args.preview_at:.2f}s...")
            sample = encoder.create_quality_preview(inp, args.preview_at, plan, preset_mode)
            print(f"  -> Gotowa probka: {sample}\\n")
        return

    # 3. Budowanie kolejki EncodeJob
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

        crop_box = parse_crop_arg(crop_arg, info, args.crop_align)
        crop_tag = f"_{crop_box.ratio_type.replace(':', '_')}" if (crop_box and crop_box.ratio_type != "original") else ""

        if args.output and len(args.inputs) == 1:
            out_file = args.output
        else:
            base_name = os.path.splitext(os.path.basename(inp))[0]
            out_file = encoder.generate_output_filepath(output_dir=out_dir, base_name=f"{base_name}{crop_tag}")

        job = encoder.EncodeJob(
            job_id=str(idx + 1),
            input_path=inp,
            output_path=out_file,
            start_s=start_s,
            end_s=end_s,
            target_mb=target_mb,
            preset_mode=preset_mode,
            cleanup_policy=args.cleanup,
            crop_box=crop_box
        )
        jobs.append(job)

    if not jobs:
        print('Brak prawidlowych zadan do przetworzenia.', file=sys.stderr)
        sys.exit(1)

    print(f'=== CutGut CLI v{__version__} | Kolejka zadan: {len(jobs)} ===')
    print(f'Limit: {target_mb:.1f} MB | Enkoder: {preset_mode} | Czyszczenie: {args.cleanup}\\n')

    if args.explain_plan:
        print('--- Analiza jakosci i planu eksportu ---')
        for j in jobs:
            info = encoder.probe_video(j.input_path)
            plan = encoder.calculate_plan(info, j.start_s, j.end_s, j.target_mb, (j.preset_mode == 'CPU_HEVC'), j.input_path, crop_box=j.crop_box)
            q = plan['quality']
            crop_str = f" [Kadr: {j.crop_box.ratio_type}]" if (j.crop_box and j.crop_box.ratio_type != "original") else ""
            print(f"Plik: {os.path.basename(j.input_path)}{crop_str}")
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
        crop_desc = f" [Kadr: {job.crop_box.ratio_type}]" if (job.crop_box and job.crop_box.ratio_type != "original") else ""
        print(f'[{idx}/{len(jobs)}] Przetwarzanie: {os.path.basename(job.input_path)}{crop_desc} ({job.start_s:.2f}s - {job.end_s:.2f}s)')
        try:
            res = encoder.encode_video(
                input_path=job.input_path,
                output_path=job.output_path,
                start_s=job.start_s,
                end_s=job.end_s,
                target_mb=job.target_mb,
                preset_mode=job.preset_mode,
                progress_callback=on_progress,
                crop_box=job.crop_box
            )
            print('\n')
            final_size = os.path.getsize(res)
            job.status = 'finished'
            job.result_size = final_size
            print(f'  -> Gotowe: {res} ({final_size / 1000000:.2f} MB / {final_size / (1024*1024):.2f} MiB)')

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
