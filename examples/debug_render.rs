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
    let mut last_frame: Option<Vec<u32>> = None;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    // Check per-line zoom values
    println!("=== Per-line zoom values ===");
    for y in [0, 50, 100, 141, 172, 204, 220, 239] {
        let (zx, zy) = emu.bus.vdc_zoom_line(y);
        let valid = emu.bus.vdc_scroll_line_valid(y);
        let (sx, sy) = emu.bus.vdc_scroll_line(y);
        let ctrl = emu.bus.vdc_control_line(y);
        println!(
            "  Line {:3}: zoom=({:04X},{:04X}) scroll=({:04X},{:04X}) ctrl={:04X} valid={}",
            y, zx, zy, sx, sy, ctrl, valid
        );
    }

    // Manually compute what the renderer would produce for Y=172
    // effective_y_scroll = (BYR + 1) & 0x1FF = (0x33 + 1) & 0x1FF = 0x34 = 52
    // y_origin_bias = -0x40 = -64
    // step_y = zoom_step_value(zoom_y for line 172)
    // active_row = active_row_for_output_row(172)
    //
    // sample_y_fp = ((52 + (-64)) << 4) + (step_y * active_row)
    //             = (-12 << 4) + (step_y * active_row)
    //             = -192 + step_y * active_row
    //
    // sample_y = sample_y_fp >> 4

    let (_, zy172) = emu.bus.vdc_zoom_line(172);
    let step_y = {
        let value = (zy172 & 0x001F) as usize;
        value.max(1).min(32)
    };
    println!("\n=== Manual computation for Y=172 ===");
    println!("zoom_y for line 172: {:04X} → step_y = {}", zy172, step_y);
    println!("effective_y_scroll = 52, y_origin_bias = -64");

    // Active row depends on vertical window timing
    // For this game: active_start_line = VSW + VDS = 2 + 15 = 17
    // active_row for Y=172: cycle_pos = (17 + 172) % 263 = 189
    // If 189 >= 17 and < 257: active_row = 189 - 17 = 172
    let active_row = 172usize;

    let sample_y_fp = ((-12i32) << 4) + (step_y as i32 * active_row as i32);
    let sample_y = {
        let raw = sample_y_fp >> 4;
        // Map height 32 tiles * 8 pixels = 256
        raw.rem_euclid(256) as usize
    };
    println!(
        "sample_y_fp = -192 + {} * {} = {}",
        step_y, active_row, sample_y_fp
    );
    println!(
        "sample_y = {} >> 4 = {} → rem_euclid(256) = {}",
        sample_y_fp,
        sample_y_fp >> 4,
        sample_y
    );
    println!(
        "tile_row = {}, line_in_tile = {}",
        sample_y / 8,
        sample_y % 8
    );

    // Check BAT at that tile row
    let tile_row = sample_y / 8;
    println!("\nBAT row {} (first 32 cols):", tile_row);
    for col in 0..32u16 {
        let addr = tile_row as u16 * 64 + col; // simplified, might need page calculation
        let entry = emu.bus.vdc_vram_word(addr);
        let tile = entry & 0x07FF;
        let pal = (entry >> 12) & 0x0F;
        if tile != 0x100 && tile != 0x000 {
            if col % 8 == 0 {
                print!("  col {:2}: ", col);
            }
            print!("{:03X}p{:X} ", tile, pal);
            if col % 8 == 7 {
                println!();
            }
        }
    }
    println!();

    // Check if any tiles at row 20 (corrected) have non-zero patterns
    // map_entry_address for (row=20, col=8) in 64-wide map
    // page_cols = 2, page_x = 0, in_page_x = 8, in_page_y = 20
    // address = 0 + 20*32 + 8 = 648 = 0x288
    let addr_20_8 = 20 * 32 + 8; // In page-based layout: row 20, col 8
    let entry = emu.bus.vdc_vram_word(addr_20_8 as u16);
    println!(
        "Direct VRAM read at 0x{:04X} (row 20, col 8): {:04X} tile={:03X} pal={}",
        addr_20_8,
        entry,
        entry & 0x07FF,
        (entry >> 12) & 0x0F
    );

    // Actually use the correct page-based address
    // For 64-wide map: row R, col C
    // page_x = C / 32, in_page_x = C % 32
    // page_y = R / 32, in_page_y = R % 32
    // page_index = page_y * 2 + page_x
    // address = page_index * 0x400 + in_page_y * 32 + in_page_x
    let calc_addr = |row: usize, col: usize| -> usize {
        let page_x = col / 32;
        let page_y = row / 32;
        let in_page_x = col % 32;
        let in_page_y = row % 32;
        let page_index = page_y * 2 + page_x;
        page_index * 0x400 + in_page_y * 32 + in_page_x
    };

    println!("\n=== BAT entries (page-aware) for text rows ===");
    for row in [20, 24, 26] {
        print!("  Row {:2} cols 7-24: ", row);
        for col in 7..25 {
            let addr = calc_addr(row, col);
            let entry = emu.bus.vdc_vram_word(addr as u16);
            let tile = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile != 0x100 && tile != 0x000 {
                print!("{:03X}p{:X} ", tile, pal);
            } else {
                print!("..... ");
            }
        }
        println!();
    }

    // But the rendering code uses map_entry_address which takes (tile_row, tile_col)
    // NOT direct flat addressing. Let me check the ACTUAL address for (20, 8)
    // The rendering code does: self.vdc.map_entry_address(tile_row, tile_col)
    // which for 64-wide map uses the page layout.
    // For (20, 8): address = 0 + 20*32 + 8 = 648 = 0x288

    // Now let me check what the map_dimensions are:
    let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
    let width_code = ((mwr >> 4) & 0x03) as usize;
    let width_tiles = match width_code {
        0 => 32,
        1 => 64,
        2 => 128,
        _ => 128,
    };
    let height_tiles = if (mwr >> 6) & 0x01 == 0 { 32 } else { 64 };
    println!(
        "\nMWR = {:04X}, map size = {}x{} tiles",
        mwr, width_tiles, height_tiles
    );

    // Check map_height for sample_y calculation
    let map_height = height_tiles;
    let map_pixel_height = map_height * 8;
    println!("Map pixel height = {}", map_pixel_height);

    // Recalculate sample_y for Y=172 with correct map height
    let sample_y_corrected = {
        let raw = sample_y_fp >> 4;
        raw.rem_euclid(map_pixel_height as i32) as usize
    };
    println!(
        "sample_y (with {}px map height) = {} → tile_row = {}",
        map_pixel_height,
        sample_y_corrected,
        sample_y_corrected / 8
    );

    Ok(())
}
