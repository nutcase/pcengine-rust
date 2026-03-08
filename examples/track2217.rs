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

    let mut frames = 0u64;
    let mut last_val = 0xFFu8;
    let mut last_report = 0u64;

    while frames < 3600 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
            let val = emu.bus.read(0x2217);
            if val != last_val || (frames - last_report) >= 300 {
                println!(
                    "frame {:5}: $2217={:02X}  CR={:04X}",
                    frames,
                    val,
                    emu.bus.vdc_register(0x05).unwrap_or(0)
                );
                last_val = val;
                last_report = frames;
            }
        }
    }

    Ok(())
}
