#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

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
    let frame = last_frame.unwrap();

    // Output frame rows 17-80 zoomed 4x (the active area top)
    let start_row = 17usize;
    let end_row = 80usize;
    let rows = end_row - start_row;
    let scale = 4;
    let out_w = 256 * scale;
    let out_h = rows * scale;

    let mut file = File::create("katoken_top_zoom4x.ppm")?;
    writeln!(file, "P6\n{out_w} {out_h}\n255")?;
    for y in start_row..end_row {
        for _ in 0..scale {
            for x in 0..256usize {
                let p = frame[y * 256 + x];
                let r = ((p >> 16) & 0xFF) as u8;
                let g = ((p >> 8) & 0xFF) as u8;
                let b = (p & 0xFF) as u8;
                for _ in 0..scale {
                    file.write_all(&[r, g, b])?;
                }
            }
        }
    }
    println!("wrote katoken_top_zoom4x.ppm (rows {start_row}-{end_row}, {scale}x zoom)");
    Ok(())
}
