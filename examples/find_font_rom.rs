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

    // Search for 1bpp font patterns (8 bytes per character, consecutive)
    // Known character patterns for common 8x8 fonts:
    let known_h_patterns: Vec<(&str, Vec<u8>)> = vec![
        ("H-v1", vec![0xC6, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0xC6, 0x00]),
        ("H-v2", vec![0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00]),
        ("H-v3", vec![0xCC, 0xCC, 0xCC, 0xFC, 0xCC, 0xCC, 0xCC, 0x00]),
        ("H-v4", vec![0xC6, 0xC6, 0xFE, 0xFE, 0xC6, 0xC6, 0xC6, 0x00]),
        ("H-v5", vec![0x63, 0x63, 0x63, 0x7F, 0x63, 0x63, 0x63, 0x00]),
    ];

    for (name, pat) in &known_h_patterns {
        for i in 0..rom.len().saturating_sub(pat.len()) {
            if &rom[i..i + pat.len()] == pat.as_slice() {
                println!("Found {} at ROM {:06X}", name, i);
                // Show surrounding characters
                let start = i.saturating_sub(128); // ~16 chars before
                // Check if this looks like part of a font table
                let char_before = i.saturating_sub(8);
                let char_after = i + 8;
                if char_after + 8 <= rom.len() {
                    print!("  Char before (should be 'G'): ");
                    for bit_row in 0..8 {
                        let b = rom[char_before + bit_row];
                        for bit in (0..8).rev() {
                            if (b >> bit) & 1 != 0 {
                                print!("#");
                            } else {
                                print!(".");
                            }
                        }
                        print!("|");
                    }
                    println!();
                    print!("  Char after (should be 'I'): ");
                    for bit_row in 0..8 {
                        let b = rom[char_after + bit_row];
                        for bit in (0..8).rev() {
                            if (b >> bit) & 1 != 0 {
                                print!("#");
                            } else {
                                print!(".");
                            }
                        }
                        print!("|");
                    }
                    println!();
                }
            }
        }
    }

    // Also search for the COMPLEMENT (inverted) patterns
    // Some games store inverted fonts
    println!("\n=== Searching for inverted 'H' patterns ===");
    for (name, pat) in &known_h_patterns {
        let inv: Vec<u8> = pat.iter().map(|b| !b).collect();
        for i in 0..rom.len().saturating_sub(inv.len()) {
            if &rom[i..i + inv.len()] == inv.as_slice() {
                println!("Found inverted {} at ROM {:06X}", name, i);
            }
        }
    }

    // Search for common number patterns too
    println!("\n=== Searching for '0' digit 1bpp ===");
    let zero_pats: Vec<(&str, Vec<u8>)> = vec![
        ("0-v1", vec![0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00]),
        ("0-v2", vec![0x7C, 0xC6, 0xCE, 0xD6, 0xE6, 0xC6, 0x7C, 0x00]),
        ("0-v3", vec![0x38, 0x6C, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x00]),
        ("0-v4", vec![0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00]),
    ];
    for (name, pat) in &zero_pats {
        for i in 0..rom.len().saturating_sub(pat.len()) {
            if &rom[i..i + pat.len()] == pat.as_slice() {
                println!("Found {} at ROM {:06X}", name, i);
            }
        }
    }

    // Try a broader search: look for sequences of 8 bytes where
    // patterns form recognizable ASCII characters in order
    // Specifically, search for 'A' followed by 'B' followed by 'C' etc.
    // 'A' typical patterns: symmetric with peak at top
    println!("\n=== Searching for font table by structure ===");
    // Look for 8-byte aligned blocks where:
    // - Each block has reasonable pixel count (10-40 out of 64)
    // - Consecutive blocks show increasing complexity
    // - At least 26 consecutive non-empty blocks (A-Z)

    let mut font_candidates: Vec<(usize, usize)> = Vec::new();
    for start in (0..rom.len().saturating_sub(26 * 8)).step_by(8) {
        let mut consecutive_good = 0;
        for c in 0..96 {
            // Check 96 characters (space to ~)
            let offset = start + c * 8;
            if offset + 8 > rom.len() {
                break;
            }
            let mut pixel_count = 0;
            for row in 0..8 {
                pixel_count += rom[offset + row].count_ones();
            }
            if pixel_count >= 4 && pixel_count <= 50 {
                consecutive_good += 1;
            } else if c >= 17 && c <= 42 {
                // Letters should all be non-empty (A=17+32, but in printable range)
                break;
            }
        }
        if consecutive_good >= 60 {
            // At least 60 good characters out of 96
            font_candidates.push((start, consecutive_good));
        }
    }

    println!("Found {} font table candidates", font_candidates.len());
    for &(start, count) in font_candidates.iter().take(5) {
        println!("\n  Candidate at ROM {:06X} ({} good chars):", start, count);
        // Show what 'H' would look like (offset 0x48 - 0x20 = 0x28 * 8 = 0x140)
        let h_offset = start + (0x48 - 0x20) * 8; // Assuming font starts at space
        if h_offset + 8 <= rom.len() {
            print!("    'H': ");
            for row in 0..8 {
                let b = rom[h_offset + row];
                for bit in (0..8).rev() {
                    if (b >> bit) & 1 != 0 {
                        print!("#");
                    } else {
                        print!(".");
                    }
                }
                print!("|");
            }
            println!();
        }
        // Show 'A'
        let a_offset = start + (0x41 - 0x20) * 8;
        if a_offset + 8 <= rom.len() {
            print!("    'A': ");
            for row in 0..8 {
                let b = rom[a_offset + row];
                for bit in (0..8).rev() {
                    if (b >> bit) & 1 != 0 {
                        print!("#");
                    } else {
                        print!(".");
                    }
                }
                print!("|");
            }
            println!();
        }
        // Show '0'
        let zero_offset = start + (0x30 - 0x20) * 8;
        if zero_offset + 8 <= rom.len() {
            print!("    '0': ");
            for row in 0..8 {
                let b = rom[zero_offset + row];
                for bit in (0..8).rev() {
                    if (b >> bit) & 1 != 0 {
                        print!("#");
                    } else {
                        print!(".");
                    }
                }
                print!("|");
            }
            println!();
        }
    }

    Ok(())
}
