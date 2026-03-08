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

    // Run to frame 10 (font should be intact, BAT should be set up)
    let mut frames = 0;
    let mut last_frame: Option<Vec<u32>> = None;
    while frames < 10 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    // Save frame 10
    if let Some(ref pixels) = last_frame {
        let width = 256;
        let height = pixels.len() / width;
        let mut data = Vec::with_capacity(width * height * 3);
        for &p in pixels {
            data.push(((p >> 16) & 0xFF) as u8);
            data.push(((p >> 8) & 0xFF) as u8);
            data.push((p & 0xFF) as u8);
        }
        let header = format!("P6\n{} {}\n255\n", width, height);
        let mut file = std::fs::File::create("frame_title_10.ppm")?;
        file.write_all(header.as_bytes())?;
        file.write_all(&data)?;
        println!("Saved frame_title_10.ppm ({}x{})", width, height);
    }

    // Also check text area pixel content
    let pixels = last_frame.as_ref().unwrap();
    let width = 256;
    let height = pixels.len() / width;

    println!("\n=== Text at Y=172 (HISCORE row) ===");
    for y in 172..180.min(height) {
        print!("  Y={:3}: ", y);
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            if r == 0 && g == 0 && b == 0 {
                print!(" ");
            }
            // black/transparent
            else if r > 200 && g > 200 && b > 200 {
                print!("W");
            }
            // white
            else if r == 0x24 && g == 0x24 && b == 0x91 {
                print!(".");
            }
            // background blue
            else {
                print!("#");
            } // other non-background
        }
        println!();
    }

    println!("\n=== Text at Y=204 (PUSH RUN BUTTON row) ===");
    for y in 204..212.min(height) {
        print!("  Y={:3}: ", y);
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            if r == 0 && g == 0 && b == 0 {
                print!(" ");
            } else if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r == 0x24 && g == 0x24 && b == 0x91 {
                print!(".");
            } else {
                print!("#");
            }
        }
        println!();
    }

    println!("\n=== Text at Y=220 (copyright row) ===");
    for y in 220..228.min(height) {
        print!("  Y={:3}: ", y);
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            if r == 0 && g == 0 && b == 0 {
                print!(" ");
            } else if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r == 0x24 && g == 0x24 && b == 0x91 {
                print!(".");
            } else {
                print!("#");
            }
        }
        println!();
    }

    // Show some specific pixel colors
    println!("\n=== Sample pixel colors ===");
    for &(desc, y, x) in &[
        ("Background", 100, 128),
        ("Text H first pixel", 172, 65),
        ("Text area center", 172, 128),
    ] {
        if y < height && x < width {
            let idx = y * width + x;
            let p = pixels[idx];
            println!("  {} ({},{}): #{:06X}", desc, x, y, p & 0xFFFFFF);
        }
    }

    Ok(())
}
