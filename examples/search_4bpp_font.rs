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

    // Search for 4bpp planar font tiles in ROM.
    // Font tile format: 16 words (32 bytes) per 8x8 tile
    //   Words 0-7: planes 0,1 (might be 0 if font only uses planes 2,3)
    //   Words 8-15: planes 2,3
    // Little-endian: each word stored as low_byte, high_byte

    // Strategy 1: Look for 16 zero bytes followed by 16 non-zero bytes
    // where the non-zero bytes form recognizable characters
    println!("=== Search: 16 zero bytes + pattern in planes 2-3 ===");
    let mut candidates = vec![];
    for i in 0..rom.len().saturating_sub(32) {
        // Check first 16 bytes are zero
        if rom[i..i + 16].iter().any(|&b| b != 0) {
            continue;
        }
        // Check next 16 bytes have some non-zero content
        let nonzero = rom[i + 16..i + 32].iter().filter(|&&b| b != 0).count();
        if nonzero < 6 {
            continue;
        }
        // Check if it looks like a character (lo=hi in word pairs)
        let mut valid = true;
        for w in 0..8 {
            let lo = rom[i + 16 + w * 2];
            let hi = rom[i + 16 + w * 2 + 1];
            if lo != hi {
                valid = false;
                break;
            }
        }
        if !valid {
            continue;
        }
        candidates.push(i);
    }
    println!(
        "Found {} candidates with 16-zero + plane2/3 pattern",
        candidates.len()
    );
    for &c in candidates.iter().take(10) {
        print!("  Offset 0x{:05X}: ", c);
        for row in 0..8 {
            let b = rom[c + 16 + row * 2];
            for bit in (0..8).rev() {
                print!("{}", if (b >> bit) & 1 == 1 { "#" } else { "." });
            }
            if row < 7 {
                print!("|");
            }
        }
        println!();
    }

    // Strategy 2: Look for paired bytes (lo=hi) forming recognizable patterns
    // without requiring planes 0,1 to be zero
    println!("\n=== Search: all planes same (lo=hi for all 16 words) ===");
    let mut candidates2 = vec![];
    for i in (0..rom.len().saturating_sub(32)).step_by(2) {
        let mut all_paired = true;
        let mut has_data = false;
        for w in 0..16 {
            let lo = rom[i + w * 2];
            let hi = rom[i + w * 2 + 1];
            if lo != hi {
                all_paired = false;
                break;
            }
            if lo != 0 {
                has_data = true;
            }
        }
        if !all_paired || !has_data {
            continue;
        }

        // Extract the 1bpp pattern from words 0-7
        let pat: Vec<u8> = (0..8).map(|r| rom[i + r * 2]).collect();
        // Check if words 0-7 == words 8-15
        let planes_match = (0..8).all(|r| rom[i + r * 2] == rom[i + 16 + r * 2]);

        // Check if it looks like a recognizable character
        let nonzero_rows = pat.iter().filter(|&&b| b != 0).count();
        if nonzero_rows < 4 {
            continue;
        }
        if pat[7] != 0 && pat[0] != 0 && nonzero_rows < 6 {
            continue;
        }

        candidates2.push((i, planes_match));
    }
    println!(
        "Found {} candidates with all-planes-same pattern",
        candidates2.len()
    );
    // Group consecutive candidates (within 32 bytes = 1 tile apart)
    if !candidates2.is_empty() {
        let mut groups: Vec<Vec<(usize, bool)>> = vec![vec![candidates2[0]]];
        for &(off, pm) in candidates2.iter().skip(1) {
            let last = groups.last().unwrap().last().unwrap().0;
            if off - last <= 32 {
                groups.last_mut().unwrap().push((off, pm));
            } else {
                groups.push(vec![(off, pm)]);
            }
        }

        // Show groups with 3+ consecutive tiles
        for group in &groups {
            if group.len() >= 3 {
                println!(
                    "  Group of {} tiles starting at 0x{:05X}:",
                    group.len(),
                    group[0].0
                );
                for &(off, pm) in group.iter().take(10) {
                    let pat: Vec<u8> = (0..8).map(|r| rom[off + r * 2]).collect();
                    print!("    0x{:05X} (p_match={}): ", off, pm);
                    for row in 0..8 {
                        for bit in (0..8).rev() {
                            print!("{}", if (pat[row] >> bit) & 1 == 1 { "#" } else { "." });
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

    // Strategy 3: Maybe the font isn't in paired-byte format.
    // Check if any part of the ROM, when interpreted as VDC tile data
    // (16 words little-endian), produces recognizable 'H' when combining
    // all 4 bitplanes
    println!("\n=== Search: VDC format H pattern (any plane combination) ===");
    for i in (0..rom.len().saturating_sub(32)).step_by(32) {
        // Read 16 words (little-endian)
        let mut words = [0u16; 16];
        for w in 0..16 {
            words[w] = u16::from_le_bytes([rom[i + w * 2], rom[i + w * 2 + 1]]);
        }
        // Combine all 4 planes per pixel row
        let mut combined = [0u8; 8];
        for row in 0..8 {
            let p0 = (words[row] & 0xFF) as u8;
            let p1 = ((words[row] >> 8) & 0xFF) as u8;
            let p2 = (words[row + 8] & 0xFF) as u8;
            let p3 = ((words[row + 8] >> 8) & 0xFF) as u8;
            combined[row] = p0 | p1 | p2 | p3;
        }
        // Check H-like pattern
        if combined[7] != 0 {
            continue;
        }
        if combined[0] == 0 {
            continue;
        }
        if combined[0] != combined[1] || combined[0] != combined[2] {
            continue;
        }
        if combined[0] != combined[4] || combined[0] != combined[5] || combined[0] != combined[6] {
            continue;
        }
        if combined[3] <= combined[0] {
            continue;
        }
        if combined[0].count_ones() < 2 {
            continue;
        }

        // Check if there's a font table around this position
        // 'H' is ASCII 0x48, offset from '!' (0x21) is 0x27 tiles,
        // or from ' ' (0x20) is 0x28 tiles
        let h_tile_offset = 0x28; // from ASCII space
        let table_start = i.wrapping_sub(h_tile_offset * 32);
        if table_start >= rom.len() {
            continue;
        }

        // Check space tile is mostly empty
        let space_empty = rom[table_start..table_start + 32]
            .iter()
            .map(|&b| if b == 0 { 0 } else { 1 })
            .sum::<u32>()
            <= 4;

        if !space_empty {
            continue;
        }

        println!(
            "  H pattern at 0x{:05X} (table_start=0x{:05X}):",
            i, table_start
        );
        print!("    H combined: ");
        for row in 0..8 {
            for bit in (0..8).rev() {
                print!(
                    "{}",
                    if (combined[row] >> bit) & 1 == 1 {
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

    Ok(())
}
