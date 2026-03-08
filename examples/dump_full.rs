#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::{env, error::Error, fs::File, io::Write, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut last_frame = None;
    let mut frames = 0;
    while frames < 150 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    let frame = last_frame.unwrap();
    // Write full 240-line frame (no crop)
    let mut file = File::create("katoken_full240.ppm")?;
    writeln!(file, "P6\n256 240\n255")?;
    for y in 0..240 {
        for x in 0..256 {
            let pixel = frame[y * 256 + x];
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            file.write_all(&[r, g, b])?;
        }
    }
    println!("Wrote katoken_full240.ppm");

    // Also analyze key rows
    let bg_color = 0x242491u32; // known BG color
    for y in 0..240 {
        let mut non_bg = 0;
        for x in 0..256 {
            if frame[y * 256 + x] != bg_color {
                non_bg += 1;
            }
        }
        if y < 5 || y > 235 || (non_bg > 0 && non_bg < 30) {
            println!("Row {:3}: {:4} non-bg pixels", y, non_bg);
        }
    }

    Ok(())
}
