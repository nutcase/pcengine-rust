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

    // Run to frame 300 (title screen fully visible)
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

    // Check font tile H after restoration
    let h_base = 0x148u16 * 16;
    let h_w0 = emu.bus.vdc_vram_word(h_base);
    let h_font_ok = (h_w0 & 0xFF) == 0x66 && (h_w0 >> 8) == 0x00;
    println!("Font 'H' at VRAM: {:04X} (correct={})", h_w0, h_font_ok);

    // Check space tile (should also be correct)
    let sp_base = 0x140u16 * 16;
    let sp_w0 = emu.bus.vdc_vram_word(sp_base);
    println!(
        "Font ' ' at VRAM: {:04X} (should be tile '@' = 0x40)",
        sp_w0
    );

    // Save frame as PPM
    let mut data = Vec::with_capacity(width * height * 3);
    for &p in pixels {
        data.push(((p >> 16) & 0xFF) as u8);
        data.push(((p >> 8) & 0xFF) as u8);
        data.push((p & 0xFF) as u8);
    }
    let header = format!("P6\n{} {}\n255\n", width, height);
    let mut file = std::fs::File::create("frame_300_restored.ppm")?;
    file.write_all(header.as_bytes())?;
    file.write_all(&data)?;
    println!("Saved frame_300_restored.ppm ({}x{})", width, height);

    // Show text row pixel patterns
    let byr = 0x33usize;
    // HISCORE at BAT row 20 → pixel Y = 20*8 = 160, screen Y = 160-byr = 109
    println!("\n=== Text row analysis (HISCORE at screen Y≈109) ===");
    let screen_y = 160 - byr; // BAT pixel Y 160, screen Y 109
    for dy in 0..8 {
        let y = screen_y + dy;
        if y >= height {
            break;
        }
        print!("Y={:3}: ", y);
        for x in 55..200 {
            let p = pixels[y * width + x];
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            if r < 0x10 && g < 0x10 && b < 0x10 {
                print!(" ");
            } else if r > 0xC0 && g > 0xC0 && b > 0xC0 {
                print!("W");
            } else if r > 0x80 {
                print!("#");
            } else {
                print!(".");
            }
        }
        println!();
    }

    // Also check PUSH RUN BUTTON row (BAT row 24 → pixel Y=192, screen Y=192-51=141)
    println!("\n=== Text row analysis (PUSH RUN BUTTON at screen Y≈141) ===");
    let screen_y = 192 - byr;
    for dy in 0..8 {
        let y = screen_y + dy;
        if y >= height {
            break;
        }
        print!("Y={:3}: ", y);
        for x in 48..210 {
            let p = pixels[y * width + x];
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            if r < 0x10 && g < 0x10 && b < 0x10 {
                print!(" ");
            } else if r > 0xC0 && g > 0xC0 && b > 0xC0 {
                print!("W");
            } else if r > 0x80 {
                print!("#");
            } else {
                print!(".");
            }
        }
        println!();
    }

    // Copyright row (BAT row 26 → pixel Y=208, screen Y=208-51=157)
    println!("\n=== Text row analysis (© 1987 HUDSON SOFT at screen Y≈157) ===");
    let screen_y = 208 - byr;
    for dy in 0..8 {
        let y = screen_y + dy;
        if y >= height {
            break;
        }
        print!("Y={:3}: ", y);
        for x in 48..210 {
            let p = pixels[y * width + x];
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            if r < 0x10 && g < 0x10 && b < 0x10 {
                print!(" ");
            } else if r > 0xC0 && g > 0xC0 && b > 0xC0 {
                print!("W");
            } else if r > 0x80 {
                print!("#");
            } else {
                print!(".");
            }
        }
        println!();
    }

    Ok(())
}
