#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;

    // Disassemble ROM around the text string area
    // Text strings are at ROM offsets:
    //   0xE177 "HUDSON", 0xE187 "SCORE", 0xE191 "HISCORE",
    //   0xE1C1 "PUSH", 0xE1CC "BUTTON"
    //
    // Look for code that references these addresses.
    // On PCE, ROM is typically mapped starting at $8000 or via bank switching.
    // This game is a HuCard, ROM mapped via MPR registers.
    //
    // For a 256KB ROM (0x40000 bytes), the mapping depends on bank settings.
    // Let's first check the ROM size and look for code patterns.

    println!("ROM size: 0x{:X} ({} bytes)", rom.len(), rom.len());

    // Search for references to the text string addresses
    // The text at ROM offset 0xE177 could be at CPU address depending on bank mapping
    // A 256KB ROM = 0x40000 bytes = 32 banks of 8KB each
    // Bank N maps to 0x2000*N in ROM
    // If offset 0xE177 is in bank 0xE177/0x2000 = 0x70 (bank 112)... that's too high.
    // Wait, for a 256KB ROM, banks wrap: bank = (offset / 0x2000) % total_banks
    // 0x40000 / 0x2000 = 32 banks, so bank = offset / 0x2000 = 0x70... that doesn't fit.
    //
    // Actually, ROM offset = (bank * 0x2000) % rom_size
    // 0xE177 / 0x2000 = 7 (bank 7), offset in bank = 0xE177 % 0x2000 = 0x0177
    // So it's in bank 7, and if bank 7 is mapped to region 0xE000-0xFFFF,
    // the CPU address would be 0xE000 + 0x0177 = 0xE177
    // (Simplest case: bank # matches MPR mapping)

    // Let's look at the reset vector and initial code
    // HuCard reset vector is at ROM offset 0x01FFC-0x01FFD (end of first 8KB bank)
    // Actually, PCE reset vector is at CPU address $FFFE-$FFFF
    // Bank at $E000-$FFFF (MPR7) is always bank 0 at reset
    // So reset vector is at ROM offset 0x1FFE-0x1FFF
    let reset_lo = rom[0x1FFE] as u16;
    let reset_hi = rom[0x1FFF] as u16;
    let reset_vector = (reset_hi << 8) | reset_lo;
    println!("Reset vector: ${:04X}", reset_vector);

    // IRQ vectors
    let irq1_vector = u16::from_le_bytes([rom[0x1FF6], rom[0x1FF7]]);
    let irq2_vector = u16::from_le_bytes([rom[0x1FF8], rom[0x1FF9]]);
    let timer_vector = u16::from_le_bytes([rom[0x1FFA], rom[0x1FFB]]);
    let nmi_vector = u16::from_le_bytes([rom[0x1FFC], rom[0x1FFD]]);
    println!(
        "IRQ1: ${:04X}, IRQ2: ${:04X}, Timer: ${:04X}, NMI: ${:04X}",
        irq1_vector, irq2_vector, timer_vector, nmi_vector
    );

    // Disassemble the area before text strings
    // ROM offset 0xE000-0xE200 is in bank 7 (0xE000/0x2000 = 7)
    // CPU address = 0xE000 + (offset % 0x2000) = 0xE000 + (0xE000 % 0x2000) = 0xE000
    // So ROM offset 0xE000 = CPU $E000, ROM offset 0xE177 = CPU $E177

    println!("\n=== Disassembly of ROM 0xE000-0xE200 ===");
    disasm(&rom, 0xE000, 0xE200);

    // Also look for ST0/ST1/ST2 patterns that reference VWR
    // ST0 = opcode 0x03, ST1 = 0x13, ST2 = 0x23
    println!("\n=== Searching for ST0 #$02 (VWR select) patterns ===");
    for i in 0..rom.len().saturating_sub(6) {
        // ST0 #$02 = 03 02
        if rom[i] == 0x03 && rom[i + 1] == 0x02 {
            // Check if preceded by MAWR setup
            // ST0 #$00 = 03 00
            let context_start = i.saturating_sub(10);
            print!("  ROM 0x{:05X}: ", i);
            for j in context_start..i.min(rom.len()) + 6 {
                if j < rom.len() {
                    if j == i {
                        print!("[");
                    }
                    print!("{:02X} ", rom[j]);
                    if j == i + 1 {
                        print!("] ");
                    }
                }
            }
            println!();
        }
    }

    // Look for TIA (0xE3) instructions that target VDC port
    println!("\n=== Searching for TIA instructions ===");
    for i in 0..rom.len().saturating_sub(7) {
        if rom[i] == 0xE3 {
            let src = u16::from_le_bytes([rom[i + 1], rom[i + 2]]);
            let dst = u16::from_le_bytes([rom[i + 3], rom[i + 4]]);
            let len = u16::from_le_bytes([rom[i + 5], rom[i + 6]]);
            // VDC data port is at 0x0002/0x0003
            if dst == 0x0002 || dst == 0x0003 {
                println!(
                    "  ROM 0x{:05X}: TIA ${:04X}, ${:04X}, ${:04X} (src, VDC_data, len)",
                    i, src, dst, len
                );
                // Show preceding bytes for context
                let ctx = i.saturating_sub(20);
                print!("    Context: ");
                for j in ctx..i {
                    print!("{:02X} ", rom[j]);
                }
                println!();
            }
        }
    }

    // Also TII (0x73) to VDC
    println!("\n=== Searching for TII instructions to VDC ===");
    for i in 0..rom.len().saturating_sub(7) {
        if rom[i] == 0x73 {
            let src = u16::from_le_bytes([rom[i + 1], rom[i + 2]]);
            let dst = u16::from_le_bytes([rom[i + 3], rom[i + 4]]);
            let len = u16::from_le_bytes([rom[i + 5], rom[i + 6]]);
            if dst == 0x0002 || dst == 0x0003 {
                println!(
                    "  ROM 0x{:05X}: TII ${:04X}, ${:04X}, ${:04X}",
                    i, src, dst, len
                );
            }
        }
    }

    Ok(())
}

fn disasm(rom: &[u8], start: usize, end: usize) {
    let mut pc = start;
    while pc < end && pc < rom.len() {
        let opcode = rom[pc];
        let (mnemonic, mode, len) = decode_6502(opcode);
        let cpu_addr = 0xE000 + (pc % 0x2000);

        match len {
            1 => println!("  ${:04X}: {:02X}         {}", cpu_addr, opcode, mnemonic),
            2 => {
                if pc + 1 < rom.len() {
                    let op1 = rom[pc + 1];
                    println!(
                        "  ${:04X}: {:02X} {:02X}      {} {}",
                        cpu_addr,
                        opcode,
                        op1,
                        mnemonic,
                        format_operand(mode, op1 as u16, cpu_addr as u16 + 2)
                    );
                }
            }
            3 => {
                if pc + 2 < rom.len() {
                    let op1 = rom[pc + 1];
                    let op2 = rom[pc + 2];
                    let word = u16::from_le_bytes([op1, op2]);
                    println!(
                        "  ${:04X}: {:02X} {:02X} {:02X}   {} {}",
                        cpu_addr,
                        opcode,
                        op1,
                        op2,
                        mnemonic,
                        format_operand(mode, word, cpu_addr as u16 + 3)
                    );
                }
            }
            7 => {
                // Block transfer
                if pc + 6 < rom.len() {
                    let src = u16::from_le_bytes([rom[pc + 1], rom[pc + 2]]);
                    let dst = u16::from_le_bytes([rom[pc + 3], rom[pc + 4]]);
                    let len = u16::from_le_bytes([rom[pc + 5], rom[pc + 6]]);
                    println!(
                        "  ${:04X}: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}  {} ${:04X}, ${:04X}, ${:04X}",
                        cpu_addr,
                        opcode,
                        rom[pc + 1],
                        rom[pc + 2],
                        rom[pc + 3],
                        rom[pc + 4],
                        rom[pc + 5],
                        rom[pc + 6],
                        mnemonic,
                        src,
                        dst,
                        len
                    );
                }
            }
            _ => println!(
                "  ${:04X}: {:02X}         {} (len={})",
                cpu_addr, opcode, mnemonic, len
            ),
        }
        pc += len;
    }
}

fn format_operand(mode: u8, value: u16, next_pc: u16) -> String {
    match mode {
        0 => String::new(),                        // implied
        1 => format!("#${:02X}", value & 0xFF),    // immediate
        2 => format!("${:02X}", value & 0xFF),     // zeropage
        3 => format!("${:02X},X", value & 0xFF),   // zeropage,X
        4 => format!("${:02X},Y", value & 0xFF),   // zeropage,Y
        5 => format!("${:04X}", value),            // absolute
        6 => format!("${:04X},X", value),          // absolute,X
        7 => format!("${:04X},Y", value),          // absolute,Y
        8 => format!("(${:02X},X)", value & 0xFF), // (zp,X)
        9 => format!("(${:02X}),Y", value & 0xFF), // (zp),Y
        10 => {
            // relative
            let offset = (value & 0xFF) as i8;
            let target = (next_pc as i32 + offset as i32) as u16;
            format!("${:04X}", target)
        }
        11 => format!("(${:04X})", value),        // indirect
        12 => format!("(${:02X})", value & 0xFF), // (zp) - 65C02
        _ => format!("${:04X}", value),
    }
}

fn decode_6502(opcode: u8) -> (&'static str, u8, usize) {
    // Returns (mnemonic, addressing_mode, byte_length)
    // mode: 0=impl, 1=imm, 2=zp, 3=zpx, 4=zpy, 5=abs, 6=absx, 7=absy
    //       8=(zpx), 9=(zp)y, 10=rel, 11=ind, 12=(zp)
    match opcode {
        0x00 => ("BRK", 0, 1),
        0x01 => ("ORA", 8, 2),
        0x02 => ("SXY", 0, 1),
        0x03 => ("ST0", 1, 2),
        0x04 => ("TSB", 2, 2),
        0x05 => ("ORA", 2, 2),
        0x06 => ("ASL", 2, 2),
        0x07 => ("RMB0", 2, 2),
        0x08 => ("PHP", 0, 1),
        0x09 => ("ORA", 1, 2),
        0x0A => ("ASL", 0, 1),
        0x0C => ("TSB", 5, 3),
        0x0D => ("ORA", 5, 3),
        0x0E => ("ASL", 5, 3),
        0x0F => ("BBR0", 2, 3), // BBR has special encoding
        0x10 => ("BPL", 10, 2),
        0x11 => ("ORA", 9, 2),
        0x12 => ("ORA", 12, 2),
        0x13 => ("ST1", 1, 2),
        0x14 => ("TRB", 2, 2),
        0x15 => ("ORA", 3, 2),
        0x16 => ("ASL", 3, 2),
        0x17 => ("RMB1", 2, 2),
        0x18 => ("CLC", 0, 1),
        0x19 => ("ORA", 7, 3),
        0x1A => ("INC", 0, 1),
        0x20 => ("JSR", 5, 3),
        0x21 => ("AND", 8, 2),
        0x22 => ("SAX", 0, 1),
        0x23 => ("ST2", 1, 2),
        0x24 => ("BIT", 2, 2),
        0x25 => ("AND", 2, 2),
        0x26 => ("ROL", 2, 2),
        0x27 => ("RMB2", 2, 2),
        0x28 => ("PLP", 0, 1),
        0x29 => ("AND", 1, 2),
        0x2A => ("ROL", 0, 1),
        0x2C => ("BIT", 5, 3),
        0x2D => ("AND", 5, 3),
        0x2E => ("ROL", 5, 3),
        0x30 => ("BMI", 10, 2),
        0x31 => ("AND", 9, 2),
        0x32 => ("AND", 12, 2),
        0x34 => ("BIT", 3, 2),
        0x35 => ("AND", 3, 2),
        0x36 => ("ROL", 3, 2),
        0x38 => ("SEC", 0, 1),
        0x39 => ("AND", 7, 3),
        0x3A => ("DEC", 0, 1),
        0x40 => ("RTI", 0, 1),
        0x41 => ("EOR", 8, 2),
        0x42 => ("SAY", 0, 1),
        0x43 => ("TMA", 1, 2),
        0x44 => ("BSR", 10, 2),
        0x45 => ("EOR", 2, 2),
        0x46 => ("LSR", 2, 2),
        0x48 => ("PHA", 0, 1),
        0x49 => ("EOR", 1, 2),
        0x4A => ("LSR", 0, 1),
        0x4C => ("JMP", 5, 3),
        0x4D => ("EOR", 5, 3),
        0x4E => ("LSR", 5, 3),
        0x50 => ("BVC", 10, 2),
        0x51 => ("EOR", 9, 2),
        0x52 => ("EOR", 12, 2),
        0x53 => ("TAM", 1, 2),
        0x54 => ("CSL", 0, 1),
        0x55 => ("EOR", 3, 2),
        0x56 => ("LSR", 3, 2),
        0x58 => ("CLI", 0, 1),
        0x59 => ("EOR", 7, 3),
        0x5A => ("PHY", 0, 1),
        0x60 => ("RTS", 0, 1),
        0x61 => ("ADC", 8, 2),
        0x62 => ("CLA", 0, 1),
        0x64 => ("STZ", 2, 2),
        0x65 => ("ADC", 2, 2),
        0x66 => ("ROR", 2, 2),
        0x68 => ("PLA", 0, 1),
        0x69 => ("ADC", 1, 2),
        0x6A => ("ROR", 0, 1),
        0x6C => ("JMP", 11, 3),
        0x6D => ("ADC", 5, 3),
        0x6E => ("ROR", 5, 3),
        0x70 => ("BVS", 10, 2),
        0x71 => ("ADC", 9, 2),
        0x72 => ("ADC", 12, 2),
        0x73 => ("TII", 0, 7),
        0x74 => ("STZ", 3, 2),
        0x75 => ("ADC", 3, 2),
        0x76 => ("ROR", 3, 2),
        0x78 => ("SEI", 0, 1),
        0x79 => ("ADC", 7, 3),
        0x7A => ("PLY", 0, 1),
        0x80 => ("BRA", 10, 2),
        0x81 => ("STA", 8, 2),
        0x82 => ("CLX", 0, 1),
        0x84 => ("STY", 2, 2),
        0x85 => ("STA", 2, 2),
        0x86 => ("STX", 2, 2),
        0x87 => ("SMB0", 2, 2),
        0x88 => ("DEY", 0, 1),
        0x89 => ("BIT", 1, 2),
        0x8A => ("TXA", 0, 1),
        0x8C => ("STY", 5, 3),
        0x8D => ("STA", 5, 3),
        0x8E => ("STX", 5, 3),
        0x90 => ("BCC", 10, 2),
        0x91 => ("STA", 9, 2),
        0x92 => ("STA", 12, 2),
        0x94 => ("STY", 3, 2),
        0x95 => ("STA", 3, 2),
        0x96 => ("STX", 4, 2),
        0x98 => ("TYA", 0, 1),
        0x99 => ("STA", 7, 3),
        0x9A => ("TXS", 0, 1),
        0x9C => ("STZ", 5, 3),
        0x9D => ("STA", 6, 3),
        0x9E => ("STZ", 6, 3),
        0xA0 => ("LDY", 1, 2),
        0xA1 => ("LDA", 8, 2),
        0xA2 => ("LDX", 1, 2),
        0xA4 => ("LDY", 2, 2),
        0xA5 => ("LDA", 2, 2),
        0xA6 => ("LDX", 2, 2),
        0xA8 => ("TAY", 0, 1),
        0xA9 => ("LDA", 1, 2),
        0xAA => ("TAX", 0, 1),
        0xAC => ("LDY", 5, 3),
        0xAD => ("LDA", 5, 3),
        0xAE => ("LDX", 5, 3),
        0xB0 => ("BCS", 10, 2),
        0xB1 => ("LDA", 9, 2),
        0xB2 => ("LDA", 12, 2),
        0xB4 => ("LDY", 3, 2),
        0xB5 => ("LDA", 3, 2),
        0xB6 => ("LDX", 4, 2),
        0xB8 => ("CLV", 0, 1),
        0xB9 => ("LDA", 7, 3),
        0xBA => ("TSX", 0, 1),
        0xBC => ("LDY", 6, 3),
        0xBD => ("LDA", 6, 3),
        0xBE => ("LDX", 7, 3),
        0xC0 => ("CPY", 1, 2),
        0xC1 => ("CMP", 8, 2),
        0xC2 => ("CLY", 0, 1),
        0xC3 => ("TDD", 0, 7),
        0xC4 => ("CPY", 2, 2),
        0xC5 => ("CMP", 2, 2),
        0xC6 => ("DEC", 2, 2),
        0xC8 => ("INY", 0, 1),
        0xC9 => ("CMP", 1, 2),
        0xCA => ("DEX", 0, 1),
        0xCC => ("CPY", 5, 3),
        0xCD => ("CMP", 5, 3),
        0xCE => ("DEC", 5, 3),
        0xD0 => ("BNE", 10, 2),
        0xD1 => ("CMP", 9, 2),
        0xD2 => ("CMP", 12, 2),
        0xD3 => ("TIN", 0, 7),
        0xD4 => ("CSH", 0, 1),
        0xD5 => ("CMP", 3, 2),
        0xD6 => ("DEC", 3, 2),
        0xD8 => ("CLD", 0, 1),
        0xD9 => ("CMP", 7, 3),
        0xDA => ("PHX", 0, 1),
        0xDD => ("CMP", 6, 3),
        0xDE => ("DEC", 6, 3),
        0xE0 => ("CPX", 1, 2),
        0xE1 => ("SBC", 8, 2),
        0xE3 => ("TIA", 0, 7),
        0xE4 => ("CPX", 2, 2),
        0xE5 => ("SBC", 2, 2),
        0xE6 => ("INC", 2, 2),
        0xE8 => ("INX", 0, 1),
        0xE9 => ("SBC", 1, 2),
        0xEA => ("NOP", 0, 1),
        0xEC => ("CPX", 5, 3),
        0xED => ("SBC", 5, 3),
        0xEE => ("INC", 5, 3),
        0xF0 => ("BEQ", 10, 2),
        0xF1 => ("SBC", 9, 2),
        0xF2 => ("SBC", 12, 2),
        0xF3 => ("TAI", 0, 7),
        0xF4 => ("SET", 0, 1),
        0xF5 => ("SBC", 3, 2),
        0xF6 => ("INC", 3, 2),
        0xF8 => ("SED", 0, 1),
        0xF9 => ("SBC", 7, 3),
        0xFA => ("PLX", 0, 1),
        0xFC => ("???", 0, 1),
        0xFD => ("SBC", 6, 3),
        0xFE => ("INC", 6, 3),
        _ => ("???", 0, 1),
    }
}
