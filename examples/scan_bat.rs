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

    let check_frames = [130, 140, 200, 300, 500];
    let mut next_check = 0;

    while frames < 501 {
        emu.tick();
        if let Some(_f) = emu.take_frame() {
            frames += 1;

            if next_check < check_frames.len() && frames == check_frames[next_check] {
                next_check += 1;

                let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
                let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
                let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
                let cr = emu.bus.vdc_register(0x05).unwrap_or(0);

                let width_code = ((mwr >> 4) & 0x03) as usize;
                let height_code = ((mwr >> 6) & 0x01) as usize;
                let map_w = match width_code {
                    0 => 32,
                    1 => 64,
                    2 | 3 => 128,
                    _ => 32,
                };
                let map_h = if height_code == 0 { 32 } else { 64 };

                println!("\n=== Frame {} ===", frames);
                println!(
                    "MWR={:04X} map={}x{} BXR={:04X} BYR={:04X} CR={:04X}",
                    mwr, map_w, map_h, bxr, byr, cr
                );

                // Scan ALL BAT entries for non-zero
                let bat_size = map_w * map_h;
                let mut non_zero_count = 0;
                let mut non_zero_rows: std::collections::BTreeMap<
                    usize,
                    Vec<(usize, u16, u16, u16)>,
                > = std::collections::BTreeMap::new();

                for row in 0..map_h {
                    for col in 0..map_w {
                        let addr = (row * map_w + col) as u16;
                        let entry = emu.bus.vdc_vram_word(addr);
                        if entry != 0 {
                            non_zero_count += 1;
                            let tile_id = entry & 0x07FF;
                            let palette = (entry >> 12) & 0x0F;
                            non_zero_rows
                                .entry(row)
                                .or_default()
                                .push((col, entry, tile_id, palette));
                        }
                    }
                }

                println!(
                    "Non-zero BAT entries: {} / {} total",
                    non_zero_count, bat_size
                );

                // Show rows with non-zero entries
                for (row, entries) in &non_zero_rows {
                    let pixel_y = row * 8;
                    let screen_y = (pixel_y as i32) - (byr as i32);
                    let screen_y_wrapped = if screen_y < 0 {
                        screen_y + (map_h * 8) as i32
                    } else {
                        screen_y
                    };

                    // Show summary of this row
                    let min_col = entries.iter().map(|e| e.0).min().unwrap();
                    let max_col = entries.iter().map(|e| e.0).max().unwrap();
                    let tile_ids: Vec<u16> = entries.iter().map(|e| e.2).collect();
                    let min_tile = *tile_ids.iter().min().unwrap();
                    let max_tile = *tile_ids.iter().max().unwrap();
                    let palettes: Vec<u16> = entries.iter().map(|e| e.3).collect();

                    // Check if any tiles look like text (0x100+ASCII range)
                    let text_tiles: Vec<_> = entries
                        .iter()
                        .filter(|e| e.2 >= 0x120 && e.2 <= 0x17F)
                        .collect();

                    println!(
                        "  row {:2} (mapY={:3} scrY={:3}): {} entries, cols {}-{}, tiles 0x{:03X}-0x{:03X} pals {:?}{}",
                        row,
                        pixel_y,
                        screen_y_wrapped,
                        entries.len(),
                        min_col,
                        max_col,
                        min_tile,
                        max_tile,
                        palettes.iter().collect::<std::collections::BTreeSet<_>>(),
                        if !text_tiles.is_empty() {
                            format!(
                                " [TEXT: {:?}]",
                                text_tiles
                                    .iter()
                                    .map(|e| {
                                        let ch = (e.2 - 0x100) as u8 as char;
                                        format!("0x{:03X}='{}'", e.2, ch)
                                    })
                                    .collect::<Vec<_>>()
                            )
                        } else {
                            String::new()
                        }
                    );

                    // If row has text tiles, show the full row content
                    if !text_tiles.is_empty() {
                        print!("    text: ");
                        let mut sorted = entries.clone();
                        sorted.sort_by_key(|e| e.0);
                        for e in &sorted {
                            if e.2 >= 0x120 && e.2 <= 0x17F {
                                let ch = (e.2 - 0x100) as u8 as char;
                                print!("{}", ch);
                            } else if e.2 == 0x120 {
                                print!(" ");
                            } else {
                                print!(".");
                            }
                        }
                        println!();
                    }
                }

                // Also show VRAM content at key addresses
                println!(
                    "\n  VRAM sample: [0x0000]={:04X} [0x1000]={:04X} [0x2000]={:04X}",
                    emu.bus.vdc_vram_word(0x0000),
                    emu.bus.vdc_vram_word(0x1000),
                    emu.bus.vdc_vram_word(0x2000)
                );
            }
        }
    }

    Ok(())
}
