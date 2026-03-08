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
    println!("ROM size: {} bytes ({} banks)", rom.len(), rom.len() / 8192);

    // Check ROM at bank 0x17, offset $0400 (ROM offset 0x2E400)
    // This is where the font loading routine reads from ($4400 with MPR2=bank $17)
    let bank17_offset = 0x17 * 0x2000;
    println!("\n=== ROM bank 0x17 (offset 0x{:05X}) ===", bank17_offset);
    println!(
        "bank 0x17 offset $0400 (ROM 0x{:05X}):",
        bank17_offset + 0x0400
    );

    // Dump first 256 bytes at ROM 0x2E400
    let font_start = bank17_offset + 0x0400;
    if font_start + 256 <= rom.len() {
        for row in 0..16 {
            let base = font_start + row * 16;
            print!("  {:05X}: ", base);
            for col in 0..16 {
                print!("{:02X} ", rom[base + col]);
            }
            // Show as ASCII
            print!(" |");
            for col in 0..16 {
                let b = rom[base + col];
                if b >= 0x20 && b < 0x7F {
                    print!("{}", b as char);
                } else {
                    print!(".");
                }
            }
            println!("|");
        }
    } else {
        println!("  Out of ROM range!");
    }

    // Check if this looks like 1bpp font data (8 bytes per char, one plane)
    // Expected: tile '0' = 0x30, offset = (0x30 - some_base) * 8 or * 16
    // The routine loads 53 tiles starting from tile 0x130 in VRAM,
    // reading 16 bytes per tile from the source
    println!("\n=== Checking if font data at bank 0x17 offset $0400 ===");
    println!(
        "(53 tiles × 16 bytes = 848 bytes, ends at 0x{:05X})",
        font_start + 53 * 16
    );

    // Display first 5 tiles as 8x8 bitmaps (using byte 0-7 as plane 0)
    for tile in 0..5 {
        let tile_offset = font_start + tile * 16;
        if tile_offset + 16 <= rom.len() {
            println!("\nTile #{} (ROM 0x{:05X}):", tile, tile_offset);
            for row in 0..8 {
                let byte = rom[tile_offset + row];
                print!("  ");
                for bit in (0..8).rev() {
                    if byte & (1 << bit) != 0 {
                        print!("#");
                    } else {
                        print!(".");
                    }
                }
                println!("  ({:02X})", byte);
            }
        }
    }

    // Also check bank 0x17 offset $0000 (the start of the bank)
    println!(
        "\n=== ROM bank 0x17 start (offset 0x{:05X}) ===",
        bank17_offset
    );
    for row in 0..4 {
        let base = bank17_offset + row * 16;
        print!("  {:05X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", rom[base + col]);
        }
        println!();
    }

    // Now check what the TIA at $E5A9 copies: TIA $4750, $0002, $0160
    // $4750 in MPR2's region. With MPR2=bank $17:
    // $4750 = $4000 + $0750 → bank 0x17 offset $0750 → ROM 0x2E750
    let tia_src = bank17_offset + 0x0750;
    println!(
        "\n=== TIA source at bank 0x17 offset $0750 (ROM 0x{:05X}) ===",
        tia_src
    );
    println!("TIA copies 0x160 bytes = 352 bytes = 11 tiles × 32 bytes");
    if tia_src + 0x160 <= rom.len() {
        for row in 0..8 {
            let base = tia_src + row * 16;
            print!("  {:05X}: ", base);
            for col in 0..16 {
                print!("{:02X} ", rom[base + col]);
            }
            println!();
        }
    }

    // Disassemble $E1D1 (ROM bank 0, offset $01D1)
    // This subroutine maps banks to MPR4/MPR5 (and possibly MPR2?)
    println!("\n=== Disassembly of $E1D1 (ROM offset 0x01D1) ===");
    let base_addr = 0x01D1;
    let mut pc = base_addr;
    let end = base_addr + 48; // disassemble 48 bytes
    while pc < end && pc < rom.len() {
        let opcode = rom[pc];
        let (mnemonic, size) = disasm_opcode(opcode, &rom[pc..]);
        print!("  ${:04X}: ", 0xE000 + (pc - 0) as u16); // bank 0 maps to $E000
        for i in 0..3 {
            if i < size {
                print!("{:02X} ", rom[pc + i]);
            } else {
                print!("   ");
            }
        }
        println!("{}", mnemonic);
        pc += size;
        if mnemonic.starts_with("RTS") || mnemonic.starts_with("JMP") {
            break;
        }
    }

    // Also disassemble $E5D8 (ROM offset $05D8) - the per-tile write routine
    println!("\n=== Disassembly of $E5D8 (ROM offset 0x05D8) ===");
    let base_addr = 0x05D8;
    let mut pc = base_addr;
    let end = base_addr + 48;
    while pc < end && pc < rom.len() {
        let opcode = rom[pc];
        let (mnemonic, size) = disasm_opcode(opcode, &rom[pc..]);
        print!("  ${:04X}: ", 0xE000 + pc as u16);
        for i in 0..3 {
            if i < size {
                print!("{:02X} ", rom[pc + i]);
            } else {
                print!("   ");
            }
        }
        println!("{}", mnemonic);
        pc += size;
        if mnemonic.starts_with("RTS") {
            break;
        }
    }

    // Also check what $4400 reads when MPR2=0xFA
    // Bank 0xFA = RAM page (0xF8 = base RAM)
    // 0xFA - 0xF8 = 2 → RAM page 2 → RAM offset 0x4000
    // But typical PCE only has 8KB RAM (1 page at $F8)
    // So 0xFA might be unmapped or mirror
    println!("\n=== MPR2 bank mapping analysis ===");
    println!("MPR2=0xFA: bank 0xFA ({})", describe_bank(0xFA, rom.len()));
    println!("MPR2=0xFF: bank 0xFF ({})", describe_bank(0xFF, rom.len()));
    println!("MPR2=0x17: bank 0x17 ({})", describe_bank(0x17, rom.len()));

    Ok(())
}

fn describe_bank(bank: u8, rom_len: usize) -> String {
    match bank {
        0xF8 => "Work RAM (8KB)".to_string(),
        0xF9..=0xFB => format!("RAM mirror/extended (bank {:02X})", bank),
        0xFF => "Hardware I/O page".to_string(),
        b if (b as usize) < rom_len / 8192 => {
            format!("ROM offset 0x{:05X}", b as usize * 0x2000)
        }
        _ => format!("Unmapped (bank {:02X})", bank),
    }
}

fn disasm_opcode(opcode: u8, bytes: &[u8]) -> (String, usize) {
    let b1 = bytes.get(1).copied().unwrap_or(0);
    let b2 = bytes.get(2).copied().unwrap_or(0);
    let addr16 = u16::from_le_bytes([b1, b2]);
    let rel = b1 as i8;

    match opcode {
        0x00 => ("BRK".into(), 1),
        0x03 => (format!("ST0 #${:02X}", b1), 2),
        0x04 => (format!("TSB ${:02X}", b1), 2),
        0x09 => (format!("ORA #${:02X}", b1), 2),
        0x0A => ("ASL A".into(), 1),
        0x10 => (
            format!(
                "BPL ${:04X}",
                (bytes.as_ptr() as usize + 2).wrapping_add(rel as usize) as u16
            ),
            2,
        ),
        0x13 => (format!("ST1 #${:02X}", b1), 2),
        0x18 => ("CLC".into(), 1),
        0x1A => ("INC A".into(), 1),
        0x20 => (format!("JSR ${:04X}", addr16), 3),
        0x23 => (format!("ST2 #${:02X}", b1), 2),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x38 => ("SEC".into(), 1),
        0x3A => ("DEC A".into(), 1),
        0x43 => (format!("TMA #${:02X}", b1), 2),
        0x48 => ("PHA".into(), 1),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4A => ("LSR A".into(), 1),
        0x4C => (format!("JMP ${:04X}", addr16), 3),
        0x53 => (format!("TAM #${:02X}", b1), 2),
        0x5A => ("PHY".into(), 1),
        0x60 => ("RTS".into(), 1),
        0x64 => (format!("STZ ${:02X}", b1), 2),
        0x65 => (format!("ADC ${:02X}", b1), 2),
        0x68 => ("PLA".into(), 1),
        0x69 => (format!("ADC #${:02X}", b1), 2),
        0x6C => (format!("JMP (${:04X})", addr16), 3),
        0x7A => ("PLY".into(), 1),
        0x78 => ("SEI".into(), 1),
        0x80 => (format!("BRA ${:04X}", addr16), 2),
        0x84 => (format!("STY ${:02X}", b1), 2),
        0x85 => (format!("STA ${:02X}", b1), 2),
        0x86 => (format!("STX ${:02X}", b1), 2),
        0x88 => ("DEY".into(), 1),
        0x8C => (format!("STY ${:04X}", addr16), 3),
        0x8D => (format!("STA ${:04X}", addr16), 3),
        0x8E => (format!("STX ${:04X}", addr16), 3),
        0x91 => (format!("STA (${:02X}),Y", b1), 2),
        0x92 => (format!("STA (${:02X})", b1), 2),
        0x99 => (format!("STA ${:04X},Y", addr16), 3),
        0x9C => (format!("STZ ${:04X}", addr16), 3),
        0x9D => (format!("STA ${:04X},X", addr16), 3),
        0xA0 => (format!("LDY #${:02X}", b1), 2),
        0xA2 => (format!("LDX #${:02X}", b1), 2),
        0xA5 => (format!("LDA ${:02X}", b1), 2),
        0xA8 => ("TAY".into(), 1),
        0xA9 => (format!("LDA #${:02X}", b1), 2),
        0xAA => ("TAX".into(), 1),
        0xAD => (format!("LDA ${:04X}", addr16), 3),
        0xB1 => (format!("LDA (${:02X}),Y", b1), 2),
        0xB5 => (format!("LDA ${:02X},X", b1), 2),
        0xC0 => (format!("CPY #${:02X}", b1), 2),
        0xC4 => (format!("CPY ${:02X}", b1), 2),
        0xC6 => (format!("DEC ${:02X}", b1), 2),
        0xC8 => ("INY".into(), 1),
        0xC9 => (format!("CMP #${:02X}", b1), 2),
        0xCA => ("DEX".into(), 1),
        0xD0 => {
            // Calculate branch target relative to current position
            (format!("BNE $+{:02X}", b1), 2)
        }
        0xD4 => (format!("CSH (set high speed)"), 1),
        0xD8 => ("CLD".into(), 1),
        0xDA => ("PHX".into(), 1),
        0xE0 => (format!("CPX #${:02X}", b1), 2),
        0xE3 => {
            let src = u16::from_le_bytes([b1, b2]);
            let dst = u16::from_le_bytes([
                bytes.get(3).copied().unwrap_or(0),
                bytes.get(4).copied().unwrap_or(0),
            ]);
            let len = u16::from_le_bytes([
                bytes.get(5).copied().unwrap_or(0),
                bytes.get(6).copied().unwrap_or(0),
            ]);
            (format!("TIA ${:04X},${:04X},${:04X}", src, dst, len), 7)
        }
        0xE6 => (format!("INC ${:02X}", b1), 2),
        0xE8 => ("INX".into(), 1),
        0xF0 => (format!("BEQ $+{:02X}", b1), 2),
        0xFA => ("PLX".into(), 1),
        _ => (format!(".db ${:02X}", opcode), 1),
    }
}
