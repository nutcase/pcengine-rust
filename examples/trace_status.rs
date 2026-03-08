#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::collections::HashMap;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0u64;
    let mut status_hist: HashMap<u8, u64> = HashMap::new();
    let irq1_addr: u16 = 0xE2AA;

    // Run 300 frames and watch what status the IRQ1 handler reads
    while frames < 300 {
        let prev_pc = emu.cpu.pc;
        emu.tick();
        // Detect entry to IRQ1 handler
        if emu.cpu.pc == irq1_addr && prev_pc != irq1_addr {
            // The handler will read $0000 next. Let's check what VDC status is
            let status = emu.bus.vdc_status_bits();
            *status_hist.entry(status).or_insert(0) += 1;
        }
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    println!("VDC status seen at IRQ1 handler entry:");
    let mut sorted: Vec<_> = status_hist.iter().collect();
    sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(*count));
    for (status, count) in sorted {
        let vbl = if status & 0x20 != 0 { "VBL" } else { "---" };
        let rcr = if status & 0x04 != 0 { "RCR" } else { "---" };
        let ds = if status & 0x02 != 0 { "DS" } else { "--" };
        println!(
            "  status={:02X} ({} {} {}) count={}",
            status, vbl, rcr, ds, count
        );
    }

    Ok(())
}
