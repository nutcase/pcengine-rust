#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use std::error::Error;

fn looks_like_h(g: &[u8]) -> bool {
    // H: two vertical bars connected by horizontal bar in middle
    // At least 2 non-zero rows matching at top and bottom,
    // middle row(s) wider
    if g.len() < 7 {
        return false;
    }
    let rows: Vec<u8> = g[..8.min(g.len())].to_vec();

    // Basic H check: rows 0-2 same, row 3 or 4 wider, rows 5-6 same as 0-2
    let top = rows[0];
    if top == 0 {
        return false;
    }
    if top.count_ones() < 2 || top.count_ones() > 6 {
        return false;
    }

    // Check symmetry: top rows similar, bottom rows similar
    let top_similar = rows[1] == top && rows[2] == top;
    let bar_wider = rows[3] > top || rows[4] > top;
    let bot_similar = (rows[5] == top || rows[4] == top) && (rows[6] == top || rows[5] == top);

    top_similar && bar_wider && bot_similar
}

fn score_font_table(rom: &[u8], base: usize, stride: usize) -> (u32, String) {
    // Score how likely this is a font table starting at 'base' with 'stride' bytes per char
    // Assumes ASCII mapping: char 0 = space (0x20), char 1 = !, etc.
    let mut score = 0u32;
    let mut evidence = String::new();

    if base + 96 * stride > rom.len() {
        return (0, evidence);
    }

    // Space (offset 0) should be mostly zeros
    let space = &rom[base..base + stride.min(8)];
    if space.iter().all(|&b| b == 0) {
        score += 5;
        evidence.push_str("space=zero ");
    }

    // '0' at offset 0x10*stride (0x30-0x20=0x10)
    let zero_off = base + 0x10 * stride;
    if zero_off + 8 <= rom.len() {
        let g = &rom[zero_off..zero_off + 8];
        // '0' should have non-zero rows, somewhat symmetric
        if g.iter().filter(|&&b| b != 0).count() >= 5 {
            score += 2;
            if g[0] == g[6] || g[1] == g[5] {
                score += 3;
                evidence.push_str("0=sym ");
            }
        }
    }

    // 'H' at offset 0x28*stride (0x48-0x20=0x28)
    let h_off = base + 0x28 * stride;
    if h_off + 8 <= rom.len() {
        if looks_like_h(&rom[h_off..h_off + 8]) {
            score += 10;
            evidence.push_str("H=ok ");
        }
    }

    // 'A' at offset 0x21*stride (0x41-0x20=0x21)
    let a_off = base + 0x21 * stride;
    if a_off + 8 <= rom.len() {
        let g = &rom[a_off..a_off + 8];
        // 'A': narrow top, wide bottom, bar in middle
        if g[0] != 0 && g.iter().filter(|&&b| b != 0).count() >= 5 {
            score += 2;
            evidence.push_str("A=nonzero ");
        }
    }

    // 'I' at offset 0x29*stride (0x49-0x20=0x29)
    let i_off = base + 0x29 * stride;
    if i_off + 8 <= rom.len() {
        let g = &rom[i_off..i_off + 8];
        // 'I': mostly same value, thin bar
        let unique: std::collections::HashSet<u8> = g[..7].iter().copied().collect();
        if unique.len() <= 3 && g.iter().filter(|&&b| b != 0).count() >= 5 {
            score += 3;
            evidence.push_str("I=consistent ");
        }
    }

    (score, evidence)
}

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    println!("ROM size: {} bytes", rom.len());

    // Try different strides (bytes per character)
    let strides = [8, 16, 32]; // 1bpp, 2bpp, 4bpp

    for &stride in &strides {
        println!(
            "\n=== Searching with stride {} ({}bpp) ===",
            stride,
            stride / 8
        );
        let mut best = Vec::new();

        for base in 0..rom.len().saturating_sub(96 * stride) {
            let (score, evidence) = score_font_table(&rom, base, stride);
            if score >= 10 {
                best.push((score, base, evidence));
            }
        }

        best.sort_by(|a, b| b.0.cmp(&a.0));
        for (score, base, evidence) in best.iter().take(5) {
            println!("  Score {} at offset 0x{:05X}: {}", score, base, evidence);

            // Print H character
            let h_off = base + 0x28 * stride;
            if h_off + 8 <= rom.len() {
                print!("    H: ");
                for row in 0..8 {
                    for bit in (0..8).rev() {
                        print!(
                            "{}",
                            if (rom[h_off + row] >> bit) & 1 == 1 {
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

            // Print A character
            let a_off = base + 0x21 * stride;
            if a_off + 8 <= rom.len() {
                print!("    A: ");
                for row in 0..8 {
                    for bit in (0..8).rev() {
                        print!(
                            "{}",
                            if (rom[a_off + row] >> bit) & 1 == 1 {
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

            // Print full table for best match
            if *score >= 15 {
                println!("    Full table:");
                for c in 0..64 {
                    let off = base + c * stride;
                    if off + 8 > rom.len() {
                        break;
                    }
                    let ascii = (0x20 + c) as u8;
                    let ch = if ascii >= 0x20 && ascii < 0x7F {
                        ascii as char
                    } else {
                        '?'
                    };
                    print!("      '{}': ", ch);
                    for row in 0..8 {
                        for bit in (0..8).rev() {
                            print!(
                                "{}",
                                if (rom[off + row] >> bit) & 1 == 1 {
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
            }
        }
    }

    // Also try: the font might be stored where tile IDs 0x130-0x15C
    // map to in the ROM. These tiles = ASCII 0x30-0x5C.
    // The game might use a lookup table to convert ASCII to ROM offset.
    // Let's search for any contiguous region that maps ASCII 0x30-0x5C
    // ('0'-'Z' roughly) with 8 or 16 or 32 byte stride
    println!("\n=== Search for partial font (0-9, A-Z) ===");
    for &stride in &strides {
        for base in 0..rom.len().saturating_sub(60 * stride) {
            // Check '0' at offset 0
            let g0 = &rom[base..base + 8.min(stride)];
            if g0.iter().all(|&b| b == 0) {
                continue;
            }
            if g0.iter().filter(|&&b| b != 0).count() < 5 {
                continue;
            }

            // Check 'H' at offset 0x18*stride (0x48-0x30)
            let h_off = base + 0x18 * stride;
            if h_off + 8 > rom.len() {
                continue;
            }
            if !looks_like_h(&rom[h_off..h_off + 8]) {
                continue;
            }

            // Check 'A' at offset 0x11*stride (0x41-0x30)
            let a_off = base + 0x11 * stride;
            if a_off + 8 > rom.len() {
                continue;
            }
            let ga = &rom[a_off..a_off + 8];
            if ga.iter().filter(|&&b| b != 0).count() < 5 {
                continue;
            }

            println!("  Partial font at 0x{:05X} stride={}", base, stride);
            for c in 0..45 {
                let off = base + c * stride;
                if off + 8 > rom.len() {
                    break;
                }
                let ascii = (0x30 + c) as u8;
                let ch = if ascii >= 0x20 && ascii < 0x7F {
                    ascii as char
                } else {
                    '?'
                };
                print!("    '{}': ", ch);
                for row in 0..8 {
                    for bit in (0..8).rev() {
                        print!(
                            "{}",
                            if (rom[off + row] >> bit) & 1 == 1 {
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
            println!();
        }
    }

    Ok(())
}
