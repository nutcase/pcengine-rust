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

    // Run to frame 147, then trace RCR register changes
    let mut frames = 0;
    while frames < 147 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Now trace VDC register 0x06 (RCR) changes for frames 148-150
    let mut prev_rcr = emu.bus.vdc_register(0x06).unwrap_or(0xFFFF);
    let mut prev_cr = emu.bus.vdc_register(0x05).unwrap_or(0xFFFF);
    println!(
        "Starting trace at frame {}. RCR={:#06X} CR={:#06X}",
        frames, prev_rcr, prev_cr
    );

    while frames < 152 {
        let pc = emu.cpu.pc;
        emu.tick();

        let rcr = emu.bus.vdc_register(0x06).unwrap_or(0);
        let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
        if rcr != prev_rcr {
            println!(
                "  RCR: {:#06X} -> {:#06X} at PC=${:04X} frame={}",
                prev_rcr, rcr, pc, frames
            );
            prev_rcr = rcr;
        }
        if cr != prev_cr {
            println!(
                "  CR: {:#06X} -> {:#06X} at PC=${:04X} frame={}",
                prev_cr, cr, pc, frames
            );
            prev_cr = cr;
        }

        if let Some(_) = emu.take_frame() {
            frames += 1;
            println!("--- Frame {} RCR={:#06X} CR={:#06X} ---", frames, rcr, cr);
        }
    }

    Ok(())
}
