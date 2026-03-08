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
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // First, dump the RCR handler code E31A-E389
    let mut frames = 0;
    while frames < 10 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    println!("RCR handler code E31A-E389:");
    for i in 0..112u16 {
        let a = 0xE31A + i;
        let b = emu.bus.read(a);
        if i % 16 == 0 {
            print!("  {:04X}:", a);
        }
        print!(" {:02X}", b);
        if i % 16 == 15 || i == 111 {
            println!();
        }
    }

    // Also dump the VBlank -> RCR transition at E308-E31A
    println!("\nVBlank tail + RCR entry E2F3-E31F:");
    for i in 0..45u16 {
        let a = 0xE2F3 + i;
        let b = emu.bus.read(a);
        if i % 16 == 0 {
            print!("  {:04X}:", a);
        }
        print!(" {:02X}", b);
        if i % 16 == 15 || i == 44 {
            println!();
        }
    }

    // Now trace execution - check if PC ever reaches E31A and what happens
    let mut rcr_hits = 0u64;
    let mut e31a_hits = 0u64;
    let mut e313_hits = 0u64;
    let mut e318_beq_taken = 0u64;
    let mut e318_beq_not_taken = 0u64;

    // Track VDC register writes during RCR handler
    let mut in_rcr_handler = false;
    let mut rcr_handler_pcs: Vec<u16> = Vec::new();

    frames = 0;
    let mut total_ticks = 0u64;
    while frames < 300 {
        let prev_pc = emu.cpu.pc;
        emu.tick();
        total_ticks += 1;

        let pc = emu.cpu.pc;

        // Track E313 (RCR check entry)
        if pc == 0xE313 && prev_pc != 0xE313 {
            e313_hits += 1;
        }

        // Track E318 BEQ result
        if prev_pc == 0xE318 {
            if pc == 0xE389 {
                e318_beq_taken += 1; // RCR bit not set, skip
            } else if pc == 0xE31A {
                e318_beq_not_taken += 1; // RCR bit set, enter handler
            }
        }

        // Track E31A (RCR handler body)
        if pc == 0xE31A && prev_pc != 0xE31A {
            e31a_hits += 1;
            in_rcr_handler = true;
            rcr_handler_pcs.clear();
        }

        if in_rcr_handler {
            if !rcr_handler_pcs.contains(&pc) {
                rcr_handler_pcs.push(pc);
            }
            // Check for RTI or RTS to detect handler exit
            if pc == 0xE389 || pc < 0xE31A || pc > 0xE400 {
                in_rcr_handler = false;
                if e31a_hits <= 3 {
                    println!("\nRCR handler #{} execution path:", e31a_hits);
                    rcr_handler_pcs.sort();
                    for &p in &rcr_handler_pcs {
                        print!(" {:04X}", p);
                    }
                    println!();
                }
            }
        }

        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    println!("\n=== RCR Handler Statistics (300 frames) ===");
    println!("E313 (RCR check) reached: {} times", e313_hits);
    println!(
        "E318 BEQ taken (RCR not set, skip): {} times",
        e318_beq_taken
    );
    println!(
        "E318 BEQ not taken (RCR set, enter handler): {} times",
        e318_beq_not_taken
    );
    println!("E31A (handler body) reached: {} times", e31a_hits);
    println!("Total ticks: {}", total_ticks);

    // Also check what $221A (saved status) value is when we reach E313
    // Let's do another pass for that
    println!("\n=== Checking saved status at E313 ===");
    emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    frames = 0;
    while frames < 10 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    let mut status_at_e313: Vec<u8> = Vec::new();
    frames = 0;
    while frames < 30 {
        let prev_pc = emu.cpu.pc;
        emu.tick();

        // Just before E313 executes LDA $221A
        if emu.cpu.pc == 0xE313 && prev_pc != 0xE313 {
            let saved_status = emu.bus.read(0x221A);
            if status_at_e313.len() < 20 {
                status_at_e313.push(saved_status);
            }
        }

        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    println!(
        "$221A values at E313 entry (first {} samples):",
        status_at_e313.len()
    );
    for (i, &s) in status_at_e313.iter().enumerate() {
        let rcr_set = s & 0x04 != 0;
        println!(
            "  #{}: $221A={:02X} RCR={}",
            i,
            s,
            if rcr_set { "YES" } else { "NO" }
        );
    }

    // Check VDC RCR register (R06) value
    println!("\n=== VDC registers ===");
    if let Some(rcr_val) = emu.bus.vdc_register(6) {
        println!("VDC R06 (RCR) = {:04X}", rcr_val);
    }
    if let Some(bxr) = emu.bus.vdc_register(7) {
        println!("VDC R07 (BXR) = {:04X}", bxr);
    }
    if let Some(byr) = emu.bus.vdc_register(8) {
        println!("VDC R08 (BYR) = {:04X}", byr);
    }
    if let Some(cr) = emu.bus.vdc_register(5) {
        println!("VDC R05 (CR) = {:04X}", cr);
        println!("  CR bit3 (VBlank IRQ enable) = {}", (cr & 0x08) != 0);
        println!("  CR bit4 (RCR IRQ enable) = {}", (cr & 0x10) != 0);
    }

    Ok(())
}
