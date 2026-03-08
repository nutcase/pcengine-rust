#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Diagnostic tool: load Jaseiken Necromancer slot4 save state,
/// run a few frames, dump per-scanline scroll/control and the rendered frame.
use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

const FRAME_HEIGHT: usize = 263;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Jaseiken Necromancer (Japan)".into());
    let slot = std::env::args().nth(2).unwrap_or_else(|| "4".into());
    let rom_path = format!("roms/{}.pce", rom_name);
    let state_path = format!("states/{}.slot{}.state", rom_name, slot);
    eprintln!("ROM: {}", rom_path);
    eprintln!("State: {}", state_path);
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    emu.load_state_from_file(&state_path)?;
    emu.set_audio_batch_size(128);

    // Run a few frames to get into steady state
    let target_frames = 5;
    let mut frames = 0;
    let mut frame_buf: Vec<u32> = Vec::new();
    while frames < target_frames {
        emu.tick();
        if emu.take_frame_into(&mut frame_buf) {
            frames += 1;
        }
    }

    // Get display dimensions
    let w = emu.display_width();
    let h = emu.display_height();
    let y_off = emu.display_y_offset();
    eprintln!("Display: {}x{}, y_offset={}", w, h, y_off);

    // Dump per-scanline VDC state
    eprintln!("\n=== Per-scanline VDC state (output rows 0..{}) ===", h);
    eprintln!(
        "{:>4} {:>5} {:>8} {:>5} {:>5} {:>6} {:>6} {:>6} {:>5} {:>5}",
        "row", "scan", "ctrl", "bg", "spr", "scr_x", "scr_y", "y_off", "zm_x", "zm_y"
    );

    let mut prev_ctrl: u16 = 0xFFFF;
    let mut prev_sx: u16 = 0xFFFF;
    for y in 0..h.min(FRAME_HEIGHT) {
        let line_idx = emu.bus.vdc_line_state_index_for_row(y);
        let ctrl = emu.bus.vdc_control_line(line_idx);
        let bg_en = (ctrl & 0x0080) != 0;
        let spr_en = (ctrl & 0x0040) != 0;
        let (sx, sy) = emu.bus.vdc_scroll_line(line_idx);
        let sy_off = emu.bus.vdc_scroll_line_y_offset(line_idx);
        let (zx, zy) = emu.bus.vdc_zoom_line(line_idx);

        // Print lines where something changes, plus first/last few
        let changed = ctrl != prev_ctrl || sx != prev_sx;
        if y < 5 || y >= h - 3 || changed || y % 40 == 0 {
            eprintln!(
                "{:4} {:5} {:#06x} {:>5} {:>5} {:6} {:6} {:6} {:5} {:5}{}",
                y,
                line_idx,
                ctrl,
                bg_en,
                spr_en,
                sx,
                sy,
                sy_off,
                zx,
                zy,
                if changed && y > 0 { " <-- CHANGED" } else { "" }
            );
        }
        prev_ctrl = ctrl;
        prev_sx = sx;
    }

    // Dump frame as PPM
    let filename = "necro_slot4_gap.ppm";
    let mut file = File::create(filename)?;
    writeln!(file, "P6\n{} {}\n255", w, h)?;
    for pixel in &frame_buf {
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        file.write_all(&[r, g, b])?;
    }
    eprintln!("\nWrote {} ({}x{})", filename, w, h);

    // Scan for "gap" lines: rows where there's an abrupt color difference
    eprintln!("\n=== Scanning for potential gap lines ===");
    if !frame_buf.is_empty() {
        let bg_color = frame_buf[0]; // top-left pixel as reference
        for y in 1..h.saturating_sub(1) {
            let row_start = y * w;
            let prev_start = (y - 1) * w;
            let next_start = (y + 1) * w;
            if row_start + w > frame_buf.len() || next_start + w > frame_buf.len() {
                break;
            }

            // Count how many pixels match bg color
            let bg_count: usize = frame_buf[row_start..row_start + w]
                .iter()
                .filter(|&&p| p == bg_color)
                .count();
            let prev_bg: usize = frame_buf[prev_start..prev_start + w]
                .iter()
                .filter(|&&p| p == bg_color)
                .count();
            let next_bg: usize = frame_buf[next_start..next_start + w]
                .iter()
                .filter(|&&p| p == bg_color)
                .count();

            if bg_count > prev_bg + 20 && bg_count > next_bg + 20 && bg_count > w / 4 {
                eprintln!(
                    "  row {:3}: bg_pixels={}/{} (prev={}, next={})",
                    y, bg_count, w, prev_bg, next_bg
                );
            }
        }
    }

    // Dump SATB
    eprintln!("\n=== SATB sprites (visible) ===");
    let mut count = 0;
    for i in 0..64 {
        let y_word = emu.bus.vdc_satb_word(i * 4);
        let x_word = emu.bus.vdc_satb_word(i * 4 + 1);
        let pattern = emu.bus.vdc_satb_word(i * 4 + 2);
        let attr = emu.bus.vdc_satb_word(i * 4 + 3);
        let y = (y_word & 0x03FF) as i32 - 64;
        let x = (x_word & 0x03FF) as i32 - 32;
        let height_code = ((attr >> 12) & 0x03) as usize;
        let h_cells = match height_code {
            0 => 1,
            1 => 2,
            _ => 4,
        };
        let sh = h_cells * 16;
        if y < 300 && y + sh as i32 > 0 && x < 300 && x + 32 > -32 {
            eprintln!(
                "  spr[{:2}]: Y={:4} X={:4} pat={:#06x} attr={:#06x} h={}",
                i, y, x, pattern, attr, sh
            );
            count += 1;
            if count >= 30 {
                break;
            }
        }
    }

    Ok(())
}
