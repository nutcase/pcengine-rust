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
use std::io::Write;

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
    let w = 256usize;

    // Crop the text area and scale 4x
    let crop_x = 50;
    let crop_w = 190;
    let crop_y = 168;
    let crop_h = 68;
    let scale = 4;
    let out_w = crop_w * scale;
    let out_h = crop_h * scale;

    let mut out = vec![0u32; out_w * out_h];
    for dy in 0..out_h {
        for dx in 0..out_w {
            let sx = crop_x + dx / scale;
            let sy = crop_y + dy / scale;
            if sx < w && sy < 240 {
                out[dy * out_w + dx] = pixels[sy * w + sx];
            }
        }
    }

    // Write PPM
    let mut f = std::fs::File::create("text_zoom.ppm")?;
    write!(f, "P6\n{} {}\n255\n", out_w, out_h)?;
    for &p in &out {
        let r = ((p >> 16) & 0xFF) as u8;
        let g = ((p >> 8) & 0xFF) as u8;
        let b = (p & 0xFF) as u8;
        f.write_all(&[r, g, b])?;
    }

    println!("Wrote text_zoom.ppm ({}x{})", out_w, out_h);
    Ok(())
}
