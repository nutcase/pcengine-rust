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

    let (map_w, _) = emu.bus.vdc_map_dimensions();
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0) as usize;

    // Check title rows 4-9 for palette 3 (title text) tiles
    println!("Title text tiles (palette 3) in BAT rows 4-9, flat addressing:");
    for bat_row in 4..=9 {
        let active_row_start = if bat_row * 8 >= byr {
            bat_row * 8 - byr
        } else {
            999
        };
        let visible = bat_row * 8 >= byr && bat_row * 8 < byr + 224;
        print!(
            "  row {:2} (sy {:3}, ar {:3}) {} |",
            bat_row,
            bat_row * 8,
            active_row_start,
            if visible { "VIS" } else { "   " }
        );
        let mut pal3_count = 0;
        for col in 0..map_w {
            let addr = bat_row * map_w + col;
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let pal = (entry >> 12) & 0xF;
            if pal == 3 {
                let tile = entry & 0x7FF;
                print!(" c{:02}:{:03X}", col, tile);
                pal3_count += 1;
            }
        }
        println!(" ({} tiles)", pal3_count);
    }

    // Check text rows for palette 5 (score text) tiles
    println!("\nScore text tiles (palette 5) in BAT rows 18-29, flat addressing:");
    for bat_row in 18..=29 {
        let active_row_start = if bat_row * 8 >= byr {
            bat_row * 8 - byr
        } else {
            999
        };
        let visible = bat_row * 8 >= byr && bat_row * 8 < byr + 224;
        print!(
            "  row {:2} (sy {:3}, ar {:3}) {} |",
            bat_row,
            bat_row * 8,
            active_row_start,
            if visible { "VIS" } else { "   " }
        );
        let mut pal5_count = 0;
        for col in 0..map_w {
            let addr = bat_row * map_w + col;
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let pal = (entry >> 12) & 0xF;
            if pal == 5 {
                let tile = entry & 0x7FF;
                print!(" c{:02}:{:03X}", col, tile);
                pal5_count += 1;
            }
        }
        println!(" ({} tiles)", pal5_count);
    }

    // Decode tile indices to ASCII for text rows
    println!("\nDecoding text (tile & 0xFF as ASCII-ish):");
    for bat_row in [20, 22, 24, 26] {
        print!("  row {:2}: \"", bat_row);
        for col in 0..64 {
            let addr = bat_row * map_w + col;
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let pal = (entry >> 12) & 0xF;
            if pal == 5 {
                let tile = (entry & 0xFF) as u8;
                // PCE font tiles often map tile index to ASCII
                if tile >= 0x20 && tile < 0x7F {
                    print!("{}", tile as char);
                } else {
                    print!("[{:02X}]", tile);
                }
            }
        }
        println!("\"");
    }

    Ok(())
}
