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
    let mut last_frame = None;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    let pixels = last_frame.as_ref().unwrap();
    let width = 256;
    let height = pixels.len() / width;

    // Scan for rows with white pixel clusters (text)
    println!("Scanning {}x{} frame for white text rows:", width, height);
    for y in 0..height {
        let mut white_count = 0;
        for x in 0..width {
            let p = pixels[y * width + x];
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            if r > 200 && g > 200 && b > 200 {
                white_count += 1;
            }
        }
        if white_count > 10 {
            println!("  Y={:3}: {} white pixels", y, white_count);
        }
    }

    Ok(())
}
