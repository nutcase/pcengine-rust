#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Check HUD tile data from save state.
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

    // Get BAT entry at (0,0)
    let bat_entry = emu.bus.vdc_vram_word(0);
    let tile_id = (bat_entry & 0x07FF) as u16;
    let pal = (bat_entry >> 12) & 0x0F;
    println!(
        "BAT(0,0) = 0x{:04X} -> tile_id=0x{:03X} ({}) pal={}",
        bat_entry, tile_id, tile_id, pal
    );

    // Check tile pattern data for the CORRECT tile
    let tile_base = tile_id * 16;
    println!(
        "\n=== Tile 0x{:03X} pattern data (base=0x{:04X}) ===",
        tile_id, tile_base
    );
    for row in 0..8u16 {
        let chr0 = emu.bus.vdc_vram_word(tile_base + row);
        let chr1 = emu.bus.vdc_vram_word(tile_base + 8 + row);
        let mut pixels = [0u8; 8];
        for bit in 0..8 {
            let shift = 7 - bit;
            let p0 = ((chr0 >> shift) & 1) as u8;
            let p1 = ((chr0 >> (shift + 8)) & 1) as u8;
            let p2 = ((chr1 >> shift) & 1) as u8;
            let p3 = ((chr1 >> (shift + 8)) & 1) as u8;
            pixels[bit] = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
        }
        println!(
            "  row {}: chr0={:04X} chr1={:04X} -> pixels={:?}",
            row, chr0, chr1, pixels
        );
    }

    // Also check a few more BAT entries
    let (map_w, _map_h) = emu.bus.vdc_map_dimensions();
    println!("\nBAT row 0 (first 16 entries):");
    for col in 0..16.min(map_w) {
        let entry = emu.bus.vdc_vram_word(col as u16);
        let tid = entry & 0x07FF;
        let p = (entry >> 12) & 0x0F;
        print!(" {:03X}p{:X}", tid, p);
    }
    println!();

    println!("\nBAT row 2 (second row of tiles for HUD text area):");
    for col in 0..16.min(map_w) {
        let addr = map_w * 2 + col;
        let entry = emu.bus.vdc_vram_word(addr as u16);
        let tid = entry & 0x07FF;
        let p = (entry >> 12) & 0x0F;
        print!(" {:03X}p{:X}", tid, p);
    }
    println!();

    // VCE palette entries for palette 15
    println!("\nPalette bank 15 entries 0-3:");
    for i in 0..4 {
        let idx = 15 * 16 + i;
        let rgb = emu.bus.vce_palette_rgb(idx);
        println!(
            "  pal[{}][{}] = idx {} -> RGB=({},{},{})",
            15,
            i,
            idx,
            (rgb >> 16) & 0xFF,
            (rgb >> 8) & 0xFF,
            rgb & 0xFF
        );
    }

    // VCE palette entry 0 (background)
    let bg_rgb = emu.bus.vce_palette_rgb(0x00);
    println!(
        "\nVCE entry 0 (background): RGB=({},{},{})",
        (bg_rgb >> 16) & 0xFF,
        (bg_rgb >> 8) & 0xFF,
        bg_rgb & 0xFF
    );

    // Now run 3 frames and re-check
    for _ in 0..3 {
        emu.bus.set_joypad_input(0xFF);
        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
    }

    let bat_entry2 = emu.bus.vdc_vram_word(0);
    let tile_id2 = (bat_entry2 & 0x07FF) as u16;
    let pal2 = (bat_entry2 >> 12) & 0x0F;
    println!("\n=== After 3 frames ===");
    println!(
        "BAT(0,0) = 0x{:04X} -> tile_id=0x{:03X} ({}) pal={}",
        bat_entry2, tile_id2, tile_id2, pal2
    );

    let tile_base2 = tile_id2 * 16;
    println!("\nTile 0x{:03X} pattern data:", tile_id2);
    for row in 0..8u16 {
        let chr0 = emu.bus.vdc_vram_word(tile_base2 + row);
        let chr1 = emu.bus.vdc_vram_word(tile_base2 + 8 + row);
        let mut pixels = [0u8; 8];
        for bit in 0..8 {
            let shift = 7 - bit;
            let p0 = ((chr0 >> shift) & 1) as u8;
            let p1 = ((chr0 >> (shift + 8)) & 1) as u8;
            let p2 = ((chr1 >> shift) & 1) as u8;
            let p3 = ((chr1 >> (shift + 8)) & 1) as u8;
            pixels[bit] = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
        }
        println!(
            "  row {}: chr0={:04X} chr1={:04X} -> pixels={:?}",
            row, chr0, chr1, pixels
        );
    }

    Ok(())
}
