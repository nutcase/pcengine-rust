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

    let mut prev_byr = 0xFFFF;
    let mut frames = 0;
    while frames < 500 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
            let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
            if byr != prev_byr {
                println!("Frame {:3}: BYR changed {} -> {}", frames, prev_byr, byr);
                prev_byr = byr;
            }
        }
    }
    println!(
        "Final BYR at frame 500: {}",
        emu.bus.vdc_register(0x08).unwrap_or(0)
    );

    Ok(())
}
