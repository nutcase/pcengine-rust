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
    let mut last_frame = None;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }
    let frame = last_frame.unwrap();

    // Check what's in frame buffer rows 0-30
    println!("Frame buffer content (256x240):");
    for row in 0..30 {
        let mut nonblack = 0;
        let mut sample_colors = Vec::new();
        for x in 0..256 {
            let p = frame[row * 256 + x];
            if p != 0 {
                nonblack += 1;
            }
            if x % 32 == 0 {
                sample_colors.push(format!("{:06X}", p));
            }
        }
        println!(
            "Row {:3}: nonblack={:3}  samples: {}",
            row,
            nonblack,
            sample_colors.join(" ")
        );
    }

    // Also check rows around where title should be in active area
    // With BYR=51, title at tile map Y=0-50 would be at active rows 461-511 (wrapping)
    // But let's check rows 17-50 (where active content starts)
    println!("\nActive area (rows 17-50):");
    for row in 17..50 {
        let mut nonblack = 0;
        let mut unique_colors = std::collections::HashSet::new();
        for x in 0..256 {
            let p = frame[row * 256 + x];
            if p != 0 {
                nonblack += 1;
            }
            unique_colors.insert(p);
        }
        println!(
            "Row {:3}: nonblack={:3} unique_colors={:3}",
            row,
            nonblack,
            unique_colors.len()
        );
    }

    // Check BYR value
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    println!("\nBXR={} BYR={}", bxr, byr);

    // Check BAT rows 0-15 to find where title tiles are
    println!("\nBAT content for rows 0-15:");
    let (map_w, _map_h) = emu.bus.vdc_map_dimensions();
    for bat_row in 0..16usize {
        let mut entries = Vec::new();
        for col in 0..map_w.min(32) {
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
            println!("  BAT row {:2}: {}", bat_row, entries.join(" "));
        } else {
            println!("  BAT row {:2}: (empty)", bat_row);
        }
    }

    Ok(())
}
