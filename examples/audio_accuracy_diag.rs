use pce::bus::AudioDiagnostics;
use pce::emulator::Emulator;
use std::error::Error;

#[derive(Default)]
struct IntervalAudioStats {
    drained_samples: u64,
    drained_chunks: u64,
    clipped_samples: u64,
    max_abs_sample: i16,
    sum_abs: u64,
    sum_abs_delta: u64,
    previous_sample: Option<i16>,
}

impl IntervalAudioStats {
    fn absorb(&mut self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }

        self.drained_samples += samples.len() as u64;
        self.drained_chunks += 1;

        for &sample in samples {
            let abs = sample.unsigned_abs() as u64;
            self.sum_abs += abs;
            if abs >= i16::MAX as u64 {
                self.clipped_samples += 1;
            }
            if sample.unsigned_abs() > self.max_abs_sample.unsigned_abs() {
                self.max_abs_sample = sample;
            }
            if let Some(previous) = self.previous_sample {
                self.sum_abs_delta += (sample as i32 - previous as i32).unsigned_abs() as u64;
            }
            self.previous_sample = Some(sample);
        }
    }

    fn average_abs(&self) -> f64 {
        if self.drained_samples == 0 {
            0.0
        } else {
            self.sum_abs as f64 / self.drained_samples as f64
        }
    }

    fn average_abs_delta(&self) -> f64 {
        if self.drained_samples <= 1 {
            0.0
        } else {
            self.sum_abs_delta as f64 / (self.drained_samples - 1) as f64
        }
    }

    fn reset(&mut self) {
        *self = Self {
            previous_sample: self.previous_sample,
            ..Self::default()
        };
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let rom_path = args.next().ok_or_else(|| {
        "usage: cargo run --release --example audio_accuracy_diag -- <rom.pce> [seconds] [warmup_frames]"
            .to_string()
    })?;
    let seconds = args
        .next()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(10);
    let warmup_frames = args
        .next()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(120);

    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.set_audio_batch_size(256);
    emu.reset();

    let mut frame_buf = Vec::new();
    let mut warmup_complete = false;
    let mut warmup_seen_frames = 0u64;
    let mut measured_frames = 0u64;
    let mut report_index = 0u64;
    let mut last_diag = AudioDiagnostics::default();
    let mut interval_stats = IntervalAudioStats::default();

    println!(
        "Audio accuracy baseline: rom={} seconds={} warmup_frames={}",
        rom_path, seconds, warmup_frames
    );

    loop {
        emu.tick();

        if emu.take_frame_into(&mut frame_buf) {
            if !warmup_complete {
                warmup_seen_frames += 1;
                if warmup_seen_frames >= warmup_frames {
                    warmup_complete = true;
                    measured_frames = 0;
                    last_diag = AudioDiagnostics::default();
                    emu.bus.reset_audio_diagnostics();
                    let _ = emu.drain_audio_samples();
                    interval_stats = IntervalAudioStats::default();
                    println!(
                        "Measurement start after warmup: frames={} cpu_cycles={}",
                        warmup_seen_frames,
                        emu.cycles()
                    );
                }
            } else {
                measured_frames += 1;
            }
        }

        if !warmup_complete {
            let _ = emu.take_audio_samples();
            continue;
        }

        if let Some(chunk) = emu.take_audio_samples() {
            interval_stats.absorb(&chunk);
        }

        let diag = emu.bus.audio_diagnostics();
        let next_report_phi = (report_index + 1) * diag.master_clock_hz as u64;
        if diag.total_phi_cycles >= next_report_phi {
            flush_report(
                report_index + 1,
                measured_frames,
                last_diag,
                diag,
                emu.pending_audio_samples(),
                &interval_stats,
                &emu,
            );
            last_diag = diag;
            interval_stats.reset();
            report_index += 1;
        }

        if diag.total_phi_cycles >= seconds * diag.master_clock_hz as u64 {
            break;
        }
    }

    let tail = emu.drain_audio_samples();
    interval_stats.absorb(&tail);
    let final_diag = emu.bus.audio_diagnostics();

    println!();
    println!("Summary:");
    println!(
        "  emulated_seconds={:.3} frames={} avg_fps={:.2}",
        final_diag.total_phi_cycles as f64 / final_diag.master_clock_hz as f64,
        measured_frames,
        measured_frames as f64 * final_diag.master_clock_hz as f64
            / final_diag.total_phi_cycles.max(1) as f64
    );
    println!(
        "  generated_samples={} drained_by_bus={} drain_calls={} pending_emu_samples={}",
        final_diag.generated_samples,
        final_diag.drained_samples,
        final_diag.drain_calls,
        emu.pending_audio_samples()
    );
    println!(
        "  effective_sample_rate={:.2}Hz avg_abs={:.1} avg_abs_delta={:.1} clipped={} max_abs={}",
        final_diag.generated_samples as f64 * final_diag.master_clock_hz as f64
            / final_diag.total_phi_cycles.max(1) as f64,
        interval_stats.average_abs(),
        interval_stats.average_abs_delta(),
        interval_stats.clipped_samples,
        interval_stats.max_abs_sample.unsigned_abs()
    );
    println!("  final_channels={}", describe_channels(&emu));

    Ok(())
}

fn flush_report(
    elapsed_seconds: u64,
    measured_frames: u64,
    previous: AudioDiagnostics,
    current: AudioDiagnostics,
    pending_emu_samples: usize,
    interval_stats: &IntervalAudioStats,
    emu: &Emulator,
) {
    let delta_phi = current
        .total_phi_cycles
        .saturating_sub(previous.total_phi_cycles);
    let delta_generated = current
        .generated_samples
        .saturating_sub(previous.generated_samples);
    let delta_drained = current
        .drained_samples
        .saturating_sub(previous.drained_samples);
    let delta_drain_calls = current.drain_calls.saturating_sub(previous.drain_calls);
    let fps = if current.total_phi_cycles == 0 {
        0.0
    } else {
        measured_frames as f64 * current.master_clock_hz as f64 / current.total_phi_cycles as f64
    };
    let sample_rate = if delta_phi == 0 {
        0.0
    } else {
        delta_generated as f64 * current.master_clock_hz as f64 / delta_phi as f64
    };

    println!(
        "[{:>3}s] rate={:>8.2}Hz frames={:>4} fps={:>5.2} gen={:>5} drained={:>5} bus_drains={:>4} pending_emu={:>4} avg_abs={:>7.1} avg_delta={:>7.1} clip={:>4} ch={}",
        elapsed_seconds,
        sample_rate,
        measured_frames,
        fps,
        delta_generated,
        delta_drained,
        delta_drain_calls,
        pending_emu_samples,
        interval_stats.average_abs(),
        interval_stats.average_abs_delta(),
        interval_stats.clipped_samples,
        describe_channels(emu),
    );
}

fn describe_channels(emu: &Emulator) -> String {
    let mut parts = Vec::new();
    for ch in 0..6 {
        let (freq, control, _, noise_ctrl) = emu.bus.psg_channel_info(ch);
        if control & 0x80 == 0 {
            continue;
        }

        let volume = control & 0x1F;
        if control & 0x40 != 0 {
            parts.push(format!("CH{}=DDA(v{:02})", ch, volume));
            continue;
        }

        if ch >= 4 && noise_ctrl & 0x80 != 0 {
            parts.push(format!(
                "CH{}=N{:02}(v{:02})",
                ch,
                noise_ctrl & 0x1F,
                volume
            ));
            continue;
        }

        let hz = if freq == 0 {
            0.0
        } else {
            3_579_545.0 / (32.0 * freq as f64)
        };
        parts.push(format!("CH{}={:.0}Hz(v{:02})", ch, hz, volume));
    }

    if parts.is_empty() {
        "idle".to_string()
    } else {
        parts.join(",")
    }
}
