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
    println!(
        "ROM size: {} bytes ({} KB, {} banks)",
        rom.len(),
        rom.len() / 1024,
        rom.len() / 8192
    );

    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    println!("Reset vector: ${:04X}", emu.cpu.pc);

    // Run past the memory clear loop (skip ahead) until PC leaves $FDxx region
    // or we see something interesting
    let mut total_ticks = 0u64;
    let mut last_pc = 0u16;
    let mut in_clear_loop = false;
    let mut clear_loop_count = 0u64;

    // Phase 1: Run 200 instructions with full trace to see boot sequence
    println!("\n=== Phase 1: First 200 instructions ===");
    for i in 0..200 {
        let pc = emu.cpu.pc;
        let op = emu.bus.read(pc);
        let b1 = emu.bus.read(pc.wrapping_add(1));
        let b2 = emu.bus.read(pc.wrapping_add(2));
        let a = emu.cpu.a;
        let x = emu.cpu.x;
        let y = emu.cpu.y;
        let sp = emu.cpu.sp;
        let p = emu.cpu.status;

        let mnemonic = disasm(op, b1, b2);
        let cycles = emu.tick();
        total_ticks += 1;

        // Print MPR info for TAM/TMA instructions
        let mpr_info = if op == 0x53 || op == 0x43 {
            format!(
                " MPR[{}]=${:02X}",
                b1.trailing_zeros(),
                emu.bus.mpr(b1.trailing_zeros() as usize)
            )
        } else {
            String::new()
        };

        println!(
            "{:4}: ${:04X}: {:02X} {:02X} {:02X}  {:12}  A={:02X} X={:02X} Y={:02X} SP={:02X} P={:02X} cy={}{}",
            i, pc, op, b1, b2, mnemonic, a, x, y, sp, p, cycles, mpr_info
        );

        if emu.cpu.halted {
            println!("  CPU HALTED!");
            break;
        }
    }

    // Phase 2: Fast forward through clear loops, trace on transitions
    println!("\n=== Phase 2: Fast forward, trace transitions ===");
    let mut prev_pc_region = emu.cpu.pc >> 8;
    let mut instructions_in_region = 0u64;

    for i in 0..500000u64 {
        let pc = emu.cpu.pc;
        let op = emu.bus.read(pc);
        let b1 = emu.bus.read(pc.wrapping_add(1));
        let b2 = emu.bus.read(pc.wrapping_add(2));
        let current_region = pc >> 8;

        // Detect tight loops (STA (zp),Y / INY / BNE pattern)
        if op == 0x91
            && emu.bus.read(pc.wrapping_add(2)) == 0xC8
            && emu.bus.read(pc.wrapping_add(3)) == 0xD0
        {
            if !in_clear_loop {
                println!("  [Entering clear loop at ${:04X}]", pc);
                in_clear_loop = true;
                clear_loop_count = 0;
            }
            // Skip silently
            for _ in 0..3 {
                emu.tick();
                total_ticks += 1;
            }
            clear_loop_count += 1;
            continue;
        }

        if in_clear_loop {
            println!("  [Clear loop ran {} iterations]", clear_loop_count);
            in_clear_loop = false;
        }

        // Print when PC enters a new region, or for certain opcodes
        let is_interesting = current_region != prev_pc_region
            || op == 0x53  // TAM
            || op == 0x43  // TMA
            || op == 0x00  // BRK
            || op == 0x20  // JSR
            || op == 0x60  // RTS
            || op == 0x40  // RTI
            || op == 0x4C  // JMP abs
            || op == 0x6C  // JMP ind
            || op == 0x7C  // JMP (abs,X)
            || op == 0xDB  // STP
            || op == 0xCB  // WAI
            || op == 0x73 || op == 0xC3 || op == 0xD3 || op == 0xE3 || op == 0xF3  // block transfers
            || (i < 50); // first few after phase 1

        if is_interesting {
            let a = emu.cpu.a;
            let x = emu.cpu.x;
            let y = emu.cpu.y;
            let sp = emu.cpu.sp;
            let p = emu.cpu.status;
            let mnemonic = disasm(op, b1, b2);

            if current_region != prev_pc_region {
                println!(
                    "  --- region change: ${:02X}xx -> ${:02X}xx ---",
                    prev_pc_region, current_region
                );
                // Print MPR state on region changes
                print!("  MPR:");
                for j in 0..8 {
                    print!(" {}=${:02X}", j, emu.bus.mpr(j));
                }
                println!();
            }

            let mpr_info = if op == 0x53 || op == 0x43 {
                format!(
                    " MPR[{}]=${:02X}",
                    b1.trailing_zeros(),
                    if op == 0x53 {
                        a
                    } else {
                        emu.bus.mpr(b1.trailing_zeros() as usize)
                    }
                )
            } else {
                String::new()
            };

            println!(
                "{:6}: ${:04X}: {:02X} {:02X} {:02X}  {:12}  A={:02X} X={:02X} Y={:02X} SP={:02X} P={:02X}{}",
                total_ticks, pc, op, b1, b2, mnemonic, a, x, y, sp, p, mpr_info
            );

            prev_pc_region = current_region;
        }

        let cycles = emu.tick();
        total_ticks += 1;
        instructions_in_region += 1;

        if emu.cpu.halted {
            println!("  CPU HALTED at ${:04X}!", emu.cpu.pc);
            break;
        }

        // Safety: stop if we've been running too long
        if total_ticks > 600000 {
            println!("  [Stopped after {} ticks]", total_ticks);
            break;
        }
    }

    // Print final state
    println!("\n=== Final State ===");
    println!("PC: ${:04X}", emu.cpu.pc);
    println!(
        "A={:02X} X={:02X} Y={:02X} SP={:02X} P={:02X}",
        emu.cpu.a, emu.cpu.x, emu.cpu.y, emu.cpu.sp, emu.cpu.status
    );
    println!(
        "Halted: {}, Waiting: {}",
        emu.cpu.halted,
        emu.cpu.is_waiting()
    );
    print!("MPR:");
    for j in 0..8 {
        print!(" {}=${:02X}", j, emu.bus.mpr(j));
    }
    println!();

    Ok(())
}

fn disasm(op: u8, b1: u8, b2: u8) -> String {
    match op {
        0x00 => "BRK".into(),
        0x01 => format!("ORA (${:02X},X)", b1),
        0x02 => "SXY".into(),
        0x04 => format!("TSB ${:02X}", b1),
        0x05 => format!("ORA ${:02X}", b1),
        0x06 => format!("ASL ${:02X}", b1),
        0x08 => "PHP".into(),
        0x09 => format!("ORA #${:02X}", b1),
        0x0A => "ASL A".into(),
        0x0C => format!("TSB ${:02X}{:02X}", b2, b1),
        0x0D => format!("ORA ${:02X}{:02X}", b2, b1),
        0x0E => format!("ASL ${:02X}{:02X}", b2, b1),
        0x10 => format!("BPL ${:+}", b1 as i8),
        0x11 => format!("ORA (${:02X}),Y", b1),
        0x12 => format!("ORA (${:02X})", b1),
        0x14 => format!("TRB ${:02X}", b1),
        0x15 => format!("ORA ${:02X},X", b1),
        0x18 => "CLC".into(),
        0x19 => format!("ORA ${:02X}{:02X},Y", b2, b1),
        0x1A => "INC A".into(),
        0x1D => format!("ORA ${:02X}{:02X},X", b2, b1),
        0x20 => format!("JSR ${:02X}{:02X}", b2, b1),
        0x21 => format!("AND (${:02X},X)", b1),
        0x22 => "SAX".into(),
        0x24 => format!("BIT ${:02X}", b1),
        0x25 => format!("AND ${:02X}", b1),
        0x28 => "PLP".into(),
        0x29 => format!("AND #${:02X}", b1),
        0x2A => "ROL A".into(),
        0x2C => format!("BIT ${:02X}{:02X}", b2, b1),
        0x2D => format!("AND ${:02X}{:02X}", b2, b1),
        0x30 => format!("BMI ${:+}", b1 as i8),
        0x31 => format!("AND (${:02X}),Y", b1),
        0x32 => format!("AND (${:02X})", b1),
        0x34 => format!("BIT ${:02X},X", b1),
        0x38 => "SEC".into(),
        0x39 => format!("AND ${:02X}{:02X},Y", b2, b1),
        0x3A => "DEC A".into(),
        0x3D => format!("AND ${:02X}{:02X},X", b2, b1),
        0x40 => "RTI".into(),
        0x41 => format!("EOR (${:02X},X)", b1),
        0x42 => "SAY".into(),
        0x43 => format!("TMA #${:02X}", b1),
        0x44 => format!("BSR ${:02X}{:02X}", b2, b1),
        0x45 => format!("EOR ${:02X}", b1),
        0x48 => "PHA".into(),
        0x49 => format!("EOR #${:02X}", b1),
        0x4A => "LSR A".into(),
        0x4C => format!("JMP ${:02X}{:02X}", b2, b1),
        0x4D => format!("EOR ${:02X}{:02X}", b2, b1),
        0x50 => format!("BVC ${:+}", b1 as i8),
        0x51 => format!("EOR (${:02X}),Y", b1),
        0x52 => format!("EOR (${:02X})", b1),
        0x53 => format!("TAM #${:02X}", b1),
        0x54 => "CSL".into(),
        0x58 => "CLI".into(),
        0x59 => format!("EOR ${:02X}{:02X},Y", b2, b1),
        0x5A => "PHY".into(),
        0x60 => "RTS".into(),
        0x62 => "CLA".into(),
        0x64 => format!("STZ ${:02X}", b1),
        0x65 => format!("ADC ${:02X}", b1),
        0x68 => "PLA".into(),
        0x69 => format!("ADC #${:02X}", b1),
        0x6A => "ROR A".into(),
        0x6C => format!("JMP (${:02X}{:02X})", b2, b1),
        0x6D => format!("ADC ${:02X}{:02X}", b2, b1),
        0x70 => format!("BVS ${:+}", b1 as i8),
        0x71 => format!("ADC (${:02X}),Y", b1),
        0x72 => format!("ADC (${:02X})", b1),
        0x73 => format!("TII ${:02X}{:02X}", b2, b1),
        0x78 => "SEI".into(),
        0x79 => format!("ADC ${:02X}{:02X},Y", b2, b1),
        0x7A => "PLY".into(),
        0x7C => format!("JMP (${:02X}{:02X},X)", b2, b1),
        0x80 => format!("BRA ${:+}", b1 as i8),
        0x81 => format!("STA (${:02X},X)", b1),
        0x82 => "CLX".into(),
        0x84 => format!("STY ${:02X}", b1),
        0x85 => format!("STA ${:02X}", b1),
        0x86 => format!("STX ${:02X}", b1),
        0x88 => "DEY".into(),
        0x89 => format!("BIT #${:02X}", b1),
        0x8A => "TXA".into(),
        0x8C => format!("STY ${:02X}{:02X}", b2, b1),
        0x8D => format!("STA ${:02X}{:02X}", b2, b1),
        0x8E => format!("STX ${:02X}{:02X}", b2, b1),
        0x90 => format!("BCC ${:+}", b1 as i8),
        0x91 => format!("STA (${:02X}),Y", b1),
        0x92 => format!("STA (${:02X})", b1),
        0x94 => format!("STY ${:02X},X", b1),
        0x95 => format!("STA ${:02X},X", b1),
        0x96 => format!("STX ${:02X},Y", b1),
        0x98 => "TYA".into(),
        0x99 => format!("STA ${:02X}{:02X},Y", b2, b1),
        0x9A => "TXS".into(),
        0x9C => format!("STZ ${:02X}{:02X}", b2, b1),
        0x9D => format!("STA ${:02X}{:02X},X", b2, b1),
        0x9E => format!("STZ ${:02X}{:02X},X", b2, b1),
        0xA0 => format!("LDY #${:02X}", b1),
        0xA1 => format!("LDA (${:02X},X)", b1),
        0xA2 => format!("LDX #${:02X}", b1),
        0xA4 => format!("LDY ${:02X}", b1),
        0xA5 => format!("LDA ${:02X}", b1),
        0xA6 => format!("LDX ${:02X}", b1),
        0xA8 => "TAY".into(),
        0xA9 => format!("LDA #${:02X}", b1),
        0xAA => "TAX".into(),
        0xAC => format!("LDY ${:02X}{:02X}", b2, b1),
        0xAD => format!("LDA ${:02X}{:02X}", b2, b1),
        0xAE => format!("LDX ${:02X}{:02X}", b2, b1),
        0xB0 => format!("BCS ${:+}", b1 as i8),
        0xB1 => format!("LDA (${:02X}),Y", b1),
        0xB2 => format!("LDA (${:02X})", b1),
        0xB4 => format!("LDY ${:02X},X", b1),
        0xB5 => format!("LDA ${:02X},X", b1),
        0xB9 => format!("LDA ${:02X}{:02X},Y", b2, b1),
        0xBA => "TSX".into(),
        0xBC => format!("LDY ${:02X}{:02X},X", b2, b1),
        0xBD => format!("LDA ${:02X}{:02X},X", b2, b1),
        0xBE => format!("LDX ${:02X}{:02X},Y", b2, b1),
        0xC0 => format!("CPY #${:02X}", b1),
        0xC1 => format!("CMP (${:02X},X)", b1),
        0xC2 => "CLY".into(),
        0xC3 => format!("TDD ${:02X}{:02X}", b2, b1),
        0xC4 => format!("CPY ${:02X}", b1),
        0xC5 => format!("CMP ${:02X}", b1),
        0xC6 => format!("DEC ${:02X}", b1),
        0xC8 => "INY".into(),
        0xC9 => format!("CMP #${:02X}", b1),
        0xCA => "DEX".into(),
        0xCB => "WAI".into(),
        0xCC => format!("CPY ${:02X}{:02X}", b2, b1),
        0xCD => format!("CMP ${:02X}{:02X}", b2, b1),
        0xCE => format!("DEC ${:02X}{:02X}", b2, b1),
        0xD0 => format!("BNE ${:+}", b1 as i8),
        0xD1 => format!("CMP (${:02X}),Y", b1),
        0xD2 => format!("CMP (${:02X})", b1),
        0xD3 => format!("TIN ${:02X}{:02X}", b2, b1),
        0xD4 => "CSH".into(),
        0xD8 => "CLD".into(),
        0xD9 => format!("CMP ${:02X}{:02X},Y", b2, b1),
        0xDA => "PHX".into(),
        0xDB => "STP".into(),
        0xE0 => format!("CPX #${:02X}", b1),
        0xE1 => format!("SBC (${:02X},X)", b1),
        0xE3 => format!("TIA ${:02X}{:02X}", b2, b1),
        0xE4 => format!("CPX ${:02X}", b1),
        0xE5 => format!("SBC ${:02X}", b1),
        0xE6 => format!("INC ${:02X}", b1),
        0xE8 => "INX".into(),
        0xE9 => format!("SBC #${:02X}", b1),
        0xEA => "NOP".into(),
        0xEC => format!("CPX ${:02X}{:02X}", b2, b1),
        0xED => format!("SBC ${:02X}{:02X}", b2, b1),
        0xEE => format!("INC ${:02X}{:02X}", b2, b1),
        0xF0 => format!("BEQ ${:+}", b1 as i8),
        0xF1 => format!("SBC (${:02X}),Y", b1),
        0xF2 => format!("SBC (${:02X})", b1),
        0xF3 => format!("TAI ${:02X}{:02X}", b2, b1),
        0xF8 => "SED".into(),
        0xF9 => format!("SBC ${:02X}{:02X},Y", b2, b1),
        0xFA => "PLX".into(),
        0xFD => format!("SBC ${:02X}{:02X},X", b2, b1),
        _ => format!("??? ${:02X}", op),
    }
}
