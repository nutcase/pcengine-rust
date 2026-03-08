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

    for &(label, addr, len) in &[
        ("Subroutine E14D", 0xE14Du16, 96u16),
        ("VBlank handler detail E2E0", 0xE2E0u16, 48),
        ("VBlank after joypad E2F3", 0xE2F3u16, 32),
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

    // Check what VBlank handler code references $2217
    println!("\nSearching for references to $2217 (17 22):");
    for base in [0xE100u16, 0xE200, 0xE300, 0xE400, 0xE500] {
        for i in 0..256u16 {
            let a = base + i;
            let b0 = emu.bus.read(a);
            let b1 = emu.bus.read(a.wrapping_add(1));
            let b2 = emu.bus.read(a.wrapping_add(2));
            if b1 == 0x17 && b2 == 0x22 {
                println!("  {:04X}: {:02X} 17 22", a, b0);
            }
        }
    }

    Ok(())
}
