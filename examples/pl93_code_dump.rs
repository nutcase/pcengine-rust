use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run for 10 frames to let boot complete
    let mut frames = 0u64;
    let mut total_ticks = 0u64;
    while frames < 10 && total_ticks < 5_000_000 {
        emu.tick();
        total_ticks += 1;
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    println!("=== After {} frames ===", frames);

    // Dump code at $FB8E (where CPU spends 49% time)
    println!("\n=== Code at $FB8E (IRQ2/NMI handler) ===");
    for i in 0..32 {
        let addr = 0xFB8Eu16.wrapping_add(i);
        print!("{:02X} ", emu.bus.read(addr));
        if (i + 1) % 16 == 0 {
            println!();
        }
    }
    println!();

    // Dump code at IRQ1 handler ($FB8F)
    println!("\n=== Code at $FB8F (IRQ1 handler) ===");
    for i in 0..64 {
        let addr = 0xFB8Fu16.wrapping_add(i);
        print!("{:02X} ", emu.bus.read(addr));
        if (i + 1) % 16 == 0 {
            println!();
        }
    }
    println!();

    // Dump code near $FE21 (boot area)
    println!("\n=== Code at $FE00 (boot/init) ===");
    for i in 0..64 {
        let addr = 0xFE00u16.wrapping_add(i);
        print!("{:02X} ", emu.bus.read(addr));
        if (i + 1) % 16 == 0 {
            println!();
        }
    }
    println!();

    // Dump reset vector code at $FAF8
    println!("\n=== Code at $FAF8 (reset entry) ===");
    for i in 0..64 {
        let addr = 0xFAF8u16.wrapping_add(i);
        print!("{:02X} ", emu.bus.read(addr));
        if (i + 1) % 16 == 0 {
            println!();
        }
    }
    println!();

    // Dump timer vector code at $FCBB
    println!("\n=== Code at $FCBB (timer handler) ===");
    for i in 0..32 {
        let addr = 0xFCBBu16.wrapping_add(i);
        print!("{:02X} ", emu.bus.read(addr));
        if (i + 1) % 16 == 0 {
            println!();
        }
    }
    println!();

    // Check what's at the key addresses in $0000-$01FF range
    // These show up as frequently visited PC addresses
    // They're in MPR0 range - check MPR0
    let mpr = emu.bus.mpr_array();
    println!("\n=== MPR state ===");
    for i in 0..8 {
        println!("  MPR{}: ${:02X}", i, mpr[i]);
    }

    // Dump the first few bytes at addresses the CPU visits
    // $0000-$01FF in address space = MPR0 range
    // If MPR0=$FF, this is hardware IO - code shouldn't execute here
    // But maybe the game temporarily remaps MPR0
    println!("\n=== VDC registers (by reading IO port) ===");
    // Read VDC register index
    println!("  VDC address register: ${:04X}", {
        let lo = emu.bus.read(0x0000) as u16;
        let hi = emu.bus.read(0x0001) as u16;
        (hi << 8) | lo
    });

    // Check if there's a main game loop by looking at the game code
    // The game's code starts in MPR2 ($4000-$5FFF) = ROM page 7
    // and MPR4-6 ($8000-$DFFF) = ROM pages 1-3
    println!("\n=== Code at $4000 (MPR2, ROM page {}) ===", mpr[2]);
    for i in 0..64 {
        let addr = 0x4000u16 + i;
        print!("{:02X} ", emu.bus.read(addr));
        if (i + 1) % 16 == 0 {
            println!();
        }
    }
    println!();

    // Look for the game's main VBlank polling loop
    // Common pattern: LDA $0000 (VDC status), AND #$20, BEQ loop
    println!("\n=== Searching for VBlank polling pattern in ROM ===");
    // Check pages mapped to $E000-$FFFF (MPR7)
    for base in (0xE000u16..=0xFFF0).step_by(1) {
        let b0 = emu.bus.read(base);
        let b1 = emu.bus.read(base + 1);
        let b2 = emu.bus.read(base + 2);
        // LDA $0000 = AD 00 00
        if b0 == 0xAD && b1 == 0x00 && b2 == 0x00 {
            print!("  ${:04X}: LDA $0000 → ", base);
            for j in 0..8 {
                print!("{:02X} ", emu.bus.read(base + j));
            }
            println!();
        }
    }

    // Also search in $4000-$DFFF range
    for base in (0x4000u16..=0xDFF0).step_by(1) {
        let b0 = emu.bus.read(base);
        let b1 = emu.bus.read(base + 1);
        let b2 = emu.bus.read(base + 2);
        if b0 == 0xAD && b1 == 0x00 && b2 == 0x00 {
            print!("  ${:04X}: LDA $0000 → ", base);
            for j in 0..8 {
                print!("{:02X} ", emu.bus.read(base + j));
            }
            println!();
        }
    }

    // Dump vectors area
    println!("\n=== Vectors ($FFF0-$FFFF) ===");
    for i in 0..16 {
        print!("{:02X} ", emu.bus.read(0xFFF0 + i));
    }
    println!();

    Ok(())
}
