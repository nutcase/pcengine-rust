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

    // First, check what tile IDs the BAT references for text rows
    // We need to run enough frames for the game to set up the screen
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
    println!("Map width: {} tiles (MWR={:04X})", map_width, mwr);

    // Read BAT entries at text rows using flat addressing
    println!("\n=== BAT tile IDs at text rows (flat addressing) ===");
    let mut text_tiles = std::collections::BTreeSet::new();
    for row in [20u16, 22, 24, 26] {
        print!("  Row {:2}: ", row);
        for col in 0..32u16 {
            let addr = (row as usize * map_width + col as usize) & 0x7FFF;
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let tile = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile != 0x100 && tile != 0x000 {
                print!("{:03X}p{} ", tile, pal);
                text_tiles.insert(tile);
            } else {
                print!(".... ");
            }
        }
        println!();
    }

    println!("\n=== Unique text tile IDs: {:?} ===", text_tiles);

    // Check font patterns at each tile ID referenced by text
    println!("\n=== Tile patterns for text tiles ===");
    for &tid in &text_tiles {
        let base = tid as usize * 16;
        let mut all_zero = true;
        let mut pixel_count = 0;
        for row in 0..8usize {
            let w0 = emu.bus.vdc_vram_word((base + row) as u16);
            let w1 = emu.bus.vdc_vram_word((base + row + 8) as u16);
            if w0 != 0 || w1 != 0 {
                all_zero = false;
            }
            let p0 = (w0 & 0xFF) as u8;
            let p1 = ((w0 >> 8) & 0xFF) as u8;
            let p2 = (w1 & 0xFF) as u8;
            let p3 = ((w1 >> 8) & 0xFF) as u8;
            for bit in 0..8 {
                let px = ((p0 >> bit) & 1)
                    | (((p1 >> bit) & 1) << 1)
                    | (((p2 >> bit) & 1) << 2)
                    | (((p3 >> bit) & 1) << 3);
                if px != 0 {
                    pixel_count += 1;
                }
            }
        }
        if all_zero {
            println!(
                "  Tile {:03X}: ALL ZERO (VRAM {:04X}-{:04X})",
                tid,
                base,
                base + 15
            );
        } else {
            print!("  Tile {:03X} ({}px): ", tid, pixel_count);
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
                        print!(".");
                    } else {
                        print!("#");
                    }
                }
                print!("|");
            }
            println!();
        }
    }

    // Now let's check: what does VRAM look like in a wider range around the font area?
    // Check which VRAM regions have non-zero content
    println!("\n=== VRAM usage map (64-word blocks) ===");
    for block in 0..512 {
        let start = block * 64;
        let mut nonzero = 0;
        for i in 0..64 {
            let w = emu.bus.vdc_vram_word((start + i) as u16);
            if w != 0 {
                nonzero += 1;
            }
        }
        if nonzero > 0 {
            println!(
                "  VRAM {:04X}-{:04X}: {}/64 non-zero words",
                start,
                start + 63,
                nonzero
            );
        }
    }

    Ok(())
}
