/// Track the music tempo counter at $3E01 and trace what $8F11 does.
use pce::emulator::Emulator;
use std::error::Error;

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

    // Map bank 4 temporarily to read $8F11
    let old_mpr4 = emu.bus.mpr(4);
    emu.bus.set_mpr(4, 4);

    // Disassemble $8F11
    println!("=== $8F11 function (called from music tick) ===");
    let mut addr: u16 = 0x8F11;
    for _ in 0..30 {
        let op = emu.bus.read(addr);
        let (mnem, len) = decode_op(op);
        let mut line = format!("${:04X}: {:02X}", addr, op);
        for i in 1..len {
            line += &format!(" {:02X}", emu.bus.read(addr + i));
        }
        while line.len() < 22 {
            line.push(' ');
        }
        line += mnem;
        if len == 2 {
            let b1 = emu.bus.read(addr + 1);
            if mnem.contains("rel") {
                let target = addr.wrapping_add(2).wrapping_add(b1 as i8 as u16);
                line += &format!(" -> ${:04X}", target);
            } else {
                line += &format!(" ${:02X}", b1);
            }
        } else if len == 3 {
            let lo = emu.bus.read(addr + 1) as u16;
            let hi = emu.bus.read(addr + 2) as u16;
            line += &format!(" ${:04X}", hi << 8 | lo);
        }
        println!("  {}", line);
        if op == 0x60 || op == 0x40 {
            break;
        }
        addr += len;
    }

    // Disassemble $81C1 (actual music processing)
    println!("\n=== $81C1 function (music processing) ===");
    addr = 0x81C1;
    for _ in 0..40 {
        let op = emu.bus.read(addr);
        let (mnem, len) = decode_op(op);
        let mut line = format!("${:04X}: {:02X}", addr, op);
        for i in 1..len {
            line += &format!(" {:02X}", emu.bus.read(addr + i));
        }
        while line.len() < 22 {
            line.push(' ');
        }
        line += mnem;
        if len == 2 {
            let b1 = emu.bus.read(addr + 1);
            if mnem.contains("rel") {
                let target = addr.wrapping_add(2).wrapping_add(b1 as i8 as u16);
                line += &format!(" -> ${:04X}", target);
            } else {
                line += &format!(" ${:02X}", b1);
            }
        } else if len == 3 {
            let lo = emu.bus.read(addr + 1) as u16;
            let hi = emu.bus.read(addr + 2) as u16;
            line += &format!(" ${:04X}", hi << 8 | lo);
        }
        println!("  {}", line);
        if op == 0x60 || op == 0x40 {
            break;
        }
        addr += len;
    }

    // Restore MPR[4]
    emu.bus.set_mpr(4, old_mpr4);

    // Now track the $3E01 counter over 60 frames
    println!("\n=== Tracking $3E01 tempo counter over 60 frames ===");
    let mut prev_counter = emu.bus.read(0x3E01);
    let mut counter_changes = 0u64;
    let mut counter_resets = 0u64; // times it goes back to 0
    let start_frame = frames;

    while frames < start_frame + 60 {
        emu.tick();

        let counter = emu.bus.read(0x3E01);
        if counter != prev_counter {
            if counter == 0 && prev_counter > 0 {
                counter_resets += 1;
            }
            counter_changes += 1;
            prev_counter = counter;
        }

        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    println!(
        "Counter changes: {} ({:.1}/sec)",
        counter_changes, counter_changes as f64
    );
    println!(
        "Counter resets (music updates): {} ({:.1}/sec)",
        counter_resets, counter_resets as f64
    );
    println!("Current counter value: {}", emu.bus.read(0x3E01));

    // Dump $3E00-$3E1F
    println!("\n=== $3E00-$3E1F ===");
    for row in 0..2 {
        let base: u16 = 0x3E00 + row * 16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    Ok(())
}

fn decode_op(op: u8) -> (&'static str, u16) {
    match op {
        0x00 => ("BRK", 1),
        0x01 => ("ORA (zp,X)", 2),
        0x04 => ("TSB zp", 2),
        0x05 => ("ORA zp", 2),
        0x06 => ("ASL zp", 2),
        0x08 => ("PHP", 1),
        0x09 => ("ORA #", 2),
        0x0A => ("ASL A", 1),
        0x0D => ("ORA abs", 3),
        0x0E => ("ASL abs", 3),
        0x10 => ("BPL rel", 2),
        0x11 => ("ORA (zp),Y", 2),
        0x12 => ("ORA (zp)", 2),
        0x15 => ("ORA zp,X", 2),
        0x18 => ("CLC", 1),
        0x19 => ("ORA abs,Y", 3),
        0x1A => ("INC A", 1),
        0x1D => ("ORA abs,X", 3),
        0x20 => ("JSR abs", 3),
        0x24 => ("BIT zp", 2),
        0x25 => ("AND zp", 2),
        0x28 => ("PLP", 1),
        0x29 => ("AND #", 2),
        0x2A => ("ROL A", 1),
        0x2C => ("BIT abs", 3),
        0x2D => ("AND abs", 3),
        0x30 => ("BMI rel", 2),
        0x31 => ("AND (zp),Y", 2),
        0x32 => ("AND (zp)", 2),
        0x35 => ("AND zp,X", 2),
        0x38 => ("SEC", 1),
        0x39 => ("AND abs,Y", 3),
        0x3A => ("DEC A", 1),
        0x3D => ("AND abs,X", 3),
        0x40 => ("RTI", 1),
        0x45 => ("EOR zp", 2),
        0x46 => ("LSR zp", 2),
        0x48 => ("PHA", 1),
        0x49 => ("EOR #", 2),
        0x4A => ("LSR A", 1),
        0x4C => ("JMP abs", 3),
        0x4D => ("EOR abs", 3),
        0x50 => ("BVC rel", 2),
        0x58 => ("CLI", 1),
        0x5A => ("PHY", 1),
        0x60 => ("RTS", 1),
        0x64 => ("STZ zp", 2),
        0x65 => ("ADC zp", 2),
        0x66 => ("ROR zp", 2),
        0x68 => ("PLA", 1),
        0x69 => ("ADC #", 2),
        0x6A => ("ROR A", 1),
        0x6C => ("JMP (abs)", 3),
        0x6D => ("ADC abs", 3),
        0x70 => ("BVS rel", 2),
        0x71 => ("ADC (zp),Y", 2),
        0x72 => ("ADC (zp)", 2),
        0x78 => ("SEI", 1),
        0x7A => ("PLY", 1),
        0x7C => ("JMP (abs,X)", 3),
        0x80 => ("BRA rel", 2),
        0x84 => ("STY zp", 2),
        0x85 => ("STA zp", 2),
        0x86 => ("STX zp", 2),
        0x88 => ("DEY", 1),
        0x89 => ("BIT #", 2),
        0x8A => ("TXA", 1),
        0x8C => ("STY abs", 3),
        0x8D => ("STA abs", 3),
        0x8E => ("STX abs", 3),
        0x90 => ("BCC rel", 2),
        0x91 => ("STA (zp),Y", 2),
        0x92 => ("STA (zp)", 2),
        0x95 => ("STA zp,X", 2),
        0x98 => ("TYA", 1),
        0x99 => ("STA abs,Y", 3),
        0x9A => ("TXS", 1),
        0x9C => ("STZ abs", 3),
        0x9D => ("STA abs,X", 3),
        0xA0 => ("LDY #", 2),
        0xA2 => ("LDX #", 2),
        0xA4 => ("LDY zp", 2),
        0xA5 => ("LDA zp", 2),
        0xA6 => ("LDX zp", 2),
        0xA8 => ("TAY", 1),
        0xA9 => ("LDA #", 2),
        0xAA => ("TAX", 1),
        0xAC => ("LDY abs", 3),
        0xAD => ("LDA abs", 3),
        0xAE => ("LDX abs", 3),
        0xB0 => ("BCS rel", 2),
        0xB1 => ("LDA (zp),Y", 2),
        0xB2 => ("LDA (zp)", 2),
        0xB5 => ("LDA zp,X", 2),
        0xB9 => ("LDA abs,Y", 3),
        0xBD => ("LDA abs,X", 3),
        0xC0 => ("CPY #", 2),
        0xC5 => ("CMP zp", 2),
        0xC6 => ("DEC zp", 2),
        0xC8 => ("INY", 1),
        0xC9 => ("CMP #", 2),
        0xCA => ("DEX", 1),
        0xCB => ("WAI", 1),
        0xCD => ("CMP abs", 3),
        0xCE => ("DEC abs", 3),
        0xD0 => ("BNE rel", 2),
        0xD5 => ("CMP zp,X", 2),
        0xD8 => ("CLD", 1),
        0xDA => ("PHX", 1),
        0xDD => ("CMP abs,X", 3),
        0xE0 => ("CPX #", 2),
        0xE5 => ("SBC zp", 2),
        0xE6 => ("INC zp", 2),
        0xE8 => ("INX", 1),
        0xE9 => ("SBC #", 2),
        0xEA => ("NOP", 1),
        0xEE => ("INC abs", 3),
        0xF0 => ("BEQ rel", 2),
        0xF5 => ("SBC zp,X", 2),
        0xF8 => ("SED", 1),
        0xFA => ("PLX", 1),
        0xFD => ("SBC abs,X", 3),
        0xFE => ("INC abs,X", 3),
        0x03 => ("ST0 #", 2),
        0x13 => ("ST1 #", 2),
        0x23 => ("ST2 #", 2),
        0x43 => ("TMA #", 2),
        0x53 => ("TAM #", 2),
        0x73 => ("TII s,d,l", 7),
        0xC3 => ("TDD s,d,l", 7),
        _ => ("???", 1),
    }
}
