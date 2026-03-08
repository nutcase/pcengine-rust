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
    while frames < 150 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Check background tile 0x200 data
    let tile_id = 0x200;
    let tile_base = tile_id * 16;
    println!("Tile 0x{:03X} data (base=0x{:04X}):", tile_id, tile_base);
    println!(
        "  chr_a (planes 0-1) = VRAM[{:#06X}..{:#06X}]:",
        tile_base,
        tile_base + 7
    );
    for row in 0..8 {
        let addr = tile_base + row;
        let word = emu.bus.vdc_vram_word(addr as u16);
        let low = word & 0xFF;
        let high = (word >> 8) & 0xFF;
        println!(
            "    row {}: word={:04X} plane0={:08b} plane1={:08b}",
            row, word, low, high
        );
    }
    println!(
        "  chr_b (planes 2-3) = VRAM[{:#06X}..{:#06X}]:",
        tile_base + 8,
        tile_base + 15
    );
    for row in 0..8 {
        let addr = tile_base + 8 + row;
        let word = emu.bus.vdc_vram_word(addr as u16);
        let low = word & 0xFF;
        let high = (word >> 8) & 0xFF;
        println!(
            "    row {}: word={:04X} plane0={:08b} plane1={:08b}",
            row, word, low, high
        );
    }

    // Decode pixel colors for tile 0x200 row 0 with palette 0
    println!("\nPixel decode for tile 0x200, row 0, palette 0:");
    let chr_a = emu.bus.vdc_vram_word(tile_base as u16);
    let chr_b = emu.bus.vdc_vram_word((tile_base + 8) as u16);
    for px in 0..8 {
        let shift = 7 - px;
        let p0 = (chr_a >> shift) & 1;
        let p1 = (chr_a >> (shift + 8)) & 1;
        let p2 = (chr_b >> shift) & 1;
        let p3 = (chr_b >> (shift + 8)) & 1;
        let idx = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
        println!(
            "  pixel {}: shift={} planes={}{}{}{} idx={}",
            px, shift, p3, p2, p1, p0, idx
        );
    }

    // Also check tile 0x26E (another common background tile)
    let tile_id2 = 0x26E;
    let tile_base2 = tile_id2 * 16;
    println!("\nTile 0x{:03X} row 6 (where stripes appear):", tile_id2);
    let chr_a2 = emu.bus.vdc_vram_word((tile_base2 + 6) as u16);
    let chr_b2 = emu.bus.vdc_vram_word((tile_base2 + 14) as u16);
    println!("  chr_a = {:04X}, chr_b = {:04X}", chr_a2, chr_b2);
    for px in 0..8 {
        let shift = 7 - px;
        let p0 = (chr_a2 >> shift) & 1;
        let p1 = (chr_a2 >> (shift + 8)) & 1;
        let p2 = (chr_b2 >> shift) & 1;
        let p3 = (chr_b2 >> (shift + 8)) & 1;
        let idx = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
        println!("  pixel {}: idx={}", px, idx);
    }

    // Check what BAT entries are at frame row 60
    // active_row = 60 - 0 = 60 (for raw frame row, not output row)
    // But actually frame row 60 in the raw frame is before/during VDS
    // Let me compute for specific active rows
    println!("\nBAT entries for active_row 43 (frame_row 60 = vsw+vds+43 = 17+43 = 60):");
    let byr = 51;
    let sample_y = byr + 43;
    let tile_row = sample_y / 8;
    let line_in_tile = sample_y % 8;
    println!(
        "  sample_y={}, tile_row={}, line_in_tile={}",
        sample_y, tile_row, line_in_tile
    );

    // BAT entry at (tile_row, 0)
    let map_w = 64;
    let page_cols = map_w / 32;
    let row = tile_row % 64;
    let col = 0;
    let page_y = row / 32;
    let page_x = col / 32;
    let in_page_y = row % 32;
    let in_page_x = col % 32;
    let page_index = page_y * page_cols + page_x;
    let addr = page_index * 0x400 + in_page_y * 32 + in_page_x;
    let entry = emu.bus.vdc_vram_word(addr as u16);
    let tile_id_bat = entry & 0x7FF;
    let pal = (entry >> 12) & 0xF;
    println!(
        "  BAT[{},0] addr={:#06X} entry={:#06X} tile={:#05X} pal={}",
        tile_row, addr, entry, tile_id_bat, pal
    );

    // Check that tile's data at line_in_tile
    let tb = (tile_id_bat as usize) * 16;
    let chr_a_tile = emu.bus.vdc_vram_word((tb + line_in_tile) as u16);
    let chr_b_tile = emu.bus.vdc_vram_word((tb + 8 + line_in_tile) as u16);
    println!(
        "  Tile {:#05X} line {}: chr_a={:04X} chr_b={:04X}",
        tile_id_bat, line_in_tile, chr_a_tile, chr_b_tile
    );
    for px in 0..8 {
        let shift = 7 - px;
        let p0 = (chr_a_tile >> shift) & 1;
        let p1 = (chr_a_tile >> (shift + 8)) & 1;
        let p2 = (chr_b_tile >> shift) & 1;
        let p3 = (chr_b_tile >> (shift + 8)) & 1;
        let idx = (p0 | (p1 << 1) | (p2 << 2) | (p3 << 3)) as usize;
        let pal_offset = (pal as usize) << 4;
        let color_idx = pal_offset + idx;
        println!("  px{}: idx={} color_idx={:#05X}", px, idx, color_idx);
    }

    Ok(())
}
