#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use sdl2::audio::{AudioCallback, AudioSpecDesired, AudioStatus};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Internal emulator sample rate (must match AUDIO_SAMPLE_RATE in bus.rs).
const EMU_SAMPLE_RATE: u32 = 44_100;

/// Target buffer size in samples – enough for ~50 ms of audio.
const TARGET_BUFFER: usize = 2205;
/// Producer high watermark. Once reached, let the callback drain before
/// running more emulation; this avoids the large sawtooth caused by coarse
/// 10 ms sleeps at MAX_BUFFER.
const HIGH_WATER_BUFFER: usize = TARGET_BUFFER * 2;
/// Maximum buffer before throttling – ~200 ms.
const MAX_BUFFER: usize = 8820;

struct AudioSdlDiagRuntime {
    enabled: bool,
    start: Instant,
    last_report: Instant,
    last_produced: u64,
    last_consumed: u64,
    last_callbacks: u64,
    last_underruns: u64,
    last_throttles: u64,
    min_buffer: usize,
    max_buffer: usize,
}

impl AudioSdlDiagRuntime {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            start: Instant::now(),
            last_report: Instant::now(),
            last_produced: 0,
            last_consumed: 0,
            last_callbacks: 0,
            last_underruns: 0,
            last_throttles: 0,
            min_buffer: usize::MAX,
            max_buffer: 0,
        }
    }

    fn observe(
        &mut self,
        current_buffer: usize,
        produced_samples: u64,
        consumed_samples: u64,
        callback_count: u64,
        underruns: u64,
        throttle_events: u64,
        peak_buffer: u64,
    ) {
        if !self.enabled {
            return;
        }

        self.min_buffer = self.min_buffer.min(current_buffer);
        self.max_buffer = self.max_buffer.max(current_buffer);

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_report);
        if elapsed < Duration::from_secs(1) {
            return;
        }

        let dt = elapsed.as_secs_f64().max(f64::EPSILON);
        let delta_produced = produced_samples.saturating_sub(self.last_produced);
        let delta_consumed = consumed_samples.saturating_sub(self.last_consumed);
        let delta_callbacks = callback_count.saturating_sub(self.last_callbacks);
        let delta_underruns = underruns.saturating_sub(self.last_underruns);
        let delta_throttles = throttle_events.saturating_sub(self.last_throttles);

        eprintln!(
            "[audio_sdl {:.1}s] buf={} min={} max={} peak={} prod={:.1}/s cons={:.1}/s cb={} underrun={} throttle={}",
            now.duration_since(self.start).as_secs_f64(),
            current_buffer,
            if self.min_buffer == usize::MAX {
                current_buffer
            } else {
                self.min_buffer
            },
            self.max_buffer,
            peak_buffer,
            delta_produced as f64 / dt,
            delta_consumed as f64 / dt,
            delta_callbacks,
            delta_underruns,
            delta_throttles,
        );

        self.last_report = now;
        self.last_produced = produced_samples;
        self.last_consumed = consumed_samples;
        self.last_callbacks = callback_count;
        self.last_underruns = underruns;
        self.last_throttles = throttle_events;
        self.min_buffer = usize::MAX;
        self.max_buffer = current_buffer;
    }
}

struct PcmStream {
    buffer: Arc<Mutex<VecDeque<i16>>>,
    /// Resampling state: converts from EMU_SAMPLE_RATE to the actual device rate.
    resample_ratio: f64, // device_rate / EMU_SAMPLE_RATE
    resample_phase: f64,
    prev_sample: i16,
    underrun_count: Arc<AtomicU64>,
    consumed_samples: Arc<AtomicU64>,
    callback_count: Arc<AtomicU64>,
}

impl AudioCallback for PcmStream {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        let mut guard = self.buffer.lock().unwrap();
        self.callback_count.fetch_add(1, Ordering::Relaxed);
        self.consumed_samples
            .fetch_add(out.len() as u64, Ordering::Relaxed);

        if self.resample_ratio == 1.0 {
            // No resampling needed – fast path.
            for sample in out.iter_mut() {
                *sample = guard.pop_front().unwrap_or_else(|| {
                    self.underrun_count.fetch_add(1, Ordering::Relaxed);
                    self.prev_sample // repeat last sample instead of zero (less click)
                });
                self.prev_sample = *sample;
            }
        } else {
            // Linear interpolation resampling.
            let step = 1.0 / self.resample_ratio; // how much to advance in source per output sample
            for sample in out.iter_mut() {
                // Consume whole source samples that we've moved past.
                let skip = self.resample_phase as usize;
                for _ in 0..skip {
                    if guard.len() > 1 {
                        self.prev_sample = guard.pop_front().unwrap();
                    }
                }
                self.resample_phase -= skip as f64;

                let frac = self.resample_phase;
                let s0 = guard.front().copied().unwrap_or(self.prev_sample);
                let s1 = guard.get(1).copied().unwrap_or(s0);
                let interp = s0 as f64 * (1.0 - frac) + s1 as f64 * frac;
                *sample = interp as i16;
                self.resample_phase += step;
            }
        }
    }
}

fn main() -> Result<(), String> {
    let rom_path = std::env::args().nth(1).ok_or_else(|| {
        "usage: cargo run --release --example audio_sdl --features audio-sdl -- <rom.pce>"
            .to_string()
    })?;
    let rom = std::fs::read(&rom_path).map_err(|e| format!("failed to read ROM: {e}"))?;
    let is_pce = Path::new(&rom_path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);

    let sdl = sdl2::init()?;
    let audio = sdl.audio()?;
    let shared = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_BUFFER)));
    let underrun_count = Arc::new(AtomicU64::new(0));
    let produced_samples = Arc::new(AtomicU64::new(0));
    let consumed_samples = Arc::new(AtomicU64::new(0));
    let callback_count = Arc::new(AtomicU64::new(0));
    let throttle_events = Arc::new(AtomicU64::new(0));
    let peak_buffer = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));
    let diag_enabled = std::env::var_os("PCE_AUDIO_DIAG").is_some();

    let desired = AudioSpecDesired {
        freq: Some(EMU_SAMPLE_RATE as i32),
        channels: Some(1),
        samples: Some(1024),
    };

    let underrun_cb = underrun_count.clone();
    let consumed_cb = consumed_samples.clone();
    let callback_count_cb = callback_count.clone();
    let device = audio.open_playback(None, &desired, |spec| {
        let actual_rate = spec.freq as u32;
        let ratio = actual_rate as f64 / EMU_SAMPLE_RATE as f64;
        eprintln!(
            "Audio device: {} Hz, {} ch, {} samples/cb (ratio={:.4})",
            spec.freq, spec.channels, spec.samples, ratio
        );
        if (ratio - 1.0).abs() > 0.001 {
            eprintln!(
                "WARNING: Device rate ({}) != emulator rate ({}), resampling active",
                actual_rate, EMU_SAMPLE_RATE
            );
        }
        PcmStream {
            buffer: shared.clone(),
            resample_ratio: ratio,
            resample_phase: 0.0,
            prev_sample: 0,
            underrun_count: underrun_cb,
            consumed_samples: consumed_cb,
            callback_count: callback_count_cb,
        }
    })?;

    // Pre-buffer audio before starting playback.
    let shared_thread = shared.clone();
    let produced_thread = produced_samples.clone();
    let throttle_thread = throttle_events.clone();
    let peak_thread = peak_buffer.clone();
    let running_thread = running.clone();
    let emu_handle = thread::spawn(move || {
        let mut emu = Emulator::new();
        if is_pce {
            if let Err(err) = emu.load_hucard(&rom) {
                eprintln!("failed to load HuCard: {err}");
                return;
            }
        } else {
            emu.load_program(0xC000, &rom);
        }
        emu.reset();
        emu.set_video_output_enabled(false);
        emu.set_audio_batch_size(128); // ~3ms chunks, balances latency and overhead

        while running_thread.load(Ordering::Relaxed) {
            let queued_before = shared_thread.lock().unwrap().len();
            if queued_before >= HIGH_WATER_BUFFER {
                if queued_before > MAX_BUFFER {
                    throttle_thread.fetch_add(1, Ordering::Relaxed);
                }
                thread::sleep(Duration::from_millis(1));
                continue;
            }

            emu.tick();

            if let Some(samples) = emu.take_audio_samples() {
                let sample_count = samples.len();
                let mut guard = shared_thread.lock().unwrap();
                for sample in samples {
                    guard.push_back(sample);
                }
                let len = guard.len();
                produced_thread.fetch_add(sample_count as u64, Ordering::Relaxed);
                peak_thread.fetch_max(len as u64, Ordering::Relaxed);
                drop(guard);

                // Safety valve only. Normal pacing is handled by HIGH_WATER_BUFFER
                // above to avoid long producer sleeps that cause underrun bursts.
                if len > MAX_BUFFER {
                    throttle_thread.fetch_add(1, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(1));
                }
            }

            if emu.cpu.halted {
                break;
            }
        }
    });

    // Wait for pre-buffer to fill before starting playback.
    eprintln!("Pre-buffering...");
    loop {
        let len = shared.lock().unwrap().len();
        if len >= TARGET_BUFFER {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    eprintln!("Starting playback");
    if diag_enabled {
        eprintln!("PCE_AUDIO_DIAG enabled for audio_sdl");
    }
    device.resume();

    // Monitor loop.
    let start = Instant::now();
    let mut last_underrun = 0u64;
    let mut diag_runtime = AudioSdlDiagRuntime::new(diag_enabled);
    while device.status() == AudioStatus::Playing && running.load(Ordering::Relaxed) {
        thread::sleep(if diag_enabled {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(500)
        });
        let buf_len = shared.lock().unwrap().len();
        let underruns = underrun_count.load(Ordering::Relaxed);
        if diag_enabled {
            diag_runtime.observe(
                buf_len,
                produced_samples.load(Ordering::Relaxed),
                consumed_samples.load(Ordering::Relaxed),
                callback_count.load(Ordering::Relaxed),
                underruns,
                throttle_events.load(Ordering::Relaxed),
                peak_buffer.load(Ordering::Relaxed),
            );
        } else if underruns > last_underrun {
            let elapsed = start.elapsed().as_secs_f64();
            eprintln!(
                "[{:.1}s] buffer={} underruns={} (try: cargo run --release)",
                elapsed, buf_len, underruns
            );
            last_underrun = underruns;
        }
    }

    running.store(false, Ordering::Relaxed);
    let _ = emu_handle.join();

    Ok(())
}
