use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run for 1,000,000 ticks to let the game initialize
    for _ in 0..1_000_000u64 {
        emu.tick();
        if emu.cpu.halted {
            break;
        }
    }

    // --- Pass 1: Disassemble VBlank ISR at $FB83, collect JSR targets ---
    println!("=== VBlank ISR at $FB83 (up to 80 instructions or RTI) ===");
    let mut addr = 0xFB83u16;
    let mut jsr_targets: Vec<u16> = Vec::new();

    for _ in 0..80 {
        let op = emu.bus.read(addr);
        let b1 = emu.bus.read(addr.wrapping_add(1));
        let b2 = emu.bus.read(addr.wrapping_add(2));
        let (mnemonic, size) = disasm(op, b1, b2, addr);

        // For JSR (0x20), show the actual target address
        let annotation = if op == 0x20 {
            let target = (b1 as u16) | ((b2 as u16) << 8);
            if !jsr_targets.contains(&target) {
                jsr_targets.push(target);
            }
            format!("  ; target=${:04X}", target)
        } else {
            String::new()
        };

        let bytes = match size {
            1 => format!("{:02X}      ", op),
            2 => format!("{:02X} {:02X}   ", op, b1),
            3 => format!("{:02X} {:02X} {:02X}", op, b1, b2),
            _ => format!("{:02X}      ", op),
        };
        println!("  ${:04X}: {} {}{}", addr, bytes, mnemonic, annotation);

        addr = addr.wrapping_add(size as u16);
        if op == 0x40 {
            // RTI
            println!("  --- RTI reached ---");
            break;
        }
    }

    // --- Pass 2: Disassemble each JSR target subroutine ---
    for target in &jsr_targets {
        println!(
            "\n=== Subroutine at ${:04X} (called from VBlank ISR) ===",
            target
        );
        let mut sub_addr = *target;
        for _ in 0..60 {
            let op = emu.bus.read(sub_addr);
            let b1 = emu.bus.read(sub_addr.wrapping_add(1));
            let b2 = emu.bus.read(sub_addr.wrapping_add(2));
            let (mnemonic, size) = disasm(op, b1, b2, sub_addr);

            let annotation = if op == 0x20 {
                let t = (b1 as u16) | ((b2 as u16) << 8);
                format!("  ; target=${:04X}", t)
            } else {
                String::new()
            };

            let bytes = match size {
                1 => format!("{:02X}      ", op),
                2 => format!("{:02X} {:02X}   ", op, b1),
                3 => format!("{:02X} {:02X} {:02X}", op, b1, b2),
                _ => format!("{:02X}      ", op),
            };
            println!("  ${:04X}: {} {}{}", sub_addr, bytes, mnemonic, annotation);

            sub_addr = sub_addr.wrapping_add(size as u16);
            if op == 0x60 {
                // RTS
                println!("  --- RTS reached ---");
                break;
            }
            if op == 0x40 {
                // RTI
                println!("  --- RTI reached ---");
                break;
            }
        }
    }

    Ok(())
}

fn disasm(op: u8, _b1: u8, _b2: u8, _pc: u16) -> (&'static str, u8) {
    match op {
        0x00 => ("BRK", 1),
        0x01 => ("ORA (zp,X)", 2),
        0x02 => ("SXY", 1),
        0x03 => ("ST0 #imm", 2),
        0x04 => ("TSB zp", 2),
        0x05 => ("ORA zp", 2),
        0x06 => ("ASL zp", 2),
        0x07 => ("RMB0 zp", 2),
        0x08 => ("PHP", 1),
        0x09 => ("ORA #imm", 2),
        0x0A => ("ASL A", 1),
        0x0C => ("TSB abs", 3),
        0x0D => ("ORA abs", 3),
        0x0E => ("ASL abs", 3),
        0x0F => ("BBR0 zp,rel", 3),
        0x10 => ("BPL rel", 2),
        0x11 => ("ORA (zp),Y", 2),
        0x12 => ("ORA (zp)", 2),
        0x13 => ("ST1 #imm", 2),
        0x14 => ("TRB zp", 2),
        0x15 => ("ORA zp,X", 2),
        0x16 => ("ASL zp,X", 2),
        0x17 => ("RMB1 zp", 2),
        0x18 => ("CLC", 1),
        0x19 => ("ORA abs,Y", 3),
        0x1A => ("INC A", 1),
        0x1F => ("BBR1 zp,rel", 3),
        0x20 => ("JSR abs", 3),
        0x21 => ("AND (zp,X)", 2),
        0x22 => ("SAX", 1),
        0x23 => ("ST2 #imm", 2),
        0x24 => ("BIT zp", 2),
        0x25 => ("AND zp", 2),
        0x26 => ("ROL zp", 2),
        0x27 => ("RMB2 zp", 2),
        0x28 => ("PLP", 1),
        0x29 => ("AND #imm", 2),
        0x2A => ("ROL A", 1),
        0x2C => ("BIT abs", 3),
        0x2D => ("AND abs", 3),
        0x2E => ("ROL abs", 3),
        0x2F => ("BBR2 zp,rel", 3),
        0x30 => ("BMI rel", 2),
        0x31 => ("AND (zp),Y", 2),
        0x32 => ("AND (zp)", 2),
        0x34 => ("BIT zp,X", 2),
        0x35 => ("AND zp,X", 2),
        0x36 => ("ROL zp,X", 2),
        0x37 => ("RMB3 zp", 2),
        0x38 => ("SEC", 1),
        0x39 => ("AND abs,Y", 3),
        0x3A => ("DEC A", 1),
        0x3C => ("BIT abs,X", 3),
        0x3F => ("BBR3 zp,rel", 3),
        0x40 => ("RTI", 1),
        0x41 => ("EOR (zp,X)", 2),
        0x42 => ("SAY", 1),
        0x43 => ("TMA #imm", 2),
        0x44 => ("BSR rel", 2),
        0x45 => ("EOR zp", 2),
        0x46 => ("LSR zp", 2),
        0x47 => ("RMB4 zp", 2),
        0x48 => ("PHA", 1),
        0x49 => ("EOR #imm", 2),
        0x4A => ("LSR A", 1),
        0x4C => ("JMP abs", 3),
        0x4D => ("EOR abs", 3),
        0x4E => ("LSR abs", 3),
        0x4F => ("BBR4 zp,rel", 3),
        0x50 => ("BVC rel", 2),
        0x51 => ("EOR (zp),Y", 2),
        0x52 => ("EOR (zp)", 2),
        0x53 => ("TAM #imm", 2),
        0x54 => ("CSL", 1),
        0x55 => ("EOR zp,X", 2),
        0x56 => ("LSR zp,X", 2),
        0x57 => ("RMB5 zp", 2),
        0x58 => ("CLI", 1),
        0x59 => ("EOR abs,Y", 3),
        0x5A => ("PHY", 1),
        0x5F => ("BBR5 zp,rel", 3),
        0x60 => ("RTS", 1),
        0x61 => ("ADC (zp,X)", 2),
        0x62 => ("CLA", 1),
        0x64 => ("STZ zp", 2),
        0x65 => ("ADC zp", 2),
        0x66 => ("ROR zp", 2),
        0x67 => ("RMB6 zp", 2),
        0x68 => ("PLA", 1),
        0x69 => ("ADC #imm", 2),
        0x6A => ("ROR A", 1),
        0x6C => ("JMP (abs)", 3),
        0x6D => ("ADC abs", 3),
        0x6E => ("ROR abs", 3),
        0x6F => ("BBR6 zp,rel", 3),
        0x70 => ("BVS rel", 2),
        0x71 => ("ADC (zp),Y", 2),
        0x72 => ("ADC (zp)", 2),
        0x73 => ("TII src,dst,len", 7),
        0x74 => ("STZ zp,X", 2),
        0x75 => ("ADC zp,X", 2),
        0x76 => ("ROR zp,X", 2),
        0x77 => ("RMB7 zp", 2),
        0x78 => ("SEI", 1),
        0x79 => ("ADC abs,Y", 3),
        0x7A => ("PLY", 1),
        0x7C => ("JMP (abs,X)", 3),
        0x80 => ("BRA rel", 2),
        0x81 => ("STA (zp,X)", 2),
        0x82 => ("CLX", 1),
        0x83 => ("TST #imm,zp", 3),
        0x84 => ("STY zp", 2),
        0x85 => ("STA zp", 2),
        0x86 => ("STX zp", 2),
        0x87 => ("SMB0 zp", 2),
        0x88 => ("DEY", 1),
        0x89 => ("BIT #imm", 2),
        0x8A => ("TXA", 1),
        0x8C => ("STY abs", 3),
        0x8D => ("STA abs", 3),
        0x8E => ("STX abs", 3),
        0x8F => ("BBS0 zp,rel", 3),
        0x90 => ("BCC rel", 2),
        0x91 => ("STA (zp),Y", 2),
        0x92 => ("STA (zp)", 2),
        0x93 => ("TST #imm,abs", 4),
        0x94 => ("STY zp,X", 2),
        0x95 => ("STA zp,X", 2),
        0x96 => ("STX zp,Y", 2),
        0x97 => ("SMB1 zp", 2),
        0x98 => ("TYA", 1),
        0x99 => ("STA abs,Y", 3),
        0x9A => ("TXS", 1),
        0x9C => ("STZ abs", 3),
        0x9D => ("STA abs,X", 3),
        0x9E => ("STZ abs,X", 3),
        0x9F => ("BBS1 zp,rel", 3),
        0xA0 => ("LDY #imm", 2),
        0xA1 => ("LDA (zp,X)", 2),
        0xA2 => ("LDX #imm", 2),
        0xA3 => ("TST #imm,zp,X", 3),
        0xA4 => ("LDY zp", 2),
        0xA5 => ("LDA zp", 2),
        0xA6 => ("LDX zp", 2),
        0xA7 => ("SMB2 zp", 2),
        0xA8 => ("TAY", 1),
        0xA9 => ("LDA #imm", 2),
        0xAA => ("TAX", 1),
        0xAC => ("LDY abs", 3),
        0xAD => ("LDA abs", 3),
        0xAE => ("LDX abs", 3),
        0xAF => ("BBS2 zp,rel", 3),
        0xB0 => ("BCS rel", 2),
        0xB1 => ("LDA (zp),Y", 2),
        0xB2 => ("LDA (zp)", 2),
        0xB3 => ("TST #imm,abs,X", 4),
        0xB4 => ("LDY zp,X", 2),
        0xB5 => ("LDA zp,X", 2),
        0xB6 => ("LDX zp,Y", 2),
        0xB7 => ("SMB3 zp", 2),
        0xB8 => ("CLV", 1),
        0xB9 => ("LDA abs,Y", 3),
        0xBA => ("TSX", 1),
        0xBC => ("LDY abs,X", 3),
        0xBD => ("LDA abs,X", 3),
        0xBE => ("LDX abs,Y", 3),
        0xBF => ("BBS3 zp,rel", 3),
        0xC0 => ("CPY #imm", 2),
        0xC1 => ("CMP (zp,X)", 2),
        0xC2 => ("CLY", 1),
        0xC3 => ("TDD src,dst,len", 7),
        0xC4 => ("CPY zp", 2),
        0xC5 => ("CMP zp", 2),
        0xC6 => ("DEC zp", 2),
        0xC7 => ("SMB4 zp", 2),
        0xC8 => ("INY", 1),
        0xC9 => ("CMP #imm", 2),
        0xCA => ("DEX", 1),
        0xCB => ("WAI", 1),
        0xCC => ("CPY abs", 3),
        0xCD => ("CMP abs", 3),
        0xCE => ("DEC abs", 3),
        0xCF => ("BBS4 zp,rel", 3),
        0xD0 => ("BNE rel", 2),
        0xD1 => ("CMP (zp),Y", 2),
        0xD2 => ("CMP (zp)", 2),
        0xD3 => ("TIN src,dst,len", 7),
        0xD4 => ("CSH", 1),
        0xD5 => ("CMP zp,X", 2),
        0xD6 => ("DEC zp,X", 2),
        0xD7 => ("SMB5 zp", 2),
        0xD8 => ("CLD", 1),
        0xD9 => ("CMP abs,Y", 3),
        0xDA => ("PHX", 1),
        0xDF => ("BBS5 zp,rel", 3),
        0xE0 => ("CPX #imm", 2),
        0xE1 => ("SBC (zp,X)", 2),
        0xE3 => ("TIA src,dst,len", 7),
        0xE4 => ("CPX zp", 2),
        0xE5 => ("SBC zp", 2),
        0xE6 => ("INC zp", 2),
        0xE7 => ("SMB6 zp", 2),
        0xE8 => ("INX", 1),
        0xE9 => ("SBC #imm", 2),
        0xEA => ("NOP", 1),
        0xEC => ("CPX abs", 3),
        0xED => ("SBC abs", 3),
        0xEE => ("INC abs", 3),
        0xEF => ("BBS6 zp,rel", 3),
        0xF0 => ("BEQ rel", 2),
        0xF1 => ("SBC (zp),Y", 2),
        0xF2 => ("SBC (zp)", 2),
        0xF3 => ("TAI src,dst,len", 7),
        0xF4 => ("SET", 1),
        0xF5 => ("SBC zp,X", 2),
        0xF6 => ("INC zp,X", 2),
        0xF7 => ("SMB7 zp", 2),
        0xF8 => ("SED", 1),
        0xF9 => ("SBC abs,Y", 3),
        0xFA => ("PLX", 1),
        0xFC => ("???", 1),
        0xFD => ("SBC abs,X", 3),
        0xFE => ("INC abs,X", 3),
        0xFF => ("BBS7 zp,rel", 3),
        _ => ("???", 1),
    }
}
