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
    println!("ROM size: {} bytes ({} KB)", rom.len(), rom.len() / 1024);

    // The PCE BIOS font for 'H' = 0x48 would be:
    // A typical 'H' pattern: 0x66,0x66,0x66,0x7E,0x66,0x66,0x66,0x00
    // or similar. Let's search for common letter patterns.

    // Search for 8-byte sequences that look like 'H' glyph
    // H pattern: two vertical bars connected by horizontal bar
    let h_patterns: &[&[u8]] = &[
        &[0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66], // common H
        &[0x42, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42], // thin H
        &[0x44, 0x44, 0x44, 0x7C, 0x44, 0x44, 0x44], // shifted H
        &[0x22, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x22], // narrow H
        &[0x24, 0x24, 0x24, 0x3C, 0x24, 0x24, 0x24], // narrow H variant
    ];

    for pat in h_patterns {
        for i in 0..rom.len().saturating_sub(pat.len()) {
            if rom[i..i + pat.len()] == **pat {
                println!(
                    "Found H-like pattern at ROM offset 0x{:05X}: {:02X?}",
                    i,
                    &rom[i..i + 8.min(rom.len() - i)]
                );
                // Show surrounding context as potential font table
                if i >= 8 * 8 {
                    // Check if there's a font table before this
                    let table_start = i - 8 * (0x48 - 0x20); // H is at ASCII 0x48, font starts at 0x20
                    if table_start < rom.len() {
                        // Check if space (0x20) at table start is all zeros
                        let space = &rom[table_start..table_start + 8];
                        if space.iter().all(|&b| b == 0) {
                            println!(
                                "  -> Possible font table at 0x{:05X} (space=00s)",
                                table_start
                            );
                            // Show 'A' at offset 0x21*8
                            let a_off = table_start + (0x41 - 0x20) * 8;
                            if a_off + 8 <= rom.len() {
                                println!("  -> 'A' would be: {:02X?}", &rom[a_off..a_off + 8]);
                            }
                        }
                    }
                }
            }
        }
    }

    // Also try: scan for a block of 96*8 = 768 bytes that looks like a font table
    // (starts with 8 zero bytes for space, then has reasonable byte patterns)
    println!("\n=== Scanning for font tables (768 bytes, starts with 8 zeros) ===");
    for i in 0..rom.len().saturating_sub(768) {
        // Space should be all zeros
        if rom[i..i + 8].iter().any(|&b| b != 0) {
            continue;
        }
        // '!' (offset 8) should have some non-zero pattern
        if rom[i + 8..i + 16].iter().all(|&b| b == 0) {
            continue;
        }
        // 'A' at offset (0x41-0x20)*8 = 0x21*8 = 264
        let a_off = i + 264;
        if a_off + 8 > rom.len() {
            continue;
        }
        let a_data = &rom[a_off..a_off + 8];
        // A should have a roughly symmetric pattern with some non-zero bytes
        let nonzero = a_data.iter().filter(|&&b| b != 0).count();
        if nonzero < 5 {
            continue;
        }
        // H at offset (0x48-0x20)*8 = 0x28*8 = 320
        let h_off = i + 320;
        if h_off + 8 > rom.len() {
            continue;
        }
        let h_data = &rom[h_off..h_off + 8];
        // H should have symmetric left-right pattern
        let h_sym = h_data.iter().filter(|&&b| b == (b.reverse_bits())).count();

        println!("Candidate at 0x{:05X}:", i);
        println!("  ' '={:02X?}", &rom[i..i + 8]);
        println!("  '!'={:02X?}", &rom[i + 8..i + 16]);
        println!(
            "  '0'={:02X?}",
            &rom[i + (0x30 - 0x20) * 8..i + (0x30 - 0x20) * 8 + 8]
        );
        println!("  'A'={:02X?}", &rom[a_off..a_off + 8]);
        println!("  'H'={:02X?}", &rom[h_off..h_off + 8]);
        println!(
            "  'P'={:02X?}",
            &rom[i + (0x50 - 0x20) * 8..i + (0x50 - 0x20) * 8 + 8]
        );
    }

    Ok(())
}
