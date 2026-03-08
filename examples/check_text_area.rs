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
    let mut last_frame: Option<Vec<u32>> = None;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    let pixels = last_frame.expect("should have frame");
    let width = 256;
    let height = pixels.len() / width;

    // The BG Y mapping with timing programmed:
    // effective_y_scroll = BYR + 1 = 52
    // y_origin_bias = -64
    // sample_y = Y - 12 (approximately, with zoom=1)
    // So BAT row R appears at display Y = R*8 + 12

    println!("=== Text area at corrected Y positions ===");
    println!("BAT row 20 (HISCORE) at Y=172:");
    for y in 172..180.min(height) {
        print!("  Y={:3}: ", y);
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            if r == 0x24 && g == 0x24 && b == 0x91 {
                print!(".");
            } else if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r > 200 && g < 50 && b < 50 {
                print!("R");
            } else if r == 0 && g == 0 && b == 0xFF {
                print!("B");
            } else if r == 0x24 && g == 0x24 && b == 0x24 {
                print!("d");
            } else {
                print!("#");
            }
        }
        println!();
    }

    println!("\nBAT row 24 (PUSH RUN BUTTON) at Y=204:");
    for y in 204..212.min(height) {
        print!("  Y={:3}: ", y);
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            if r == 0x24 && g == 0x24 && b == 0x91 {
                print!(".");
            } else if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r > 200 && g < 50 && b < 50 {
                print!("R");
            } else if r == 0 && g == 0 && b == 0xFF {
                print!("B");
            } else if r == 0x24 && g == 0x24 && b == 0x24 {
                print!("d");
            } else {
                print!("#");
            }
        }
        println!();
    }

    println!("\nBAT row 26 (C) 1987... at Y=220:");
    for y in 220..228.min(height) {
        print!("  Y={:3}: ", y);
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            if r == 0x24 && g == 0x24 && b == 0x91 {
                print!(".");
            } else if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r > 200 && g < 50 && b < 50 {
                print!("R");
            } else if r == 0 && g == 0 && b == 0xFF {
                print!("B");
            } else if r == 0x24 && g == 0x24 && b == 0x24 {
                print!("d");
            } else {
                print!("#");
            }
        }
        println!();
    }

    // Also check what's at Y=235-239 (bottom of visible area)
    println!("\nBottom of visible area Y=235-239:");
    for y in 235..height.min(240) {
        print!("  Y={:3}: ", y);
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            if r == 0x24 && g == 0x24 && b == 0x91 {
                print!(".");
            } else if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r > 200 && g < 50 && b < 50 {
                print!("R");
            } else if r == 0 && g == 0 && b == 0xFF {
                print!("B");
            } else {
                print!("#");
            }
        }
        println!();
    }

    // Sample specific pixel colors at text positions
    println!("\n=== Specific pixels ===");
    for &(desc, y, x) in &[
        ("HISCORE 'H' (corrected)", 172usize, 64usize),
        ("PUSH 'P' (corrected)", 204, 56),
        ("(C) '(' (corrected)", 220, 56),
        ("between text", 172, 0),
        ("Y=204 center", 204, 128),
    ] {
        if y < height && x < width {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            println!("  {} ({},{}): #{:02X}{:02X}{:02X}", desc, x, y, r, g, b);
        } else {
            println!("  {} ({},{}): out of bounds", desc, x, y);
        }
    }

    Ok(())
}
