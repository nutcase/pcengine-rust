use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 600u64; // ~10 seconds
    let max_ticks = 100_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut timer_irq_count = 0u64;
    let mut irq1_count = 0u64;
    let mut last_report_frame = 0u64;
    let mut last_timer_irq = 0u64;
    let mut last_irq1 = 0u64;

    // Track timer enable transitions
    let mut prev_timer_enabled = false;
    let mut timer_enable_events = Vec::new();

    while frames < target_frames && total_ticks < max_ticks {
        let prev_pc = emu.cpu.pc;
        emu.tick();
        total_ticks += 1;

        if emu.take_frame().is_some() {
            frames += 1;

            // Check timer state every frame
            let timer_control = emu.bus.read_io(0x0C01);
            let timer_counter = emu.bus.read_io(0x0C00);
            let timer_enabled = timer_control & 0x01 != 0;

            if timer_enabled != prev_timer_enabled {
                timer_enable_events.push((frames, timer_enabled, timer_counter));
                prev_timer_enabled = timer_enabled;
            }

            // Report every 60 frames
            if frames % 60 == 0 || frames == 1 || frames == 5 {
                let timer_irqs_this_period = timer_irq_count - last_timer_irq;
                let irq1s_this_period = irq1_count - last_irq1;

                println!("=== Frame {} ===", frames);
                println!(
                    "  Timer: enabled={}, counter={}, control=${:02X}",
                    timer_enabled, timer_counter, timer_control
                );
                let irq_mask = emu.bus.read_io(0x1402);
                println!(
                    "  IRQ mask: ${:02X} (timer_disabled={})",
                    irq_mask,
                    irq_mask & 0x04 != 0
                );
                println!(
                    "  IRQs this period: VBlank={}, Timer={}",
                    irq1s_this_period, timer_irqs_this_period
                );

                // Check PSG channels
                print!("  PSG channels active: ");
                for ch in 0..6 {
                    // Read PSG channel control
                    // PSG address port is at $0800, data at $0801
                    // Channel select at $0800, control at $0804
                    // We can read the direct registers
                    let ch_ctrl = emu.bus.read_io(0x0800 + 4 + 0); // This might not work
                    // Actually let's just print what we can
                }
                println!();

                last_timer_irq = timer_irq_count;
                last_irq1 = irq1_count;
                last_report_frame = frames;
            }
        }

        // Count IRQ entries by watching PC
        let timer_vector = emu.bus.read_u16(0xFFFA);
        let irq1_vector = emu.bus.read_u16(0xFFF8);

        if emu.cpu.pc == timer_vector
            && prev_pc != timer_vector
            && timer_vector != 0
            && timer_vector != 0xFFFF
        {
            timer_irq_count += 1;
            if timer_irq_count <= 10 {
                println!(
                    "  Timer IRQ #{} at frame {} tick {} from PC=${:04X}",
                    timer_irq_count, frames, total_ticks, prev_pc
                );
            }
        }

        if emu.cpu.pc == irq1_vector
            && prev_pc != irq1_vector
            && irq1_vector != 0
            && irq1_vector != 0xFFFF
        {
            irq1_count += 1;
        }

        if emu.cpu.halted {
            println!("CPU halted at tick {}", total_ticks);
            break;
        }
    }

    println!(
        "\n=== Summary ({} frames, {} ticks) ===",
        frames, total_ticks
    );
    println!("Total VBlank IRQs: {}", irq1_count);
    println!("Total Timer IRQs: {}", timer_irq_count);
    if frames > 0 {
        println!(
            "VBlank IRQs per frame: {:.2}",
            irq1_count as f64 / frames as f64
        );
        println!(
            "Timer IRQs per frame: {:.2}",
            timer_irq_count as f64 / frames as f64
        );
    }

    println!("\nTimer enable/disable events:");
    if timer_enable_events.is_empty() {
        println!("  (none - timer was never toggled)");
    }
    for (frame, enabled, counter) in &timer_enable_events {
        println!(
            "  Frame {}: {} (counter={})",
            frame,
            if *enabled { "ENABLED" } else { "DISABLED" },
            counter
        );
    }

    // Final timer state
    let timer_control = emu.bus.read_io(0x0C01);
    let timer_counter = emu.bus.read_io(0x0C00);
    println!(
        "\nFinal timer state: control=${:02X}, counter={}",
        timer_control, timer_counter
    );

    // Read IRQ vectors
    println!("\nIRQ vectors:");
    println!("  Timer ($FFFA): ${:04X}", emu.bus.read_u16(0xFFFA));
    println!("  IRQ1  ($FFF8): ${:04X}", emu.bus.read_u16(0xFFF8));

    Ok(())
}
