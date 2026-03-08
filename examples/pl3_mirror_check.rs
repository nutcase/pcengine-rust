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
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let rom_pages = rom.len() / 8192;
    println!("ROM: {} bytes, {} banks", rom.len(), rom_pages);

    // Compare data at bank $0B (pow2 mirror) vs bank $1B (modulo mirror)
    // at offset $1CA2 (the $5CA2 address within bank 2)
    let offset = 0x1CA2;

    let bank_0b_offset = 0x0B * 8192 + offset;
    let bank_1b_offset = 0x1B * 8192 + offset;

    println!(
        "\nBank $0B (power-of-2 mirror) at offset ${:05X}:",
        bank_0b_offset
    );
    print!("  ");
    for i in 0..32 {
        print!("{:02X} ", rom[bank_0b_offset + i]);
    }
    println!();

    // Disassemble the first few bytes
    println!("  Disassembly:");
    let mut pc = 0;
    while pc < 32 {
        let op = rom[bank_0b_offset + pc];
        let b1 = if pc + 1 < 32 {
            rom[bank_0b_offset + pc + 1]
        } else {
            0
        };
        let b2 = if pc + 2 < 32 {
            rom[bank_0b_offset + pc + 2]
        } else {
            0
        };
        let (mnemonic, size) = disasm_with_size(op, b1, b2);
        print!("    ${:04X}: ", 0x5CA2 + pc as u16);
        for j in 0..size {
            print!("{:02X} ", rom[bank_0b_offset + pc + j]);
        }
        for _ in size..3 {
            print!("   ");
        }
        println!(" {}", mnemonic);
        pc += size;
    }

    println!(
        "\nBank $1B (simple modulo mirror) at offset ${:05X}:",
        bank_1b_offset
    );
    print!("  ");
    for i in 0..32 {
        print!("{:02X} ", rom[bank_1b_offset + i]);
    }
    println!();

    println!("  Disassembly:");
    let mut pc = 0;
    while pc < 32 {
        let op = rom[bank_1b_offset + pc];
        let b1 = if pc + 1 < 32 {
            rom[bank_1b_offset + pc + 1]
        } else {
            0
        };
        let b2 = if pc + 2 < 32 {
            rom[bank_1b_offset + pc + 2]
        } else {
            0
        };
        let (mnemonic, size) = disasm_with_size(op, b1, b2);
        print!("    ${:04X}: ", 0x5CA2 + pc as u16);
        for j in 0..size {
            print!("{:02X} ", rom[bank_1b_offset + pc + j]);
        }
        for _ in size..3 {
            print!("   ");
        }
        println!(" {}", mnemonic);
        pc += size;
    }

    // Also check the code at $CD85 (bank 6, MPR[6]=$03, so ROM bank $03)
    // $CD85 is in bank 6 ($C000-$DFFF), offset = $CD85 - $C000 = $0D85
    let cd85_rom_offset = 0x03 * 8192 + 0x0D85;
    println!(
        "\nSubroutine at $CD85 (ROM bank $03, offset ${:05X}):",
        cd85_rom_offset
    );
    let mut pc = 0;
    while pc < 20 {
        let op = rom[cd85_rom_offset + pc];
        let b1 = if pc + 1 < 20 {
            rom[cd85_rom_offset + pc + 1]
        } else {
            0
        };
        let b2 = if pc + 2 < 20 {
            rom[cd85_rom_offset + pc + 2]
        } else {
            0
        };
        let (mnemonic, size) = disasm_with_size(op, b1, b2);
        print!("    ${:04X}: ", 0xCD85u16.wrapping_add(pc as u16));
        for j in 0..size {
            print!("{:02X} ", rom[cd85_rom_offset + pc + j]);
        }
        for _ in size..3 {
            print!("   ");
        }
        println!(" {}", mnemonic);
        pc += size;
    }

    Ok(())
}

fn disasm_with_size(op: u8, b1: u8, b2: u8) -> (String, usize) {
    match op {
        0x00 => ("BRK".into(), 2),
        0x08 => ("PHP".into(), 1),
        0x09 => (format!("ORA #${:02X}", b1), 2),
        0x0A => ("ASL A".into(), 1),
        0x18 => ("CLC".into(), 1),
        0x1A => ("INC A".into(), 1),
        0x20 => (format!("JSR ${:02X}{:02X}", b2, b1), 3),
        0x28 => ("PLP".into(), 1),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x2A => ("ROL A".into(), 1),
        0x38 => ("SEC".into(), 1),
        0x3A => ("DEC A".into(), 1),
        0x40 => ("RTI".into(), 1),
        0x43 => (format!("TMA #${:02X}", b1), 2),
        0x48 => ("PHA".into(), 1),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4A => ("LSR A".into(), 1),
        0x4C => (format!("JMP ${:02X}{:02X}", b2, b1), 3),
        0x53 => (format!("TAM #${:02X}", b1), 2),
        0x58 => ("CLI".into(), 1),
        0x5A => ("PHY".into(), 1),
        0x60 => ("RTS".into(), 1),
        0x62 => ("CLA".into(), 1),
        0x64 => (format!("STZ ${:02X}", b1), 2),
        0x65 => (format!("ADC ${:02X}", b1), 2),
        0x68 => ("PLA".into(), 1),
        0x69 => (format!("ADC #${:02X}", b1), 2),
        0x6A => ("ROR A".into(), 1),
        0x6C => (format!("JMP (${:02X}{:02X})", b2, b1), 3),
        0x78 => ("SEI".into(), 1),
        0x7A => ("PLY".into(), 1),
        0x7C => (format!("JMP (${:02X}{:02X},X)", b2, b1), 3),
        0x80 => (format!("BRA ${:+}", b1 as i8), 2),
        0x82 => ("CLX".into(), 1),
        0x84 => (format!("STY ${:02X}", b1), 2),
        0x85 => (format!("STA ${:02X}", b1), 2),
        0x86 => (format!("STX ${:02X}", b1), 2),
        0x88 => ("DEY".into(), 1),
        0x89 => (format!("BIT #${:02X}", b1), 2),
        0x8A => ("TXA".into(), 1),
        0x8C => (format!("STY ${:02X}{:02X}", b2, b1), 3),
        0x8D => (format!("STA ${:02X}{:02X}", b2, b1), 3),
        0x8E => (format!("STX ${:02X}{:02X}", b2, b1), 3),
        0x90 => (format!("BCC ${:+}", b1 as i8), 2),
        0x91 => (format!("STA (${:02X}),Y", b1), 2),
        0x92 => (format!("STA (${:02X})", b1), 2),
        0x98 => ("TYA".into(), 1),
        0x99 => (format!("STA ${:02X}{:02X},Y", b2, b1), 3),
        0x9A => ("TXS".into(), 1),
        0x9C => (format!("STZ ${:02X}{:02X}", b2, b1), 3),
        0x9D => (format!("STA ${:02X}{:02X},X", b2, b1), 3),
        0xA0 => (format!("LDY #${:02X}", b1), 2),
        0xA2 => (format!("LDX #${:02X}", b1), 2),
        0xA4 => (format!("LDY ${:02X}", b1), 2),
        0xA5 => (format!("LDA ${:02X}", b1), 2),
        0xA6 => (format!("LDX ${:02X}", b1), 2),
        0xA8 => ("TAY".into(), 1),
        0xA9 => (format!("LDA #${:02X}", b1), 2),
        0xAA => ("TAX".into(), 1),
        0xAC => (format!("LDY ${:02X}{:02X}", b2, b1), 3),
        0xAD => (format!("LDA ${:02X}{:02X}", b2, b1), 3),
        0xAE => (format!("LDX ${:02X}{:02X}", b2, b1), 3),
        0xB0 => (format!("BCS ${:+}", b1 as i8), 2),
        0xB2 => (format!("LDA (${:02X})", b1), 2),
        0xB9 => (format!("LDA ${:02X}{:02X},Y", b2, b1), 3),
        0xC0 => (format!("CPY #${:02X}", b1), 2),
        0xC2 => ("CLY".into(), 1),
        0xC4 => (format!("CPY ${:02X}", b1), 2),
        0xC5 => (format!("CMP ${:02X}", b1), 2),
        0xC6 => (format!("DEC ${:02X}", b1), 2),
        0xC8 => ("INY".into(), 1),
        0xC9 => (format!("CMP #${:02X}", b1), 2),
        0xCA => ("DEX".into(), 1),
        0xCB => ("WAI".into(), 1),
        0xD0 => (format!("BNE ${:+}", b1 as i8), 2),
        0xD4 => ("CSH".into(), 1),
        0xD8 => ("CLD".into(), 1),
        0xDA => ("PHX".into(), 1),
        0xE0 => (format!("CPX #${:02X}", b1), 2),
        0xE4 => (format!("CPX ${:02X}", b1), 2),
        0xE6 => (format!("INC ${:02X}", b1), 2),
        0xE8 => ("INX".into(), 1),
        0xE9 => (format!("SBC #${:02X}", b1), 2),
        0xEA => ("NOP".into(), 1),
        0xF0 => (format!("BEQ ${:+}", b1 as i8), 2),
        0xF8 => ("SED".into(), 1),
        0xFA => ("PLX".into(), 1),
        _ => (format!("??? ${:02X}", op), 1),
    }
}
