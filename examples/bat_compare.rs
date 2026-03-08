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

    let map_w = 64usize;
    let map_h = 64usize;

    println!("Comparing page-based vs flat BAT addressing for rows 0-15, cols 0-7:");
    println!(
        "{:>3} {:>3} | {:>6} {:>6} | {:>6} {:>6} | match?",
        "row", "col", "pg_adr", "pg_val", "fl_adr", "fl_val"
    );
    println!("{}", "-".repeat(60));

    let mut page_match = 0;
    let mut flat_match = 0;
    let mut both_match = 0;
    let mut total = 0;

    for row in 0..16 {
        for col in 0..8 {
            // Page-based address
            let page_cols = map_w / 32;
            let page_x = col / 32;
            let page_y = row / 32;
            let in_page_x = col % 32;
            let in_page_y = row % 32;
            let page_index = page_y * page_cols + page_x;
            let page_addr = (page_index * 0x400 + in_page_y * 32 + in_page_x) & 0x7FFF;

            // Flat address
            let flat_addr = (row * map_w + col) & 0x7FFF;

            let page_val = emu.bus.vdc_vram_word(page_addr as u16);
            let flat_val = emu.bus.vdc_vram_word(flat_addr as u16);

            let page_tile = page_val & 0x7FF;
            let flat_tile = flat_val & 0x7FF;

            // Check which one has a "reasonable" tile (non-zero, in expected range)
            let page_ok = page_val != 0 && page_tile < 0x400;
            let flat_ok = flat_val != 0 && flat_tile < 0x400;

            total += 1;
            if page_ok {
                page_match += 1;
            }
            if flat_ok {
                flat_match += 1;
            }
            if page_ok && flat_ok {
                both_match += 1;
            }

            if page_addr != flat_addr {
                println!(
                    "{:3} {:3} | {:#06X} {:#06X} | {:#06X} {:#06X} | pg={} fl={}",
                    row,
                    col,
                    page_addr,
                    page_val,
                    flat_addr,
                    flat_val,
                    if page_ok { "ok" } else { "--" },
                    if flat_ok { "ok" } else { "--" }
                );
            }
        }
    }

    println!("\nSummary for {} entries:", total);
    println!("  Page-based has valid tile: {}", page_match);
    println!("  Flat has valid tile: {}", flat_match);
    println!("  Both valid: {}", both_match);

    // Check the critical title area
    println!("\n--- Title area (row 8, cols 0-15) ---");
    for col in 0..16 {
        let page_addr = 8 * 32 + col; // page 0
        let flat_addr = 8 * 64 + col;
        let pv = emu.bus.vdc_vram_word(page_addr as u16);
        let fv = emu.bus.vdc_vram_word(flat_addr as u16);
        println!(
            "  col {:2}: page[{:#06X}]={:#06X}  flat[{:#06X}]={:#06X}",
            col, page_addr, pv, flat_addr, fv
        );
    }

    // Extended check: rows 8-15, full width scan for "which addressing has valid tiles"
    println!("\n--- Rows 8-15 valid tile counts (cols 0-31) ---");
    for row in 8..16 {
        let mut pg_valid = 0;
        let mut fl_valid = 0;
        for col in 0..32 {
            let pa = (row % 32) * 32 + col;
            let fa = row * 64 + col;
            let pv = emu.bus.vdc_vram_word(pa as u16);
            let fv = emu.bus.vdc_vram_word(fa as u16);
            if pv != 0 {
                pg_valid += 1;
            }
            if fv != 0 {
                fl_valid += 1;
            }
        }
        println!(
            "  row {}: page_valid={}/32  flat_valid={}/32",
            row, pg_valid, fl_valid
        );
    }

    // Check if the game writes with stride 32 or stride 64 by examining
    // contiguous BAT row data patterns
    println!("\n--- VRAM data at low addresses (0x000-0x1FF) showing BAT/tile overlap ---");
    println!("First 32 words of VRAM (BAT row 0 in page-based = row 0 cols 0-31):");
    for i in 0..32 {
        let v = emu.bus.vdc_vram_word(i as u16);
        if i % 8 == 0 {
            print!("  [{:04X}]: ", i);
        }
        print!("{:04X} ", v);
        if (i + 1) % 8 == 0 {
            println!();
        }
    }

    println!("Words 0x020-0x03F (BAT row 1 in page-based = rows 1 cols 0-31):");
    for i in 0x20..0x40 {
        let v = emu.bus.vdc_vram_word(i as u16);
        if i % 8 == 0 {
            print!("  [{:04X}]: ", i);
        }
        print!("{:04X} ", v);
        if (i + 1) % 8 == 0 {
            println!();
        }
    }

    println!("Words 0x040-0x05F (flat row 1 = BAT row 1 cols 0-31 in 64-wide):");
    for i in 0x40..0x60 {
        let v = emu.bus.vdc_vram_word(i as u16);
        if i % 8 == 0 {
            print!("  [{:04X}]: ", i);
        }
        print!("{:04X} ", v);
        if (i + 1) % 8 == 0 {
            println!();
        }
    }

    Ok(())
}
