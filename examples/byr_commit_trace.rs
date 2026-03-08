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

    // We want to log all writes to register 0x08 (BYR)
    // Let's read the register value before and after each tick
    let mut frames = 0;
    let mut prev_byr = 0xFFFFu16;
    let mut tick_count = 0u64;
    let mut last_frame_end_tick = 0u64;

    println!("Tracing BYR changes, tick by tick, for first 15 frames:");
    while frames < 15 {
        emu.tick();
        tick_count += 1;
        let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
        if byr != prev_byr {
            let scanline = emu.bus.vdc_current_scanline();
            let ticks_in_frame = tick_count - last_frame_end_tick;
            println!(
                "  tick {:8} (frame {} + {:6} ticks, scanline {:3}): BYR {} -> {}",
                tick_count, frames, ticks_in_frame, scanline, prev_byr, byr
            );
            prev_byr = byr;
        }
        if let Some(_) = emu.take_frame() {
            frames += 1;
            let scanline = emu.bus.vdc_current_scanline();
            println!(
                "--- Frame {} complete at tick {} scanline {} BYR={} ---",
                frames, tick_count, scanline, prev_byr
            );
            last_frame_end_tick = tick_count;
        }
    }

    Ok(())
}
