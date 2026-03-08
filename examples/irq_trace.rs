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

    // Read interrupt vectors
    let irq1 = emu.bus.read_u16(0xFFF6); // VDC IRQ
    let timer = emu.bus.read_u16(0xFFF8); // Timer IRQ
    let irq2 = emu.bus.read_u16(0xFFFA); // External IRQ
    let nmi = emu.bus.read_u16(0xFFFC); // NMI
    let reset = emu.bus.read_u16(0xFFFE); // Reset

    println!("Interrupt vectors:");
    println!("  IRQ1 (VDC):  ${:04X}", irq1);
    println!("  Timer:       ${:04X}", timer);
    println!("  IRQ2 (ext):  ${:04X}", irq2);
    println!("  NMI:         ${:04X}", nmi);
    println!("  Reset:       ${:04X}", reset);

    // Dump the first 50 bytes of the IRQ1 handler
    println!("\nIRQ1 handler disassembly at ${:04X}:", irq1);
    let mut addr = irq1;
    for _ in 0..30 {
        let opcode = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));

        let (mnemonic, size) = decode_instruction(opcode, b1, b2);
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
        // Stop if we hit RTI
        if opcode == 0x40 {
            break;
        }
    }

    Ok(())
}

fn decode_instruction(opcode: u8, b1: u8, b2: u8) -> (String, usize) {
    match opcode {
        0x00 => ("BRK".to_string(), 1),
        0x03 => (format!("ST0 #${:02X}", b1), 2),
        0x13 => (format!("ST1 #${:02X}", b1), 2),
        0x23 => (format!("ST2 #${:02X}", b1), 2),
        0x40 => ("RTI".to_string(), 1),
        0x48 => ("PHA".to_string(), 1),
        0x08 => ("PHP".to_string(), 1),
        0x58 => ("CLI".to_string(), 1),
        0x78 => ("SEI".to_string(), 1),
        0x68 => ("PLA".to_string(), 1),
        0x28 => ("PLP".to_string(), 1),
        0xDA => ("PHX".to_string(), 1),
        0x5A => ("PHY".to_string(), 1),
        0xFA => ("PLX".to_string(), 1),
        0x7A => ("PLY".to_string(), 1),
        0x60 => ("RTS".to_string(), 1),
        0xA9 => (format!("LDA #${:02X}", b1), 2),
        0xA2 => (format!("LDX #${:02X}", b1), 2),
        0xA0 => (format!("LDY #${:02X}", b1), 2),
        0x85 => (format!("STA ${:02X}", b1), 2),
        0x95 => (format!("STA ${:02X},X", b1), 2),
        0x8D => (format!("STA ${:02X}{:02X}", b2, b1), 3),
        0x9D => (format!("STA ${:02X}{:02X},X", b2, b1), 3),
        0x99 => (format!("STA ${:02X}{:02X},Y", b2, b1), 3),
        0x86 => (format!("STX ${:02X}", b1), 2),
        0x84 => (format!("STY ${:02X}", b1), 2),
        0xAD => (format!("LDA ${:02X}{:02X}", b2, b1), 3),
        0xBD => (format!("LDA ${:02X}{:02X},X", b2, b1), 3),
        0xB9 => (format!("LDA ${:02X}{:02X},Y", b2, b1), 3),
        0xA5 => (format!("LDA ${:02X}", b1), 2),
        0xB5 => (format!("LDA ${:02X},X", b1), 2),
        0xE9 => (format!("SBC #${:02X}", b1), 2),
        0xC9 => (format!("CMP #${:02X}", b1), 2),
        0xE0 => (format!("CPX #${:02X}", b1), 2),
        0xC0 => (format!("CPY #${:02X}", b1), 2),
        0x29 => (format!("AND #${:02X}", b1), 2),
        0x09 => (format!("ORA #${:02X}", b1), 2),
        0x49 => (format!("EOR #${:02X}", b1), 2),
        0x4C => (format!("JMP ${:02X}{:02X}", b2, b1), 3),
        0x6C => (format!("JMP (${:02X}{:02X})", b2, b1), 3),
        0x20 => (format!("JSR ${:02X}{:02X}", b2, b1), 3),
        0xF0 => (format!("BEQ ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0xD0 => (format!("BNE ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0x90 => (format!("BCC ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0xB0 => (format!("BCS ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0x10 => (format!("BPL ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0x30 => (format!("BMI ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0x80 => (format!("BRA ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0x18 => ("CLC".to_string(), 1),
        0x38 => ("SEC".to_string(), 1),
        0xE6 => (format!("INC ${:02X}", b1), 2),
        0xC6 => (format!("DEC ${:02X}", b1), 2),
        0xEE => (format!("INC ${:02X}{:02X}", b2, b1), 3),
        0xCE => (format!("DEC ${:02X}{:02X}", b2, b1), 3),
        0xE8 => ("INX".to_string(), 1),
        0xC8 => ("INY".to_string(), 1),
        0xCA => ("DEX".to_string(), 1),
        0x88 => ("DEY".to_string(), 1),
        0xEA => ("NOP".to_string(), 1),
        0x0A => ("ASL A".to_string(), 1),
        0x4A => ("LSR A".to_string(), 1),
        0x2A => ("ROL A".to_string(), 1),
        0x6A => ("ROR A".to_string(), 1),
        0xAA => ("TAX".to_string(), 1),
        0xA8 => ("TAY".to_string(), 1),
        0x8A => ("TXA".to_string(), 1),
        0x98 => ("TYA".to_string(), 1),
        0xBA => ("TSX".to_string(), 1),
        0x9A => ("TXS".to_string(), 1),
        0x44 => (format!("BSR ${:04X}", (addr_rel(b1) as i32 + 2) as u16), 2),
        0x73 => (
            format!("TII ${:02X}{:02X},${:04X},${:04X}", b2, b1, 0, 0),
            7,
        ),
        _ => (format!("??? (${:02X})", opcode), 1),
    }
}

fn addr_rel(offset: u8) -> i16 {
    offset as i8 as i16
}
