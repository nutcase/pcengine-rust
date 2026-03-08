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

    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vsw = (vpr & 0x001F) as usize;
    let vds = ((vpr >> 8) & 0x00FF) as usize;
    let frame_start = vsw + vds; // = 17

    // Output active rows 0-80 at 3x zoom
    let rows = 80usize;
    let scale = 3;
    let w = 256 * scale;
    let h = rows * scale;
    let mut file = File::create("katoken_zoom_active_top.ppm")?;
    writeln!(file, "P6\n{w} {h}\n255")?;
    for y in 0..rows {
        let fy = frame_start + y;
        for _ in 0..scale {
            for x in 0..256usize {
                let p = if fy < 240 { frame[fy * 256 + x] } else { 0 };
                let r = ((p >> 16) & 0xFF) as u8;
                let g = ((p >> 8) & 0xFF) as u8;
                let b = (p & 0xFF) as u8;
                for _ in 0..scale {
                    file.write_all(&[r, g, b])?;
                }
            }
        }
    }
    println!("wrote katoken_zoom_active_top.ppm (active rows 0-79 at {scale}x)");
    Ok(())
}
