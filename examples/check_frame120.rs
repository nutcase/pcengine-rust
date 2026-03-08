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
    let mut last_frame: Option<Vec<u32>> = None;

    // Check multiple frames around the transition
    for target in [110, 115, 118, 120, 125, 128, 130, 135, 300] {
        while frames < target {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                last_frame = Some(f);
                frames += 1;
            }
        }

        let pixels = last_frame.as_ref().unwrap();
        let width = 256;
        let height = pixels.len() / width;

        // Check font H integrity
        let base = 0x148 * 16;
        let w0 = emu.bus.vdc_vram_word(base as u16);
        let font_ok = (w0 & 0xFF) == 0x66;

        // Check CR
        let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
        let bg_en = (cr >> 7) & 1;

        // Sample background and text colors
        let bg = pixels[100 * width + 128]; // center background
        let text_y = 173; // middle of text row
        let text_at_h = if text_y < height {
            pixels[text_y * width + 65]
        } else {
            0
        }; // should be text 'H' area

        // Save PPM for key frames
        if target == 120 || target == 300 {
            let mut data = Vec::with_capacity(width * height * 3);
            for &p in pixels {
                data.push(((p >> 16) & 0xFF) as u8);
                data.push(((p >> 8) & 0xFF) as u8);
                data.push((p & 0xFF) as u8);
            }
            let header = format!("P6\n{} {}\n255\n", width, height);
            let filename = format!("frame_{:03}.ppm", target);
            let mut file = std::fs::File::create(&filename)?;
            file.write_all(header.as_bytes())?;
            file.write_all(&data)?;
            eprintln!("Saved {}", filename);
        }

        println!(
            "Frame {:3}: bg=#{:06X} text=#{:06X} font_H_ok={} CR={:04X} BG={}",
            target,
            bg & 0xFFFFFF,
            text_at_h & 0xFFFFFF,
            font_ok,
            cr,
            bg_en
        );

        // If font is OK and BG enabled and background not white/black, show text row
        if font_ok && bg_en == 1 && bg != 0 && bg != 0xFFFFFF {
            print!("  Text Y=173: ");
            for x in 48..220 {
                let idx = text_y * width + x;
                let p = pixels[idx];
                if p == bg as u32 {
                    print!(".");
                } else if p == 0 {
                    print!(" ");
                } else if (p >> 16) & 0xFF > 200 {
                    print!("W");
                } else {
                    print!("#");
                }
            }
            println!();
        }
    }

    Ok(())
}
