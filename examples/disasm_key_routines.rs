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

    // $ED95 in bank 0 = ROM offset 0x0D95
    println!("=== $ED95 routine (ROM 0x0D95, bank 0) ===");
    disasm(&rom, 0x0D95, 0x0E00, 0xED95);

    // $E5A9 in bank 0 (TIA location) = ROM offset 0x05A9, with context
    println!("\n=== $E5A9 area (ROM 0x0580, bank 0) ===");
    disasm(&rom, 0x0580, 0x0620, 0xE580);

    // $EDBF in bank 0 = ROM offset 0x0DBF
    println!("\n=== $EDBF routine (ROM 0x0DBF, bank 0) ===");
    disasm(&rom, 0x0DBF, 0x0E00, 0xEDBF);

    // $8000 in bank 1 = ROM offset 0x2000
    println!("\n=== $8000 game entry (ROM 0x2000, bank 1) ===");
    disasm(&rom, 0x2000, 0x2100, 0x8000);

    // $E1A6 in bank 0 = ROM offset 0x01A6 - this is a VRAM fill routine
    println!("\n=== $E1A6 VRAM fill routine (ROM 0x01A6, bank 0) ===");
    disasm(&rom, 0x01A6, 0x01E0, 0xE1A6);

    // Look at $E1EE (ROM 0x01EE) which seems to be SATB/DMA related
    println!("\n=== $E1EE routine (ROM 0x01EE, bank 0) ===");
    disasm(&rom, 0x01EE, 0x0220, 0xE1EE);

    // Hex dump of ROM area near the TIA source $4750
    // With MPR mapping: MPR4=bank1, so $4750 can't be in MPR4 ($8000-$9FFF)
    // $4750 is in MPR2 region ($4000-$5FFF). MPR2=$F8 initially (I/O page)
    // But the game might remap MPR2 before this TIA. Let's check what $4750 looks like
    // if MPR2 is set to different banks
    println!("\n=== Checking what data would be at $4750 with different bank mappings ===");
    for bank in 0u8..32 {
        let rom_offset = (bank as usize) * 0x2000 + 0x0750; // $4750 within the bank
        if rom_offset + 0x20 <= rom.len() {
            let nonzero = rom[rom_offset..rom_offset + 32]
                .iter()
                .filter(|&&b| b != 0)
                .count();
            if nonzero > 0 && nonzero < 30 {
                // Interesting patterns, not all zeros or all 0xFF
                print!("  Bank {:2} (ROM 0x{:05X}): ", bank, rom_offset);
                for i in 0..16 {
                    print!("{:02X} ", rom[rom_offset + i]);
                }
                println!();
            }
        }
    }

    // Search for code that sets MAWR to font VRAM area (0x1200-0x17FF)
    // Pattern: selecting R00 then writing 0x12-0x17 as high byte to VDC port
    // Look for: LDA #$12..17 followed by STA $0003 (VDC data high)
    println!("\n=== Searching for MAWR high byte set to 0x12-0x17 ===");
    for i in 0..rom.len().saturating_sub(5) {
        if rom[i] == 0xA9 && rom[i + 1] >= 0x12 && rom[i + 1] <= 0x17 {
            if i + 4 < rom.len() && rom[i + 2] == 0x8D && rom[i + 3] == 0x03 && rom[i + 4] == 0x00 {
                let bank = i / 0x2000;
                let cpu_addr = 0xE000 + (i % 0x2000);
                println!(
                    "  ROM 0x{:05X} (bank {} CPU ${:04X}): LDA #${:02X}, STA $0003",
                    i,
                    bank,
                    cpu_addr,
                    rom[i + 1]
                );
                // Show surrounding context
                let start = i.saturating_sub(10);
                print!("    Context: ");
                for j in start..(i + 10).min(rom.len()) {
                    print!("{:02X} ", rom[j]);
                }
                println!();
            }
        }
        // Also: ST2 #$12..17 (opcode 0x23)
        if rom[i] == 0x23 && rom[i + 1] >= 0x12 && rom[i + 1] <= 0x17 {
            // Check if preceded by ST0 #$00
            for j in i.saturating_sub(6)..i {
                if rom[j] == 0x03 && rom[j + 1] == 0x00 {
                    let bank = i / 0x2000;
                    let cpu_addr = 0xE000 + (i % 0x2000);
                    println!(
                        "  ROM 0x{:05X} (bank {} CPU ${:04X}): ST2 #${:02X} (after ST0 #$00 at -{:})",
                        i,
                        bank,
                        cpu_addr,
                        rom[i + 1],
                        i - j
                    );
                    break;
                }
            }
        }
    }

    Ok(())
}

fn disasm(rom: &[u8], rom_start: usize, rom_end: usize, cpu_base: usize) {
    let mut pc = rom_start;
    while pc < rom_end && pc < rom.len() {
        let opcode = rom[pc];
        let (mnemonic, _mode, len) = decode_6502(opcode);
        let cpu_addr = cpu_base + (pc - rom_start);

        let mut hex = String::new();
        for i in 0..len.min(7) {
            if pc + i < rom.len() {
                hex.push_str(&format!("{:02X} ", rom[pc + i]));
            }
        }

        match len {
            1 => println!("  ${:04X}: {:9}{}", cpu_addr, hex, mnemonic),
            2 => {
                let op1 = rom.get(pc + 1).copied().unwrap_or(0);
                println!(
                    "  ${:04X}: {:9}{} {}",
                    cpu_addr,
                    hex,
                    mnemonic,
                    format_operand(_mode, op1 as u16, cpu_addr as u16 + 2)
                );
            }
            3 => {
                let word = u16::from_le_bytes([
                    rom.get(pc + 1).copied().unwrap_or(0),
                    rom.get(pc + 2).copied().unwrap_or(0),
                ]);
                println!(
                    "  ${:04X}: {:9}{} {}",
                    cpu_addr,
                    hex,
                    mnemonic,
                    format_operand(_mode, word, cpu_addr as u16 + 3)
                );
            }
            7 => {
                let src = u16::from_le_bytes([rom[pc + 1], rom[pc + 2]]);
                let dst = u16::from_le_bytes([rom[pc + 3], rom[pc + 4]]);
                let ln = u16::from_le_bytes([rom[pc + 5], rom[pc + 6]]);
                println!(
                    "  ${:04X}: {:22}{} ${:04X}, ${:04X}, ${:04X}",
                    cpu_addr, hex, mnemonic, src, dst, ln
                );
            }
            _ => println!("  ${:04X}: {:9}{}", cpu_addr, hex, mnemonic),
        }
        pc += len;
    }
}

fn format_operand(mode: u8, value: u16, next_pc: u16) -> String {
    match mode {
        0 => String::new(),
        1 => format!("#${:02X}", value & 0xFF),
        2 => format!("${:02X}", value & 0xFF),
        3 => format!("${:02X},X", value & 0xFF),
        4 => format!("${:02X},Y", value & 0xFF),
        5 => format!("${:04X}", value),
        6 => format!("${:04X},X", value),
        7 => format!("${:04X},Y", value),
        8 => format!("(${:02X},X)", value & 0xFF),
        9 => format!("(${:02X}),Y", value & 0xFF),
        10 => {
            let o = (value & 0xFF) as i8;
            format!("${:04X}", (next_pc as i32 + o as i32) as u16)
        }
        11 => format!("(${:04X})", value),
        12 => format!("(${:02X})", value & 0xFF),
        _ => format!("${:04X}", value),
    }
}

fn decode_6502(opcode: u8) -> (&'static str, u8, usize) {
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
        0xFD => ("SBC", 6, 3),
        0xFE => ("INC", 6, 3),
        _ => ("???", 0, 1),
    }
}
