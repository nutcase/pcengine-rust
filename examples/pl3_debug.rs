#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    println!("ROM size: {} bytes ({} KB)", rom.len(), rom.len() / 1024);

    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Check initial state
    println!("Initial PC: ${:04X}", emu.cpu.pc);
    println!("Initial SP: ${:02X}", emu.cpu.sp);
    println!(
        "Initial A: ${:02X} X: ${:02X} Y: ${:02X}",
        emu.cpu.a, emu.cpu.x, emu.cpu.y
    );
    println!("Initial P: ${:02X}", emu.cpu.status);

    // Check MPR mapping
    for i in 0..8 {
        let bank = emu.bus.mpr(i);
        println!(
            "MPR{}: bank ${:02X} -> ${:04X}-${:04X}",
            i,
            bank,
            i * 0x2000,
            (i + 1) * 0x2000 - 1
        );
    }

    // Check reset vector
    let reset_lo = emu.bus.read(0xFFFE);
    let reset_hi = emu.bus.read(0xFFFF);
    println!(
        "\nReset vector: ${:04X}",
        u16::from_le_bytes([reset_lo, reset_hi])
    );
    let irq1_lo = emu.bus.read(0xFFF8);
    let irq1_hi = emu.bus.read(0xFFF9);
    println!(
        "IRQ1 vector: ${:04X}",
        u16::from_le_bytes([irq1_lo, irq1_hi])
    );
    let timer_lo = emu.bus.read(0xFFFA);
    let timer_hi = emu.bus.read(0xFFFB);
    println!(
        "Timer vector: ${:04X}",
        u16::from_le_bytes([timer_lo, timer_hi])
    );

    // Trace first 50 instructions
    println!("\nTracing first 50 instructions:");
    for i in 0..50 {
        let pc = emu.cpu.pc;
        let op = emu.bus.read(pc);
        let b1 = emu.bus.read(pc.wrapping_add(1));
        let b2 = emu.bus.read(pc.wrapping_add(2));
        let a = emu.cpu.a;
        let x = emu.cpu.x;
        let y = emu.cpu.y;
        let sp = emu.cpu.sp;
        let p = emu.cpu.status;
        let cycles = emu.tick();

        println!(
            "  {:3}: ${:04X}: {:02X} {:02X} {:02X}  A={:02X} X={:02X} Y={:02X} SP={:02X} P={:02X} cy={}",
            i, pc, op, b1, b2, a, x, y, sp, p, cycles
        );

        if emu.cpu.halted {
            println!("  CPU HALTED!");
            break;
        }
    }

    // Run 10000 more ticks and find loop
    println!("\nRunning 10000 more ticks...");
    let mut pc_counts: std::collections::HashMap<u16, u32> = std::collections::HashMap::new();
    for _ in 0..10000 {
        let pc = emu.cpu.pc;
        *pc_counts.entry(pc).or_insert(0) += 1;
        emu.tick();
        if emu.cpu.halted {
            println!("CPU HALTED at PC=${:04X}", emu.cpu.pc);
            break;
        }
    }

    let mut top_pcs: Vec<_> = pc_counts.iter().collect();
    top_pcs.sort_by(|a, b| b.1.cmp(a.1));
    println!("\nTop 15 most visited PCs:");
    for (pc, count) in top_pcs.iter().take(15) {
        let op = emu.bus.read(**pc);
        let b1 = emu.bus.read(pc.wrapping_add(1));
        let b2 = emu.bus.read(pc.wrapping_add(2));
        println!(
            "  ${:04X}: {:02X} {:02X} {:02X}  count={}",
            pc, op, b1, b2, count
        );
    }

    println!("\nFinal PC: ${:04X}", emu.cpu.pc);
    println!("CPU halted: {}", emu.cpu.halted);
    println!("CPU waiting: {}", emu.cpu.is_waiting());

    // Check MPR after run
    println!("\nMPR mapping after run:");
    for i in 0..8 {
        let bank = emu.bus.mpr(i);
        println!("  MPR{}: bank ${:02X}", i, bank);
    }

    Ok(())
}
