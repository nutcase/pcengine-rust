use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let max_ticks = 80_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut prev_pc = 0u16;

    // Measure phi_cycles per frame
    let mut frame_start_cycles = 0u64;

    // Track sound driver calls - scan for common sound entry points
    let mut jsr_counts: std::collections::HashMap<u16, u64> = std::collections::HashMap::new();

    // Track VBlank ISR entries
    let mut vbl_isr_addr = 0u16;
    let mut vbl_isr_entries = 0u64;

    // Track timer IRQ handler
    let mut timer_vector = 0u16;
    let mut timer_irq_count = 0u64;

    // Vectors
    let mut vectors_read = false;

    // Track PSG frequency changes
    let mut prev_freq = [0u16; 6];
    let mut freq_changes: Vec<(u64, usize, u16, u16)> = Vec::new();

    // Track PSG control/volume changes
    let mut prev_ctrl = [0u8; 6];

    // Track specific PC addresses related to sound
    let mut pc_histogram: std::collections::HashMap<u16, u64> = std::collections::HashMap::new();
    let mut in_range_d000_dfff = false;
    let mut in_range_4000_5fff = false;

    while frames < 600 && total_ticks < max_ticks {
        let current_pc = emu.cpu.pc;
        emu.tick();
        total_ticks += 1;

        // Read vectors after boot
        if !vectors_read && frames >= 5 {
            timer_vector = emu.bus.read(0xFFFA) as u16 | ((emu.bus.read(0xFFFB) as u16) << 8);
            vbl_isr_addr = emu.bus.read(0xFFF8) as u16 | ((emu.bus.read(0xFFF9) as u16) << 8);
            let irq2_vector = emu.bus.read(0xFFF6) as u16 | ((emu.bus.read(0xFFF7) as u16) << 8);
            let reset_vector = emu.bus.read(0xFFFE) as u16 | ((emu.bus.read(0xFFFF) as u16) << 8);
            println!("=== PL'93 Interrupt Vectors ===");
            println!("  Timer ($FFFA): ${:04X}", timer_vector);
            println!("  IRQ1  ($FFF8): ${:04X}", vbl_isr_addr);
            println!("  IRQ2  ($FFF6): ${:04X}", irq2_vector);
            println!("  Reset ($FFFE): ${:04X}", reset_vector);
            vectors_read = true;
        }

        // Detect VBL ISR entry
        if vectors_read && emu.cpu.pc == vbl_isr_addr && prev_pc != vbl_isr_addr {
            vbl_isr_entries += 1;
        }

        // Detect timer vector entry
        if vectors_read
            && emu.cpu.pc == timer_vector
            && prev_pc != timer_vector
            && timer_vector != vbl_isr_addr
        {
            timer_irq_count += 1;
            if timer_irq_count <= 5 {
                println!(
                    "[frame {:3} tick {:8}] Timer IRQ #{}",
                    frames, total_ticks, timer_irq_count
                );
            }
        }

        // Track sound-related address ranges
        if emu.cpu.pc >= 0xD000 && emu.cpu.pc < 0xE000 && prev_pc < 0xD000 {
            if !in_range_d000_dfff {
                *jsr_counts.entry(emu.cpu.pc).or_insert(0) += 1;
                in_range_d000_dfff = true;
            }
        } else if emu.cpu.pc < 0xD000 || emu.cpu.pc >= 0xE000 {
            in_range_d000_dfff = false;
        }

        if emu.cpu.pc >= 0x4000 && emu.cpu.pc < 0x6000 && prev_pc < 0x4000 {
            if !in_range_4000_5fff {
                *jsr_counts.entry(emu.cpu.pc).or_insert(0) += 1;
                in_range_4000_5fff = true;
            }
        } else if emu.cpu.pc < 0x4000 || emu.cpu.pc >= 0x6000 {
            in_range_4000_5fff = false;
        }

        prev_pc = current_pc;

        if emu.take_frame().is_some() {
            frames += 1;

            // Track frequency changes
            for ch in 0..6 {
                let (freq, ctrl, _, _) = emu.bus.psg_channel_info(ch);
                if freq != prev_freq[ch] && frames > 1 {
                    freq_changes.push((frames, ch, prev_freq[ch], freq));
                }
                prev_freq[ch] = freq;

                if (ctrl & 0x80) != (prev_ctrl[ch] & 0x80) && frames > 1 {
                    // Key on/off change
                    if frames <= 100 || (frames >= 300 && frames <= 320) {
                        let on = ctrl & 0x80 != 0;
                        println!(
                            "Frame {:3}: CH{} key {}",
                            frames,
                            ch,
                            if on { "ON" } else { "OFF" }
                        );
                    }
                }
                prev_ctrl[ch] = ctrl;
            }

            let cycle_count = emu.cycles() - frame_start_cycles;
            frame_start_cycles = emu.cycles();

            if frames == 1 || frames % 60 == 0 {
                println!(
                    "Frame {:3}: cycles/frame={}, timer_irqs={}, vbl_isr={}",
                    frames, cycle_count, timer_irq_count, vbl_isr_entries
                );
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    // Summary
    println!(
        "\n=== PL'93 Timing Summary ({} frames, {} total cycles) ===",
        frames,
        emu.cycles()
    );
    let avg_cycles = emu.cycles() as f64 / frames as f64;
    println!("Avg cycles/frame: {:.1}", avg_cycles);

    let total_audio_samples = (emu.cycles() as f64 * 44_100.0 / 7_159_090.0) as u64;
    let duration_sec = total_audio_samples as f64 / 44_100.0;
    println!("Audio duration: {:.3}s for {} frames", duration_sec, frames);
    println!(
        "Effective frame rate: {:.3} Hz",
        frames as f64 / duration_sec
    );

    println!(
        "\nVBL ISR entries: {} ({:.2}/frame)",
        vbl_isr_entries,
        vbl_isr_entries as f64 / frames as f64
    );
    println!(
        "Timer IRQs: {} ({:.4}/frame)",
        timer_irq_count,
        timer_irq_count as f64 / frames as f64
    );

    // Most frequent sound entry points
    let mut sorted_entries: Vec<_> = jsr_counts.into_iter().collect();
    sorted_entries.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\n=== Most frequent entries to $D000-DFFF and $4000-5FFF ===");
    for &(addr, count) in sorted_entries.iter().take(15) {
        println!(
            "  ${:04X}: {} calls ({:.2}/frame)",
            addr,
            count,
            count as f64 / frames as f64
        );
    }

    // Note change intervals
    println!("\n=== Note Change Intervals (BPM analysis) ===");
    for ch in 0..6 {
        let changes: Vec<u64> = freq_changes
            .iter()
            .filter(|&&(f, c, _, _)| c == ch && f >= 300)
            .map(|&(f, _, _, _)| f)
            .collect();
        if changes.len() >= 2 {
            let intervals: Vec<u64> = changes.windows(2).map(|w| w[1] - w[0]).collect();
            let avg_frames = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
            let avg_seconds = avg_frames / 60.0;
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
            } else {
                println!("    First 20: {:?}", &intervals[..20]);
            }
        } else {
            println!("  CH{}: {} changes (insufficient data)", ch, changes.len());
        }
    }

    // Frequency values for first few notes per channel (frames >= 300)
    println!("\n=== First 20 freq changes per channel (frame >= 300) ===");
    for ch in 0..6 {
        let ch_changes: Vec<_> = freq_changes
            .iter()
            .filter(|&&(f, c, _, _)| c == ch && f >= 300)
            .take(20)
            .collect();
        if !ch_changes.is_empty() {
            print!("  CH{}: ", ch);
            for &&(f, _, old, new) in &ch_changes {
                let hz = if new > 0 {
                    3_579_545.0 / (32.0 * new as f64)
                } else {
                    0.0
                };
                print!("f{}:{}->{} ({:.0}Hz) ", f, old, new, hz);
            }
            println!();
        }
    }

    // Timer state
    println!("\n=== Timer State ===");
    println!("  Timer counter: {}", emu.bus.read_io(0x0C00));
    println!(
        "  Timer control: ${:02X} (enabled={})",
        emu.bus.read_io(0x0C01),
        emu.bus.read_io(0x0C01) & 1 != 0
    );
    println!("  IRQ disable: ${:02X}", emu.bus.read_io(0x1402));

    // Dump work RAM area (check for sound driver state)
    println!("\n=== RAM $2000-$2080 ===");
    for row in 0..8 {
        let base = 0x2000u16 + (row * 16) as u16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col as u16));
        }
        println!();
    }

    println!("\n=== RAM $2A00-$2A80 ===");
    for row in 0..8 {
        let base = 0x2A00u16 + (row * 16) as u16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col as u16));
        }
        println!();
    }

    Ok(())
}
