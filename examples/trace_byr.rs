#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Trace BYR values per scanline and check first few tile pixel rows.
use pce::emulator::Emulator;
use std::error::Error;

const WIDTH: usize = 256;
const HEIGHT: usize = 240;

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

    // Settle 3 frames
    for _ in 0..3 {
        emu.bus.set_joypad_input(0xFF);
        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
    }

    // One more frame to capture
    emu.bus.set_joypad_input(0xFF);
    let frame = loop {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            break f;
        }
    };

    println!("=== BYR/scroll values per output row ===");
    let mut prev_byr = 0xFFFFu16;
    let mut prev_bxr = 0xFFFFu16;
    for y in 0..HEIGHT {
        let line = emu.bus.vdc_line_state_index_for_row(y);
        let (bxr, byr) = emu.bus.vdc_scroll_line(line);
        let y_off = emu.bus.vdc_scroll_line_y_offset(line);
        let sample_y = byr as usize + y_off as usize;
        let tile_pixel_row = sample_y % 8;

        let changed = byr != prev_byr || bxr != prev_bxr;
        let is_tile_row0 = tile_pixel_row == 0;

        if y < 30 || changed || is_tile_row0 || y >= 140 && y <= 170 {
            let marker = if is_tile_row0 { " <<< TILE_ROW_0" } else { "" };
            let scroll_change = if changed && prev_byr != 0xFFFF {
                format!(" *** SCROLL CHANGE ***")
            } else {
                String::new()
            };
            println!(
                "  row {:3}: BXR={:3} BYR={:3} y_off={:3} sample_y={:3} tile_px_row={}{}{}",
                y, bxr, byr, y_off, sample_y, tile_pixel_row, marker, scroll_change
            );
        }
        prev_byr = byr;
        prev_bxr = bxr;
    }

    // Check the pixel colors at tile boundary rows in the HUD area
    println!("\n=== HUD pixel colors at tile boundaries (x=10,50,100) ===");
    let bg_color = emu.bus.vce_palette_rgb(0x00);
    println!(
        "Backdrop (VCE entry 0): RGB({},{},{})",
        (bg_color >> 16) & 0xFF,
        (bg_color >> 8) & 0xFF,
        bg_color & 0xFF
    );

    for y in 0..30 {
        let line = emu.bus.vdc_line_state_index_for_row(y);
        let (_bxr, byr) = emu.bus.vdc_scroll_line(line);
        let y_off = emu.bus.vdc_scroll_line_y_offset(line);
        let sample_y = byr as usize + y_off as usize;
        let tile_pixel_row = sample_y % 8;

        if tile_pixel_row == 0 {
            let p10 = frame[y * WIDTH + 10];
            let p50 = frame[y * WIDTH + 50];
            let p100 = frame[y * WIDTH + 100];
            println!(
                "  row {:3} (tile_px=0): px@10=RGB({},{},{}) px@50=RGB({},{},{}) px@100=RGB({},{},{})",
                y,
                (p10 >> 16) & 0xFF,
                (p10 >> 8) & 0xFF,
                p10 & 0xFF,
                (p50 >> 16) & 0xFF,
                (p50 >> 8) & 0xFF,
                p50 & 0xFF,
                (p100 >> 16) & 0xFF,
                (p100 >> 8) & 0xFF,
                p100 & 0xFF,
            );
        }
    }

    // Also check: what tile is at the HUD area and what does its row 0 look like vs row 1
    println!("\n=== Sample tile pixel check ===");
    let (map_w, _map_h) = emu.bus.vdc_map_dimensions();
    // Check a few columns in the first BAT row
    for col in [0, 5, 10, 15] {
        let entry = emu.bus.vdc_vram_word(col as u16);
        let tile_id = (entry & 0x07FF) as usize;
        let pal = (entry >> 12) & 0x0F;
        let base = (tile_id * 16) as u16;
        let chr0_r0 = emu.bus.vdc_vram_word(base);
        let chr0_r1 = emu.bus.vdc_vram_word(base + 1);
        let chr1_r0 = emu.bus.vdc_vram_word(base + 8);
        let chr1_r1 = emu.bus.vdc_vram_word(base + 9);
        println!(
            "  BAT col={:2}: tile=0x{:03X} pal={} row0: chr0={:04X} chr1={:04X} | row1: chr0={:04X} chr1={:04X}",
            col, tile_id, pal, chr0_r0, chr1_r0, chr0_r1, chr1_r1
        );
    }

    // Check the VDC vertical window params
    println!("\n=== VDC timing registers ===");
    let r0c = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let r0d = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let r0e = emu.bus.vdc_register(0x0E).unwrap_or(0);
    let vsw = (r0c & 0x1F) as usize;
    let vds = ((r0c >> 8) & 0xFF) as usize;
    let vdw = (r0d & 0x01FF) as usize;
    let vcr = (r0e & 0xFF) as usize;
    println!("  VSW: {}", vsw);
    println!("  VDS: {}", vds);
    println!("  VDW: {}", vdw);
    println!("  VCR: {}", vcr);
    println!(
        "  active_start_line = VSW+1+VDS+2 = {}+1+{}+2 = {}",
        vsw,
        vds,
        vsw + 1 + vds + 2
    );
    println!(
        "  active_end_line = active_start + VDW + 1 = {} + {} + 1 = {}",
        vsw + 1 + vds + 2,
        vdw,
        vsw + 1 + vds + 2 + vdw + 1
    );

    Ok(())
}
