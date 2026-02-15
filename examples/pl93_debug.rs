use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let max_ticks = 30_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;

    // Track CPU state
    let mut in_wai = false;

    // Sample CPU state periodically
    let mut pc_histogram: std::collections::HashMap<u16, u64> = std::collections::HashMap::new();

    // Track VDC control register (register 5)
    let mut vdc_ctrl_checked = false;

    // Track MPR state
    let mut mpr_dumped = false;

    // Check interrupt state
    let mut irq_ever_pending = false;

    while frames < 200 && total_ticks < max_ticks {
        emu.tick();
        total_ticks += 1;

        // Track PC histogram (sample every tick)
        *pc_histogram.entry(emu.cpu.pc).or_insert(0) += 1;

        // Check WAI state
        if emu.cpu.is_waiting() && !in_wai {
            in_wai = true;
            if total_ticks < 1_000_000 {
                println!(
                    "[tick {:8} frame {:3}] CPU entered WAI at PC=${:04X}",
                    total_ticks, frames, emu.cpu.pc
                );
            }
        } else if !emu.cpu.is_waiting() && in_wai {
            in_wai = false;
        }

        // Check for pending IRQs
        if emu.bus.irq_pending() && !irq_ever_pending {
            irq_ever_pending = true;
            println!(
                "[tick {:8} frame {:3}] First IRQ pending! CPU I-flag={}",
                total_ticks,
                frames,
                emu.cpu.status & 0x04 != 0
            );
        }

        // Dump MPR and VDC state after boot
        if !mpr_dumped && frames >= 5 {
            mpr_dumped = true;
            let mpr = emu.bus.mpr_array();
            println!("\n=== MPR state at frame {} ===", frames);
            for i in 0..8 {
                println!(
                    "  MPR{}: ${:02X} (maps ${}000-${:X}FFF)",
                    i,
                    mpr[i],
                    i * 2,
                    i * 2 + 1
                );
            }

            println!("\n=== CPU state at frame {} ===", frames);
            println!("  PC: ${:04X}", emu.cpu.pc);
            println!(
                "  A: ${:02X} X: ${:02X} Y: ${:02X}",
                emu.cpu.a, emu.cpu.x, emu.cpu.y
            );
            println!("  SP: ${:02X}", emu.cpu.sp);
            println!(
                "  Status: ${:02X} (I-flag={})",
                emu.cpu.status,
                emu.cpu.status & 0x04 != 0
            );
            println!(
                "  Halted: {}, Waiting: {}",
                emu.cpu.halted,
                emu.cpu.is_waiting()
            );

            println!("\n=== IRQ state at frame {} ===", frames);
            println!(
                "  IRQ disable reg ($1402): ${:02X}",
                emu.bus.read_io(0x1402)
            );
            println!("  IRQ status reg ($1403): ${:02X}", emu.bus.read_io(0x1403));
            println!("  irq_pending(): {}", emu.bus.irq_pending());
        }

        if !vdc_ctrl_checked && frames >= 10 {
            vdc_ctrl_checked = true;
            let status = emu.bus.read_io(0x0000);
            println!("\n=== VDC status at frame {} ===", frames);
            println!(
                "  Status: ${:02X} (VBL={}, DS={}, DV={}, RCR={})",
                status,
                status & 0x20 != 0,
                status & 0x08 != 0,
                status & 0x10 != 0,
                status & 0x04 != 0
            );

            let irq_status = emu.bus.read_io(0x1403);
            println!("  IRQ status after VDC read: ${:02X}", irq_status);
        }

        if emu.take_frame().is_some() {
            frames += 1;

            if frames <= 3 || frames % 50 == 0 {
                let iflag = emu.cpu.status & 0x04 != 0;
                println!(
                    "Frame {:3}: PC=${:04X} Status=${:02X} I-flag={} waiting={}",
                    frames,
                    emu.cpu.pc,
                    emu.cpu.status,
                    iflag,
                    emu.cpu.is_waiting()
                );
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    // PC histogram - where does the CPU spend its time?
    let mut sorted_pcs: Vec<_> = pc_histogram.into_iter().collect();
    sorted_pcs.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\n=== Top 30 PC addresses by time spent ===");
    for &(pc, count) in sorted_pcs.iter().take(30) {
        let pct = count as f64 / total_ticks as f64 * 100.0;
        println!("  ${:04X}: {:8} ticks ({:.2}%)", pc, count, pct);
    }

    // Check for tight loops
    println!("\n=== Looking for tight loops (top PC clusters) ===");
    let mut clusters: Vec<(u16, u64)> = Vec::new();
    let sorted_pcs_copy: Vec<_> = sorted_pcs.iter().map(|&(pc, count)| (pc, count)).collect();
    for &(pc, count) in sorted_pcs_copy.iter().take(50) {
        let nearby_total: u64 = sorted_pcs_copy
            .iter()
            .filter(|&&(p, _)| (p as i32 - pc as i32).unsigned_abs() <= 10)
            .map(|&(_, c)| c)
            .sum();
        clusters.push((pc, nearby_total));
    }
    clusters.sort_by(|a, b| b.1.cmp(&a.1));
    clusters.dedup_by_key(|c| c.0 / 16); // Group by 16-byte blocks
    println!("Top clusters (±10 bytes):");
    for &(pc, total) in clusters.iter().take(10) {
        let pct = total as f64 / total_ticks as f64 * 100.0;
        println!("  Near ${:04X}: {:8} ticks ({:.2}%)", pc, total, pct);
    }

    // Final state
    println!("\n=== Final state at frame {} ===", frames);
    println!("  PC: ${:04X}", emu.cpu.pc);
    println!(
        "  Status: ${:02X} (I-flag={})",
        emu.cpu.status,
        emu.cpu.status & 0x04 != 0
    );
    println!(
        "  Timer: counter={}, control=${:02X}",
        emu.bus.read_io(0x0C00),
        emu.bus.read_io(0x0C01)
    );
    println!("  IRQ disable: ${:02X}", emu.bus.read_io(0x1402));

    Ok(())
}
