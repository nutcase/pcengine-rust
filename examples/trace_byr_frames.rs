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
    let mut last_byr = 0xFFFFu16;
    while frames < 300 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
            let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
            let rcr = emu.bus.vdc_register(0x06).unwrap_or(0);
            let status = emu.bus.vdc_status_bits();
            if byr != last_byr || frames <= 10 || frames % 50 == 0 {
                println!(
                    "Frame {:4}: BYR={:3} RCR={:#06X} status={:02X} ram_2209={:02X}",
                    frames,
                    byr,
                    rcr,
                    status,
                    emu.bus.read(0x2209)
                );
                last_byr = byr;
            }
        }
    }

    Ok(())
}
