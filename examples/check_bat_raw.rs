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
        if let Some(_f) = emu.take_frame() {
            frames += 1;
        }
    }

    // Read raw BAT entries at positions where text should be
    // Map is 64x64 (MWR=0x0050)
    let map_w = 64;

    println!("=== Raw BAT entries for text tiles ===");
    // From dump_bat_text: 'H' at (20,8), 'I' at (20,9), 'S' at (20,10)
    // BAT row 20, cols 7-24 should have text
    println!("Row 20 (HISCORE):");
    for col in 5..30 {
        let bat_addr = 20 * map_w + col;
        let raw = emu.bus.vdc_vram_word(bat_addr as u16);
        let tile_id = raw & 0x07FF;
        let palette = (raw >> 12) & 0x0F;
        if tile_id != 0 && tile_id != 0x200 {
            println!(
                "  BAT[{},{}] = 0x{:04X} (tile=0x{:03X} pal={})",
                20, col, raw, tile_id, palette
            );
        }
    }

    println!("\nRow 24 (PUSH RUN BUTTON!):");
    for col in 5..30 {
        let bat_addr = 24 * map_w + col;
        let raw = emu.bus.vdc_vram_word(bat_addr as u16);
        let tile_id = raw & 0x07FF;
        let palette = (raw >> 12) & 0x0F;
        if tile_id != 0 && tile_id != 0x200 {
            println!(
                "  BAT[{},{}] = 0x{:04X} (tile=0x{:03X} pal={})",
                24, col, raw, tile_id, palette
            );
        }
    }

    // Also check what tile data exists at VRAM 0x2000+ (the game's actual tile data)
    println!("\n=== Tile patterns at VRAM 0x2000-0x2100 ===");
    for tile_id in 0x200u16..0x210 {
        let base = tile_id * 16;
        let all_zero = (0..16).all(|i| emu.bus.vdc_vram_word(base + i) == 0);
        if !all_zero {
            let w0 = emu.bus.vdc_vram_word(base);
            let w8 = emu.bus.vdc_vram_word(base + 8);
            // Extract plane 2 (low byte of words 8-15) for visualization
            print!("  Tile 0x{:03X}: ", tile_id);
            for row in 0..8 {
                let w = emu.bus.vdc_vram_word(base + 8 + row);
                let p2 = w & 0xFF;
                for bit in (0..8).rev() {
                    print!("{}", if (p2 >> bit) & 1 == 1 { "#" } else { "." });
                }
                if row < 7 {
                    print!("|");
                }
            }
            println!(" w0={:04X} w8={:04X}", w0, w8);
        }
    }

    // Check what's at VRAM addresses corresponding to tiles 0x130-0x15C
    println!("\n=== Tile patterns at tiles 0x130-0x15C (expected font location) ===");
    for &tid in &[0x130u16, 0x141, 0x148, 0x150, 0x15C] {
        let base = tid * 16;
        let all_zero = (0..16).all(|i| emu.bus.vdc_vram_word(base + i) == 0);
        print!("  Tile 0x{:03X}: ", tid);
        if all_zero {
            println!("[ALL ZERO]");
        } else {
            let w0 = emu.bus.vdc_vram_word(base);
            println!("w0={:04X} (has data)", w0);
        }
    }

    // Double-check: what tiles does the BAT ACTUALLY reference?
    // Read all BAT entries and find unique non-zero tile IDs with palette 5
    println!("\n=== All BAT entries with palette 5 ===");
    let mut pal5_tiles: std::collections::BTreeMap<u16, Vec<(usize, usize)>> =
        std::collections::BTreeMap::new();
    for row in 0..64 {
        for col in 0..64 {
            let raw = emu.bus.vdc_vram_word((row * map_w + col) as u16);
            let palette = (raw >> 12) & 0x0F;
            if palette == 5 {
                let tile_id = raw & 0x07FF;
                pal5_tiles.entry(tile_id).or_default().push((row, col));
            }
        }
    }
    for (tid, positions) in &pal5_tiles {
        // Check if tile data exists at this tile's VRAM location
        let base = *tid as usize * 16;
        let has_data = (0..16).any(|i| emu.bus.vdc_vram_word((base + i) as u16) != 0);
        println!(
            "  Tile 0x{:03X} ({} uses) data_exists={}",
            tid,
            positions.len(),
            has_data
        );
    }

    Ok(())
}
