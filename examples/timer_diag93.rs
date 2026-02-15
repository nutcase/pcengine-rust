use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run to frame 200 to let the game initialize and start music
    let mut frames = 0u64;
    let mut total_ticks = 0u64;
    while frames < 200 && total_ticks < 100_000_000 {
        emu.tick();
        total_ticks += 1;
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Now measure timer state and IRQ rates over the next 60 frames (1 second)
    println!("=== Timer Diagnostic at frame {} ===", frames);
    let (reload, counter, enabled, prescaler) = emu.bus.timer_info();
    let (irq_disable, irq_request) = emu.bus.irq_state();
    println!(
        "Timer: reload={} counter={} enabled={} prescaler={}",
        reload, counter, enabled, prescaler
    );
    println!(
        "IRQ: disable_mask=${:02X} request=${:02X}",
        irq_disable, irq_request
    );

    if enabled {
        let period_phi = 1024 * (reload as u64 + 1);
        let fire_rate = 7_159_090.0 / period_phi as f64;
        println!(
            "Expected timer fire rate: {:.1} Hz (period={}+1={} * 1024 = {} phi cycles)",
            fire_rate,
            reload,
            reload as u64 + 1,
            period_phi
        );
    } else {
        println!("Timer is DISABLED");
    }

    // Count timer IRQs over 60 frames
    let mut timer_irqs = 0u64;
    let mut vblank_irqs = 0u64;
    let start_cycles = total_ticks;
    let target_frames = frames + 60;

    // Snapshot PSG channel registers at start
    for ch in 0..6 {
        let (freq, control, balance, noise_ctrl) = emu.bus.psg_channel_info(ch);
        let key_on = (control & 0x80) != 0;
        let dda = (control & 0x40) != 0;
        let volume = control & 0x1F;
        let hz = if freq == 0 {
            0.0
        } else {
            3_579_545.0 / (32.0 * freq as f64)
        };
        println!(
            "  CH{}: freq=${:03X}({:.1}Hz) vol={} key={} dda={} bal=${:02X} noise=${:02X}",
            ch, freq, hz, volume, key_on, dda, balance, noise_ctrl
        );
    }

    // Track timer reload changes (indicates music driver activity)
    let mut last_reload = reload;
    let mut reload_changes = 0u64;
    let mut prev_counter = counter;
    let mut counter_wraps = 0u64;

    while frames < target_frames && total_ticks < 200_000_000 {
        // Check timer before tick to detect IRQ firing
        let (_, old_counter, _, _) = emu.bus.timer_info();
        let (_, old_irq) = emu.bus.irq_state();

        emu.tick();
        total_ticks += 1;

        let (new_reload, new_counter, _, _) = emu.bus.timer_info();
        let (_, new_irq) = emu.bus.irq_state();

        // Detect timer IRQ: request bit newly set
        if (new_irq & 0x04) != 0 && (old_irq & 0x04) == 0 {
            timer_irqs += 1;
        }

        // Detect VBlank IRQ would need VDC status checking...
        // Instead just count frames
        if emu.take_frame().is_some() {
            frames += 1;
            vblank_irqs += 1;
        }

        // Track reload changes
        if new_reload != last_reload {
            reload_changes += 1;
            last_reload = new_reload;
        }

        // Track counter wraps (counter goes from 0 -> reload)
        if new_counter > old_counter && old_counter == 0 {
            counter_wraps += 1;
        }

        if emu.cpu.halted {
            break;
        }
    }

    let elapsed_cycles = total_ticks - start_cycles;
    let elapsed_seconds = elapsed_cycles as f64 * 4.0 / 7_159_090.0; // approx

    println!("\n=== 60-frame measurement ===");
    println!(
        "CPU cycles: {} ({:.3}s approx)",
        elapsed_cycles, elapsed_seconds
    );
    println!(
        "Timer IRQs detected: {} ({:.1}/sec)",
        timer_irqs,
        timer_irqs as f64 / elapsed_seconds
    );
    println!("VBlank frames: {}", vblank_irqs);
    println!("Timer reload changes: {}", reload_changes);
    println!("Counter wraps: {}", counter_wraps);

    // Final timer state
    let (reload, counter, enabled, prescaler) = emu.bus.timer_info();
    let (irq_disable, _) = emu.bus.irq_state();
    println!(
        "\nFinal timer: reload={} counter={} enabled={} prescaler={}",
        reload, counter, enabled, prescaler
    );
    println!("Timer IRQ enabled in mask: {}", (irq_disable & 0x04) == 0);

    if timer_irqs > 0 {
        let measured_rate = timer_irqs as f64 / elapsed_seconds;
        let expected_rate = 7_159_090.0 / (1024.0 * (reload as f64 + 1.0));
        println!("\nMeasured timer rate: {:.1} Hz", measured_rate);
        println!("Expected timer rate: {:.1} Hz", expected_rate);
        println!("Ratio: {:.4}", measured_rate / expected_rate);
    }

    // Also check CPU speed mode
    println!("\nCPU high_speed: {}", emu.cpu.clock_high_speed);

    Ok(())
}
