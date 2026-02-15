use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let max_ticks = 5_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut timer_irq_count = 0u64;
    let mut irq1_count = 0u64;
    let mut last_timer_reload = 0u8;
    let mut last_timer_enabled = false;

    // Track IRQ vectors
    let mut reported_vectors = false;

    while total_ticks < max_ticks {
        let prev_pc = emu.cpu.pc;
        let cycles = emu.tick();

        if emu.take_frame().is_some() {
            frames += 1;
        }

        // Detect timer ISR entry (PC jumps to timer vector)
        let timer_vector = emu.bus.read_u16(0xFFFA);
        let irq1_vector = emu.bus.read_u16(0xFFF8);
        let irq2_vector = emu.bus.read_u16(0xFFF6);

        if !reported_vectors && frames >= 1 {
            println!("IRQ vectors after frame 1:");
            println!("  Timer ($FFFA): ${:04X}", timer_vector);
            println!("  IRQ1  ($FFF8): ${:04X}", irq1_vector);
            println!("  IRQ2  ($FFF6): ${:04X}", irq2_vector);

            // Read timer state via I/O
            // Timer reload at $0C00, control at $0C01
            let timer_counter = emu.bus.read(0x0C00);
            let timer_control = emu.bus.read(0x0C01);
            println!(
                "  Timer counter: ${:02X} ({})",
                timer_counter, timer_counter
            );
            println!("  Timer control: ${:02X}", timer_control);

            // Read IRQ mask
            let irq_mask = emu.bus.read(0x1402);
            let irq_status = emu.bus.read(0x1403);
            println!(
                "  IRQ mask:   ${:02X} (disable bits: IRQ2={} IRQ1={} Timer={})",
                irq_mask,
                irq_mask & 0x01 != 0,
                irq_mask & 0x02 != 0,
                irq_mask & 0x04 != 0
            );
            println!("  IRQ status: ${:02X}", irq_status);
            println!(
                "  CPU speed: {}",
                if emu.cpu.clock_high_speed {
                    "HIGH"
                } else {
                    "LOW"
                }
            );
            reported_vectors = true;
        }

        // Count timer ISR entries
        if emu.cpu.pc == timer_vector
            && prev_pc != timer_vector
            && timer_vector != 0
            && timer_vector != 0xFFFF
        {
            timer_irq_count += 1;
            if timer_irq_count <= 5 {
                println!(
                    "Timer IRQ #{} at tick {} frame {} from PC=${:04X}",
                    timer_irq_count, total_ticks, frames, prev_pc
                );
            }
        }

        // Count IRQ1 ISR entries
        if emu.cpu.pc == irq1_vector
            && prev_pc != irq1_vector
            && irq1_vector != 0
            && irq1_vector != 0xFFFF
        {
            irq1_count += 1;
            if irq1_count <= 5 {
                println!(
                    "IRQ1 #{} at tick {} frame {} from PC=${:04X}",
                    irq1_count, total_ticks, frames, prev_pc
                );
            }
        }

        total_ticks += 1;

        if emu.cpu.halted {
            println!("CPU halted at tick {}", total_ticks);
            break;
        }
    }

    println!("\n=== IRQ Summary ({} frames) ===", frames);
    println!("Timer IRQ count: {}", timer_irq_count);
    println!("IRQ1 (VBlank) count: {}", irq1_count);
    if frames > 0 {
        println!(
            "Timer IRQs per frame: {:.2}",
            timer_irq_count as f64 / frames as f64
        );
        println!("IRQ1 per frame: {:.2}", irq1_count as f64 / frames as f64);
    }

    Ok(())
}
