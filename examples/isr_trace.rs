/// Trace ISR code at vector addresses to understand the music driver.
use pce::emulator::Emulator;
use std::error::Error;

fn disasm_6502(bus: &mut pce::bus::Bus, addr: u16) -> (String, u16) {
    let op = bus.read(addr);
    let (mnem, len) = match op {
        0x00 => ("BRK", 1),
        0x01 => ("ORA (zp,X)", 2),
        0x04 => ("TSB zp", 2),
        0x05 => ("ORA zp", 2),
        0x06 => ("ASL zp", 2),
        0x08 => ("PHP", 1),
        0x09 => ("ORA #imm", 2),
        0x0A => ("ASL A", 1),
        0x10 => ("BPL rel", 2),
        0x18 => ("CLC", 1),
        0x20 => ("JSR abs", 3),
        0x24 => ("BIT zp", 2),
        0x25 => ("AND zp", 2),
        0x28 => ("PLP", 1),
        0x29 => ("AND #imm", 2),
        0x2C => ("BIT abs", 3),
        0x30 => ("BMI rel", 2),
        0x38 => ("SEC", 1),
        0x40 => ("RTI", 1),
        0x45 => ("EOR zp", 2),
        0x48 => ("PHA", 1),
        0x49 => ("EOR #imm", 2),
        0x4A => ("LSR A", 1),
        0x4C => ("JMP abs", 3),
        0x50 => ("BVC rel", 2),
        0x58 => ("CLI", 1),
        0x5A => ("PHY", 1),
        0x60 => ("RTS", 1),
        0x64 => ("STZ zp", 2),
        0x65 => ("ADC zp", 2),
        0x68 => ("PLA", 1),
        0x69 => ("ADC #imm", 2),
        0x6C => ("JMP (abs)", 3),
        0x70 => ("BVS rel", 2),
        0x78 => ("SEI", 1),
        0x7A => ("PLY", 1),
        0x80 => ("BRA rel", 2),
        0x84 => ("STY zp", 2),
        0x85 => ("STA zp", 2),
        0x86 => ("STX zp", 2),
        0x88 => ("DEY", 1),
        0x8A => ("TXA", 1),
        0x8C => ("STY abs", 3),
        0x8D => ("STA abs", 3),
        0x8E => ("STX abs", 3),
        0x90 => ("BCC rel", 2),
        0x95 => ("STA zp,X", 2),
        0x98 => ("TYA", 1),
        0x99 => ("STA abs,Y", 3),
        0x9A => ("TXS", 1),
        0x9C => ("STZ abs", 3),
        0x9D => ("STA abs,X", 3),
        0xA0 => ("LDY #imm", 2),
        0xA2 => ("LDX #imm", 2),
        0xA4 => ("LDY zp", 2),
        0xA5 => ("LDA zp", 2),
        0xA6 => ("LDX zp", 2),
        0xA8 => ("TAY", 1),
        0xA9 => ("LDA #imm", 2),
        0xAA => ("TAX", 1),
        0xAC => ("LDY abs", 3),
        0xAD => ("LDA abs", 3),
        0xAE => ("LDX abs", 3),
        0xB0 => ("BCS rel", 2),
        0xB5 => ("LDA zp,X", 2),
        0xB9 => ("LDA abs,Y", 3),
        0xBD => ("LDA abs,X", 3),
        0xC0 => ("CPY #imm", 2),
        0xC4 => ("CPY zp", 2),
        0xC5 => ("CMP zp", 2),
        0xC6 => ("DEC zp", 2),
        0xC8 => ("INY", 1),
        0xC9 => ("CMP #imm", 2),
        0xCA => ("DEX", 1),
        0xCB => ("WAI", 1),
        0xD0 => ("BNE rel", 2),
        0xD5 => ("CMP zp,X", 2),
        0xDA => ("PHX", 1),
        0xE0 => ("CPX #imm", 2),
        0xE5 => ("SBC zp", 2),
        0xE6 => ("INC zp", 2),
        0xE8 => ("INX", 1),
        0xE9 => ("SBC #imm", 2),
        0xEA => ("NOP", 1),
        0xF0 => ("BEQ rel", 2),
        0xFA => ("PLX", 1),
        0x03 => ("ST0 #imm", 2),
        0x13 => ("ST1 #imm", 2),
        0x23 => ("ST2 #imm", 2),
        0x43 => ("TMA #imm", 2),
        0x53 => ("TAM #imm", 2),
        0x54 => ("CSL", 1),
        0xD4 => ("CSH", 1),
        0x44 => ("BSR rel", 2),
        0x73 => ("TII src,dst,len", 7),
        0xC3 => ("TDD src,dst,len", 7),
        0xD3 => ("TIN src,dst,len", 7),
        0xE3 => ("TIA src,dst,len", 7),
        0xF3 => ("TAI src,dst,len", 7),
        _ => ("???", 1),
    };

    let mut s = format!("{:04X}: {:02X}", addr, op);
    for i in 1..len {
        s += &format!(" {:02X}", bus.read(addr.wrapping_add(i)));
    }
    // Pad to fixed width
    while s.len() < 20 {
        s.push(' ');
    }
    s += &format!("  {}", mnem);

    if len == 2 && mnem.contains("rel") {
        let offset = bus.read(addr + 1) as i8;
        let target = addr.wrapping_add(2).wrapping_add(offset as u16);
        s += &format!(" -> ${:04X}", target);
    } else if len == 2 && mnem.contains("#imm") {
        s += &format!(" ${:02X}", bus.read(addr + 1));
    } else if len == 2 && mnem.contains("zp") {
        s += &format!(" ${:02X}", bus.read(addr + 1));
    } else if len == 3 && (mnem.contains("abs") || mnem.contains("(abs")) {
        let lo = bus.read(addr + 1) as u16;
        let hi = bus.read(addr + 2) as u16;
        s += &format!(" ${:04X}", hi << 8 | lo);
    }

    (s, addr.wrapping_add(len))
}

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run 200 frames
    let mut frames = 0u64;
    while frames < 200 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Read vectors
    let vectors = [
        ("IRQ2/BRK", 0xFFF6u16),
        ("IRQ1/VDC", 0xFFF8),
        ("Timer", 0xFFFA),
        ("NMI", 0xFFFC),
        ("Reset", 0xFFFE),
    ];

    for (name, addr) in &vectors {
        let lo = emu.bus.read(*addr) as u16;
        let hi = emu.bus.read(addr.wrapping_add(1)) as u16;
        let vector = hi << 8 | lo;
        println!("=== {} ISR at ${:04X} ===", name, vector);
        let mut pc = vector;
        for _ in 0..30 {
            let (line, next) = disasm_6502(&mut emu.bus, pc);
            println!("  {}", line);
            // Stop at RTI or RTS
            let op = emu.bus.read(pc);
            if op == 0x40 || op == 0x60 {
                break;
            }
            pc = next;
        }
        println!();
    }

    // Also dump some zero-page state that might be music-related
    println!("=== Zero Page (music state?) ===");
    for row in 0..16 {
        let base = row * 16;
        print!("  ${:02X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    Ok(())
}
