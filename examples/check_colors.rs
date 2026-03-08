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

    let bg_color = emu.bus.vce_palette_rgb(0);
    let overscan_color = emu.bus.vce_palette_rgb(0x100);
    println!("BG color (palette 0): #{:06X}", bg_color);
    println!("Overscan color (palette 0x100): #{:06X}", overscan_color);
    println!("Same: {}", bg_color == overscan_color);

    // Check frame pixel at (0,0)
    let mut emu2 = Emulator::new();
    emu2.load_hucard(&rom)?;
    emu2.reset();
    let mut last_frame = None;
    let mut frames = 0;
    while frames < 300 {
        emu2.tick();
        if let Some(f) = emu2.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }
    let frame = last_frame.unwrap();
    println!("frame[0] (pixel at 0,0): #{:06X}", frame[0]);

    // What's at the left edge of rows 0-5 (outside building area)?
    for y in 0..10 {
        println!(
            "Row {:2} pixel colors at x=0,128,255: #{:06X} #{:06X} #{:06X}",
            y,
            frame[y * 256],
            frame[y * 256 + 128],
            frame[y * 256 + 255]
        );
    }

    Ok(())
}
