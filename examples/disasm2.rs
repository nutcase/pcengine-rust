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
    while frames < 300 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    // Dump more code regions
    for &(label, addr, len) in &[
        ("Before main loop @E1F0", 0xE1F0u16, 48u16),
        ("RCR handler @E313", 0xE313u16, 80),
        ("Exit IRQ @E380", 0xE380u16, 32),
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

    // Key RAM values
    println!("\nRAM $2200-$2220:");
    for base in (0x2200u16..0x2220).step_by(16) {
        print!("  {:04X}:", base);
        for i in 0..16u16 {
            print!(" {:02X}", emu.bus.read(base + i));
        }
        println!();
    }

    // What does E20D-E212 look like?
    println!("\nDetailed E200-E214:");
    for i in 0..20u16 {
        let a = 0xE200 + i;
        print!(" {:02X}", emu.bus.read(a));
    }
    println!();

    Ok(())
}
