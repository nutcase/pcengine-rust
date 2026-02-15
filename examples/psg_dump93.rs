use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run to frame 400 (music should be playing)
    let mut frames = 0u64;
    let mut total_ticks = 0u64;
    while frames < 400 && total_ticks < 100_000_000 {
        emu.tick();
        total_ticks += 1;
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    println!("=== PSG State at frame {} ===\n", frames);

    // Dump all channel info
    for ch in 0..6 {
        let (freq, control, balance, noise_ctrl) = emu.bus.psg_channel_info(ch);
        let key_on = (control & 0x80) != 0;
        let dda = (control & 0x40) != 0;
        let volume = control & 0x1F;
        let noise_en = (noise_ctrl & 0x80) != 0;
        let noise_freq = noise_ctrl & 0x1F;

        let hz = if freq == 0 {
            0.0
        } else {
            3_579_545.0 / (32.0 * freq as f64)
        };

        println!(
            "CH{}: freq=${:03X}({:.1}Hz) vol={} key={} dda={} bal=${:02X} noise={}/freq={}",
            ch, freq, hz, volume, key_on, dda, balance, noise_en, noise_freq
        );
    }

    // Dump waveform RAM via reading PSG direct registers
    // We can't directly access waveform_ram from outside, so let's read from
    // the bus I/O ports. Actually, we need to use the bus internals.
    // For now, let's capture audio samples and do FFT analysis.

    println!("\n=== Audio sample analysis (1 frame worth) ===");
    emu.set_audio_batch_size(1);
    let mut frame_samples: Vec<i16> = Vec::new();
    let samples_per_frame = (44100.0 / 60.0) as usize;

    // Collect exactly 1 frame of audio
    loop {
        emu.tick();
        total_ticks += 1;
        if let Some(samples) = emu.take_audio_samples() {
            for &s in &samples {
                frame_samples.push(s);
            }
        }
        if frame_samples.len() >= samples_per_frame * 2 {
            break;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Analyze the audio
    let samples = &frame_samples[..samples_per_frame.min(frame_samples.len())];
    let min = samples.iter().copied().min().unwrap_or(0);
    let max = samples.iter().copied().max().unwrap_or(0);
    let rms: f64 =
        (samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();

    println!("Samples: {}", samples.len());
    println!("Min: {}, Max: {}", min, max);
    println!("RMS: {:.1}", rms);
    println!("Peak-to-peak: {}", max as i32 - min as i32);

    // Count zero crossings (rough frequency estimate)
    let mut zero_crossings = 0;
    for i in 1..samples.len() {
        if (samples[i] >= 0) != (samples[i - 1] >= 0) {
            zero_crossings += 1;
        }
    }
    println!(
        "Zero crossings: {} (~{:.0} Hz)",
        zero_crossings,
        zero_crossings as f64 / 2.0 * 60.0
    );

    // Print first 64 samples
    println!("\nFirst 64 samples:");
    for (i, &s) in samples.iter().take(64).enumerate() {
        print!("{:6}", s);
        if (i + 1) % 16 == 0 {
            println!();
        }
    }

    // Look for repeating patterns (basic period detection)
    println!("\n\n=== Autocorrelation (period detection) ===");
    let analyze_len = 1024.min(samples.len());
    let samples_f64: Vec<f64> = samples[..analyze_len].iter().map(|&s| s as f64).collect();
    let mean: f64 = samples_f64.iter().sum::<f64>() / analyze_len as f64;

    let mut best_corr = 0.0f64;
    let mut best_lag = 0usize;
    for lag in 10..500 {
        let mut corr = 0.0;
        let mut norm1 = 0.0;
        let mut norm2 = 0.0;
        for i in 0..(analyze_len - lag) {
            let a = samples_f64[i] - mean;
            let b = samples_f64[i + lag] - mean;
            corr += a * b;
            norm1 += a * a;
            norm2 += b * b;
        }
        let denom = (norm1 * norm2).sqrt();
        if denom > 0.0 {
            let normalized = corr / denom;
            if normalized > best_corr {
                best_corr = normalized;
                best_lag = lag;
            }
        }
    }
    let detected_freq = if best_lag > 0 {
        44100.0 / best_lag as f64
    } else {
        0.0
    };
    println!(
        "Best autocorrelation: lag={} ({:.1} Hz), correlation={:.3}",
        best_lag, detected_freq, best_corr
    );

    // Look for rapid sample-to-sample changes (noise indicator)
    let mut big_jumps = 0;
    let mut total_diff = 0i64;
    for i in 1..samples.len() {
        let diff = (samples[i] as i32 - samples[i - 1] as i32).unsigned_abs();
        total_diff += diff as i64;
        if diff > 5000 {
            big_jumps += 1;
        }
    }
    let avg_diff = total_diff as f64 / (samples.len() - 1) as f64;
    println!("\nSample-to-sample analysis:");
    println!("  Average abs diff: {:.1}", avg_diff);
    println!("  Large jumps (>5000): {}", big_jumps);

    Ok(())
}
