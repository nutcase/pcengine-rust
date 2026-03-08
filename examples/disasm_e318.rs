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

    let mut frames = 0;
    while frames < 150 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Disassemble $E318-$E3A0
    println!("ISR code $E318-$E3A0:");
    let mut addr = 0xE318u16;
    while addr <= 0xE3A0 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let b3 = emu.bus.read(addr.wrapping_add(3));
        let b4 = emu.bus.read(addr.wrapping_add(4));
        let b5 = emu.bus.read(addr.wrapping_add(5));
        let b6 = emu.bus.read(addr.wrapping_add(6));
        let (mnem, size) = disasm_full(op, b1, b2, b3, b4, b5, b6, addr);
        print!("  ${:04X}:", addr);
        for i in 0..size.min(7) {
            print!(" {:02X}", emu.bus.read(addr.wrapping_add(i as u16)));
        }
        for _ in size..4 {
            print!("   ");
        }
        println!("  {}", mnem);
        addr = addr.wrapping_add(size as u16);
        if op == 0x40 {
            break;
        } // RTI
    }

    Ok(())
}

fn disasm_full(
    opcode: u8,
    b1: u8,
    b2: u8,
    b3: u8,
    b4: u8,
    b5: u8,
    b6: u8,
    pc: u16,
) -> (String, usize) {
    match opcode {
        0x00 => ("BRK".into(), 1),
        0x01 => (format!("ORA (${:02X},X)", b1), 2),
        0x02 => ("SXY".into(), 1),
        0x03 => (format!("ST0 #${:02X}", b1), 2),
        0x05 => (format!("ORA ${:02X}", b1), 2),
        0x06 => (format!("ASL ${:02X}", b1), 2),
        0x08 => ("PHP".into(), 1),
        0x09 => (format!("ORA #${:02X}", b1), 2),
        0x0A => ("ASL A".into(), 1),
        0x0D => (format!("ORA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x10 => (
            format!(
                "BPL ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x11 => (format!("ORA (${:02X}),Y", b1), 2),
        0x12 => (format!("ORA (${:02X})", b1), 2),
        0x13 => (format!("ST1 #${:02X}", b1), 2),
        0x18 => ("CLC".into(), 1),
        0x20 => (format!("JSR ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x21 => (format!("AND (${:02X},X)", b1), 2),
        0x22 => ("SAX".into(), 1),
        0x23 => (format!("ST2 #${:02X}", b1), 2),
        0x24 => (format!("BIT ${:02X}", b1), 2),
        0x25 => (format!("AND ${:02X}", b1), 2),
        0x28 => ("PLP".into(), 1),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x2A => ("ROL A".into(), 1),
        0x2C => (format!("BIT ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x2D => (format!("AND ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x30 => (
            format!(
                "BMI ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x38 => ("SEC".into(), 1),
        0x40 => ("RTI".into(), 1),
        0x42 => ("SAY".into(), 1),
        0x43 => (format!("TMA #${:02X}", b1), 2),
        0x48 => ("PHA".into(), 1),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4A => ("LSR A".into(), 1),
        0x4C => (format!("JMP ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x50 => (
            format!(
                "BVC ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x53 => (format!("TAM #${:02X}", b1), 2),
        0x54 => ("CSL".into(), 1),
        0x58 => ("CLI".into(), 1),
        0x5A => ("PHY".into(), 1),
        0x60 => ("RTS".into(), 1),
        0x64 => (format!("STZ ${:02X}", b1), 2),
        0x65 => (format!("ADC ${:02X}", b1), 2),
        0x68 => ("PLA".into(), 1),
        0x69 => (format!("ADC #${:02X}", b1), 2),
        0x6A => ("ROR A".into(), 1),
        0x6C => (format!("JMP (${:04X})", u16::from_le_bytes([b1, b2])), 3),
        0x6D => (format!("ADC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x70 => (
            format!(
                "BVS ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x73 => (
            format!(
                "TII ${:04X},{:04X},{:04X}",
                u16::from_le_bytes([b1, b2]),
                u16::from_le_bytes([b3, b4]),
                u16::from_le_bytes([b5, b6])
            ),
            7,
        ),
        0x78 => ("SEI".into(), 1),
        0x7A => ("PLY".into(), 1),
        0x7C => (format!("JMP (${:04X},X)", u16::from_le_bytes([b1, b2])), 3),
        0x80 => (
            format!(
                "BRA ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x84 => (format!("STY ${:02X}", b1), 2),
        0x85 => (format!("STA ${:02X}", b1), 2),
        0x86 => (format!("STX ${:02X}", b1), 2),
        0x88 => ("DEY".into(), 1),
        0x89 => (format!("BIT #${:02X}", b1), 2),
        0x8A => ("TXA".into(), 1),
        0x8C => (format!("STY ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x8D => (format!("STA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x8E => (format!("STX ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x90 => (
            format!(
                "BCC ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x92 => (format!("STA (${:02X})", b1), 2),
        0x95 => (format!("STA ${:02X},X", b1), 2),
        0x98 => ("TYA".into(), 1),
        0x99 => (format!("STA ${:04X},Y", u16::from_le_bytes([b1, b2])), 3),
        0x9A => ("TXS".into(), 1),
        0x9C => (format!("STZ ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x9D => (format!("STA ${:04X},X", u16::from_le_bytes([b1, b2])), 3),
        0x9E => (format!("STZ ${:04X},X", u16::from_le_bytes([b1, b2])), 3),
        0xA0 => (format!("LDY #${:02X}", b1), 2),
        0xA2 => (format!("LDX #${:02X}", b1), 2),
        0xA5 => (format!("LDA ${:02X}", b1), 2),
        0xA8 => ("TAY".into(), 1),
        0xA9 => (format!("LDA #${:02X}", b1), 2),
        0xAA => ("TAX".into(), 1),
        0xAD => (format!("LDA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xAE => (format!("LDX ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xB0 => (
            format!(
                "BCS ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0xB2 => (format!("LDA (${:02X})", b1), 2),
        0xB5 => (format!("LDA ${:02X},X", b1), 2),
        0xB9 => (format!("LDA ${:04X},Y", u16::from_le_bytes([b1, b2])), 3),
        0xBD => (format!("LDA ${:04X},X", u16::from_le_bytes([b1, b2])), 3),
        0xC0 => (format!("CPY #${:02X}", b1), 2),
        0xC6 => (format!("DEC ${:02X}", b1), 2),
        0xC8 => ("INY".into(), 1),
        0xC9 => (format!("CMP #${:02X}", b1), 2),
        0xCA => ("DEX".into(), 1),
        0xCE => (format!("DEC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xD0 => (
            format!(
                "BNE ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0xD4 => ("CSH".into(), 1), // HuC6280 Change Speed High
        0xDA => ("PHX".into(), 1),
        0xE0 => (format!("CPX #${:02X}", b1), 2),
        0xE5 => (format!("SBC ${:02X}", b1), 2),
        0xE6 => (format!("INC ${:02X}", b1), 2),
        0xE8 => ("INX".into(), 1),
        0xE9 => (format!("SBC #${:02X}", b1), 2),
        0xEA => ("NOP".into(), 1),
        0xED => (format!("SBC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xEE => (format!("INC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xF0 => (
            format!(
                "BEQ ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0xFA => ("PLX".into(), 1),
        _ => (format!("??? (${:02X})", opcode), 1),
    }
}
