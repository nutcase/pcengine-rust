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
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    println!("BYR register = {}", emu.bus.vdc_register(0x08).unwrap_or(0));

    // Check per-line scroll values for the active display lines
    // line_state_index = (active_start_line + row) % 263
    // active_start_line = VSW + VDS = 2 + 15 = 17
    println!("\nPer-line BYR values (framebuffer rows → scanlines):");
    let mut prev_y = 0xFFFF;
    for row in 0..240 {
        let scanline = (17 + row) % 263;
        let (_, y_scroll) = emu.bus.vdc_scroll_line(scanline);
        if y_scroll != prev_y {
            println!(
                "  fb row {:3} (scanline {:3}): BYR={}",
                row, scanline, y_scroll
            );
            prev_y = y_scroll;
        }
    }

    Ok(())
}
