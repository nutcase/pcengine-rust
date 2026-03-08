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

    // Run to frame 148 normally
    let mut frames = 0;
    while frames < 148 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Now trace VDC port writes for 2 frames
    // We'll check all ST0/ST1/ST2 writes and BYR changes
    let mut ticks = 0u64;
    let mut prev_byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let mut in_vblank = false;

    while frames < 150 {
        let pc = emu.cpu.pc;
        let cycles = emu.tick();
        ticks += cycles as u64;

        let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
        if byr != prev_byr {
            let scanline = emu.bus.vdc_current_scanline();
            println!(
                "  BYR {:3} -> {:3} at tick {:6} scanline {:3} PC=${:04X}",
                prev_byr, byr, ticks, scanline, pc
            );
            prev_byr = byr;
        }

        if let Some(_) = emu.take_frame() {
            frames += 1;
            let scanline = emu.bus.vdc_current_scanline();
            println!(
                "--- Frame {} at tick {} scanline {} BYR={} ---",
                frames, ticks, scanline, prev_byr
            );
            ticks = 0;
            in_vblank = true;
        }
    }

    Ok(())
}
