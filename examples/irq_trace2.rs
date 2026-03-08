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

    // Read interrupt vectors AFTER game has been running
    let irq1 = emu.bus.read_u16(0xFFF6);
    let timer = emu.bus.read_u16(0xFFF8);
    let reset = emu.bus.read_u16(0xFFFE);

    println!("Interrupt vectors at frame 150:");
    println!("  IRQ1 (VDC):  ${:04X}", irq1);
    println!("  Timer:       ${:04X}", timer);
    println!("  Reset:       ${:04X}", reset);

    // Check MPR registers (bank mapping)
    println!("\nMPR bank mapping:");
    for i in 0..8 {
        let bank = emu.bus.mpr(i);
        println!("  MPR{}: bank ${:02X}", i, bank);
    }

    // Dump the first 50 bytes at the IRQ1 handler address
    println!("\nIRQ1 handler at ${:04X}:", irq1);
    let mut addr = irq1;
    for _ in 0..40 {
        let opcode = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnemonic, size) = disasm(opcode, b1, b2, addr);
        print!("  ${:04X}: {:02X}", addr, opcode);
        if size >= 2 {
            print!(" {:02X}", b1);
        } else {
            print!("   ");
        }
        if size >= 3 {
            print!(" {:02X}", b2);
        } else {
            print!("   ");
        }
        println!("  {}", mnemonic);
        addr = addr.wrapping_add(size as u16);
        if opcode == 0x40 {
            break;
        } // RTI
    }

    Ok(())
}

fn disasm(opcode: u8, b1: u8, b2: u8, pc: u16) -> (String, usize) {
    match opcode {
        0x00 => ("BRK".into(), 1),
        0x03 => (format!("ST0 #${:02X}", b1), 2),
        0x13 => (format!("ST1 #${:02X}", b1), 2),
        0x23 => (format!("ST2 #${:02X}", b1), 2),
        0x40 => ("RTI".into(), 1),
        0x48 => ("PHA".into(), 1),
        0x08 => ("PHP".into(), 1),
        0x58 => ("CLI".into(), 1),
        0x78 => ("SEI".into(), 1),
        0x68 => ("PLA".into(), 1),
        0x28 => ("PLP".into(), 1),
        0xDA => ("PHX".into(), 1),
        0x5A => ("PHY".into(), 1),
        0xFA => ("PLX".into(), 1),
        0x7A => ("PLY".into(), 1),
        0x60 => ("RTS".into(), 1),
        0xA9 => (format!("LDA #${:02X}", b1), 2),
        0xA2 => (format!("LDX #${:02X}", b1), 2),
        0xA0 => (format!("LDY #${:02X}", b1), 2),
        0x85 => (format!("STA ${:02X}", b1), 2),
        0x95 => (format!("STA ${:02X},X", b1), 2),
        0x8D => (format!("STA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x9D => (format!("STA ${:04X},X", u16::from_le_bytes([b1, b2])), 3),
        0x99 => (format!("STA ${:04X},Y", u16::from_le_bytes([b1, b2])), 3),
        0x86 => (format!("STX ${:02X}", b1), 2),
        0x84 => (format!("STY ${:02X}", b1), 2),
        0x8E => (format!("STX ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x8C => (format!("STY ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xAD => (format!("LDA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xBD => (format!("LDA ${:04X},X", u16::from_le_bytes([b1, b2])), 3),
        0xB9 => (format!("LDA ${:04X},Y", u16::from_le_bytes([b1, b2])), 3),
        0xA5 => (format!("LDA ${:02X}", b1), 2),
        0xB5 => (format!("LDA ${:02X},X", b1), 2),
        0xC9 => (format!("CMP #${:02X}", b1), 2),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x09 => (format!("ORA #${:02X}", b1), 2),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4C => (format!("JMP ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x6C => (format!("JMP (${:04X})", u16::from_le_bytes([b1, b2])), 3),
        0x7C => (format!("JMP (${:04X},X)", u16::from_le_bytes([b1, b2])), 3),
        0x20 => (format!("JSR ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xF0 => (
            format!(
                "BEQ ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0xD0 => (
            format!(
                "BNE ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x90 => (
            format!(
                "BCC ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0xB0 => (
            format!(
                "BCS ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x10 => (
            format!(
                "BPL ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x30 => (
            format!(
                "BMI ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x80 => (
            format!(
                "BRA ${:04X}",
                pc.wrapping_add(2).wrapping_add(b1 as i8 as u16)
            ),
            2,
        ),
        0x18 => ("CLC".into(), 1),
        0x38 => ("SEC".into(), 1),
        0xE6 => (format!("INC ${:02X}", b1), 2),
        0xC6 => (format!("DEC ${:02X}", b1), 2),
        0xEE => (format!("INC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xCE => (format!("DEC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xE8 => ("INX".into(), 1),
        0xC8 => ("INY".into(), 1),
        0xCA => ("DEX".into(), 1),
        0x88 => ("DEY".into(), 1),
        0xEA => ("NOP".into(), 1),
        0x0A => ("ASL A".into(), 1),
        0x4A => ("LSR A".into(), 1),
        0x2A => ("ROL A".into(), 1),
        0x6A => ("ROR A".into(), 1),
        0xAA => ("TAX".into(), 1),
        0xA8 => ("TAY".into(), 1),
        0x8A => ("TXA".into(), 1),
        0x98 => ("TYA".into(), 1),
        0x64 => (format!("STZ ${:02X}", b1), 2),
        0x9C => (format!("STZ ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        _ => (format!("??? (${:02X})", opcode), 1),
    }
}
