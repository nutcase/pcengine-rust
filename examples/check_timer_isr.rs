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

    // Disassemble timer ISR at $E3A0
    println!("Timer ISR at $E3A0:");
    let mut addr = 0xE3A0u16;
    for _ in 0..30 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnem, size) = disasm(op, b1, b2, addr);
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
        if op == 0x40 || op == 0x60 {
            break;
        }
    }

    // Also disassemble VDC ISR from $E2AA
    println!("\nVDC ISR at $E2AA:");
    addr = 0xE2AAu16;
    for _ in 0..5 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnem, size) = disasm(op, b1, b2, addr);
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

    // Now trace: count how many times PC hits $E3A0 vs $E2AA in one frame
    frames = 0;
    let mut vdc_isr_count = 0u32;
    let mut timer_isr_count = 0u32;
    while frames < 1 {
        let pc = emu.cpu.pc;
        if pc == 0xE2AA || pc == 0xE2AB {
            vdc_isr_count += 1;
        }
        if pc == 0xE3A0 {
            timer_isr_count += 1;
        }
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }
    println!(
        "\nIn 1 frame: VDC ISR entries={}, Timer ISR entries={}",
        vdc_isr_count, timer_isr_count
    );

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
        0xE0 => (format!("CPX #${:02X}", b1), 2),
        0xC0 => (format!("CPY #${:02X}", b1), 2),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x09 => (format!("ORA #${:02X}", b1), 2),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4C => (format!("JMP ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0x6C => (format!("JMP (${:04X})", u16::from_le_bytes([b1, b2])), 3),
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
        0xAA => ("TAX".into(), 1),
        0xA8 => ("TAY".into(), 1),
        0x8A => ("TXA".into(), 1),
        0x98 => ("TYA".into(), 1),
        0x69 => (format!("ADC #${:02X}", b1), 2),
        0x65 => (format!("ADC ${:02X}", b1), 2),
        0x6D => (format!("ADC ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        0xE9 => (format!("SBC #${:02X}", b1), 2),
        0x53 => (format!("TAM #${:02X}", b1), 2),
        0x43 => (format!("TMA #${:02X}", b1), 2),
        0x64 => (format!("STZ ${:02X}", b1), 2),
        0x9C => (format!("STZ ${:04X}", u16::from_le_bytes([b1, b2])), 3),
        _ => (format!("??? (${:02X})", opcode), 1),
    }
}
