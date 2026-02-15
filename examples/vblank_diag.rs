/// VBlank timing diagnostic: measures CPU cycles per frame,
/// IRQ delivery rates, and music activity.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run 200 frames to let the game initialize
    let mut frames = 0u64;
    let mut total_cycles = 0u64;
    while frames < 200 {
        let c = emu.tick();
        total_cycles += c as u64;
        if c == 0 && emu.cpu.is_waiting() {
            total_cycles += 1;
        }
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            eprintln!("CPU halted at frame {}", frames);
            return Ok(());
        }
    }

    println!("=== VBlank Timing Diagnostic ===");
    println!("ROM: {}", rom_path);
    println!("CPU high_speed: {}", emu.cpu.clock_high_speed);

    let (reload, _counter, enabled, _prescaler) = emu.bus.timer_info();
    println!("Timer: enabled={} reload={}", enabled, reload);
    if enabled {
        let period_phi = 1024 * (reload as u64 + 1);
        let fire_rate = 7_159_090.0 / period_phi as f64;
        println!(
            "  Expected timer rate: {:.1} Hz ({} phi per tick)",
            fire_rate, period_phi
        );
    }

    // Read ISR vectors
    let vdc_isr = {
        let lo = emu.bus.read(0xFFF8) as u16;
        let hi = emu.bus.read(0xFFF9) as u16;
        hi << 8 | lo
    };
    let timer_isr = {
        let lo = emu.bus.read(0xFFFA) as u16;
        let hi = emu.bus.read(0xFFFB) as u16;
        hi << 8 | lo
    };
    println!("VDC ISR vector: ${:04X}", vdc_isr);
    println!("Timer ISR vector: ${:04X}", timer_isr);

    // Measure 120 frames (~2 seconds)
    let measure_frames = 120u64;
    let start_cycles = total_cycles;
    let start_frame = frames;

    let mut frame_cycles = Vec::new();
    let mut frame_start_cycle = total_cycles;
    let mut prev_freqs = [0u16; 6];
    let mut total_freq_changes = 0u64;

    // Count ISR entries
    let mut vdc_isr_count = 0u64;
    let mut timer_isr_count = 0u64;
    let mut other_isr_count = 0u64;
    let mut prev_pc = emu.cpu.pc;

    // Track timer IRQ request bit transitions
    let mut timer_irq_fires = 0u64;
    let mut prev_irq_request = emu.bus.irq_state().1;

    while frames < start_frame + measure_frames {
        prev_pc = emu.cpu.pc;
        let c = emu.tick();
        let new_pc = emu.cpu.pc;
        total_cycles += c as u64;
        if c == 0 && emu.cpu.is_waiting() {
            total_cycles += 1;
        }

        // Detect ISR entry: PC was somewhere and jumped to an ISR vector
        // The CPU pushes PC+? and flags, then loads the vector
        if new_pc == vdc_isr && prev_pc != vdc_isr && prev_pc != vdc_isr.wrapping_sub(1) {
            vdc_isr_count += 1;
        }
        if new_pc == timer_isr && prev_pc != timer_isr && prev_pc != timer_isr.wrapping_sub(1) {
            timer_isr_count += 1;
        }

        // Track timer IRQ request bit
        let (_, irq_req) = emu.bus.irq_state();
        if (irq_req & 0x04) != 0 && (prev_irq_request & 0x04) == 0 {
            timer_irq_fires += 1;
        }
        prev_irq_request = irq_req;

        if emu.take_frame().is_some() {
            frames += 1;
            let fc = total_cycles - frame_start_cycle;
            frame_cycles.push(fc);
            frame_start_cycle = total_cycles;

            // Check PSG changes
            for ch in 0..6 {
                let (freq, control, _, _) = emu.bus.psg_channel_info(ch);
                let key_on = (control & 0x80) != 0;
                if key_on && freq != prev_freqs[ch] {
                    total_freq_changes += 1;
                }
                prev_freqs[ch] = freq;
            }
        }
        if emu.cpu.halted {
            break;
        }
    }

    let elapsed_cycles = total_cycles - start_cycles;
    let elapsed_frames = frames - start_frame;
    let phi_per_frame = elapsed_cycles as f64 / elapsed_frames as f64;
    let elapsed_seconds = elapsed_frames as f64 / 60.0;

    println!(
        "\n=== Results over {} frames ({:.2}s) ===",
        elapsed_frames, elapsed_seconds
    );
    println!("Total CPU cycles: {}", elapsed_cycles);
    println!("Avg cycles/frame: {:.1}", phi_per_frame);
    println!("Implied frame rate: {:.2} Hz", 7_159_090.0 / phi_per_frame);

    // Frame cycle stats
    if !frame_cycles.is_empty() {
        let min = *frame_cycles.iter().min().unwrap();
        let max = *frame_cycles.iter().max().unwrap();
        let mean = frame_cycles.iter().sum::<u64>() as f64 / frame_cycles.len() as f64;
        println!(
            "\nFrame cycle stats: min={} max={} mean={:.1}",
            min, max, mean
        );
    }

    // IRQ stats
    println!("\n=== IRQ Stats ===");
    println!(
        "VDC ISR entries:   {} ({:.1}/frame, {:.1}/sec)",
        vdc_isr_count,
        vdc_isr_count as f64 / elapsed_frames as f64,
        vdc_isr_count as f64 / elapsed_seconds
    );
    println!(
        "Timer ISR entries: {} ({:.1}/frame, {:.1}/sec)",
        timer_isr_count,
        timer_isr_count as f64 / elapsed_frames as f64,
        timer_isr_count as f64 / elapsed_seconds
    );
    println!(
        "Timer IRQ fires:   {} ({:.1}/frame, {:.1}/sec)",
        timer_irq_fires,
        timer_irq_fires as f64 / elapsed_frames as f64,
        timer_irq_fires as f64 / elapsed_seconds
    );

    if enabled {
        let expected_rate = 7_159_090.0 / (1024.0 * (reload as f64 + 1.0));
        println!("\nExpected timer rate: {:.1}/sec", expected_rate);
        println!(
            "Actual timer fires:  {:.1}/sec",
            timer_irq_fires as f64 / elapsed_seconds
        );
        if timer_irq_fires > 0 {
            println!(
                "Ratio: {:.4}",
                (timer_irq_fires as f64 / elapsed_seconds) / expected_rate
            );
        }
    }

    // PSG activity
    let freq_per_sec = total_freq_changes as f64 / elapsed_seconds;
    println!(
        "\nPSG freq changes: {} total ({:.1}/sec)",
        total_freq_changes, freq_per_sec
    );

    // Final state
    println!("\nFinal CPU high_speed: {}", emu.cpu.clock_high_speed);
    let (reload, counter, enabled, prescaler) = emu.bus.timer_info();
    println!(
        "Final timer: reload={} counter={} enabled={} prescaler={}",
        reload, counter, enabled, prescaler
    );
    let (irq_disable, irq_request) = emu.bus.irq_state();
    println!(
        "IRQ: disable=${:02X} request=${:02X}",
        irq_disable, irq_request
    );
    println!("  Timer IRQ enabled: {}", (irq_disable & 0x04) == 0);
    println!("  IRQ1 enabled:      {}", (irq_disable & 0x02) == 0);

    Ok(())
}
