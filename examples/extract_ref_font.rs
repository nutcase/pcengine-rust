#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use std::error::Error;
use std::io::Read;

fn main() -> Result<(), Box<dyn Error>> {
    // Read BMP file
    let data = std::fs::read("/tmp/katoken_ref.bmp")?;

    let w = u32::from_le_bytes([data[18], data[19], data[20], data[21]]) as usize;
    let h = u32::from_le_bytes([data[22], data[23], data[24], data[25]]) as usize;
    let offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
    eprintln!("BMP: {}x{}, offset={}", w, h, offset);

    let row_stride = ((w * 3 + 3) / 4) * 4;

    // BMP is bottom-up; read pixel at (x, y) where y=0 is top
    let pixel = |x: usize, y: usize| -> (u8, u8, u8) {
        let row_start = offset + (h - 1 - y) * row_stride;
        let px = row_start + x * 3;
        (data[px + 2], data[px + 1], data[px]) // BGR -> RGB
    };

    let is_white = |x: usize, y: usize| -> bool {
        let (r, g, b) = pixel(x, y);
        r > 200 && g > 200 && b > 200
    };

    // Reference is 2x scale (512x448 for 256x224 PCE)
    // PCE pixel (px, py) -> ref pixel (px*2, py*2)
    // Sample at center of each 2x2 block
    let pce_white = |px: usize, py: usize| -> bool {
        is_white(px * 2, py * 2)
            || is_white(px * 2 + 1, py * 2)
            || is_white(px * 2, py * 2 + 1)
            || is_white(px * 2 + 1, py * 2 + 1)
    };

    // Find white text rows in PCE coordinates
    let pce_w = 256;
    let pce_h = 224;

    println!("=== Finding text rows ===");
    let mut text_rows = vec![];
    for y in 0..pce_h {
        let wc: usize = (0..pce_w).filter(|&x| pce_white(x, y)).count();
        if wc > 10 {
            text_rows.push(y);
            if text_rows.len() <= 30 {
                println!("Y={}: {} white", y, wc);
            }
        }
    }

    // Identify text line start Y positions (groups of 7-8 consecutive rows)
    let mut line_starts = vec![];
    let mut prev_y = 0usize;
    for &y in &text_rows {
        if y > 100 && (line_starts.is_empty() || y > prev_y + 2) {
            line_starts.push(y);
        }
        prev_y = y;
    }
    println!("\nText line starts: {:?}", line_starts);

    // For the PUSH RUN BUTTON! line, dump character patterns
    // We know the text content: PUSH  RUN  BUTTON!
    // Need to find X start position

    // Let's dump all 4 text lines with character boundaries
    let known_text = [
        "HISCORE       0",
        "SCORE       0",
        "PUSH  RUN  BUTTON!",
        // copyright line
    ];

    // For each text line, find first white pixel X
    for &start_y in &line_starts {
        println!("\n=== Line starting at Y={} ===", start_y);

        // Find X range of white pixels
        let mut min_x = pce_w;
        let mut max_x = 0;
        for dy in 0..8 {
            let y = start_y + dy;
            if y >= pce_h {
                break;
            }
            for x in 0..pce_w {
                if pce_white(x, y) {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
        }
        println!("White X range: {}-{}", min_x, max_x);

        // Align to 8-pixel tile boundary
        let tile_x_start = (min_x / 8) * 8;
        let tile_x_end = ((max_x + 8) / 8) * 8;

        // Dump each 8-pixel-wide character
        for tile_x in (tile_x_start..tile_x_end).step_by(8) {
            let mut all_blank = true;
            let mut pattern = [0u8; 8];
            for dy in 0..8 {
                let y = start_y + dy;
                if y >= pce_h {
                    break;
                }
                let mut byte = 0u8;
                for dx in 0..8 {
                    let x = tile_x + dx;
                    if x < pce_w && pce_white(x, y) {
                        byte |= 1 << (7 - dx);
                        all_blank = false;
                    }
                }
                pattern[dy] = byte;
            }

            if !all_blank {
                print!("  X={:3} [", tile_x);
                for (i, &b) in pattern.iter().enumerate() {
                    if i > 0 {
                        print!(",");
                    }
                    print!("0x{:02X}", b);
                }
                print!("] ");
                // Visual
                for dy in 0..8 {
                    for dx in 0..8 {
                        print!(
                            "{}",
                            if (pattern[dy] >> (7 - dx)) & 1 == 1 {
                                "#"
                            } else {
                                "."
                            }
                        );
                    }
                    if dy < 7 {
                        print!("|");
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}
