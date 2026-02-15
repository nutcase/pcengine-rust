use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let max_ticks = 80_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut prev_pc = 0u16;

    // Measure phi_cycles per frame
    let mut frame_start_cycles = 0u64;
    let mut frame_cycle_counts: Vec<u64> = Vec::new();

    // Count audio samples per frame
    let mut audio_samples_total = 0u64;
    let mut audio_at_frame_start = 0u64;
    let mut samples_per_frame: Vec<u64> = Vec::new();

    // Track sound driver calls
    let mut sound_driver_calls = 0u64;
    let mut sd_calls_per_frame: Vec<u64> = Vec::new();
    let mut sd_at_frame_start = 0u64;

    // Track VBlank ISR entries
    let mut vbl_isr_entries = 0u64;

    // Track timer IRQ handler
    let mut timer_irq_count = 0u64;

    // Read vectors after boot
    let mut timer_vector = 0u16;
    let mut irq1_vector = 0u16;
    let mut vectors_read = false;

    // Track frequency changes with audio sample position
    let mut prev_freq = [0u16; 6];
    let mut freq_changes: Vec<(u64, u64, usize, u16, u16)> = Vec::new(); // (frame, audio_sample, ch, old, new)

    // Track tick counter at $2A19
    let mut prev_tick_ch0 = 0u8;
    let mut tick_change_frames: Vec<(u64, u8, u8)> = Vec::new(); // (frame, old, new)

    // Run until boot completes (frame 60) then read vectors
    while frames < 450 && total_ticks < max_ticks {
        let current_pc = emu.cpu.pc;
        emu.tick();
        total_ticks += 1;

        // Count audio samples
        // Audio is generated in bus.tick(), we track via the accumulator
        // Actually we need to count samples from the emulator's buffer

        // Detect sound driver entry at $D094
        if emu.cpu.pc == 0xD094 && prev_pc != 0xD094 {
            sound_driver_calls += 1;
        }

        // Detect VBlank ISR entry at $FB83
        if emu.cpu.pc == 0xFB83 && prev_pc != 0xFB83 {
            vbl_isr_entries += 1;
        }

        // Read vectors after boot
        if !vectors_read && frames >= 10 {
            timer_vector = emu.bus.read(0xFFFA) as u16 | ((emu.bus.read(0xFFFB) as u16) << 8);
            irq1_vector = emu.bus.read(0xFFF8) as u16 | ((emu.bus.read(0xFFF9) as u16) << 8);
            let irq2_vector = emu.bus.read(0xFFF6) as u16 | ((emu.bus.read(0xFFF7) as u16) << 8);
            let nmi_vector = emu.bus.read(0xFFFC) as u16 | ((emu.bus.read(0xFFFD) as u16) << 8);
            let reset_vector = emu.bus.read(0xFFFE) as u16 | ((emu.bus.read(0xFFFF) as u16) << 8);
            println!("=== Interrupt Vectors ===");
            println!("  Timer ($FFFA): ${:04X}", timer_vector);
            println!("  IRQ1  ($FFF8): ${:04X}", irq1_vector);
            println!("  IRQ2  ($FFF6): ${:04X}", irq2_vector);
            println!("  NMI   ($FFFC): ${:04X}", nmi_vector);
            println!("  Reset ($FFFE): ${:04X}", reset_vector);
            vectors_read = true;
        }

        // Detect timer vector entry
        if vectors_read && emu.cpu.pc == timer_vector && prev_pc != timer_vector {
            timer_irq_count += 1;
        }

        prev_pc = current_pc;

        if emu.take_frame().is_some() {
            frames += 1;

            let cycle_count = emu.cycles() - frame_start_cycles;
            frame_cycle_counts.push(cycle_count);
            frame_start_cycles = emu.cycles();

            let sd_this_frame = sound_driver_calls - sd_at_frame_start;
            sd_calls_per_frame.push(sd_this_frame);
            sd_at_frame_start = sound_driver_calls;

            // Count audio samples (approximate: from the emulator's audio buffer drain)
            // We can't directly count, but we'll estimate from cycles
            // Actually, let's just count total and compare at the end.

            // Track frequency changes
            for ch in 0..6 {
                let (freq, _, _, _) = emu.bus.psg_channel_info(ch);
                if freq != prev_freq[ch] && frames > 1 {
                    freq_changes.push((frames, total_ticks, ch, prev_freq[ch], freq));
                }
                prev_freq[ch] = freq;
            }

            // Track tick counter for CH0
            if frames >= 300 {
                let tick_ch0 = emu.bus.read(0x2A19);
                if tick_ch0 != prev_tick_ch0 {
                    tick_change_frames.push((frames, prev_tick_ch0, tick_ch0));
                }
                prev_tick_ch0 = tick_ch0;
            }

            // Print periodic status
            if frames == 1 || frames % 60 == 0 {
                println!(
                    "Frame {:3}: cycles/frame={}, sd_calls/frame={}, timer_irqs={}, vbl_isr={}",
                    frames, cycle_count, sd_this_frame, timer_irq_count, vbl_isr_entries
                );
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    // Final statistics
    println!(
        "\n=== Timing Summary ({} frames, {} total cycles) ===",
        frames,
        emu.cycles()
    );
    let avg_cycles = emu.cycles() as f64 / frames as f64;
    println!("Avg cycles/frame: {:.1}", avg_cycles);
    println!(
        "Expected cycles/frame (59.82 Hz): {:.1}",
        7_159_090.0 / 59.82
    );
    println!(
        "Expected cycles/frame (60.00 Hz): {:.1}",
        7_159_090.0 / 60.0
    );

    // Audio timing
    let total_audio_samples = (emu.cycles() as f64 * 44_100.0 / 7_159_090.0) as u64;
    let duration_sec = total_audio_samples as f64 / 44_100.0;
    println!("\nEstimated audio samples: {}", total_audio_samples);
    println!("Duration: {:.3}s for {} frames", duration_sec, frames);
    println!(
        "Effective frame rate: {:.3} Hz",
        frames as f64 / duration_sec
    );

    // Sound driver statistics
    println!("\nSound driver ($D094) total calls: {}", sound_driver_calls);
    println!(
        "Avg calls/frame: {:.2}",
        sound_driver_calls as f64 / frames as f64
    );
    println!("VBL ISR ($FB83) entries: {}", vbl_isr_entries);
    println!(
        "Timer IRQ (${:04X}) entries: {}",
        timer_vector, timer_irq_count
    );

    // Frame cycle distribution
    if !frame_cycle_counts.is_empty() {
        let min = frame_cycle_counts.iter().min().unwrap();
        let max = frame_cycle_counts.iter().max().unwrap();
        let avg = frame_cycle_counts.iter().sum::<u64>() as f64 / frame_cycle_counts.len() as f64;
        println!(
            "\nCycles per frame: min={}, max={}, avg={:.1}",
            min, max, avg
        );
    }

    // SD calls distribution
    let sd_zeros = sd_calls_per_frame.iter().filter(|&&x| x == 0).count();
    let sd_ones = sd_calls_per_frame.iter().filter(|&&x| x == 1).count();
    let sd_twos = sd_calls_per_frame.iter().filter(|&&x| x >= 2).count();
    println!(
        "SD calls distribution: 0_per_frame={}, 1_per_frame={}, 2+_per_frame={}",
        sd_zeros, sd_ones, sd_twos
    );

    // Frequency change intervals (BPM calculation)
    println!("\n=== Note Change Intervals (BPM analysis) ===");
    for ch in 0..6 {
        let changes: Vec<u64> = freq_changes
            .iter()
            .filter(|&&(f, _, c, _, _)| c == ch && f >= 310)
            .map(|&(f, _, _, _, _)| f)
            .collect();
        if changes.len() >= 2 {
            let intervals: Vec<u64> = changes.windows(2).map(|w| w[1] - w[0]).collect();
            let avg_frames = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
            let avg_seconds = avg_frames / 60.0; // Assuming 60 Hz
            let bpm = 60.0 / avg_seconds;
            println!(
                "  CH{}: {} notes, avg interval={:.1} frames ({:.2}s), ~{:.1} BPM",
                ch,
                changes.len(),
                avg_frames,
                avg_seconds,
                bpm
            );
            if intervals.len() <= 20 {
                println!("    Intervals: {:?}", intervals);
            }
        }
    }

    // Tick counter changes (CH0)
    println!("\n=== CH0 Tick Counter Changes (frames 300-450) ===");
    for &(frame, old, new) in &tick_change_frames {
        if frame <= 370 {
            println!("  Frame {}: {} -> {}", frame, old, new);
        }
    }

    // Dump timer state
    println!("\n=== Timer State ===");
    println!("  Timer counter: {}", emu.bus.read_io(0x0C00));
    println!(
        "  Timer control: ${:02X} (enabled={})",
        emu.bus.read_io(0x0C01),
        emu.bus.read_io(0x0C01) & 1 != 0
    );
    println!("  IRQ disable: ${:02X}", emu.bus.read_io(0x1402));
    println!("  IRQ status: ${:02X}", emu.bus.read_io(0x1403));

    Ok(())
}
