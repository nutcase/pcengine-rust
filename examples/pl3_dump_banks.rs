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
    let offset = 0x1CA2; // $5CA2 within bank 2

    println!("Dump of all {} banks at offset ${:04X}:", rom_pages, offset);
    for bank in 0..rom_pages {
        let rom_offset = bank * 8192 + offset;
        print!("  Bank ${:02X}: ", bank);
        for i in 0..24 {
            print!("{:02X} ", rom[rom_offset + i]);
        }
        // Check if it's all zeros or all FFs (likely data/empty)
        let all_zero = (0..24).all(|i| rom[rom_offset + i] == 0x00);
        let all_ff = (0..24).all(|i| rom[rom_offset + i] == 0xFF);
        let has_rts = (0..24).any(|i| rom[rom_offset + i] == 0x60);
        if all_zero {
            print!(" [ZERO]");
        }
        if all_ff {
            print!(" [FF]");
        }
        if has_rts {
            print!(" [+RTS]");
        }
        println!();
    }

    // The $CD85 trampoline loads bank $4B.
    // Maybe the game intended a different byte? Check the bytes around $CD88
    println!("\nROM data around $CD85-$CD9F (bank $03):");
    let cd85_rom = 0x03 * 8192 + 0x0D85;
    for i in 0..32 {
        print!("{:02X} ", rom[cd85_rom + i]);
    }
    println!();

    // Disassemble properly
    println!("\nDisassembly of trampoline at $CD85:");
    let mut pc = 0;
    while pc < 32 {
        let off = cd85_rom + pc;
        let op = rom[off];
        let b1 = rom.get(off + 1).copied().unwrap_or(0);
        let b2 = rom.get(off + 2).copied().unwrap_or(0);
        let size = opcode_size(op);
        print!("  ${:04X}: ", 0xCD85u16.wrapping_add(pc as u16));
        for j in 0..size {
            print!("{:02X} ", rom[off + j]);
        }
        for _ in size..3 {
            print!("   ");
        }
        println!("{}", disasm_simple(op, b1, b2));
        pc += size;
    }

    // Check what bank $0B has at the same offset (power-of-2 mirror of $4B)
    println!("\nDisassembly at bank $0B, offset $1CA2:");
    let base = 0x0B * 8192 + offset;
    let mut pc = 0;
    while pc < 40 {
        let off = base + pc;
        let op = rom[off];
        let b1 = rom.get(off + 1).copied().unwrap_or(0);
        let b2 = rom.get(off + 2).copied().unwrap_or(0);
        let size = opcode_size(op);
        print!("  ${:04X}: ", 0x5CA2u16.wrapping_add(pc as u16));
        for j in 0..size {
            print!("{:02X} ", rom[off + j]);
        }
        for _ in size..3 {
            print!("   ");
        }
        println!("{}", disasm_simple(op, b1, b2));
        pc += size;
        if op == 0x60 || op == 0x40 {
            break;
        }
    }

    // Check other possible banks that could be the target
    // Try bank $2B (43 decimal, which is $4B - $20 = $2B)
    // This would be the case if the ROM is meant to be read from the upper half
    for check_bank in [0x0Bu8, 0x1B, 0x2B, 0x0B + 0x20] {
        if (check_bank as usize) < rom_pages {
            let base = (check_bank as usize) * 8192 + offset;
            println!("\nFirst bytes at bank ${:02X}, offset $1CA2:", check_bank);
            print!("  ");
            for i in 0..16 {
                print!("{:02X} ", rom[base + i]);
            }
            println!();
        }
    }

    Ok(())
}

fn opcode_size(op: u8) -> usize {
    match op {
        // 1-byte instructions
        0x02 | 0x08 | 0x0A | 0x18 | 0x1A | 0x22 | 0x28 | 0x2A | 0x38 | 0x3A | 0x40 | 0x42
        | 0x48 | 0x4A | 0x54 | 0x58 | 0x5A | 0x60 | 0x62 | 0x68 | 0x6A | 0x78 | 0x7A | 0x82
        | 0x88 | 0x8A | 0x98 | 0x9A | 0xA8 | 0xAA | 0xB8 | 0xBA | 0xC2 | 0xC8 | 0xCA | 0xCB
        | 0xD4 | 0xD8 | 0xDA | 0xDB | 0xE8 | 0xEA | 0xE2 | 0xF4 | 0xF8 | 0xFA | 0xFB | 0xFC => 1,
        // 3-byte instructions (absolute, etc.)
        0x0C | 0x0D | 0x0E | 0x19 | 0x1D | 0x1E | 0x20 | 0x2C | 0x2D | 0x2E | 0x39 | 0x3C
        | 0x3D | 0x3E | 0x44 | 0x4C | 0x4D | 0x4E | 0x59 | 0x6C | 0x6D | 0x6E | 0x79 | 0x7C
        | 0x7D | 0x7E | 0x83 | 0x8C | 0x8D | 0x8E | 0x99 | 0x9C | 0x9D | 0x9E | 0xAC | 0xAD
        | 0xAE | 0xB9 | 0xBC | 0xBD | 0xBE | 0xCC | 0xCD | 0xCE | 0xD9 | 0xEC | 0xED | 0xEE
        | 0xF9 | 0xFD | 0xFE => 3,
        // BBR/BBS: 3 bytes
        0x0F | 0x1F | 0x2F | 0x3F | 0x4F | 0x5F | 0x6F | 0x7F | 0x8F | 0x9F | 0xAF | 0xBF
        | 0xCF | 0xDF | 0xEF | 0xFF => 3,
        // 4-byte: TST abs
        0x93 | 0xA3 | 0xB3 => 3, // simplified
        // 7-byte: block transfers
        0x73 | 0xC3 | 0xD3 | 0xE3 | 0xF3 => 7,
        // 2-byte instructions (default for most)
        _ => 2,
    }
}

fn disasm_simple(op: u8, b1: u8, b2: u8) -> String {
    match op {
        0x00 => "BRK".into(),
        0x08 => "PHP".into(),
        0x09 => format!("ORA #${:02X}", b1),
        0x0A => "ASL A".into(),
        0x18 => "CLC".into(),
        0x1A => "INC A".into(),
        0x1D => format!("ORA ${:02X}{:02X},X", b2, b1),
        0x20 => format!("JSR ${:02X}{:02X}", b2, b1),
        0x28 => "PLP".into(),
        0x29 => format!("AND #${:02X}", b1),
        0x38 => "SEC".into(),
        0x40 => "RTI".into(),
        0x43 => format!("TMA #${:02X}", b1),
        0x48 => "PHA".into(),
        0x49 => format!("EOR #${:02X}", b1),
        0x4C => format!("JMP ${:02X}{:02X}", b2, b1),
        0x53 => format!("TAM #${:02X}", b1),
        0x58 => "CLI".into(),
        0x60 => "RTS".into(),
        0x64 => format!("STZ ${:02X}", b1),
        0x65 => format!("ADC ${:02X}", b1),
        0x68 => "PLA".into(),
        0x69 => format!("ADC #${:02X}", b1),
        0x70 => format!("BVS ${:+}", b1 as i8),
        0x78 => "SEI".into(),
        0x80 => format!("BRA ${:+}", b1 as i8),
        0x84 => format!("STY ${:02X}", b1),
        0x85 => format!("STA ${:02X}", b1),
        0x86 => format!("STX ${:02X}", b1),
        0x8D => format!("STA ${:02X}{:02X}", b2, b1),
        0x8E => format!("STX ${:02X}{:02X}", b2, b1),
        0x91 => format!("STA (${:02X}),Y", b1),
        0xA0 => format!("LDY #${:02X}", b1),
        0xA2 => format!("LDX #${:02X}", b1),
        0xA5 => format!("LDA ${:02X}", b1),
        0xA8 => "TAY".into(),
        0xA9 => format!("LDA #${:02X}", b1),
        0xAD => format!("LDA ${:02X}{:02X}", b2, b1),
        0xB0 => format!("BCS ${:+}", b1 as i8),
        0xC6 => format!("DEC ${:02X}", b1),
        0xC8 => "INY".into(),
        0xC9 => format!("CMP #${:02X}", b1),
        0xCA => "DEX".into(),
        0xD0 => format!("BNE ${:+}", b1 as i8),
        0xD8 => "CLD".into(),
        0xDA => "PHX".into(),
        0xE0 => format!("CPX #${:02X}", b1),
        0xED => format!("SBC ${:02X}{:02X}", b2, b1),
        0xF0 => format!("BEQ ${:+}", b1 as i8),
        0xFA => "PLX".into(),
        _ => format!("{:02X} {:02X} {:02X}", op, b1, b2),
    }
}
