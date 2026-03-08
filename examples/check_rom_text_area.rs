#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    println!("ROM size: 0x{:X}", rom.len());

    // Text string offsets found by user
    let text_offsets = [
        (0xE177, "HUDSON"),
        (0xE187, "SCORE"),
        (0xE191, "HISCORE"),
        (0xE1C1, "PUSH"),
        (0xE1CC, "BUTTON"),
    ];

    for &(offset, label) in &text_offsets {
        if offset + 20 <= rom.len() {
            print!("{} at 0x{:05X}: ", label, offset);
            let slice = &rom[offset..offset + 20.min(rom.len() - offset)];
            // Print as ASCII
            for &b in slice {
                if b >= 0x20 && b < 0x7F {
                    print!("{}", b as char);
                } else {
                    print!("[{:02X}]", b);
                }
            }
            println!();
            // Print as hex
            print!("  hex: ");
            for &b in &rom[offset..offset + 32.min(rom.len() - offset)] {
                print!("{:02X} ", b);
            }
            println!();
        }
    }

    // Look for font data in the ROM page around 0xE100-0xE200
    println!("\n=== ROM 0xE100-0xE200 dump ===");
    let start = 0xE100;
    let end = 0xE200.min(rom.len());
    for addr in (start..end).step_by(16) {
        print!("0x{:05X}: ", addr);
        for i in 0..16 {
            if addr + i < rom.len() {
                print!("{:02X} ", rom[addr + i]);
            }
        }
        print!(" ");
        for i in 0..16 {
            if addr + i < rom.len() {
                let b = rom[addr + i];
                if b >= 0x20 && b < 0x7F {
                    print!("{}", b as char);
                } else {
                    print!(".");
                }
            }
        }
        println!();
    }

    // Also look for structured data before the text strings
    // Maybe font data is right before the text area
    println!("\n=== ROM 0xE000-0xE100 (before text strings) ===");
    let start = 0xE000;
    let end = 0xE100.min(rom.len());
    for addr in (start..end).step_by(8) {
        let g = &rom[addr..addr + 8.min(rom.len() - addr)];
        // Check if it looks like a font glyph (non-trivial pattern)
        let nonzero = g.iter().filter(|&&b| b != 0).count();
        if nonzero >= 3 && nonzero <= 7 {
            print!("0x{:05X}: ", addr);
            for row in 0..8 {
                for bit in (0..8).rev() {
                    print!("{}", if (g[row] >> bit) & 1 == 1 { "#" } else { "." });
                }
                print!("|");
            }
            print!("  hex:");
            for &b in g {
                print!(" {:02X}", b);
            }
            println!();
        }
    }

    // Search the ENTIRE ROM for a region where 8-byte chunks look like font characters
    // Use a scoring system: each 8-byte chunk gets a "font-likeness" score
    println!("\n=== Scanning ROM for font-like regions ===");
    let mut best_score = 0u32;
    let mut best_offset = 0;

    for start in (0..rom.len().saturating_sub(96 * 8)).step_by(8) {
        let mut score = 0u32;
        // Check 96 consecutive 8-byte chunks
        for c in 0..96 {
            let off = start + c * 8;
            if off + 8 > rom.len() {
                break;
            }
            let g = &rom[off..off + 8];

            // Score based on font-like properties
            let nonzero = g.iter().filter(|&&b| b != 0).count();

            // Space (first char) should be all zeros
            if c == 0 && nonzero == 0 {
                score += 3;
            }

            // Regular characters should have 3-7 non-zero rows
            if c >= 1 && c <= 62 && nonzero >= 3 && nonzero <= 7 {
                score += 1;
            }

            // Last row often zero (descenders excluded)
            if c >= 1 && c <= 62 && g[7] == 0 {
                score += 1;
            }

            // Characters should be reasonably bounded (not too wide)
            if c >= 1 && c <= 62 {
                let max_width = g.iter().map(|&b| b.count_ones()).max().unwrap_or(0);
                if max_width >= 3 && max_width <= 7 {
                    score += 1;
                }
            }
        }

        if score > best_score {
            best_score = score;
            best_offset = start;
        }
    }

    println!(
        "Best font candidate: score={} at offset 0x{:05X}",
        best_score, best_offset
    );
    // Show first 30 characters of best candidate
    for c in 0..30 {
        let off = best_offset + c * 8;
        if off + 8 > rom.len() {
            break;
        }
        let g = &rom[off..off + 8];
        let ascii = (0x20 + c) as u8;
        let ch = if ascii >= 0x20 && ascii < 0x7F {
            ascii as char
        } else {
            '?'
        };
        print!("  '{}' 0x{:02X}: ", ch, ascii);
        for row in 0..8 {
            for bit in (0..8).rev() {
                print!("{}", if (g[row] >> bit) & 1 == 1 { "#" } else { "." });
            }
            if row < 7 {
                print!("|");
            }
        }
        println!();
    }

    Ok(())
}
