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
    let width = 256;
    let scale = 4;

    let regions: &[(&str, usize, usize, usize, usize)] = &[
        ("text_hiscore", 171, 179, 48, 210),
        ("text_score", 187, 195, 48, 210),
        ("text_push", 203, 211, 48, 210),
        ("text_copyright", 219, 227, 48, 210),
    ];

    for &(name, y0, y1, x0, x1) in regions {
        let rw = x1 - x0;
        let rh = y1 - y0;
        let ow = rw * scale;
        let oh = rh * scale;
        let header = format!("P6\n{} {}\n255\n", ow, oh);
        let mut data = Vec::with_capacity(ow * oh * 3);
        for sy in 0..oh {
            for sx in 0..ow {
                let px = x0 + sx / scale;
                let py = y0 + sy / scale;
                let p = pixels[py * width + px];
                data.push(((p >> 16) & 0xFF) as u8);
                data.push(((p >> 8) & 0xFF) as u8);
                data.push((p & 0xFF) as u8);
            }
        }
        let path = format!("{}.ppm", name);
        let mut file = std::fs::File::create(&path)?;
        file.write_all(header.as_bytes())?;
        file.write_all(&data)?;
        println!("Saved {}", path);
    }
    Ok(())
}
