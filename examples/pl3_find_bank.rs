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

    // The game calls JSR $5CA2 after mapping MPR[2] = $4B
    // We need to find which physical bank has a valid function at offset $1CA2
    let offset = 0x1CA2;

    println!(
        "\nLooking for valid function entry at offset ${:04X} in all banks:",
        offset
    );
    for bank in 0..rom_pages {
        let rom_offset = bank * 8192 + offset;
        if rom_offset + 16 > rom.len() {
            break;
        }

        // Check if it looks like a function entry
        let first_byte = rom[rom_offset];
        // Common function entry instructions on HuC6280
        let looks_like_entry = match first_byte {
            0x48 => true, // PHA
            0x08 => true, // PHP
            0xDA => true, // PHX
            0x5A => true, // PHY
            0x78 => true, // SEI
            0x18 => true, // CLC
            0x38 => true, // SEC
            0xD8 => true, // CLD
            0xA9 => true, // LDA #imm
            0xA2 => true, // LDX #imm
            0xA0 => true, // LDY #imm
            0x64 => true, // STZ zp
            0x9C => true, // STZ abs
            0x85 => true, // STA zp
            0x86 => true, // STX zp
            0x84 => true, // STY zp
            _ => false,
        };

        if looks_like_entry {
            let b0 = rom[rom_offset];
            let b1 = rom[rom_offset + 1];
            let b2 = rom[rom_offset + 2];
            let b3 = rom[rom_offset + 3];

            // Extra check: look for JSR or common patterns in first 16 bytes
            let has_jsr = (0..14).any(|i| rom[rom_offset + i] == 0x20);
            let has_rts = (0..20).any(|i| rom[rom_offset + i] == 0x60);

            if has_jsr || has_rts {
                print!("  Bank ${:02X} ({:2}): ", bank, bank);
                for i in 0..16 {
                    print!("{:02X} ", rom[rom_offset + i]);
                }
                print!(" entry=${:02X}", first_byte);
                if has_jsr {
                    print!(" +JSR");
                }
                if has_rts {
                    print!(" +RTS");
                }
                println!();

                // Full disassembly
                let mut pc = 0;
                while pc < 32 {
                    let op = rom[rom_offset + pc];
                    let b1 = if pc + 1 < 32 {
                        rom[rom_offset + pc + 1]
                    } else {
                        0
                    };
                    let b2 = if pc + 2 < 32 {
                        rom[rom_offset + pc + 2]
                    } else {
                        0
                    };
                    let (mnemonic, size) = disasm_full(op, b1, b2);
                    print!("         ${:04X}: ", 0x5CA2 + pc as u16);
                    for j in 0..size {
                        if pc + j < 32 {
                            print!("{:02X} ", rom[rom_offset + pc + j]);
                        } else {
                            print!("?? ");
                        }
                    }
                    for _ in size..3 {
                        print!("   ");
                    }
                    println!("  {}", mnemonic);
                    pc += size;
                    if op == 0x60 || op == 0x40 {
                        break;
                    } // RTS or RTI
                }
                println!();
            }
        }
    }

    // Also show what different mirroring strategies give for bank $4B
    println!("\n--- Mirroring comparison for bank $4B ({}) ---", 0x4B);
    println!(
        "  Simple modulo (% {}): bank ${:02X}",
        rom_pages,
        0x4B % rom_pages
    );
    let pow2 = rom_pages.next_power_of_two();
    println!(
        "  Power-of-2 modulo (% {}): bank ${:02X}",
        pow2,
        0x4B % pow2
    );
    println!(
        "  Bitmask (& {}): bank ${:02X}",
        pow2 - 1,
        0x4B & (pow2 - 1)
    );

    Ok(())
}

fn disasm_full(op: u8, b1: u8, b2: u8) -> (String, usize) {
    match op {
        0x00 => ("BRK".into(), 2),
        0x01 => (format!("ORA (${:02X},X)", b1), 2),
        0x02 => ("SXY".into(), 1),
        0x03 => (format!("ST0 #${:02X}", b1), 2),
        0x04 => (format!("TSB ${:02X}", b1), 2),
        0x05 => (format!("ORA ${:02X}", b1), 2),
        0x06 => (format!("ASL ${:02X}", b1), 2),
        0x07 => (format!("RMB0 ${:02X}", b1), 2),
        0x08 => ("PHP".into(), 1),
        0x09 => (format!("ORA #${:02X}", b1), 2),
        0x0A => ("ASL A".into(), 1),
        0x0C => (format!("TSB ${:02X}{:02X}", b2, b1), 3),
        0x0D => (format!("ORA ${:02X}{:02X}", b2, b1), 3),
        0x0E => (format!("ASL ${:02X}{:02X}", b2, b1), 3),
        0x0F => (format!("BBR0 ${:02X},${:+}", b1, b2 as i8), 3),
        0x10 => (format!("BPL ${:+}", b1 as i8), 2),
        0x11 => (format!("ORA (${:02X}),Y", b1), 2),
        0x12 => (format!("ORA (${:02X})", b1), 2),
        0x13 => (format!("ST1 #${:02X}", b1), 2),
        0x14 => (format!("TRB ${:02X}", b1), 2),
        0x15 => (format!("ORA ${:02X},X", b1), 2),
        0x16 => (format!("ASL ${:02X},X", b1), 2),
        0x17 => (format!("RMB1 ${:02X}", b1), 2),
        0x18 => ("CLC".into(), 1),
        0x19 => (format!("ORA ${:02X}{:02X},Y", b2, b1), 3),
        0x1A => ("INC A".into(), 1),
        0x1D => (format!("ORA ${:02X}{:02X},X", b2, b1), 3),
        0x1E => (format!("ASL ${:02X}{:02X},X", b2, b1), 3),
        0x1F => (format!("BBR1 ${:02X},${:+}", b1, b2 as i8), 3),
        0x20 => (format!("JSR ${:02X}{:02X}", b2, b1), 3),
        0x21 => (format!("AND (${:02X},X)", b1), 2),
        0x22 => ("SAX".into(), 1),
        0x23 => (format!("ST2 #${:02X}", b1), 2),
        0x24 => (format!("BIT ${:02X}", b1), 2),
        0x25 => (format!("AND ${:02X}", b1), 2),
        0x26 => (format!("ROL ${:02X}", b1), 2),
        0x27 => (format!("RMB2 ${:02X}", b1), 2),
        0x28 => ("PLP".into(), 1),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x2A => ("ROL A".into(), 1),
        0x2C => (format!("BIT ${:02X}{:02X}", b2, b1), 3),
        0x2D => (format!("AND ${:02X}{:02X}", b2, b1), 3),
        0x2E => (format!("ROL ${:02X}{:02X}", b2, b1), 3),
        0x2F => (format!("BBR2 ${:02X},${:+}", b1, b2 as i8), 3),
        0x30 => (format!("BMI ${:+}", b1 as i8), 2),
        0x31 => (format!("AND (${:02X}),Y", b1), 2),
        0x32 => (format!("AND (${:02X})", b1), 2),
        0x34 => (format!("BIT ${:02X},X", b1), 2),
        0x35 => (format!("AND ${:02X},X", b1), 2),
        0x36 => (format!("ROL ${:02X},X", b1), 2),
        0x37 => (format!("RMB3 ${:02X}", b1), 2),
        0x38 => ("SEC".into(), 1),
        0x39 => (format!("AND ${:02X}{:02X},Y", b2, b1), 3),
        0x3A => ("DEC A".into(), 1),
        0x3C => (format!("BIT ${:02X}{:02X},X", b2, b1), 3),
        0x3D => (format!("AND ${:02X}{:02X},X", b2, b1), 3),
        0x3E => (format!("ROL ${:02X}{:02X},X", b2, b1), 3),
        0x3F => (format!("BBR3 ${:02X},${:+}", b1, b2 as i8), 3),
        0x40 => ("RTI".into(), 1),
        0x41 => (format!("EOR (${:02X},X)", b1), 2),
        0x42 => ("SAY".into(), 1),
        0x43 => (format!("TMA #${:02X}", b1), 2),
        0x44 => (format!("BSR ${:02X}{:02X}", b2, b1), 3),
        0x45 => (format!("EOR ${:02X}", b1), 2),
        0x46 => (format!("LSR ${:02X}", b1), 2),
        0x47 => (format!("RMB4 ${:02X}", b1), 2),
        0x48 => ("PHA".into(), 1),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4A => ("LSR A".into(), 1),
        0x4C => (format!("JMP ${:02X}{:02X}", b2, b1), 3),
        0x4D => (format!("EOR ${:02X}{:02X}", b2, b1), 3),
        0x4E => (format!("LSR ${:02X}{:02X}", b2, b1), 3),
        0x4F => (format!("BBR4 ${:02X},${:+}", b1, b2 as i8), 3),
        0x50 => (format!("BVC ${:+}", b1 as i8), 2),
        0x51 => (format!("EOR (${:02X}),Y", b1), 2),
        0x52 => (format!("EOR (${:02X})", b1), 2),
        0x53 => (format!("TAM #${:02X}", b1), 2),
        0x54 => ("CSL".into(), 1),
        0x55 => (format!("EOR ${:02X},X", b1), 2),
        0x56 => (format!("LSR ${:02X},X", b1), 2),
        0x57 => (format!("RMB5 ${:02X}", b1), 2),
        0x58 => ("CLI".into(), 1),
        0x59 => (format!("EOR ${:02X}{:02X},Y", b2, b1), 3),
        0x5A => ("PHY".into(), 1),
        0x60 => ("RTS".into(), 1),
        0x61 => (format!("ADC (${:02X},X)", b1), 2),
        0x62 => ("CLA".into(), 1),
        0x64 => (format!("STZ ${:02X}", b1), 2),
        0x65 => (format!("ADC ${:02X}", b1), 2),
        0x66 => (format!("ROR ${:02X}", b1), 2),
        0x67 => (format!("RMB6 ${:02X}", b1), 2),
        0x68 => ("PLA".into(), 1),
        0x69 => (format!("ADC #${:02X}", b1), 2),
        0x6A => ("ROR A".into(), 1),
        0x6C => (format!("JMP (${:02X}{:02X})", b2, b1), 3),
        0x6D => (format!("ADC ${:02X}{:02X}", b2, b1), 3),
        0x6E => (format!("ROR ${:02X}{:02X}", b2, b1), 3),
        0x6F => (format!("BBR6 ${:02X},${:+}", b1, b2 as i8), 3),
        0x70 => (format!("BVS ${:+}", b1 as i8), 2),
        0x71 => (format!("ADC (${:02X}),Y", b1), 2),
        0x72 => (format!("ADC (${:02X})", b1), 2),
        0x73 => (format!("TII ${:02X}{:02X},...", b2, b1), 7),
        0x74 => (format!("STZ ${:02X},X", b1), 2),
        0x75 => (format!("ADC ${:02X},X", b1), 2),
        0x76 => (format!("ROR ${:02X},X", b1), 2),
        0x77 => (format!("RMB7 ${:02X}", b1), 2),
        0x78 => ("SEI".into(), 1),
        0x79 => (format!("ADC ${:02X}{:02X},Y", b2, b1), 3),
        0x7A => ("PLY".into(), 1),
        0x7C => (format!("JMP (${:02X}{:02X},X)", b2, b1), 3),
        0x7D => (format!("ADC ${:02X}{:02X},X", b2, b1), 3),
        0x7E => (format!("ROR ${:02X}{:02X},X", b2, b1), 3),
        0x7F => (format!("BBR7 ${:02X},${:+}", b1, b2 as i8), 3),
        0x80 => (format!("BRA ${:+}", b1 as i8), 2),
        0x81 => (format!("STA (${:02X},X)", b1), 2),
        0x82 => ("CLX".into(), 1),
        0x83 => (format!("TST #${:02X},${:02X}", b1, b2), 3),
        0x84 => (format!("STY ${:02X}", b1), 2),
        0x85 => (format!("STA ${:02X}", b1), 2),
        0x86 => (format!("STX ${:02X}", b1), 2),
        0x87 => (format!("SMB0 ${:02X}", b1), 2),
        0x88 => ("DEY".into(), 1),
        0x89 => (format!("BIT #${:02X}", b1), 2),
        0x8A => ("TXA".into(), 1),
        0x8C => (format!("STY ${:02X}{:02X}", b2, b1), 3),
        0x8D => (format!("STA ${:02X}{:02X}", b2, b1), 3),
        0x8E => (format!("STX ${:02X}{:02X}", b2, b1), 3),
        0x8F => (format!("BBS0 ${:02X},${:+}", b1, b2 as i8), 3),
        0x90 => (format!("BCC ${:+}", b1 as i8), 2),
        0x91 => (format!("STA (${:02X}),Y", b1), 2),
        0x92 => (format!("STA (${:02X})", b1), 2),
        0x93 => (format!("TST #${:02X},${:02X}{:02X}", b1, b2, 0), 4),
        0x94 => (format!("STY ${:02X},X", b1), 2),
        0x95 => (format!("STA ${:02X},X", b1), 2),
        0x96 => (format!("STX ${:02X},Y", b1), 2),
        0x97 => (format!("SMB1 ${:02X}", b1), 2),
        0x98 => ("TYA".into(), 1),
        0x99 => (format!("STA ${:02X}{:02X},Y", b2, b1), 3),
        0x9A => ("TXS".into(), 1),
        0x9C => (format!("STZ ${:02X}{:02X}", b2, b1), 3),
        0x9D => (format!("STA ${:02X}{:02X},X", b2, b1), 3),
        0x9E => (format!("STZ ${:02X}{:02X},X", b2, b1), 3),
        0x9F => (format!("BBS1 ${:02X},${:+}", b1, b2 as i8), 3),
        0xA0 => (format!("LDY #${:02X}", b1), 2),
        0xA1 => (format!("LDA (${:02X},X)", b1), 2),
        0xA2 => (format!("LDX #${:02X}", b1), 2),
        0xA3 => (format!("TST #${:02X},(${:02X},X)", b1, b2), 3),
        0xA4 => (format!("LDY ${:02X}", b1), 2),
        0xA5 => (format!("LDA ${:02X}", b1), 2),
        0xA6 => (format!("LDX ${:02X}", b1), 2),
        0xA7 => (format!("SMB2 ${:02X}", b1), 2),
        0xA8 => ("TAY".into(), 1),
        0xA9 => (format!("LDA #${:02X}", b1), 2),
        0xAA => ("TAX".into(), 1),
        0xAC => (format!("LDY ${:02X}{:02X}", b2, b1), 3),
        0xAD => (format!("LDA ${:02X}{:02X}", b2, b1), 3),
        0xAE => (format!("LDX ${:02X}{:02X}", b2, b1), 3),
        0xAF => (format!("BBS2 ${:02X},${:+}", b1, b2 as i8), 3),
        0xB0 => (format!("BCS ${:+}", b1 as i8), 2),
        0xB1 => (format!("LDA (${:02X}),Y", b1), 2),
        0xB2 => (format!("LDA (${:02X})", b1), 2),
        0xB3 => (format!("TST #${:02X},(${:02X}),Y", b1, b2), 3),
        0xB4 => (format!("LDY ${:02X},X", b1), 2),
        0xB5 => (format!("LDA ${:02X},X", b1), 2),
        0xB6 => (format!("LDX ${:02X},Y", b1), 2),
        0xB7 => (format!("SMB3 ${:02X}", b1), 2),
        0xB8 => ("CLV".into(), 1),
        0xB9 => (format!("LDA ${:02X}{:02X},Y", b2, b1), 3),
        0xBA => ("TSX".into(), 1),
        0xBC => (format!("LDY ${:02X}{:02X},X", b2, b1), 3),
        0xBD => (format!("LDA ${:02X}{:02X},X", b2, b1), 3),
        0xBE => (format!("LDX ${:02X}{:02X},Y", b2, b1), 3),
        0xBF => (format!("BBS3 ${:02X},${:+}", b1, b2 as i8), 3),
        0xC0 => (format!("CPY #${:02X}", b1), 2),
        0xC1 => (format!("CMP (${:02X},X)", b1), 2),
        0xC2 => ("CLY".into(), 1),
        0xC3 => (format!("TDD ..."), 7),
        0xC4 => (format!("CPY ${:02X}", b1), 2),
        0xC5 => (format!("CMP ${:02X}", b1), 2),
        0xC6 => (format!("DEC ${:02X}", b1), 2),
        0xC7 => (format!("SMB4 ${:02X}", b1), 2),
        0xC8 => ("INY".into(), 1),
        0xC9 => (format!("CMP #${:02X}", b1), 2),
        0xCA => ("DEX".into(), 1),
        0xCB => ("WAI".into(), 1),
        0xCC => (format!("CPY ${:02X}{:02X}", b2, b1), 3),
        0xCD => (format!("CMP ${:02X}{:02X}", b2, b1), 3),
        0xCE => (format!("DEC ${:02X}{:02X}", b2, b1), 3),
        0xCF => (format!("BBS4 ${:02X},${:+}", b1, b2 as i8), 3),
        0xD0 => (format!("BNE ${:+}", b1 as i8), 2),
        0xD1 => (format!("CMP (${:02X}),Y", b1), 2),
        0xD2 => (format!("CMP (${:02X})", b1), 2),
        0xD3 => (format!("TIN ..."), 7),
        0xD4 => ("CSH".into(), 1),
        0xD5 => (format!("CMP ${:02X},X", b1), 2),
        0xD6 => (format!("DEC ${:02X},X", b1), 2),
        0xD7 => (format!("SMB5 ${:02X}", b1), 2),
        0xD8 => ("CLD".into(), 1),
        0xD9 => (format!("CMP ${:02X}{:02X},Y", b2, b1), 3),
        0xDA => ("PHX".into(), 1),
        0xDB => ("STP".into(), 1),
        0xE0 => (format!("CPX #${:02X}", b1), 2),
        0xE1 => (format!("SBC (${:02X},X)", b1), 2),
        0xE2 => ("NOP".into(), 1), // undocumented NOP
        0xE3 => (format!("TIA ..."), 7),
        0xE4 => (format!("CPX ${:02X}", b1), 2),
        0xE5 => (format!("SBC ${:02X}", b1), 2),
        0xE6 => (format!("INC ${:02X}", b1), 2),
        0xE7 => (format!("SMB6 ${:02X}", b1), 2),
        0xE8 => ("INX".into(), 1),
        0xE9 => (format!("SBC #${:02X}", b1), 2),
        0xEA => ("NOP".into(), 1),
        0xEC => (format!("CPX ${:02X}{:02X}", b2, b1), 3),
        0xED => (format!("SBC ${:02X}{:02X}", b2, b1), 3),
        0xEE => (format!("INC ${:02X}{:02X}", b2, b1), 3),
        0xEF => (format!("BBS6 ${:02X},${:+}", b1, b2 as i8), 3),
        0xF0 => (format!("BEQ ${:+}", b1 as i8), 2),
        0xF1 => (format!("SBC (${:02X}),Y", b1), 2),
        0xF2 => (format!("SBC (${:02X})", b1), 2),
        0xF3 => (format!("TAI ..."), 7),
        0xF4 => ("SET".into(), 1),
        0xF5 => (format!("SBC ${:02X},X", b1), 2),
        0xF6 => (format!("INC ${:02X},X", b1), 2),
        0xF7 => (format!("SMB7 ${:02X}", b1), 2),
        0xF8 => ("SED".into(), 1),
        0xF9 => (format!("SBC ${:02X}{:02X},Y", b2, b1), 3),
        0xFA => ("PLX".into(), 1),
        0xFB => ("NOP".into(), 1),
        0xFC => ("NOP".into(), 1),
        0xFD => (format!("SBC ${:02X}{:02X},X", b2, b1), 3),
        0xFE => (format!("INC ${:02X}{:02X},X", b2, b1), 3),
        0xFF => (format!("BBS7 ${:02X},${:+}", b1, b2 as i8), 3),
        _ => (format!("??? ${:02X}", op), 1),
    }
}
