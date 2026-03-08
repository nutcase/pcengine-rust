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

    let mut frames = 0;
    while frames < 300 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
    let width_code = ((mwr >> 4) & 0x03) as usize;
    let map_width = match width_code {
        0 => 32,
        1 => 64,
        2 => 128,
        _ => 128,
    };
    let map_height = if (mwr >> 6) & 0x01 == 0 { 32 } else { 64 };
    println!("Map: {}x{} tiles (MWR={:04X})", map_width, map_height, mwr);

    // Flat addressing: address = row * map_width + col
    println!("\n=== BAT content (flat addressing) for text rows ===");
    for row in [20u16, 22, 24, 26] {
        print!("  Row {:2}: ", row);
        for col in 0..32u16 {
            let addr = (row as usize * map_width + col as usize) & 0x7FFF;
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let tile = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile != 0x100 && tile != 0x000 {
                print!("{:03X}p{:X} ", tile, pal);
            } else {
                print!("..... ");
            }
        }
        println!();
    }

    // Also show page-based addressing for comparison
    println!("\n=== BAT content (old page-based addressing) for text rows ===");
    let calc_page_addr = |row: usize, col: usize| -> usize {
        let r = row % map_height;
        let c = col % map_width;
        let page_cols = (map_width / 32).max(1);
        let px = c / 32;
        let py = r / 32;
        let ix = c % 32;
        let iy = r % 32;
        let pi = py * page_cols + px;
        (pi * 0x400 + iy * 32 + ix) & 0x7FFF
    };
    for row in [20u16, 22, 24, 26] {
        print!("  Row {:2}: ", row);
        for col in 0..32u16 {
            let addr = calc_page_addr(row as usize, col as usize);
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let tile = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile != 0x100 && tile != 0x000 {
                print!("{:03X}p{:X} ", tile, pal);
            } else {
                print!("..... ");
            }
        }
        println!();
    }

    // Check what's at the row the renderer ACTUALLY uses for Y=172
    // With timing_programmed: sample_y = Y - 12, so Y=172 → sample_y=160 → tile_row=20
    println!("\n=== Renderer BAT for Y=172 (tile_row=20) ===");
    println!("Flat addresses and VRAM content:");
    for col in 0..32u16 {
        let addr = (20 * map_width + col as usize) & 0x7FFF;
        let entry = emu.bus.vdc_vram_word(addr as u16);
        let tile = entry & 0x07FF;
        let pal = (entry >> 12) & 0x0F;
        if col % 8 == 0 {
            print!("  col {:2}-{:2}: ", col, col + 7);
        }
        if tile != 0x100 && tile != 0x000 {
            print!("{:03X}p{:X} ", tile, pal);
        } else {
            print!("..... ");
        }
        if col % 8 == 7 {
            println!();
        }
    }

    // Verify tile pattern for tile 0x148 (should be 'H')
    // Check if the VRAM at tile address actually has a font glyph
    println!("\n=== Font tile patterns ===");
    for &(ch, tid) in &[
        ('H', 0x148u16),
        ('I', 0x149),
        ('S', 0x153),
        ('P', 0x150),
        ('0', 0x130),
    ] {
        let base = tid as usize * 16;
        print!("  Tile {:03X} '{}': ", tid, ch);
        let mut all_zero = true;
        for row in 0..8 {
            let w0 = emu.bus.vdc_vram_word((base + row) as u16);
            let w1 = emu.bus.vdc_vram_word((base + row + 8) as u16);
            if w0 != 0 || w1 != 0 {
                all_zero = false;
            }
        }
        if all_zero {
            println!("EMPTY (all zeros)");
        } else {
            println!();
            for row in 0..8 {
                let w0 = emu.bus.vdc_vram_word((base + row) as u16);
                let w1 = emu.bus.vdc_vram_word((base + row + 8) as u16);
                let p0 = (w0 & 0xFF) as u8;
                let p1 = ((w0 >> 8) & 0xFF) as u8;
                let p2 = (w1 & 0xFF) as u8;
                let p3 = ((w1 >> 8) & 0xFF) as u8;
                print!("    ");
                for bit in (0..8).rev() {
                    let px = ((p0 >> bit) & 1)
                        | (((p1 >> bit) & 1) << 1)
                        | (((p2 >> bit) & 1) << 2)
                        | (((p3 >> bit) & 1) << 3);
                    if px == 0 {
                        print!(".");
                    } else {
                        print!("#");
                    }
                }
                println!();
            }
        }
    }

    // Check if there's a DIFFERENT tile range that has font data
    // Look for tiles that form recognizable letter patterns
    println!("\n=== Scanning for font-like tiles (tiles with simple 1-color patterns) ===");
    for tid in 0..512u16 {
        let base = tid as usize * 16;
        let mut nonzero_pixels = 0;
        let mut pixel_set = std::collections::HashSet::new();
        for row in 0..8usize {
            let w0 = emu.bus.vdc_vram_word((base + row) as u16);
            let w1 = emu.bus.vdc_vram_word((base + row + 8) as u16);
            let p0 = (w0 & 0xFF) as u8;
            let p1 = ((w0 >> 8) & 0xFF) as u8;
            let p2 = (w1 & 0xFF) as u8;
            let p3 = ((w1 >> 8) & 0xFF) as u8;
            for bit in (0..8).rev() {
                let px = ((p0 >> bit) & 1)
                    | (((p1 >> bit) & 1) << 1)
                    | (((p2 >> bit) & 1) << 2)
                    | (((p3 >> bit) & 1) << 3);
                if px != 0 {
                    nonzero_pixels += 1;
                    pixel_set.insert(px);
                }
            }
        }
        // Font tiles typically use 1-2 colors and have 10-40 non-zero pixels
        if pixel_set.len() == 1 && nonzero_pixels >= 8 && nonzero_pixels <= 45 {
            let base = tid as usize * 16;
            let mut pattern = String::new();
            for row in 0..8usize {
                let w0 = emu.bus.vdc_vram_word((base + row) as u16);
                let w1 = emu.bus.vdc_vram_word((base + row + 8) as u16);
                let p0 = (w0 & 0xFF) as u8;
                let p1 = ((w0 >> 8) & 0xFF) as u8;
                let p2 = (w1 & 0xFF) as u8;
                let p3 = ((w1 >> 8) & 0xFF) as u8;
                for bit in (0..8).rev() {
                    let px = ((p0 >> bit) & 1)
                        | (((p1 >> bit) & 1) << 1)
                        | (((p2 >> bit) & 1) << 2)
                        | (((p3 >> bit) & 1) << 3);
                    if px == 0 {
                        pattern.push('.');
                    } else {
                        pattern.push('#');
                    }
                }
                pattern.push('|');
            }
            println!(
                "  Tile {:03X} ({}px, color {:?}): {}",
                tid, nonzero_pixels, pixel_set, pattern
            );
        }
    }

    Ok(())
}
