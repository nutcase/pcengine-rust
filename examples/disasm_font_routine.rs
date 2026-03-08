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

    // Disassemble the complete font loading routine $E583 and its subroutines
    println!("=== $E583: Font loading main routine ===");
    disasm_range(&rom, 0x0583, 0x05B5);

    println!("\n=== $E5D8: Write 8 data words (planes 0-1) ===");
    disasm_range(&rom, 0x05D8, 0x05FD);

    println!("\n=== $E5FD: Write 8 zero words (planes 2-3) ===");
    disasm_range(&rom, 0x05FD, 0x0630);

    println!("\n=== $E609: Set MAWR and select VWR ===");
    disasm_range(&rom, 0x0609, 0x0630);

    println!("\n=== $E1D1: Bank mapping routine ===");
    disasm_range(&rom, 0x01D1, 0x01E0);

    // Check: what's the VDC increment step?
    // The init code at $E06C loads VDC registers from a table.
    // R05 (CR) controls auto-increment.
    // Let's find where R05 is set.
    println!("\n=== Searching for R05 (CR) initialization ===");
    // $E06C is the VDC register init routine. Let's disassemble it.
    disasm_range(&rom, 0x006C, 0x00C0);

    // Check the VDC init table
    // The init routine probably reads from a table. Let's look for it.
    println!("\n=== Raw bytes around ROM 0x0583-0x05B0 ===");
    for i in (0x0583..0x05B5).step_by(16) {
        print!("  {:04X}: ", i);
        for j in 0..16.min(0x05B5 - i) {
            if i + j < rom.len() {
                print!("{:02X} ", rom[i + j]);
            }
        }
        println!();
    }

    println!("\n=== Raw bytes ROM 0x05D8-0x0630 ===");
    for i in (0x05D8..0x0630).step_by(16) {
        print!("  {:04X}: ", i);
        for j in 0..16.min(0x0630 - i) {
            if i + j < rom.len() {
                print!("{:02X} ", rom[i + j]);
            }
        }
        println!();
    }

    Ok(())
}

fn disasm_range(rom: &[u8], start: usize, end: usize) {
    let mut pc = start;
    while pc < end && pc < rom.len() {
        let opcode = rom[pc];
        let remaining = &rom[pc..rom.len().min(pc + 8)];
        let (mnemonic, size) = disasm_65c02(opcode, remaining, 0xE000 + pc as u16);
        print!("  ${:04X}: ", 0xE000u16.wrapping_add(pc as u16));
        for i in 0..size.min(7) {
            print!("{:02X} ", rom[pc + i]);
        }
        for _ in size..4 {
            print!("   ");
        }
        println!("{}", mnemonic);
        pc += size;
        if mnemonic == "RTS" || mnemonic == "RTI" {
            break;
        }
    }
}

fn disasm_65c02(opcode: u8, bytes: &[u8], pc: u16) -> (String, usize) {
    let b1 = bytes.get(1).copied().unwrap_or(0);
    let b2 = bytes.get(2).copied().unwrap_or(0);
    let w = u16::from_le_bytes([b1, b2]);

    match opcode {
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
        0x10 => (format!("BPL ${:04X}", branch_target(pc, b1)), 2),
        0x11 => (format!("ORA (${:02X}),Y", b1), 2),
        0x12 => (format!("ORA (${:02X})", b1), 2),
        0x13 => (format!("ST1 #${:02X}", b1), 2),
        0x18 => ("CLC".into(), 1),
        0x19 => (format!("ORA ${:04X},Y", w), 3),
        0x1A => ("INC A".into(), 1),
        0x20 => (format!("JSR ${:04X}", w), 3),
        0x23 => (format!("ST2 #${:02X}", b1), 2),
        0x24 => (format!("BIT ${:02X}", b1), 2),
        0x25 => (format!("AND ${:02X}", b1), 2),
        0x26 => (format!("ROL ${:02X}", b1), 2),
        0x28 => ("PLP".into(), 1),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x2A => ("ROL A".into(), 1),
        0x2C => (format!("BIT ${:04X}", w), 3),
        0x30 => (format!("BMI ${:04X}", branch_target(pc, b1)), 2),
        0x38 => ("SEC".into(), 1),
        0x3A => ("DEC A".into(), 1),
        0x40 => ("RTI".into(), 1),
        0x43 => (format!("TMA #${:02X}", b1), 2),
        0x44 => (format!("BSR ${:04X}", branch_target(pc, b1)), 2),
        0x45 => (format!("EOR ${:02X}", b1), 2),
        0x48 => ("PHA".into(), 1),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4A => ("LSR A".into(), 1),
        0x4C => (format!("JMP ${:04X}", w), 3),
        0x50 => (format!("BVC ${:04X}", branch_target(pc, b1)), 2),
        0x53 => (format!("TAM #${:02X}", b1), 2),
        0x58 => ("CLI".into(), 1),
        0x5A => ("PHY".into(), 1),
        0x60 => ("RTS".into(), 1),
        0x64 => (format!("STZ ${:02X}", b1), 2),
        0x65 => (format!("ADC ${:02X}", b1), 2),
        0x66 => (format!("ROR ${:02X}", b1), 2),
        0x68 => ("PLA".into(), 1),
        0x69 => (format!("ADC #${:02X}", b1), 2),
        0x6A => ("ROR A".into(), 1),
        0x6C => (format!("JMP (${:04X})", w), 3),
        0x6D => (format!("ADC ${:04X}", w), 3),
        0x70 => (format!("BVS ${:04X}", branch_target(pc, b1)), 2),
        0x78 => ("SEI".into(), 1),
        0x7A => ("PLY".into(), 1),
        0x7C => (format!("JMP (${:04X},X)", w), 3),
        0x80 => (format!("BRA ${:04X}", branch_target(pc, b1)), 2),
        0x84 => (format!("STY ${:02X}", b1), 2),
        0x85 => (format!("STA ${:02X}", b1), 2),
        0x86 => (format!("STX ${:02X}", b1), 2),
        0x88 => ("DEY".into(), 1),
        0x89 => (format!("BIT #${:02X}", b1), 2),
        0x8A => ("TXA".into(), 1),
        0x8C => (format!("STY ${:04X}", w), 3),
        0x8D => (format!("STA ${:04X}", w), 3),
        0x8E => (format!("STX ${:04X}", w), 3),
        0x90 => (format!("BCC ${:04X}", branch_target(pc, b1)), 2),
        0x91 => (format!("STA (${:02X}),Y", b1), 2),
        0x92 => (format!("STA (${:02X})", b1), 2),
        0x94 => (format!("STY ${:02X},X", b1), 2),
        0x95 => (format!("STA ${:02X},X", b1), 2),
        0x98 => ("TYA".into(), 1),
        0x99 => (format!("STA ${:04X},Y", w), 3),
        0x9A => ("TXS".into(), 1),
        0x9C => (format!("STZ ${:04X}", w), 3),
        0x9D => (format!("STA ${:04X},X", w), 3),
        0xA0 => (format!("LDY #${:02X}", b1), 2),
        0xA2 => (format!("LDX #${:02X}", b1), 2),
        0xA4 => (format!("LDY ${:02X}", b1), 2),
        0xA5 => (format!("LDA ${:02X}", b1), 2),
        0xA6 => (format!("LDX ${:02X}", b1), 2),
        0xA8 => ("TAY".into(), 1),
        0xA9 => (format!("LDA #${:02X}", b1), 2),
        0xAA => ("TAX".into(), 1),
        0xAC => (format!("LDY ${:04X}", w), 3),
        0xAD => (format!("LDA ${:04X}", w), 3),
        0xAE => (format!("LDX ${:04X}", w), 3),
        0xB0 => (format!("BCS ${:04X}", branch_target(pc, b1)), 2),
        0xB1 => (format!("LDA (${:02X}),Y", b1), 2),
        0xB5 => (format!("LDA ${:02X},X", b1), 2),
        0xB9 => (format!("LDA ${:04X},Y", w), 3),
        0xBD => (format!("LDA ${:04X},X", w), 3),
        0xC0 => (format!("CPY #${:02X}", b1), 2),
        0xC2 => ("CLY".into(), 1),
        0xC4 => (format!("CPY ${:02X}", b1), 2),
        0xC5 => (format!("CMP ${:02X}", b1), 2),
        0xC6 => (format!("DEC ${:02X}", b1), 2),
        0xC8 => ("INY".into(), 1),
        0xC9 => (format!("CMP #${:02X}", b1), 2),
        0xCA => ("DEX".into(), 1),
        0xD0 => (format!("BNE ${:04X}", branch_target(pc, b1)), 2),
        0xD4 => ("CSH".into(), 1),
        0xD8 => ("CLD".into(), 1),
        0xDA => ("PHX".into(), 1),
        0xE0 => (format!("CPX #${:02X}", b1), 2),
        0xE3 => {
            let src = u16::from_le_bytes([b1, b2]);
            let b3 = bytes.get(3).copied().unwrap_or(0);
            let b4 = bytes.get(4).copied().unwrap_or(0);
            let dst = u16::from_le_bytes([b3, b4]);
            let b5 = bytes.get(5).copied().unwrap_or(0);
            let b6 = bytes.get(6).copied().unwrap_or(0);
            let len = u16::from_le_bytes([b5, b6]);
            (format!("TIA ${:04X},${:04X},${:04X}", src, dst, len), 7)
        }
        0xE4 => (format!("CPX ${:02X}", b1), 2),
        0xE6 => (format!("INC ${:02X}", b1), 2),
        0xE8 => ("INX".into(), 1),
        0xF0 => (format!("BEQ ${:04X}", branch_target(pc, b1)), 2),
        0xF3 => {
            let src = u16::from_le_bytes([b1, b2]);
            let b3 = bytes.get(3).copied().unwrap_or(0);
            let b4 = bytes.get(4).copied().unwrap_or(0);
            let dst = u16::from_le_bytes([b3, b4]);
            let b5 = bytes.get(5).copied().unwrap_or(0);
            let b6 = bytes.get(6).copied().unwrap_or(0);
            let len = u16::from_le_bytes([b5, b6]);
            (format!("TAI ${:04X},${:04X},${:04X}", src, dst, len), 7)
        }
        0xFA => ("PLX".into(), 1),
        _ => (format!(".db ${:02X}", opcode), 1),
    }
}

fn branch_target(pc: u16, offset: u8) -> u16 {
    let signed = offset as i8;
    pc.wrapping_add(2).wrapping_add(signed as u16)
}
