//! Native preview audio (Windows `waveOut`).
//!
//! FFmpeg decodes the clip's audio track to raw PCM which is streamed straight
//! into the default output device. The stream is *armed* immediately (so the
//! decoder is warm) but the first sample is only queued once the video pipeline
//! reports that its first frame is on screen — that keeps sound locked to the
//! picture instead of running ahead of it.
//!
//! A speed change cannot be applied to a running stream (ffmpeg bakes `atempo`
//! into the PCM), so a second decoder is warmed up at the new tempo and takes the
//! device over as soon as it has buffers ready, fast-forwarding itself to the
//! picture before it starts. That way the sound never has to catch up with a hole.
//!
//! Volume is applied sample-by-sample while filling the device buffers, so the
//! slider reacts instantly without restarting the decoder.

use std::ffi::c_void;
use std::io::Read;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::player::{Cursor, Handover, PlaybackRate};
use crate::process::Process;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const BYTES_PER_SAMPLE: usize = 2;
const FRAMES_PER_BUFFER: usize = 1024;
const BUFFER_BYTES: usize = FRAMES_PER_BUFFER * CHANNELS * BYTES_PER_SAMPLE;
const BUFFER_COUNT: usize = 4;

/// How long the previous stream gets to release the device before the one taking
/// over starts writing. `waveOutReset` is quick, but it runs on the other thread.
const HANDOVER_SETTLE: Duration = Duration::from_millis(25);

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_MAPPER: u32 = u32::MAX;
const WHDR_DONE: u32 = 0x0000_0001;
const WHDR_PREPARED: u32 = 0x0000_0002;

#[repr(C)]
struct WaveFormatEx {
    format_tag: u16,
    channels: u16,
    samples_per_sec: u32,
    avg_bytes_per_sec: u32,
    block_align: u16,
    bits_per_sample: u16,
    cb_size: u16,
}

#[repr(C)]
struct WaveHdr {
    data: *mut u8,
    buffer_length: u32,
    bytes_recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: *mut WaveHdr,
    reserved: usize,
}

#[link(name = "winmm")]
unsafe extern "system" {
    fn waveOutOpen(
        handle: *mut *mut c_void,
        device: u32,
        format: *const WaveFormatEx,
        callback: usize,
        instance: usize,
        flags: u32,
    ) -> u32;
    fn waveOutPrepareHeader(handle: *mut c_void, header: *mut WaveHdr, size: u32) -> u32;
    fn waveOutUnprepareHeader(handle: *mut c_void, header: *mut WaveHdr, size: u32) -> u32;
    fn waveOutWrite(handle: *mut c_void, header: *mut WaveHdr, size: u32) -> u32;
    fn waveOutReset(handle: *mut c_void) -> u32;
    fn waveOutClose(handle: *mut c_void) -> u32;
}

struct Buffer {
    data: Box<[u8]>,
    header: Box<WaveHdr>,
}

impl Buffer {
    fn new() -> Self {
        let mut data = vec![0u8; BUFFER_BYTES].into_boxed_slice();
        let header = Box::new(WaveHdr {
            data: data.as_mut_ptr(),
            buffer_length: 0,
            bytes_recorded: 0,
            user: 0,
            flags: 0,
            loops: 0,
            next: std::ptr::null_mut(),
            reserved: 0,
        });
        Self { data, header }
    }
}

/// Starts a decoder + output stream for `path` between `start_s` and `end_s`.
///
/// `clock` is set by the video pipeline as soon as the first frame is shown, and
/// `cursor` tells where the picture stands; `handover` is only used when this
/// stream replaces one that is already playing. `volume` holds the linear gain
/// encoded as `f32` bits (0.0..=1.0).
#[allow(clippy::too_many_arguments)]
pub fn run(
    path: String,
    start_s: f64,
    end_s: f64,
    rate: PlaybackRate,
    volume: Arc<AtomicU32>,
    clock: Arc<OnceLock<Instant>>,
    cursor: Arc<Mutex<Option<Cursor>>>,
    handover: Option<Handover>,
    stop: Arc<AtomicBool>,
    decoder: Process,
) {
    if spawn_decoder(&path, start_s, end_s, rate, &decoder).is_err() {
        return;
    }
    let Some(mut pcm) = decoder.stdout() else {
        decoder.finish();
        return;
    };

    let format = WaveFormatEx {
        format_tag: WAVE_FORMAT_PCM,
        channels: CHANNELS as u16,
        samples_per_sec: SAMPLE_RATE,
        avg_bytes_per_sec: SAMPLE_RATE * CHANNELS as u32 * BYTES_PER_SAMPLE as u32,
        block_align: (CHANNELS * BYTES_PER_SAMPLE) as u16,
        bits_per_sample: (BYTES_PER_SAMPLE * 8) as u16,
        cb_size: 0,
    };
    let mut device: *mut c_void = std::ptr::null_mut();
    let result = unsafe { waveOutOpen(&mut device, WAVE_MAPPER, &format, 0, 0, 0) };
    if result != 0 || device.is_null() {
        decoder.finish();
        return;
    }

    let header_size = std::mem::size_of::<WaveHdr>() as u32;
    let mut buffers: Vec<Buffer> = (0..BUFFER_COUNT).map(|_| Buffer::new()).collect();
    let mut prepared = 0;
    for buffer in &mut buffers {
        if unsafe { waveOutPrepareHeader(device, buffer.header.as_mut(), header_size) } != 0 {
            break;
        }
        prepared += 1;
    }
    if prepared < BUFFER_COUNT {
        // The device disappeared while buffers were being prepared. Nothing has
        // been queued yet, so unwind and let the clip play without sound.
        unsafe {
            for buffer in buffers.iter_mut().take(prepared) {
                waveOutUnprepareHeader(device, buffer.header.as_mut(), header_size);
            }
            waveOutClose(device);
        }
        decoder.finish();
        return;
    }

    let mut lengths = [0usize; BUFFER_COUNT];
    let mut ready = [false; BUFFER_COUNT];
    let mut queued = [false; BUFFER_COUNT];
    let mut eof = false;

    // Decode ahead while the picture pipeline prepares its first frame, so sound
    // is available the very moment the picture appears.
    for index in 0..BUFFER_COUNT {
        let bytes = fill(&mut pcm, &mut buffers[index].data);
        if bytes == 0 {
            eof = true;
            break;
        }
        lengths[index] = bytes;
        ready[index] = true;
    }

    // Sound that is already late must be dropped, not replayed: a stream started
    // at a new tempo took a moment to warm up, and replaying that moment would
    // put the sound behind the picture for the rest of the clip.
    let Some(media_now) = sync_to_picture(
        &clock,
        &cursor,
        handover.as_ref(),
        rate.scale(),
        start_s,
        &stop,
    ) else {
        unsafe { release(device, &mut buffers, header_size) };
        decoder.finish();
        return;
    };
    let behind = media_now - start_s;
    if behind > 0.005 {
        ready = [false; BUFFER_COUNT];
        eof = skip_pcm(&mut pcm, behind);
    }

    while !stop.load(Ordering::Relaxed) {
        let gain = gain(&volume);
        let mut device_lost = false;
        for index in 0..BUFFER_COUNT {
            if queued[index] {
                if header_flags(&buffers[index].header) & WHDR_DONE == 0 {
                    continue;
                }
                queued[index] = false;
            }
            if !ready[index] {
                if eof {
                    continue;
                }
                let bytes = fill(&mut pcm, &mut buffers[index].data);
                if bytes == 0 {
                    eof = true;
                    continue;
                }
                lengths[index] = bytes;
                ready[index] = true;
            }
            apply_gain(&mut buffers[index].data[..lengths[index]], gain);
            let header = buffers[index].header.as_mut();
            header.flags &= !WHDR_DONE;
            header.buffer_length = lengths[index] as u32;
            header.bytes_recorded = 0;
            if unsafe { waveOutWrite(device, header, header_size) } != 0 {
                // A rejected buffer never reports WHDR_DONE, so treating it as
                // queued would spin this loop forever without any sound.
                device_lost = true;
                break;
            }
            queued[index] = true;
            ready[index] = false;
        }
        if device_lost {
            break;
        }
        if eof && !queued.iter().any(|queued| *queued) {
            break;
        }
        std::thread::sleep(Duration::from_millis(4));
    }

    unsafe { release(device, &mut buffers, header_size) };
    decoder.finish();
}

/// Stops the device and gives every buffer back to the system.
unsafe fn release(device: *mut c_void, buffers: &mut [Buffer], header_size: u32) {
    waveOutReset(device);
    for buffer in buffers {
        if header_flags(&buffer.header) & WHDR_PREPARED != 0 {
            waveOutUnprepareHeader(device, buffer.header.as_mut(), header_size);
        }
    }
    waveOutClose(device);
}

/// Clip time the sound has to start at, or `None` when playback stopped first.
///
/// A fresh stream waits for the picture clock, which starts with the first frame.
/// A stream taking over is told when to go and then reads where the picture is at
/// that instant, so the change of tempo happens exactly under the picture.
fn sync_to_picture(
    clock: &OnceLock<Instant>,
    cursor: &Mutex<Option<Cursor>>,
    handover: Option<&Handover>,
    scale: f64,
    start_s: f64,
    stop: &AtomicBool,
) -> Option<f64> {
    let Some(handover) = handover else {
        while clock.get().is_none() && !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(2));
        }
        let origin = clock.get()?;
        return Some(start_s + origin.elapsed().as_secs_f64() * scale);
    };
    // Wait for a picture to follow (a takeover can come before the first frame),
    // then report that this stream is ready to take the device over.
    while cursor.lock().unwrap().is_none() {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    handover.armed.store(true, Ordering::Relaxed);
    while !handover.go.load(Ordering::Relaxed) {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    // The old stream is closing; start once it is silent, on the picture that is
    // on screen at that instant.
    std::thread::sleep(HANDOVER_SETTLE);
    let picture = (*cursor.lock().unwrap())?;
    let advance = Instant::now()
        .saturating_duration_since(picture.wall)
        .as_secs_f64();
    Some(picture.media + advance * scale)
}

fn spawn_decoder(
    path: &str,
    start_s: f64,
    end_s: f64,
    rate: PlaybackRate,
    decoder: &Process,
) -> std::io::Result<()> {
    let duration = (end_s - start_s).max(0.05);
    let args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostdin".into(),
        "-ss".into(),
        format!("{start_s:.3}"),
        "-t".into(),
        format!("{duration:.3}"),
        "-i".into(),
        path.to_string(),
        "-af".into(),
        rate.audio_filter(duration),
        "-vn".into(),
        "-ac".into(),
        CHANNELS.to_string(),
        "-ar".into(),
        SAMPLE_RATE.to_string(),
        "-acodec".into(),
        "pcm_s16le".into(),
        "-f".into(),
        "s16le".into(),
        "pipe:1".into(),
    ];
    let mut command = cutgut::ffmpeg::command(&cutgut::ffmpeg::ffmpeg_path());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    decoder.spawn(&mut command)
}

/// `flags` is written by the waveOut device thread, so it is read through a
/// volatile load: a cached or hoisted read could miss `WHDR_DONE` forever.
fn header_flags(header: &WaveHdr) -> u32 {
    unsafe { std::ptr::read_volatile(&header.flags) }
}

/// Reads one device buffer worth of PCM; returns 0 at end of stream.
fn fill(pcm: &mut impl Read, data: &mut [u8]) -> usize {
    let mut read = 0;
    while read < data.len() {
        match pcm.read(&mut data[read..]) {
            Ok(0) => break,
            Ok(bytes) => read += bytes,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    read
}

/// Drops `seconds` worth of PCM; returns `true` when the decoded stream ended.
fn skip_pcm(pcm: &mut impl Read, seconds: f64) -> bool {
    let mut left = (seconds.max(0.0) * SAMPLE_RATE as f64) as usize * CHANNELS * BYTES_PER_SAMPLE;
    let mut scratch = [0u8; BUFFER_BYTES];
    while left > 0 {
        let take = left.min(scratch.len());
        match pcm.read(&mut scratch[..take]) {
            Ok(0) => return true,
            Ok(bytes) => left -= bytes,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return true,
        }
    }
    false
}

fn gain(volume: &AtomicU32) -> f32 {
    f32::from_bits(volume.load(Ordering::Relaxed)).clamp(0.0, 1.0)
}

fn apply_gain(samples: &mut [u8], gain: f32) {
    if (gain - 1.0).abs() < f32::EPSILON {
        return;
    }
    for sample in samples.chunks_exact_mut(BYTES_PER_SAMPLE) {
        let value = i16::from_le_bytes([sample[0], sample[1]]) as f32 * gain;
        let value = value.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        sample.copy_from_slice(&value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn gain_scales_samples() {
        let mut samples = vec![0u8; 8];
        samples[0..2].copy_from_slice(&1000i16.to_le_bytes());
        super::apply_gain(&mut samples, 0.5);
        assert_eq!(i16::from_le_bytes([samples[0], samples[1]]), 500);
    }

    #[test]
    fn skipping_drops_exactly_the_elapsed_sound() {
        let second = super::SAMPLE_RATE as usize * super::CHANNELS * super::BYTES_PER_SAMPLE;
        let mut pcm = std::io::Cursor::new(vec![7u8; second * 3]);
        let ended = super::skip_pcm(&mut pcm, 1.5);
        assert!(!ended);
        assert_eq!(pcm.position() as usize, second * 3 / 2);
        // Skipping past the end reports the stream as finished.
        assert!(super::skip_pcm(&mut pcm, 5.0));
    }
}
