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
    // Run to frame 300 to get correct bank mapping
    let mut frames = 0;
    while frames < 300 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }
    println!("MPR: {:?}", emu.bus.mpr_array());

    // Dump bytes at key addresses
    for &(label, addr, len) in &[
        ("Main loop @E210", 0xE210u16, 48u16),
        ("IRQ1 handler @E2AA", 0xE2AA, 80),
        ("After IRQ1 @E300", 0xE300, 48),
    ] {
        println!("\n{}:", label);
        for i in 0..len {
            let a = addr.wrapping_add(i);
            let b = emu.bus.read(a);
            if i % 16 == 0 {
                print!("  {:04X}:", a);
            }
            print!(" {:02X}", b);
            if i % 16 == 15 || i == len - 1 {
                println!();
            }
        }
    }

    // Also dump the first 16 zero-page bytes used by the game
    println!("\nZero page:");
    for base in (0x2000u16..0x2060).step_by(16) {
        print!("  {:04X}:", base);
        for i in 0..16u16 {
            print!(" {:02X}", emu.bus.read(base + i));
        }
        println!();
    }

    Ok(())
}
