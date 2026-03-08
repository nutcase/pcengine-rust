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
    while frames < 300 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    // Dump BG palette 5 (entries 80-95)
    println!("=== BG Palette 5 (text) - entries 80-95 ===");
    for i in 80..96u16 {
        let raw = emu.bus.vce_palette_word(i as usize);
        let rgb = emu.bus.vce_palette_rgb(i as usize);
        let r = (rgb >> 16) & 0xFF;
        let g = (rgb >> 8) & 0xFF;
        let b = rgb & 0xFF;
        println!(
            "  [{:3}] raw={:04X} RGB=({:3},{:3},{:3}) #{:02X}{:02X}{:02X}",
            i, raw, r, g, b, r, g, b
        );
    }

    // Also dump BG palette 0 (background)
    println!("\n=== BG Palette 0 (background) - entries 0-15 ===");
    for i in 0..16u16 {
        let raw = emu.bus.vce_palette_word(i as usize);
        let rgb = emu.bus.vce_palette_rgb(i as usize);
        let r = (rgb >> 16) & 0xFF;
        let g = (rgb >> 8) & 0xFF;
        let b = rgb & 0xFF;
        println!("  [{:3}] raw={:04X} RGB=({:3},{:3},{:3})", i, raw, r, g, b);
    }

    // Let's also render and check a specific text tile pixel
    // Tile 0x148 'H' at BAT row 20, col 8, palette 5
    let tile_id = 0x148usize;
    let base = tile_id * 16;
    println!("\n=== Tile 0x148 'H' decoded with palette 5 ===");
    for row in 0..8 {
        let w01 = emu.bus.vdc_vram_word((base + row) as u16);
        let w23 = emu.bus.vdc_vram_word((base + row + 8) as u16);
        let plane0 = (w01 & 0xFF) as u8;
        let plane1 = ((w01 >> 8) & 0xFF) as u8;
        let plane2 = (w23 & 0xFF) as u8;
        let plane3 = ((w23 >> 8) & 0xFF) as u8;

        print!("  ");
        for bit in (0..8).rev() {
            let p0 = (plane0 >> bit) & 1;
            let p1 = (plane1 >> bit) & 1;
            let p2 = (plane2 >> bit) & 1;
            let p3 = (plane3 >> bit) & 1;
            let color_idx = (p0 | (p1 << 1) | (p2 << 2) | (p3 << 3)) as usize;
            let pal_entry = 80 + color_idx;
            let rgb = emu.bus.vce_palette_rgb(pal_entry);
            let r = (rgb >> 16) & 0xFF;
            let g = (rgb >> 8) & 0xFF;
            let b = rgb & 0xFF;
            if color_idx == 0 {
                print!(".");
            } else {
                print!("{:X}", color_idx);
            }
        }
        println!();
    }

    // Check if the render actually produces the frame correctly
    // Let's take the last frame and check pixels at the text area
    let frame = emu.take_frame();
    if let Some(pixels) = &frame {
        let width = 256;
        println!("\n=== Pixel check at text area ===");
        // Row 22 text starts at display Y ≈ (22-6)*8 - 3 = 125
        // Actually: BYR = 0x33 = 51. First tile row visible = 51/8 = 6, offset = 51%8 = 3
        // BAT row 20 at display Y = (20-6)*8 - 3 = 109
        // Let's check Y=109 to Y=117 (row 20 text "HISCORE")
        for y in [109, 110, 125, 141, 157] {
            let x_start = 56; // col 7 * 8
            print!("  Y={:3} X={:3}-{:3}: ", y, x_start, x_start + 80);
            for x in x_start..x_start + 80 {
                let idx = y * width + x;
                if idx < pixels.len() {
                    let pixel = pixels[idx];
                    let r = (pixel >> 16) & 0xFF;
                    let g = (pixel >> 8) & 0xFF;
                    let b = pixel & 0xFF;
                    if r == 0 && g == 0 && b == 0 {
                        print!(".");
                    } else if r > 200 && g > 200 && b > 200 {
                        print!("W");
                    } else {
                        print!("#");
                    }
                }
            }
            println!();
        }
    }

    Ok(())
}
