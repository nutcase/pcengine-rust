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

/// Targeted trace of Power League III secondary crash.
/// Fast-forwards to near the crash point (~45K ticks), then traces every instruction.
fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    println!("Reset vector: ${:04X}", emu.cpu.pc);

    // Phase 1: Fast-forward to near crash, tracking JSR/RTS call stack
    let mut total_ticks = 0u64;
    let mut call_stack: Vec<(u16, u16, u8)> = Vec::new(); // (call_site, target, sp_after_push)

    let crash_search_start = 44000u64; // Start full trace a bit before expected crash at ~45396

    println!("=== Fast-forwarding to tick {} ===", crash_search_start);

    while total_ticks < crash_search_start {
        let pc = emu.cpu.pc;
        let op = emu.bus.read(pc);
        let b1 = emu.bus.read(pc.wrapping_add(1));
        let b2 = emu.bus.read(pc.wrapping_add(2));

        // Track call stack
        match op {
            0x20 => {
                // JSR
                let target = (b2 as u16) << 8 | b1 as u16;
                let sp = emu.cpu.sp;
                call_stack.push((pc, target, sp));
                if call_stack.len() > 64 {
                    call_stack.remove(0);
                }
            }
            0x44 => {
                // BSR
                let sp = emu.cpu.sp;
                let offset = b1 as i8;
                let target = (pc.wrapping_add(2) as i32 + offset as i32) as u16;
                call_stack.push((pc, target, sp));
                if call_stack.len() > 64 {
                    call_stack.remove(0);
                }
            }
            0x60 => {
                // RTS
                if !call_stack.is_empty() {
                    call_stack.pop();
                }
            }
            _ => {}
        }

        emu.tick();
        total_ticks += 1;

        if emu.cpu.halted {
            println!(
                "CPU HALTED at tick {} PC=${:04X} (during fast-forward!)",
                total_ticks, emu.cpu.pc
            );
            break;
        }
    }

    if emu.cpu.halted {
        // Crash happened before our target. Restart with lower threshold.
        println!(
            "Crash happened before tick {}. Re-running with full trace from start.",
            crash_search_start
        );
        return Ok(());
    }

    // Print call stack state at the start of full trace
    println!("\n=== Call stack at tick {} ===", total_ticks);
    for (i, (call_site, target, sp)) in call_stack.iter().enumerate() {
        println!(
            "  [{:2}] JSR from ${:04X} -> ${:04X} (SP was ${:02X})",
            i, call_site, target, sp
        );
    }

    // Phase 2: Full trace from here
    println!("\n=== Full trace from tick {} ===", total_ticks);
    let trace_limit = 3000u64;

    for _ in 0..trace_limit {
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

        // Extra info for JSR/RTS/BRK
        let extra = match op {
            0x20 => {
                // JSR
                let target = (b2 as u16) << 8 | b1 as u16;
                let ret = pc.wrapping_add(2); // return address pushed = PC+2 (last byte of JSR)
                format!(
                    " [push ${:04X} to stack, SP ${:02X}->${:02X}]",
                    ret,
                    sp,
                    sp.wrapping_sub(2)
                )
            }
            0x60 => {
                // RTS
                // Peek at stack to see what address will be popped
                let lo = emu.bus.read(0x0100 | sp.wrapping_add(1) as u16) as u16;
                let hi = emu.bus.read(0x0100 | sp.wrapping_add(2) as u16) as u16;
                let ret_addr = ((hi << 8) | lo).wrapping_add(1);
                format!(
                    " [pop ${:04X} from stack SP=${:02X}, return to ${:04X}]",
                    (hi << 8) | lo,
                    sp,
                    ret_addr
                )
            }
            0x40 => {
                // RTI
                let status = emu.bus.read(0x0100 | sp.wrapping_add(1) as u16);
                let lo = emu.bus.read(0x0100 | sp.wrapping_add(2) as u16) as u16;
                let hi = emu.bus.read(0x0100 | sp.wrapping_add(3) as u16) as u16;
                let ret_addr = (hi << 8) | lo;
                format!(" [pop P=${:02X}, return to ${:04X}]", status, ret_addr)
            }
            0x53 => {
                // TAM
                format!(" [MPR[{}] <- ${:02X}]", b1.trailing_zeros(), a)
            }
            0x43 => {
                // TMA
                format!(
                    " [MPR[{}] = ${:02X}]",
                    b1.trailing_zeros(),
                    emu.bus.mpr(b1.trailing_zeros() as usize)
                )
            }
            0x48 => {
                // PHA
                format!(" [push A=${:02X}]", a)
            }
            0x08 => {
                // PHP
                format!(" [push P=${:02X}]", p)
            }
            0x68 => {
                // PLA
                let val = emu.bus.read(0x0100 | sp.wrapping_add(1) as u16);
                format!(" [pop ${:02X}]", val)
            }
            0x28 => {
                // PLP
                let val = emu.bus.read(0x0100 | sp.wrapping_add(1) as u16);
                format!(" [pop P=${:02X}]", val)
            }
            0x00 => {
                // BRK
                let vector = emu.bus.read(0xFFF6) as u16 | (emu.bus.read(0xFFF7) as u16) << 8;
                format!(" [vector=${:04X}]", vector)
            }
            _ => String::new(),
        };

        // Print MPR state on region changes or interesting opcodes
        let mpr_line =
            if op == 0x53 || op == 0x43 || op == 0x20 || op == 0x60 || op == 0x40 || op == 0x00 {
                let mut s = String::from(" MPR:[");
                for j in 0..8 {
                    if j > 0 {
                        s.push(' ');
                    }
                    s.push_str(&format!("{}={:02X}", j, emu.bus.mpr(j)));
                }
                s.push(']');
                s
            } else {
                String::new()
            };

        println!(
            "{:6}: ${:04X}: {:02X} {:02X} {:02X}  {:12}  A={:02X} X={:02X} Y={:02X} SP={:02X} P={:02X}{}{}",
            total_ticks, pc, op, b1, b2, mnemonic, a, x, y, sp, p, extra, mpr_line
        );

        let cycles = emu.tick();
        total_ticks += 1;

        if emu.cpu.halted {
            println!("  >>> CPU HALTED at tick {}!", total_ticks);
            // Dump stack contents
            println!("\n=== Stack dump (SP=${:02X}) ===", emu.cpu.sp);
            for i in (emu.cpu.sp as u16 + 1)..=0xFF {
                let val = emu.bus.read(0x0100 | i);
                print!("{:02X} ", val);
                if (i - emu.cpu.sp as u16) % 16 == 0 {
                    println!();
                }
            }
            println!();
            break;
        }

        // Detect if PC is in I/O space (suspicious)
        if pc >= 0x0000 && pc < 0x2000 && op == 0x00 {
            println!("  >>> BRK in RAM/zero-page area! Likely crash.");
        }
    }

    println!("\n=== Final State ===");
    println!("PC: ${:04X}", emu.cpu.pc);
    println!(
        "A={:02X} X={:02X} Y={:02X} SP={:02X} P={:02X}",
        emu.cpu.a, emu.cpu.x, emu.cpu.y, emu.cpu.sp, emu.cpu.status
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
        0x0F => format!("BBR0 ${:02X},$+{}", b1, b2 as i8),
        0x10 => format!("BPL ${:+}", b1 as i8),
        0x11 => format!("ORA (${:02X}),Y", b1),
        0x12 => format!("ORA (${:02X})", b1),
        0x14 => format!("TRB ${:02X}", b1),
        0x15 => format!("ORA ${:02X},X", b1),
        0x16 => format!("ASL ${:02X},X", b1),
        0x18 => "CLC".into(),
        0x19 => format!("ORA ${:02X}{:02X},Y", b2, b1),
        0x1A => "INC A".into(),
        0x1D => format!("ORA ${:02X}{:02X},X", b2, b1),
        0x1F => format!("BBR1 ${:02X},$+{}", b1, b2 as i8),
        0x20 => format!("JSR ${:02X}{:02X}", b2, b1),
        0x21 => format!("AND (${:02X},X)", b1),
        0x22 => "SAX".into(),
        0x24 => format!("BIT ${:02X}", b1),
        0x25 => format!("AND ${:02X}", b1),
        0x26 => format!("ROL ${:02X}", b1),
        0x28 => "PLP".into(),
        0x29 => format!("AND #${:02X}", b1),
        0x2A => "ROL A".into(),
        0x2C => format!("BIT ${:02X}{:02X}", b2, b1),
        0x2D => format!("AND ${:02X}{:02X}", b2, b1),
        0x2F => format!("BBR2 ${:02X},$+{}", b1, b2 as i8),
        0x30 => format!("BMI ${:+}", b1 as i8),
        0x31 => format!("AND (${:02X}),Y", b1),
        0x32 => format!("AND (${:02X})", b1),
        0x34 => format!("BIT ${:02X},X", b1),
        0x38 => "SEC".into(),
        0x39 => format!("AND ${:02X}{:02X},Y", b2, b1),
        0x3A => "DEC A".into(),
        0x3D => format!("AND ${:02X}{:02X},X", b2, b1),
        0x3F => format!("BBR3 ${:02X},$+{}", b1, b2 as i8),
        0x40 => "RTI".into(),
        0x41 => format!("EOR (${:02X},X)", b1),
        0x42 => "SAY".into(),
        0x43 => format!("TMA #${:02X}", b1),
        0x44 => format!("BSR ${:02X}{:02X}", b2, b1),
        0x45 => format!("EOR ${:02X}", b1),
        0x46 => format!("LSR ${:02X}", b1),
        0x48 => "PHA".into(),
        0x49 => format!("EOR #${:02X}", b1),
        0x4A => "LSR A".into(),
        0x4C => format!("JMP ${:02X}{:02X}", b2, b1),
        0x4D => format!("EOR ${:02X}{:02X}", b2, b1),
        0x4F => format!("BBR4 ${:02X},$+{}", b1, b2 as i8),
        0x50 => format!("BVC ${:+}", b1 as i8),
        0x51 => format!("EOR (${:02X}),Y", b1),
        0x52 => format!("EOR (${:02X})", b1),
        0x53 => format!("TAM #${:02X}", b1),
        0x54 => "CSL".into(),
        0x55 => format!("EOR ${:02X},X", b1),
        0x58 => "CLI".into(),
        0x59 => format!("EOR ${:02X}{:02X},Y", b2, b1),
        0x5A => "PHY".into(),
        0x60 => "RTS".into(),
        0x62 => "CLA".into(),
        0x64 => format!("STZ ${:02X}", b1),
        0x65 => format!("ADC ${:02X}", b1),
        0x66 => format!("ROR ${:02X}", b1),
        0x68 => "PLA".into(),
        0x69 => format!("ADC #${:02X}", b1),
        0x6A => "ROR A".into(),
        0x6C => format!("JMP (${:02X}{:02X})", b2, b1),
        0x6D => format!("ADC ${:02X}{:02X}", b2, b1),
        0x6F => format!("BBR5 ${:02X},$+{}", b1, b2 as i8),
        0x70 => format!("BVS ${:+}", b1 as i8),
        0x71 => format!("ADC (${:02X}),Y", b1),
        0x72 => format!("ADC (${:02X})", b1),
        0x73 => format!("TII ${:02X}{:02X}", b2, b1),
        0x78 => "SEI".into(),
        0x79 => format!("ADC ${:02X}{:02X},Y", b2, b1),
        0x7A => "PLY".into(),
        0x7C => format!("JMP (${:02X}{:02X},X)", b2, b1),
        0x7F => format!("BBR7 ${:02X},$+{}", b1, b2 as i8),
        0x80 => format!("BRA ${:+}", b1 as i8),
        0x81 => format!("STA (${:02X},X)", b1),
        0x82 => "CLX".into(),
        0x84 => format!("STY ${:02X}", b1),
        0x85 => format!("STA ${:02X}", b1),
        0x86 => format!("STX ${:02X}", b1),
        0x87 => format!("SMB0 ${:02X}", b1),
        0x88 => "DEY".into(),
        0x89 => format!("BIT #${:02X}", b1),
        0x8A => "TXA".into(),
        0x8C => format!("STY ${:02X}{:02X}", b2, b1),
        0x8D => format!("STA ${:02X}{:02X}", b2, b1),
        0x8E => format!("STX ${:02X}{:02X}", b2, b1),
        0x8F => format!("BBS0 ${:02X},$+{}", b1, b2 as i8),
        0x90 => format!("BCC ${:+}", b1 as i8),
        0x91 => format!("STA (${:02X}),Y", b1),
        0x92 => format!("STA (${:02X})", b1),
        0x93 => format!("TST #${:02X},${:02X}{:02X}", b1, b2, 0),
        0x94 => format!("STY ${:02X},X", b1),
        0x95 => format!("STA ${:02X},X", b1),
        0x96 => format!("STX ${:02X},Y", b1),
        0x98 => "TYA".into(),
        0x99 => format!("STA ${:02X}{:02X},Y", b2, b1),
        0x9A => "TXS".into(),
        0x9C => format!("STZ ${:02X}{:02X}", b2, b1),
        0x9D => format!("STA ${:02X}{:02X},X", b2, b1),
        0x9E => format!("STZ ${:02X}{:02X},X", b2, b1),
        0x9F => format!("BBS1 ${:02X},$+{}", b1, b2 as i8),
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
        0xAF => format!("BBS2 ${:02X},$+{}", b1, b2 as i8),
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
        0xBF => format!("BBS3 ${:02X},$+{}", b1, b2 as i8),
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
        0xCF => format!("BBS4 ${:02X},$+{}", b1, b2 as i8),
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
        0xEF => format!("BBS5 ${:02X},$+{}", b1, b2 as i8),
        0xF0 => format!("BEQ ${:+}", b1 as i8),
        0xF1 => format!("SBC (${:02X}),Y", b1),
        0xF2 => format!("SBC (${:02X})", b1),
        0xF3 => format!("TAI ${:02X}{:02X}", b2, b1),
        0xF4 => "SET".into(),
        0xF8 => "SED".into(),
        0xF9 => format!("SBC ${:02X}{:02X},Y", b2, b1),
        0xFA => "PLX".into(),
        0xFD => format!("SBC ${:02X}{:02X},X", b2, b1),
        0xFE => format!("INC ${:02X}{:02X},X", b2, b1),
        _ => format!("??? ${:02X}", op),
    }
}
