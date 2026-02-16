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

    let pixels = last_frame.expect("should have a frame");
    let width = emu.display_width();
    let height = pixels.len() / width;
    println!("Frame: {}x{}", width, height);

    // BYR = 0x33 = 51
    // BAT row 20 text "HISCORE" starts at col 8
    // Display Y for BAT row R: (R * 8 - BYR) mod map_height_pixels
    // BYR = 51, so row 20: 20*8 - 51 = 160 - 51 = 109
    // Row 22: 22*8 - 51 = 176 - 51 = 125
    // Row 24: 24*8 - 51 = 192 - 51 = 141
    // Row 26: 26*8 - 51 = 208 - 51 = 157
    //
    // But BYR offset within tile: 51 % 8 = 3
    // So the actual pixel mapping is more nuanced

    println!("\n=== Pixel rows at text area ===");
    for y in 105..170 {
        if y >= height {
            break;
        }
        // Check cols 56-200 (tiles 7-25)
        let mut has_nonblack = false;
        let mut row_str = String::new();
        for x in 48..210 {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            // Background is dark blue #242491
            if r == 0x24 && g == 0x24 && b == 0x91 {
                row_str.push('.');
            } else if r == 0 && g == 0 && b == 0 {
                row_str.push(' ');
            } else if r > 200 && g > 200 && b > 200 {
                row_str.push('W');
                has_nonblack = true;
            } else if r > 200 && g == 0 && b == 0 {
                row_str.push('R');
                has_nonblack = true;
            } else {
                row_str.push('#');
                has_nonblack = true;
            }
        }
        if has_nonblack || y == 109 || y == 125 || y == 141 || y == 157 {
            println!("  Y={:3}: {}", y, row_str);
        }
    }

    // Also check what color is at specific text tile positions
    println!("\n=== Specific pixel colors at text positions ===");
    for &(desc, y, x) in &[
        ("Row 20 'H' top-left", 109, 64),
        ("Row 20 'H' mid", 112, 67),
        ("Row 24 'P' top-left", 141, 56),
        ("Row 24 'P' mid", 144, 59),
        ("Row 26 '(' top-left", 157, 56),
        ("Background at Y=50", 50, 128),
        ("Background at Y=160", 160, 0),
    ] {
        if y < height && x < width {
            let idx = y * width + x;
            let pixel = pixels[idx];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            println!(
                "  {} ({},{}): RGB=({:3},{:3},{:3}) #{:02X}{:02X}{:02X}",
                desc, x, y, r, g, b, r, g, b
            );
        }
    }

    // Write PPM for manual inspection
    let mut ppm = format!("P6\n{} {}\n255\n", width, height);
    let mut ppm_bytes = ppm.into_bytes();
    for &pixel in &pixels {
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        ppm_bytes.push(r);
        ppm_bytes.push(g);
        ppm_bytes.push(b);
    }
    std::fs::write("frame_text_check.ppm", &ppm_bytes)?;
    println!("\nWrote frame_text_check.ppm");

    Ok(())
}
