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
    let bios = std::fs::read("roms/syscard3.pce")?;
    println!("BIOS ROM size: {} bytes", bios.len());

    // Search for 'H' glyph patterns: two vertical bars + horizontal bar
    // Relaxed: just look for pattern where row[3] > row[0] and row[0]==row[1]
    // and row[0]==row[4]
    let mut candidates = vec![];
    for i in 0..bios.len().saturating_sub(8) {
        let g = &bios[i..i + 8];
        if g[7] != 0 {
            continue;
        } // last row should be blank
        if g[0] == 0 {
            continue;
        } // first row shouldn't be blank
        if g[0] != g[1] {
            continue;
        }
        if g[0] != g[2] {
            continue;
        }
        if g[0] != g[4] {
            continue;
        }
        if g[0] != g[5] {
            continue;
        }
        if g[0] != g[6] {
            continue;
        }
        if g[3] <= g[0] {
            continue;
        } // horizontal bar wider
        // Check if it really looks like H: two distinct columns
        let bits = g[0];
        let popcount = bits.count_ones();
        if popcount < 2 || popcount > 4 {
            continue;
        }
        // The bar should fill in between
        let bar = g[3];
        if bar.count_ones() < popcount + 1 {
            continue;
        }

        // This might be 'H'. Check if offset-0x140 is a valid font table start
        // 'H' is ASCII 0x48, so offset from table start = (0x48-0x20)*8 = 0x140
        if i < 0x140 {
            continue;
        }
        let table_start = i - 0x140;
        // Check space at table_start is all zeros
        if bios[table_start..table_start + 8].iter().any(|&b| b != 0) {
            continue;
        }

        candidates.push((table_start, i));
    }

    println!("Found {} H-based candidates", candidates.len());

    for &(base, h_off) in &candidates {
        // Verify more characters
        let a_off = base + 0x108; // 'A'
        if a_off + 8 > bios.len() {
            continue;
        }
        let a = &bios[a_off..a_off + 8];
        if a[0] == 0 && a[1] == 0 {
            continue;
        } // 'A' shouldn't start blank

        let o_off = base + (0x4F - 0x20) * 8; // 'O'
        if o_off + 8 > bios.len() {
            continue;
        }
        let o = &bios[o_off..o_off + 8];
        if o[0] == 0 {
            continue;
        }
        if o[7] != 0 {
            continue;
        }
        // O should have similar first and last non-blank rows
        if o[0] != o[6] {
            continue;
        }

        println!("\n=== FONT TABLE at ROM offset 0x{:05X} ===", base);
        for ascii in 0x20u8..0x80 {
            let off = base + (ascii as usize - 0x20) * 8;
            if off + 8 > bios.len() {
                break;
            }
            let glyph = &bios[off..off + 8];
            let ch = if ascii >= 0x20 && ascii < 0x7F {
                ascii as char
            } else {
                '?'
            };
            print!("  '{}' 0x{:02X}: [", ch, ascii);
            for (j, &b) in glyph.iter().enumerate() {
                if j > 0 {
                    print!(",");
                }
                print!("0x{:02X}", b);
            }
            print!("]  ");
            for row in 0..8 {
                for bit in (0..8).rev() {
                    print!(
                        "{}",
                        if (glyph[row] >> bit) & 1 == 1 {
                            "#"
                        } else {
                            "."
                        }
                    );
                }
                if row < 7 {
                    print!("|");
                }
            }
            println!();
        }
        break; // Only show first match
    }

    Ok(())
}
