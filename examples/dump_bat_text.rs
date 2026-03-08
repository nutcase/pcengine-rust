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

    // BAT layout: 64x64 tiles (MWR=0x0050)
    // BYR=0x0033 (51 pixels = 6 tile rows + 3 pixels)
    // Text rows from reference: Y=172, 188, 204, 220
    // Tile rows: (Y + BYR) / 8
    // Y=172 → (172+51)/8 = 27.875 → row 27
    // Y=188 → (188+51)/8 = 29.875 → row 29
    // Y=204 → (204+51)/8 = 31.875 → row 31
    // Y=220 → (220+51)/8 = 33.875 → row 33

    println!("=== BAT entries for text rows ===");
    let map_w = 64; // from MWR
    for (label, tile_row) in &[
        ("Row 27 (HISCORE)", 27),
        ("Row 28", 28),
        ("Row 29 (SCORE)", 29),
        ("Row 30", 30),
        ("Row 31 (PUSH RUN BUTTON!)", 31),
        ("Row 32", 32),
        ("Row 33 (© 1989...)", 33),
        ("Row 34", 34),
    ] {
        print!("{}: ", label);
        for col in 0..40 {
            let bat_addr = tile_row * map_w + col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            let palette = (entry >> 12) & 0x0F;
            if tile_id >= 0x100 && tile_id < 0x200 {
                let ascii = tile_id - 0x100;
                let ch = if ascii >= 0x20 && ascii < 0x7F {
                    ascii as u8 as char
                } else {
                    '?'
                };
                print!("[{:03X}='{}' p{}] ", tile_id, ch, palette);
            } else if tile_id == 0 || tile_id == 0x20 {
                print!(".");
            } else {
                // Skip non-text tiles
            }
        }
        println!();
    }

    // Let's also look more broadly for any tile IDs in the 0x130-0x15F range
    println!("\n=== Scanning ALL BAT for tiles 0x100-0x1FF ===");
    let mut tile_usage: std::collections::BTreeMap<u16, Vec<(usize, usize)>> =
        std::collections::BTreeMap::new();
    for row in 0..64 {
        for col in 0..64 {
            let bat_addr = row * map_w + col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            if tile_id >= 0x100 && tile_id < 0x200 {
                tile_usage.entry(tile_id).or_default().push((row, col));
            }
        }
    }
    for (tile_id, positions) in &tile_usage {
        let ascii = tile_id - 0x100;
        let ch = if ascii >= 0x20 && ascii < 0x7F {
            ascii as u8 as char
        } else {
            '?'
        };
        println!(
            "  Tile 0x{:03X} (ascii 0x{:02X} '{}') used {} times: {:?}",
            tile_id,
            ascii,
            ch,
            positions.len(),
            &positions[..positions.len().min(10)]
        );
    }

    Ok(())
}
