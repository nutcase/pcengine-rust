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
    while frames < 10 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    // Search for "56 E1" (address E156) and "4D E1" (address E14D) in code
    // JSR = 20, JMP = 4C
    println!("References to E156 ($2217=0 path):");
    for bank_base in (0x0000u32..0x10000).step_by(0x2000) {
        for i in 0..0x2000u32 {
            let a = (bank_base + i) as u16;
            let b0 = emu.bus.read(a);
            let b1 = emu.bus.read(a.wrapping_add(1));
            let b2 = emu.bus.read(a.wrapping_add(2));
            if (b0 == 0x20 || b0 == 0x4C) && b1 == 0x56 && b2 == 0xE1 {
                println!(
                    "  {:04X}: {:02X} 56 E1  ({})",
                    a,
                    b0,
                    if b0 == 0x20 { "JSR" } else { "JMP" }
                );
            }
        }
    }

    // Also check the code around E130-E160
    println!("\nCode E120-E160:");
    for i in 0..64u16 {
        let a = 0xE120 + i;
        let b = emu.bus.read(a);
        if i % 16 == 0 {
            print!("  {:04X}:", a);
        }
        print!(" {:02X}", b);
        if i % 16 == 15 {
            println!();
        }
    }

    // Search in broader ROM area via different banks
    println!("\nSearching all accessible banks:");
    let mprs = emu.bus.mpr_array();
    println!("Current MPR: {:?}", mprs);

    // The subroutine E156 is in the E000-FFFF range (MPR7 bank)
    // Check if other banks reference it via absolute addressing
    // Since JSR/JMP uses absolute addressing within the 64K space,
    // the caller must be in a context where E156 is reachable

    Ok(())
}
