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

    // Check font tile area at each frame
    let check_tiles: &[(char, u16)] = &[
        ('H', 0x148),
        ('I', 0x149),
        ('0', 0x130),
        ('U', 0x155),
        (' ', 0x140),
    ];

    let mut frames = 0;
    let mut prev_checksums: Vec<u32> = vec![0; check_tiles.len()];

    while frames < 30 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;

            // Calculate checksum for each tile
            let mut changed = false;
            let mut checksums = Vec::new();
            for (i, &(ch, tid)) in check_tiles.iter().enumerate() {
                let base = tid as usize * 16;
                let mut sum: u32 = 0;
                for w in 0..16usize {
                    sum = sum.wrapping_add(emu.bus.vdc_vram_word((base + w) as u16) as u32);
                }
                checksums.push(sum);
                if sum != prev_checksums[i] {
                    changed = true;
                }
            }

            if changed || frames <= 5 {
                println!("Frame {:3}:", frames);
                for (i, &(ch, tid)) in check_tiles.iter().enumerate() {
                    let base = tid as usize * 16;
                    let p0_rows: Vec<u8> = (0..8)
                        .map(|r| (emu.bus.vdc_vram_word((base + r) as u16) & 0xFF) as u8)
                        .collect();
                    let all_zero = checksums[i] == 0;
                    let marker = if checksums[i] != prev_checksums[i] {
                        " *** CHANGED"
                    } else {
                        ""
                    };
                    print!("  '{}' {:03X}: ", ch, tid);
                    if all_zero {
                        print!("ALL ZERO");
                    } else {
                        for &b in &p0_rows {
                            for bit in (0..8).rev() {
                                if (b >> bit) & 1 != 0 {
                                    print!("#");
                                } else {
                                    print!(".");
                                }
                            }
                            print!("|");
                        }
                    }
                    println!("{}", marker);
                }
            }
            prev_checksums = checksums;
        }
    }

    // Also check what font data looks like in the ROM
    // The game should have font data somewhere in ROM
    // Search ROM for a recognizable '0' character pattern
    // '0' in 6x8 or 8x8 font is typically: 0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00
    // or variations thereof
    println!("\n=== Searching ROM for font patterns ===");
    let rom_data = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;

    // Search for common '0' patterns
    let zero_patterns: Vec<&[u8]> = vec![
        &[0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00], // Common '0'
        &[0x3C, 0x7E, 0xE7, 0xE7, 0xE7, 0xE7, 0x7E, 0x3C], // Rounded '0'
        &[0x7C, 0xC6, 0xCE, 0xD6, 0xE6, 0xC6, 0x7C, 0x00], // Another '0'
    ];

    for (pi, pat) in zero_patterns.iter().enumerate() {
        for i in 0..rom_data.len().saturating_sub(pat.len()) {
            if &rom_data[i..i + pat.len()] == *pat {
                println!("  Found pattern {} at ROM offset {:06X}", pi, i);
                // Show nearby context
                let start = i.saturating_sub(8);
                let end = (i + pat.len() + 48).min(rom_data.len());
                print!("  Context: ");
                for j in start..end {
                    if j == i {
                        print!("[");
                    }
                    print!("{:02X}", rom_data[j]);
                    if j == i + pat.len() - 1 {
                        print!("]");
                    } else {
                        print!(" ");
                    }
                }
                println!();
            }
        }
    }

    // Let me also search for the actual pattern we found in VRAM for '0'
    // plane 0: 0x3C, 0x7E, 0xEF, 0xE7, 0xE7, 0xF7, 0x7E, 0x3C
    let vram_zero = &[0x3Cu8, 0x7E, 0xEF, 0xE7, 0xE7, 0xF7, 0x7E, 0x3C];
    println!(
        "\n  Searching for VRAM-observed '0' pattern: {:02X?}",
        vram_zero
    );
    for i in 0..rom_data.len().saturating_sub(vram_zero.len()) {
        if &rom_data[i..i + vram_zero.len()] == vram_zero {
            println!("  Found at ROM offset {:06X}", i);
        }
    }

    // Search for the pattern with different stride (every 2nd byte for interleaved format)
    println!("\n  Searching for '0' pattern with stride 2 (interleaved):");
    for i in 0..rom_data.len().saturating_sub(16) {
        let mut matches = true;
        for (j, &b) in vram_zero.iter().enumerate() {
            if rom_data[i + j * 2] != b {
                matches = false;
                break;
            }
        }
        if matches {
            println!("  Found stride-2 at ROM offset {:06X}", i);
            let end = (i + 32).min(rom_data.len());
            print!("  Full data: ");
            for j in i..end {
                print!("{:02X} ", rom_data[j]);
            }
            println!();
            break; // Just show first match
        }
    }

    Ok(())
}
