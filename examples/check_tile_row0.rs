#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Check multiple tiles for transparent row 0 pattern.
/// If many tiles have transparent row 0 but solid other rows, it indicates
/// a systematic bug in VRAM write/DMA handling.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let state_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "states/Kato-chan & Ken-chan (Japan).slot1.state".to_string());

    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.load_state_from_file(&state_path)?;

    let vram_size = 0x8000usize; // 32K words
    let mut transparent_row0_count = 0;
    let mut total_nonblank_tiles = 0;
    let mut examples = Vec::new();

    // Check tiles used in the BAT (first 64x64 = 4096 entries)
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    let mut tiles_in_use = std::collections::HashSet::new();
    for row in 0..map_h {
        for col in 0..map_w {
            let addr = row * map_w + col;
            if addr >= vram_size {
                continue;
            }
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let tile_id = (entry & 0x07FF) as usize;
            tiles_in_use.insert(tile_id);
        }
    }

    println!("Map dimensions: {}x{}", map_w, map_h);
    println!("Unique tiles in BAT: {}", tiles_in_use.len());

    let mut sorted_tiles: Vec<usize> = tiles_in_use.into_iter().collect();
    sorted_tiles.sort();

    for &tile_id in &sorted_tiles {
        let tile_base = tile_id * 16;
        if tile_base + 15 >= vram_size {
            continue;
        }

        // Check if tile is all blank (all rows zero)
        let mut all_blank = true;
        let mut row0_blank = true;
        let mut other_rows_have_data = false;

        for row in 0..8u16 {
            let chr0 = emu.bus.vdc_vram_word((tile_base as u16) + row);
            let chr1 = emu.bus.vdc_vram_word((tile_base as u16) + 8 + row);
            if chr0 != 0 || chr1 != 0 {
                all_blank = false;
                if row == 0 {
                    row0_blank = false;
                } else {
                    other_rows_have_data = true;
                }
            }
        }

        if all_blank {
            continue;
        }
        total_nonblank_tiles += 1;

        if row0_blank && other_rows_have_data {
            transparent_row0_count += 1;
            if examples.len() < 20 {
                let chr0_r0 = emu.bus.vdc_vram_word(tile_base as u16);
                let chr0_r1 = emu.bus.vdc_vram_word(tile_base as u16 + 1);
                let chr1_r0 = emu.bus.vdc_vram_word(tile_base as u16 + 8);
                let chr1_r1 = emu.bus.vdc_vram_word(tile_base as u16 + 9);
                examples.push((tile_id, chr0_r0, chr0_r1, chr1_r0, chr1_r1));
            }
        }
    }

    println!("\nNon-blank tiles in use: {}", total_nonblank_tiles);
    println!(
        "Tiles with transparent row 0 but data in rows 1-7: {}",
        transparent_row0_count
    );
    println!(
        "Ratio: {:.1}%",
        if total_nonblank_tiles > 0 {
            transparent_row0_count as f64 / total_nonblank_tiles as f64 * 100.0
        } else {
            0.0
        }
    );

    if !examples.is_empty() {
        println!("\nExamples of transparent-row-0 tiles:");
        for (tid, cr0_0, cr0_1, cr1_0, cr1_1) in &examples {
            println!(
                "  tile 0x{:03X}: row0 chr0={:04X} chr1={:04X} | row1 chr0={:04X} chr1={:04X}",
                tid, cr0_0, cr1_0, cr0_1, cr1_1
            );
        }
    }

    // Also check: do any tiles have transparent row 7 but data in rows 0-6?
    let mut transparent_row7_count = 0;
    for &tile_id in &sorted_tiles {
        let tile_base = tile_id * 16;
        if tile_base + 15 >= vram_size {
            continue;
        }

        let chr0_r7 = emu.bus.vdc_vram_word(tile_base as u16 + 7);
        let chr1_r7 = emu.bus.vdc_vram_word(tile_base as u16 + 15);
        let row7_blank = chr0_r7 == 0 && chr1_r7 == 0;

        let mut rows06_have_data = false;
        for row in 0..7u16 {
            let chr0 = emu.bus.vdc_vram_word(tile_base as u16 + row);
            let chr1 = emu.bus.vdc_vram_word(tile_base as u16 + 8 + row);
            if chr0 != 0 || chr1 != 0 {
                rows06_have_data = true;
                break;
            }
        }

        if row7_blank && rows06_have_data {
            transparent_row7_count += 1;
        }
    }
    println!(
        "\nTiles with transparent row 7 but data in rows 0-6: {}",
        transparent_row7_count
    );

    // Check a specific tile that should be fully solid (if any)
    // Let's check the BAT entry at row 0, col 0
    let bat_00 = emu.bus.vdc_vram_word(0);
    let tid_00 = (bat_00 & 0x07FF) as usize;
    println!("\n=== BAT(0,0) tile 0x{:03X} full dump ===", tid_00);
    let tbase = tid_00 * 16;
    for row in 0..8u16 {
        let chr0 = emu.bus.vdc_vram_word(tbase as u16 + row);
        let chr1 = emu.bus.vdc_vram_word(tbase as u16 + 8 + row);
        println!("  row {}: chr0={:04X} chr1={:04X}", row, chr0, chr1);
    }

    // Check what's at VRAM address 0x1400 and nearby
    println!("\n=== VRAM dump around 0x1400 ===");
    for addr in 0x13F0u16..0x1410 {
        let word = emu.bus.vdc_vram_word(addr);
        if word != 0 || addr == 0x1400 {
            println!("  VRAM[{:04X}] = {:04X}", addr, word);
        }
    }

    Ok(())
}
