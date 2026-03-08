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
    while frames < 149 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    let mut prev_byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let mut tick_count = 0u64;
    println!("Starting frame 150 trace. BYR={}", prev_byr);

    while frames < 150 {
        let pc = emu.cpu.pc;
        let scanline = emu.bus.vdc_current_scanline();
        let cycles = emu.tick();
        tick_count += cycles as u64;

        let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
        if byr != prev_byr {
            let new_scanline = emu.bus.vdc_current_scanline();
            println!(
                "  BYR {} -> {} at tick {} scanline {} -> {} PC=${:04X}",
                prev_byr, byr, tick_count, scanline, new_scanline, pc
            );
            prev_byr = byr;
        }

        if let Some(_) = emu.take_frame() {
            frames += 1;
            println!("=== Frame 150 complete at tick {} ===", tick_count);
        }
    }

    // Dump per-line scroll_y values for active display
    println!("\nPer-line scroll_y at key scanlines:");
    for line in [
        0usize, 1, 15, 16, 17, 18, 19, 20, 50, 51, 52, 53, 100, 200, 255, 256, 257,
    ] {
        let (sx, sy) = emu.bus.vdc_scroll_line(line);
        let valid = emu.bus.vdc_scroll_line_valid(line);
        println!(
            "  scanline {:3}: scroll_y={:4} scroll_x={:4} valid={}",
            line, sy, sx, valid
        );
    }

    Ok(())
}
