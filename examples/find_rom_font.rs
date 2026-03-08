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
    println!("ROM size: {} bytes (0x{:X})", rom.len(), rom.len());

    // Search for 8x8 font patterns in ROM
    // Strategy: look for sequences of 8 bytes that resemble known characters
    // The font could be 1bpp (8 bytes per char) stored sequentially

    // Known 'H' pattern possibilities (two vertical bars + horizontal bar):
    // Various H patterns to search for
    let h_patterns: Vec<&[u8]> = vec![
        // Standard H patterns
        &[0xC6, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0xC6, 0x00],
        &[0xC2, 0xC2, 0xC2, 0xFE, 0xC2, 0xC2, 0xC2, 0x00],
        &[0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
        &[0xCC, 0xCC, 0xCC, 0xFC, 0xCC, 0xCC, 0xCC, 0x00],
        &[0x42, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
    ];

    // Search for H patterns
    for (pi, pat) in h_patterns.iter().enumerate() {
        for i in 0..rom.len().saturating_sub(pat.len()) {
            if &rom[i..i + pat.len()] == *pat {
                println!("H pattern #{} found at ROM offset 0x{:05X}", pi, i);
                // Check if this is part of a font table
                // 'H' is ASCII 0x48, offset from 0x20 = 0x28 * 8 = 0x140 bytes from table start
                let table_start = i.wrapping_sub(0x140);
                if table_start < rom.len() {
                    // Check space (0x20) = all zeros
                    let sp = &rom[table_start..table_start + 8];
                    let sp_zero = sp.iter().all(|&b| b == 0);
                    println!(
                        "  Table start 0x{:05X}, space={:?} (zero={})",
                        table_start, sp, sp_zero
                    );
                }
            }
        }
    }

    // Also try searching with more relaxed H heuristic
    println!("\n=== Relaxed H search (symmetric rows, center bar) ===");
    let mut candidates = vec![];
    for i in 0..rom.len().saturating_sub(8) {
        let g = &rom[i..i + 8];
        // H: rows 0,1,2 same, row 3 wider, rows 4,5,6 same as 0,1,2, row 7 = 0
        if g[7] != 0 {
            continue;
        }
        if g[0] == 0 {
            continue;
        }
        if g[0] != g[1] || g[0] != g[2] {
            continue;
        }
        if g[0] != g[4] || g[0] != g[5] || g[0] != g[6] {
            continue;
        }
        if g[3] <= g[0] {
            continue;
        } // bar should be wider
        let bits = g[0];
        let popcount = bits.count_ones();
        if popcount < 2 || popcount > 4 {
            continue;
        }
        let bar = g[3];
        if bar.count_ones() <= popcount {
            continue;
        }

        // Check if there's a valid font table starting 0x140 bytes before this
        if i < 0x140 {
            continue;
        }
        let table_start = i - 0x140;
        if table_start + 0x300 > rom.len() {
            continue;
        }

        // Space at table_start should be zeros
        if rom[table_start..table_start + 8].iter().any(|&b| b != 0) {
            continue;
        }

        // '0' at table_start + 0x80 should be non-zero
        let zero_off = table_start + 0x80;
        if rom[zero_off..zero_off + 8].iter().all(|&b| b == 0) {
            continue;
        }

        // 'A' at table_start + 0x108 should be non-zero
        let a_off = table_start + 0x108;
        if rom[a_off..a_off + 8].iter().all(|&b| b == 0) {
            continue;
        }

        candidates.push(table_start);
    }

    candidates.sort();
    candidates.dedup();
    println!("Found {} font table candidates", candidates.len());

    for &base in &candidates {
        println!(
            "\n=== CANDIDATE FONT TABLE at ROM offset 0x{:05X} ===",
            base
        );
        for ascii in 0x20u8..0x60 {
            let off = base + (ascii as usize - 0x20) * 8;
            let glyph = &rom[off..off + 8];
            let ch = if ascii >= 0x20 && ascii < 0x7F {
                ascii as char
            } else {
                '?'
            };
            print!("  '{}' 0x{:02X}: ", ch, ascii);
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
    }

    // Also search for the specific BIOS font pattern to see if ROM has it
    println!("\n=== Search for BIOS-style '0' pattern ===");
    let zero_patterns: Vec<&[u8]> = vec![
        &[0x38, 0x4C, 0xC6, 0xC6, 0xC6, 0x64, 0x38, 0x00], // System Card 3.0 '0'
        &[0x38, 0x44, 0xC6, 0xC6, 0xC6, 0x44, 0x38, 0x00],
        &[0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00],
        &[0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
    ];
    for (pi, pat) in zero_patterns.iter().enumerate() {
        for i in 0..rom.len().saturating_sub(8) {
            if &rom[i..i + 8] == *pat {
                println!("'0' pattern #{} at offset 0x{:05X}", pi, i);
            }
        }
    }

    Ok(())
}
