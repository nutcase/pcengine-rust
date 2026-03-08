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

    // Dump raw bytes $E310-$E320
    println!("Raw bytes $E310-$E320:");
    for addr in 0xE310u16..=0xE320 {
        let byte = emu.bus.read(addr);
        print!(" {:02X}", byte);
    }
    println!();

    // Disassemble the critical section
    println!("\nDisassembly $E310-$E320:");
    let mut addr = 0xE310u16;
    while addr <= 0xE320 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnem, size) = match op {
            0xAD => (format!("LDA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
            0x29 => (format!("AND #${:02X}", b1), 2),
            0xF0 => (
                format!(
                    "BEQ ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0xD0 => (
                format!(
                    "BNE ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0x13 => (format!("ST1 #${:02X}", b1), 2),
            0x03 => (format!("ST0 #${:02X}", b1), 2),
            0x23 => (format!("ST2 #${:02X}", b1), 2),
            _ => (format!("??? ({:02X})", op), 1),
        };
        print!("  ${:04X}: {:02X}", addr, op);
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
        println!("  {}", mnem);
        addr = addr.wrapping_add(size as u16);
    }

    // Also check the full ISR section $E2AB-$E320
    println!("\nFull ISR disassembly $E2AB-$E320:");
    addr = 0xE2AB;
    while addr <= 0xE320 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnem, size) = match op {
            0x48 => ("PHA".into(), 1),
            0xDA => ("PHX".into(), 1),
            0x5A => ("PHY".into(), 1),
            0xFA => ("PLX".into(), 1),
            0x7A => ("PLY".into(), 1),
            0x68 => ("PLA".into(), 1),
            0x40 => ("RTI".into(), 1),
            0xAD => (format!("LDA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
            0xA5 => (format!("LDA ${:02X}", b1), 2),
            0x8D => (format!("STA ${:04X}", u16::from_le_bytes([b1, b2])), 3),
            0x85 => (format!("STA ${:02X}", b1), 2),
            0xA9 => (format!("LDA #${:02X}", b1), 2),
            0x29 => (format!("AND #${:02X}", b1), 2),
            0x09 => (format!("ORA #${:02X}", b1), 2),
            0x49 => (format!("EOR #${:02X}", b1), 2),
            0xF0 => (
                format!(
                    "BEQ ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0xD0 => (
                format!(
                    "BNE ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0x03 => (format!("ST0 #${:02X}", b1), 2),
            0x13 => (format!("ST1 #${:02X}", b1), 2),
            0x23 => (format!("ST2 #${:02X}", b1), 2),
            0x20 => (format!("JSR ${:04X}", u16::from_le_bytes([b1, b2])), 3),
            0x60 => ("RTS".into(), 1),
            0x4C => (format!("JMP ${:04X}", u16::from_le_bytes([b1, b2])), 3),
            0x18 => ("CLC".into(), 1),
            0x38 => ("SEC".into(), 1),
            0x69 => (format!("ADC #${:02X}", b1), 2),
            0x65 => (format!("ADC ${:02X}", b1), 2),
            0x6D => (format!("ADC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
            0xE9 => (format!("SBC #${:02X}", b1), 2),
            0xC9 => (format!("CMP #${:02X}", b1), 2),
            0x10 => (
                format!(
                    "BPL ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0x30 => (
                format!(
                    "BMI ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0x80 => (
                format!(
                    "BRA ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0x90 => (
                format!(
                    "BCC ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0xB0 => (
                format!(
                    "BCS ${:04X}",
                    addr.wrapping_add(2).wrapping_add(b1 as i8 as u16)
                ),
                2,
            ),
            0xEA => ("NOP".into(), 1),
            0xE6 => (format!("INC ${:02X}", b1), 2),
            0xC6 => (format!("DEC ${:02X}", b1), 2),
            0xE8 => ("INX".into(), 1),
            0xCA => ("DEX".into(), 1),
            0xC8 => ("INY".into(), 1),
            0x88 => ("DEY".into(), 1),
            0xAA => ("TAX".into(), 1),
            0xA8 => ("TAY".into(), 1),
            0x8A => ("TXA".into(), 1),
            0x98 => ("TYA".into(), 1),
            _ => (format!("??? ({:02X})", op), 1),
        };
        print!("  ${:04X}: {:02X}", addr, op);
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
        println!("  {}", mnem);
        addr = addr.wrapping_add(size as u16);
    }

    Ok(())
}
