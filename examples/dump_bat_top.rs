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

    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    println!("BYR={} (0x{:04X}) map={}x{}", byr, byr, map_w, map_h);
    println!("Framebuffer row 0 → tile map Y={}", byr);
    println!("Framebuffer row 239 → tile map Y={}", byr as usize + 239);

    // Dump BAT rows 0-31 to see where title content is
    for bat_row in 0..32usize {
        let mut entries = Vec::new();
        for col in 0..map_w.min(64) {
            let page_cols = (map_w / 32).max(1);
            let page_x = col / 32;
            let page_y = bat_row / 32;
            let in_page_x = col % 32;
            let in_page_y = bat_row % 32;
            let page_index = page_y * page_cols + page_x;
            let addr = (page_index * 0x400 + in_page_y * 32 + in_page_x) & 0x7FFF;
            let entry = emu.bus.vdc_vram_word(addr as u16);
            if entry != 0 {
                let tile_id = entry & 0x07FF;
                let pal = (entry >> 12) & 0x0F;
                entries.push(format!("[{col}:{tile_id:03X}p{pal:X}]"));
            }
        }
        if !entries.is_empty() {
            let pixel_y = bat_row * 8;
            let fb_row_start = pixel_y as i32 - byr as i32;
            println!(
                "BAT row {:2} (Y={:3}-{:3}, fb≈{:4}): {} entries, first: {}",
                bat_row,
                pixel_y,
                pixel_y + 7,
                fb_row_start,
                entries.len(),
                entries
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }

    Ok(())
}
