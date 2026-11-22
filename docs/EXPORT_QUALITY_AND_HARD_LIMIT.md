# Export quality and the hard size limit

This is the technical companion to the README. It explains how CutGut turns "make this clip
fit in 20 MB" into a bitrate, how the encoder is chosen, and what happens when the first
attempt is still too large. The short version: the limit is a byte count, not an estimate, and
an export that misses it is retried instead of being reported as successful.

## Codecs

| Codec | Best for | NVIDIA | AMD | CPU fallback |
| --- | --- | --- | --- | --- |
| H.264 / AVC | Maximum compatibility: Discord, browsers, old phones | `h264_nvenc` | `h264_amf` | `libx264` |
| HEVC / H.265 | About 30–40% better compression than H.264 at the same quality | `hevc_nvenc` | `hevc_amf` | `libx265` |
| AV1 | The most quality per byte at a tight limit | `av1_nvenc` (RTX 40 series and newer) | `av1_amf` (RDNA 3 and newer) | `libsvtav1` |

AV1 is the best choice when the limit is tight, but it is also the least compatible option.
Someone on an older phone or an older browser may not be able to play it. HEVC is a good
middle ground; H.264 is the safe choice for something sent to a group chat.

## Encoder detection

`ffmpeg -encoders` only says that an encoder was compiled in. It does not prove that the
driver, the GPU and the current session can actually use it. CutGut therefore runs a real
one-frame encode to a null sink (`color=c=black:s=256x256:d=0.04`) for each candidate.
The result is cached in memory for the rest of the process.

The 256×256 size is deliberate: some NVENC paths reject very small frames, which would look
like "the GPU is not supported" when it is.

If AV1 hardware encoding was requested on a GPU without it (for example an RTX 20 or 30
series card), the plan falls back to `libsvtav1` and says so in the log and in the UI. It does
not silently keep a hardware plan for a software encoder.

## The byte budget

CutGut uses decimal megabytes, because that is what Discord and the file properties dialog
show:

```text
hard_limit_bytes = target_mb × 1,000,000
```

So 20 MB is exactly 20,000,000 bytes. The first plan leaves a safety margin, because an MP4
has a `moov` atom, audio packets and keyframe variance that are hard to predict exactly:

```text
planning_bytes = hard_limit_bytes × 0.96   (H.264 and HEVC)
planning_bytes = hard_limit_bytes × 0.94   (AV1)
```

AV1 gets a slightly larger margin because SVT-AV1 and NVENC AV1 VBR can overshoot a little
more often. The rest is divided between audio and video:

```text
audio_bytes    = audio_bitrate × duration_seconds / 8
overhead_bytes = 30,000 + 350 × duration_seconds
net_video_bytes = planning_bytes - audio_bytes - overhead_bytes
video_kbps     = max(floor(net_video_bytes × 8 / duration_seconds / 1000), 30)
```

Audio is planned at 96 kbps for HEVC/AV1 and 128 kbps for H.264, or 0 if the source has no
audio track. If the source has 5.1 audio it is downmixed to stereo at that bitrate.

If the calculated video bitrate would be below 30 kbps, the plan stops with a clear error
instead of producing a broken file. The usual reason is a very long clip and a very small
limit; the fix is to trim the clip or raise the limit.

### Bits-per-pixel ceiling

A size limit can be much larger than a clip needs. A four-second 360p clip under a 500 MB
limit would ask the encoder for about 1 Gb/s. NVENC and AMF reject that with
`InitializeEncoder failed: invalid param`, even though the byte arithmetic is correct.

The plan therefore caps the video bitrate at half a bit per pixel:

```text
bpp_ceiling_kbps = width × height × fps × 0.5 / 1000
video_kbps = min(wanted_kbps, max(bpp_ceiling_kbps, 30))
```

Half a bit per pixel is visually close to lossless for H.264/HEVC. When the cap is applied,
the plan sets `bitrate_capped` and the UI says the limit is larger than the clip needs, so the
file will come out smaller. It does not pretend it can spend the missing bytes.

## Quality at the same file size

The numbers below come from one 12-second 1080p60 gameplay clip at about 8.9 MB
(≈5.6 Mb/s), encoded through CutGut's own pipeline. VMAF is measured against the source. They
are a useful direction, not a universal benchmark: animation, dark scenes and high-motion
gameplay all behave differently.

| Path | Size | VMAF | Time for 12 s |
| --- | --- | --- | --- |
| AV1 CPU (SVT-AV1 preset 5, 2-pass) | 8.52 MB | **95.37** | 122 s |
| AV1 GPU (`av1_nvenc`, `-tune uhq -preset p7`) | 8.39 MB | **95.23** | ~20 s |
| HEVC GPU (`hevc_nvenc`, `-tune uhq -preset p7 -b_ref_mode middle`) | 8.41 MB | 94.36 | ~15 s |
| HEVC GPU (older flags: `p6` / `hq`) | 8.10 MB | 93.63 | ~7 s |
| H.264 CPU (`libx264`, 2-pass slow) | 8.69 MB | 93.35 | 13.5 s |
| H.264 GPU (`h264_nvenc`, preset `p6`) | 8.15 MB | 88.88 | ~6 s |

What made it into the code:

- For NVENC, the ultra-quality tune with B-frame references (`-tune uhq -preset p7
  -b_ref_mode middle`) is worth roughly +0.7 VMAF on HEVC and +0.9 on H.264 at the same size.
  Each flag set is probed with a one-frame encode; an older driver falls back to the previous
  set instead of failing the export.
- At a tight limit, AV1 is the best use of the bytes: about +0.9 VMAF over HEVC and +6 over
  hardware H.264.
- Hardware H.264 is the worst choice at a tight limit. When the plan sees a very low
  bits-per-pixel value, it suggests AV1 or HEVC instead.

Ideas that were measured and rejected:

- **Scaling down to 720p** at 4.6 Mb/s: VMAF 86.2. The resolution loss costs more than the
  extra bits gain. The plan already takes resolution into account.
- **Dropping to 30 fps**: VMAF 56.6 (VMAF compares frame by frame) and visibly less smooth.
- **Denoising before encoding** (`hqdn3d`): 92.9. It smooths textures and makes gameplay look
  soft.

One idea works but is deliberately not the default:

- **HEVC 10-bit** (`-profile:v main10 -pix_fmt p010le -highbitdepth 1`): 94.24 VMAF at
  5.34 Mb/s versus 93.63 at 5.53 Mb/s for 8-bit. That is about +0.6 VMAF and a smaller file,
  because 10-bit kills banding in skies, smoke and shadows. The cost is compatibility:
  QuickTime, iOS and some social platforms are less reliable with it. It belongs behind a
  "maximum quality" option, not in the default path.

## NVENC rate control

For `h264_nvenc`, `hevc_nvenc` and `av1_nvenc`, CutGut probes the best flag set the installed
driver accepts and then uses VBR with a ceiling:

- `-tune uhq` and `-preset p7` for high quality (the fallback sets omit what the driver
  rejects).
- `-multipass fullres` for a full-resolution analysis pass inside the hardware encoder. This
  is not the same as a two-pass CPU encode.
- `-b_ref_mode middle` for B-frame references.
- `-spatial-aq 1` and `-temporal-aq 1` for adaptive quantization.
- `-rc-lookahead 32` to see difficult scenes (smoke, particles, fire) before they arrive.
- `-maxrate 1.5 × video_kbps` and `-bufsize 2 × video_kbps` so hard scenes can borrow bits
  without turning the whole export into CBR.

Fast mode uses a lighter preset (`p3`) and skips the most expensive analysis flags. The
quality difference is real; the speed difference is bigger.

## Safe output and the correction loop

### Never overwrite the source

Before encoding, the input and output paths are canonicalized and compared. If the output
would resolve to the input (including through a symlink or a hard link), the export is
refused.

### Write, verify, then replace

The encoder writes to a temporary file in the destination directory
(`.cg_tmp_{tag}_{name}`), so the final rename stays on the same volume. The existing output
file is not touched during encoding. Only after the size check passes is the temporary file
moved into place with `MoveFileExW(..., MOVEFILE_REPLACE_EXISTING)`. A failed export, a
cancelled export or a permission error leaves the user's previous file intact.

Temporary files are cleaned up per job, so a second CutGut instance cannot delete an encode
that is still running.

### When the file is still too large

The file is accepted when its real size is at most 99.5% of the hard limit; the final attempt
may use the full limit. If the first attempt overshoots, only the video budget is corrected,
because audio and container overhead are effectively fixed:

```text
measured_video = max(actual_size - audio_bytes - overhead_bytes, 1000)
target_video   = max(hard_limit_bytes × safety - audio_bytes - overhead_bytes, 1000)
new_vkbps      = max(floor(current_vkbps × min(target_video / measured_video, 0.95)), 25)
```

The safety factor is 0.96 on the second attempt and 0.93 on the third. Each retry starts from
the original source and keeps the same codec, preset and VBR settings. There are at most three
attempts. If the file is still over the limit after that, the export fails with the measured
size and the target in the error. A file over the limit is never reported as a success.

## Remux instead of re-encode

If the clip already fits and only the container or the stream copy needs to change, CutGut
tries `-c copy` first. That is much faster and lossless, but it has two rules:

- Remux is only allowed when the frame was not cropped (or the crop is `original`) and the
  source codec matches the requested output codec. Asking for AV1 from an H.264 source must
  re-encode.
- A stream copy can only start at a keyframe. Cutting between keyframes used to produce a
  file with extra seconds before the requested in-point (a 6–10 s cut could start at -1.835 s
  and contain 5.9 s of video for a requested 4.077 s). After the remux, `content_span()` reads
  the first video PTS and the stream duration, and `trim_is_exact()` compares them with the
  request (about two frames or 0.12 s of tolerance). If the cut missed, the remux is thrown
  away and the export does an exact re-encode. A full-clip copy (`in = 0`, `out = duration`)
  does not need this check.

## Preview trim

The **Preview trim** button in the editor plays only the IN–OUT range from the source file,
without re-encoding. The quality panel (bitrate, bits per pixel per frame, predicted size) is
recomputed on every change of profile, limit, codec, engine or range.

## Exporting AV1

In the GUI:

1. Open a clip in the editor.
2. In the **Export** panel choose **Codec → AV1**.
3. Choose **Engine → GPU – high quality** (RTX 40 series or newer) or **CPU – high quality**
   (`libsvtav1`).
4. Set the limit to 20 MB for Discord's free upload.
5. Press **Export**.

In the CLI:

```bat
cutgut encode "clip.mp4" --out out.mp4 --start 10 --end 30 --target 20 --codec av1 --speed hq
cutgut plan "clip.mp4" --target 20 --codec av1
cutgut encode "clip.mp4" --out out.mp4 --target 20 --codec av1 --cpu
```
