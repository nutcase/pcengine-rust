use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run for a few frames
    let mut frames = 0u64;
    let mut total_ticks = 0u64;
    while frames < 3 && total_ticks < 5_000_000 {
        emu.tick();
        total_ticks += 1;
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Dump code at $FF00-$FFFF (where the crash happens)
    println!("=== Code at $FF00-$FFFF ===");
    for row in 0..16 {
        let base = 0xFF00u16 + (row * 16);
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    // Dump code at $FDC0-$FE00 (the JSR $FDC6 target)
    println!("\n=== Code at $FDC0-$FE00 ===");
    for row in 0..4 {
        let base = 0xFDC0u16 + (row * 16);
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    // Dump code at $FB20-$FB80 (the boot main sequence)
    println!("\n=== Code at $FB20-$FB90 ===");
    for row in 0..7 {
        let base = 0xFB20u16 + (row * 16);
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    // Also check what ROM page 0 has at $FF00 offset
    // MPR7 = $00, so $E000-$FFFF = ROM page 0
    // ROM page 0 = first 8KB of ROM
    // $FF00-$FFFF in address space = offset $1F00-$1FFF in ROM page 0
    println!("\n=== Raw ROM bytes at page 0, offset $1F00-$1FFF ===");
    let rom_offset = 0x1F00;
    for row in 0..16 {
        let base = rom_offset + row * 16;
        print!("$1F{:02X}: ", (row * 16));
        for col in 0..16 {
            if base + col < rom.len() {
                print!("{:02X} ", rom[base + col]);
            } else {
                print!("?? ");
            }
        }
        println!();
    }

    Ok(())
}
