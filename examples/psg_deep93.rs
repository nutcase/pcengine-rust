use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.set_audio_batch_size(1);

    let mut frames = 0u64;
    let mut total_ticks = 0u64;
    let mut samples_collected = 0u64;
    let mut nonzero_samples = 0u64;

    // Run to frame 300, collecting stats
    while frames < 300 && total_ticks < 100_000_000 {
        emu.tick();
        total_ticks += 1;
        if let Some(samples) = emu.take_audio_samples() {
            for &s in &samples {
                samples_collected += 1;
                if s != 0 {
                    nonzero_samples += 1;
                }
            }
        }
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    println!("=== At frame {} ===", frames);
    println!(
        "Samples so far: {} (nonzero: {} = {:.1}%)",
        samples_collected,
        nonzero_samples,
        nonzero_samples as f64 / samples_collected.max(1) as f64 * 100.0
    );

    // Dump full PSG state
    let main_bal = emu.bus.psg_main_balance();
    println!(
        "\nMain balance: ${:02X} (L={}, R={})",
        main_bal,
        (main_bal >> 4) & 0x0F,
        main_bal & 0x0F
    );

    for ch in 0..6 {
        let (freq, control, balance, noise_ctrl) = emu.bus.psg_channel_info(ch);
        let (wave_pos, wave_write_pos, phase, phase_step, dda) = emu.bus.psg_channel_detail(ch);
        let key_on = (control & 0x80) != 0;
        let dda_mode = (control & 0x40) != 0;
        let volume = control & 0x1F;
        let noise_en = (noise_ctrl & 0x80) != 0;

        let hz = if freq == 0 {
            0.0
        } else {
            3_579_545.0 / (32.0 * freq as f64)
        };

        println!(
            "\nCH{}: freq=${:03X}({:.1}Hz) ctrl=${:02X}(key={},dda={},vol={}) bal=${:02X} noise={}",
            ch, freq, hz, control, key_on, dda_mode, volume, balance, noise_en
        );
        println!(
            "  wave_pos={} write_pos={} phase={} step={} dda_val={}",
            wave_pos, wave_write_pos, phase, phase_step, dda
        );

        // Dump waveform
        let wave = emu.bus.psg_waveform(ch);
        print!("  waveform: ");
        for (i, &v) in wave.iter().enumerate() {
            print!("{:02X}", v);
            if i % 16 == 15 {
                print!("\n            ");
            } else {
                print!(" ");
            }
        }
        println!();

        // Check if waveform is all the same value
        let all_same = wave.iter().all(|&v| v == wave[0]);
        if all_same {
            println!("  WARNING: waveform is constant (all ${:02X})", wave[0]);
        }

        // Simulate what sample_channel would return
        if key_on {
            let raw = if dda_mode {
                dda as i32 - 0x10
            } else if ch >= 4 && noise_en {
                0x0F // depends on LFSR
            } else {
                let wave_val = wave[wave_pos as usize & 31];
                wave_val as i32 - 0x10
            };
            let ch_left = ((balance >> 4) & 0x0F) as i32;
            let ch_right = (balance & 0x0F) as i32;
            let master_left = ((main_bal >> 4) & 0x0F) as i32;
            let master_right = (main_bal & 0x0F) as i32;
            let left = raw * volume as i32 * ch_left * master_left;
            let right = raw * volume as i32 * ch_right * master_right;
            let sample = (left + right) / 2;
            println!(
                "  raw={} vol={} ch_L={} ch_R={} master_L={} master_R={}",
                raw, volume, ch_left, ch_right, master_left, master_right
            );
            println!("  left={} right={} mixed={}", left, right, sample);
        }
    }

    // Now generate a single sample and trace it
    println!("\n=== Generating one sample ===");
    let sample = emu.bus.psg_sample();
    println!("Sample: {}", sample);

    // Generate 10 more samples
    print!("Next 10: ");
    for _ in 0..10 {
        emu.tick();
        total_ticks += 1;
        if let Some(samples) = emu.take_audio_samples() {
            for &s in &samples {
                print!("{} ", s);
            }
        }
    }
    println!();

    // Run one more frame and collect ALL samples
    println!("\n=== Next frame audio ===");
    let mut frame_samples: Vec<i16> = Vec::new();
    let start_frame = frames;
    while frames == start_frame && total_ticks < 100_000_000 {
        emu.tick();
        total_ticks += 1;
        if let Some(samples) = emu.take_audio_samples() {
            for &s in &samples {
                frame_samples.push(s);
            }
        }
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    let nonzero = frame_samples.iter().filter(|&&s| s != 0).count();
    let min = frame_samples.iter().copied().min().unwrap_or(0);
    let max = frame_samples.iter().copied().max().unwrap_or(0);
    println!(
        "Frame {} samples: {} (nonzero: {}, min={}, max={})",
        frames,
        frame_samples.len(),
        nonzero,
        min,
        max
    );
    print!("First 32: ");
    for &s in frame_samples.iter().take(32) {
        print!("{} ", s);
    }
    println!();

    Ok(())
}
