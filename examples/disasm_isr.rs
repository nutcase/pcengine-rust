use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run until the game is set up (a few frames)
    let max_ticks = 1_000_000u64;
    for _ in 0..max_ticks {
        emu.tick();
        if emu.cpu.halted {
            break;
        }
    }

    // Disassemble the VBlank ISR at $FB83
    println!("=== VBlank ISR at $FB83 ===");
    let mut addr = 0xFB83u16;
    for _ in 0..40 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnemonic, size) = disasm(op, b1, b2, addr);
        let bytes = match size {
            1 => format!("{:02X}      ", op),
            2 => format!("{:02X} {:02X}   ", op, b1),
            3 => format!("{:02X} {:02X} {:02X}", op, b1, b2),
            _ => format!("{:02X}      ", op),
        };
        println!("  ${:04X}: {} {}", addr, bytes, mnemonic);
        addr = addr.wrapping_add(size as u16);
        if op == 0x40 || op == 0x60 {
            // RTI or RTS
            break;
        }
    }

    // Also look at the Timer ISR at $FC5F
    println!("\n=== Timer ISR at $FC5F ===");
    addr = 0xFC5Fu16;
    for _ in 0..30 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnemonic, size) = disasm(op, b1, b2, addr);
        let bytes = match size {
            1 => format!("{:02X}      ", op),
            2 => format!("{:02X} {:02X}   ", op, b1),
            3 => format!("{:02X} {:02X} {:02X}", op, b1, b2),
            _ => format!("{:02X}      ", op),
        };
        println!("  ${:04X}: {} {}", addr, bytes, mnemonic);
        addr = addr.wrapping_add(size as u16);
        if op == 0x40 || op == 0x60 {
            break;
        }
    }

    Ok(())
}

fn disasm(op: u8, b1: u8, b2: u8, pc: u16) -> (&'static str, u8) {
    match op {
        0x00 => ("BRK", 1),
        0x08 => ("PHP", 1),
        0x09 => ("ORA #imm", 2),
        0x0A => ("ASL A", 1),
        0x10 => ("BPL rel", 2),
        0x18 => ("CLC", 1),
        0x20 => ("JSR abs", 3),
        0x28 => ("PLP", 1),
        0x29 => ("AND #imm", 2),
        0x2C => ("BIT abs", 3),
        0x30 => ("BMI rel", 2),
        0x40 => ("RTI", 1),
        0x48 => ("PHA", 1),
        0x49 => ("EOR #imm", 2),
        0x4C => ("JMP abs", 3),
        0x58 => ("CLI", 1),
        0x5A => ("PHY", 1),
        0x60 => ("RTS", 1),
        0x68 => ("PLA", 1),
        0x69 => ("ADC #imm", 2),
        0x6C => ("JMP (abs)", 3),
        0x78 => ("SEI", 1),
        0x7A => ("PLY", 1),
        0x80 => ("BRA rel", 2),
        0x85 => ("STA zp", 2),
        0x88 => ("DEY", 1),
        0x8D => ("STA abs", 3),
        0x90 => ("BCC rel", 2),
        0x98 => ("TYA", 1),
        0x99 => ("STA abs,Y", 3),
        0x9C => ("STZ abs", 3),
        0x9D => ("STA abs,X", 3),
        0xA0 => ("LDY #imm", 2),
        0xA2 => ("LDX #imm", 2),
        0xA5 => ("LDA zp", 2),
        0xA8 => ("TAY", 1),
        0xA9 => ("LDA #imm", 2),
        0xAD => ("LDA abs", 3),
        0xAE => ("LDX abs", 3),
        0xB0 => ("BCS rel", 2),
        0xB9 => ("LDA abs,Y", 3),
        0xBD => ("LDA abs,X", 3),
        0xC0 => ("CPY #imm", 2),
        0xC5 => ("CMP zp", 2),
        0xC8 => ("INY", 1),
        0xC9 => ("CMP #imm", 2),
        0xCA => ("DEX", 1),
        0xCD => ("CMP abs", 3),
        0xD0 => ("BNE rel", 2),
        0xDA => ("PHX", 1),
        0xE0 => ("CPX #imm", 2),
        0xE5 => ("SBC zp", 2),
        0xE6 => ("INC zp", 2),
        0xE8 => ("INX", 1),
        0xE9 => ("SBC #imm", 2),
        0xEA => ("NOP", 1),
        0xEE => ("INC abs", 3),
        0xF0 => ("BEQ rel", 2),
        0xF4 => ("SET", 1),
        0xFA => ("PLX", 1),
        0x03 => ("ST0 #imm", 2),
        0x13 => ("ST1 #imm", 2),
        0x23 => ("ST2 #imm", 2),
        0x43 => ("TMA #imm", 2),
        0x53 => ("TAM #imm", 2),
        0xCB => ("WAI", 1),
        0xD4 => ("CSH", 1),
        0x54 => ("CSL", 1),
        _ => ("???", 1),
    }
}
