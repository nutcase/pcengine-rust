#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Debug rendering gaps: run a ROM, dump frames showing per-scanline state
use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let rom_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Jaseiken Necromancer (Japan)".into());
    let slot_num = std::env::args().nth(2).unwrap_or_else(|| "4".into());
    let target_frame: usize = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let rom_path = format!("roms/{}.pce", rom_name);
    eprintln!(
        "ROM: {} slot={} (capturing frame {})",
        rom_path, slot_num, target_frame
    );

    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.set_audio_batch_size(128);

    // Load save state
    let state_path = format!("states/{}.slot{}.state", rom_name, slot_num);
    if std::path::Path::new(&state_path).exists() {
        match emu.load_state_from_file(&state_path) {
            Ok(()) => {
                eprintln!("Loaded save state: {}", state_path);
                emu.set_audio_batch_size(128);
            }
            Err(e) => eprintln!("Save state {} failed: {}", state_path, e),
        }
    } else {
        eprintln!("No save state at {}", state_path);
    }

    let mut frames = 0;
    let mut frame_buf: Vec<u32> = Vec::new();
    while frames < target_frame {
        emu.tick();
        let _ = emu.take_audio_samples();
        if emu.take_frame_into(&mut frame_buf) {
            frames += 1;
        }
    }

    let w = emu.display_width();
    let h = emu.display_height();
    let y_off = emu.display_y_offset();
    eprintln!("Display: {}x{}, y_offset={}", w, h, y_off);

    // Dump per-scanline VDC state, showing ALL lines with their scroll/control
    eprintln!("\n=== Per-scanline VDC state ===");
    let mut prev_sx: u16 = 0xFFFF;
    let mut prev_sy: u16 = 0xFFFF;
    let mut prev_ctrl: u16 = 0xFFFF;
    let mut rcr_count = 0;
    for y in 0..h.min(263) {
        let line_idx = emu.bus.vdc_line_state_index_for_row(y);
        let ctrl = emu.bus.vdc_control_line(line_idx);
        let bg_en = (ctrl & 0x0080) != 0;
        let spr_en = (ctrl & 0x0040) != 0;
        let (sx, sy) = emu.bus.vdc_scroll_line(line_idx);
        let sy_off = emu.bus.vdc_scroll_line_y_offset(line_idx);

        let changed = ctrl != prev_ctrl || sx != prev_sx || sy != prev_sy;
        if changed {
            rcr_count += 1;
            eprintln!(
                "row {:3} (sl {:3}): ctrl={:#06x} bg={} spr={} sx={:4} sy={:4} y_off={:4}  <== CHANGE",
                y, line_idx, ctrl, bg_en as u8, spr_en as u8, sx, sy, sy_off
            );
        }
        prev_ctrl = ctrl;
        prev_sx = sx;
        prev_sy = sy;
    }
    eprintln!("Total RCR-induced changes: {}", rcr_count);

    // Dump the frame
    let filename = format!("debug_gap_f{}.ppm", target_frame);
    let mut file = File::create(&filename)?;
    writeln!(file, "P6\n{} {}\n255", w, h)?;
    for pixel in &frame_buf {
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        file.write_all(&[r, g, b])?;
    }
    eprintln!("\nWrote {} ({}x{})", filename, w, h);

    // Scan for gap lines
    eprintln!("\n=== Scanning for gap lines ===");
    if frame_buf.len() >= w * h && w > 0 {
        // Find the most common color (likely the background)
        let mut color_counts = std::collections::HashMap::new();
        for &pixel in &frame_buf[..w.min(frame_buf.len())] {
            *color_counts.entry(pixel).or_insert(0usize) += 1;
        }
        let bg_color = color_counts
            .into_iter()
            .max_by_key(|&(_, c)| c)
            .map(|(p, _)| p)
            .unwrap_or(0);
        eprintln!("Background color estimate: {:#010x}", bg_color);

        for y in 1..h.saturating_sub(1) {
            let row_start = y * w;
            let prev_start = (y - 1) * w;
            let next_start = (y + 1) * w;
            if next_start + w > frame_buf.len() {
                break;
            }

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

            if bg_count > prev_bg + 30 && bg_count > next_bg + 30 {
                eprintln!(
                    "  row {:3}: bg_pixels={}/{} (prev={}, next={})",
                    y, bg_count, w, prev_bg, next_bg
                );
            }
        }
    }

    // Dump SATB (first 20 visible sprites)
    eprintln!("\n=== Visible sprites ===");
    let mut spr_count = 0;
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
        let w_cells = if (attr & 0x0100) != 0 { 2 } else { 1 };
        let sw = w_cells * 16;
        if y < (h as i32) && y + sh as i32 > 0 && x < (w as i32) && x + sw as i32 > 0 {
            let pri = if (attr & 0x0080) != 0 { "HI" } else { "lo" };
            eprintln!(
                "  spr[{:2}]: Y={:4}..{:<4} X={:4}..{:<4} {}x{} pat={:#06x} attr={:#06x} pri={}",
                i,
                y,
                y + sh as i32,
                x,
                x + sw as i32,
                sw,
                sh,
                pattern,
                attr,
                pri
            );
            spr_count += 1;
            if spr_count >= 30 {
                break;
            }
        }
    }

    // Dump magnified crops - centered around character sprites
    // For slot1 overworld: characters around (110-160, 100-170)
    // For slot4 crystal ball: characters around (80-180, 60-155)
    let crop_x0 = 80usize;
    let crop_y0 = 80usize;
    let crop_x1 = 190usize.min(w);
    let crop_y1 = 180usize.min(h);
    let crop_w = crop_x1 - crop_x0;
    let crop_h = crop_y1 - crop_y0;
    let scale = 6;
    let mag_filename = format!("debug_gap_f{}_zoom.ppm", target_frame);
    {
        let mut file = File::create(&mag_filename)?;
        writeln!(file, "P6\n{} {}\n255", crop_w * scale, crop_h * scale)?;
        for y in crop_y0..crop_y1 {
            let row = &frame_buf[y * w..(y * w + w)];
            for _sy in 0..scale {
                for x in crop_x0..crop_x1 {
                    let pixel = row[x];
                    let r = ((pixel >> 16) & 0xFF) as u8;
                    let g = ((pixel >> 8) & 0xFF) as u8;
                    let b = (pixel & 0xFF) as u8;
                    for _sx in 0..scale {
                        file.write_all(&[r, g, b])?;
                    }
                }
            }
        }
        eprintln!(
            "Wrote {} ({}x{} @ {}x)",
            mag_filename,
            crop_w * scale,
            crop_h * scale,
            scale
        );
    }

    // Dump pixel rows around sprite boundaries to spot gaps
    eprintln!("\n=== Pixel analysis around sprite[9] bottom (Y=145) ===");
    for row_y in 140..150.min(h) {
        eprint!("  row {:3}: ", row_y);
        for x in 108..148.min(w) {
            let pixel = frame_buf[row_y * w + x];
            if pixel == 0 {
                eprint!(".");
            } else {
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                // Classify pixel by dominant color channel
                if r > g && r > b {
                    eprint!("R");
                } else if g > r && g > b {
                    eprint!("G");
                } else if b > r && b > g {
                    eprint!("B");
                } else {
                    eprint!("#");
                }
            }
        }
        eprintln!();
    }

    eprintln!("\n=== Pixel analysis around sprite[11] bottom (Y=113) ===");
    for row_y in 108..118.min(h) {
        eprint!("  row {:3}: ", row_y);
        for x in 108..148.min(w) {
            let pixel = frame_buf[row_y * w + x];
            if pixel == 0 {
                eprint!(".");
            } else {
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                if r > g && r > b {
                    eprint!("R");
                } else if g > r && g > b {
                    eprint!("G");
                } else if b > r && b > g {
                    eprint!("B");
                } else {
                    eprint!("#");
                }
            }
        }
        eprintln!();
    }

    // Compare adjacent rows in the sprite area — look for single-row shifts
    eprintln!("\n=== Row-by-row hash comparison (sprite area x=80..192) ===");
    let hash_range = 80..192usize.min(w);
    let mut prev_hash = 0u64;
    for row_y in 40..h.min(170) {
        let mut hash: u64 = 0;
        for x in hash_range.clone() {
            let p = frame_buf[row_y * w + x];
            hash = hash.wrapping_mul(31).wrapping_add(p as u64);
        }
        // Check if this row duplicates or is very different
        let row_start = row_y * w;
        let non_black: usize = hash_range
            .clone()
            .filter(|&x| frame_buf[row_start + x] != 0)
            .count();
        if row_y > 40 {
            let prev_non_black: usize = hash_range
                .clone()
                .filter(|&x| frame_buf[(row_y - 1) * w + x] != 0)
                .count();
            let next_non_black: usize = if row_y + 1 < h {
                hash_range
                    .clone()
                    .filter(|&x| frame_buf[(row_y + 1) * w + x] != 0)
                    .count()
            } else {
                0
            };
            if non_black + 5 < prev_non_black && non_black + 5 < next_non_black {
                // Get the y_offset for this row
                let line_idx = emu.bus.vdc_line_state_index_for_row(row_y);
                let (sx, sy) = emu.bus.vdc_scroll_line(line_idx);
                let y_off = emu.bus.vdc_scroll_line_y_offset(line_idx);
                let effective_y = sy as usize + y_off as usize;
                eprintln!(
                    "  row {:3}: non_black={:3} (prev={:3}, next={:3}) eff_y={} tile_row={} line_in_tile={} ← GAP",
                    row_y,
                    non_black,
                    prev_non_black,
                    next_non_black,
                    effective_y,
                    effective_y / 8,
                    effective_y % 8
                );
            }
        }
        prev_hash = hash;
    }

    // Dump y_offset for first 20 rows to verify
    eprintln!("\n=== y_offset for first 20 rows ===");
    for y in 0..20.min(h) {
        let line_idx = emu.bus.vdc_line_state_index_for_row(y);
        let (sx, sy) = emu.bus.vdc_scroll_line(line_idx);
        let y_off = emu.bus.vdc_scroll_line_y_offset(line_idx);
        eprintln!(
            "  row {:3}: scanline={:3} sx={} sy={} y_off={} eff_y={}",
            y,
            line_idx,
            sx,
            sy,
            y_off,
            sy as usize + y_off as usize
        );
    }

    // Dump actual BG tile data at a gap position to verify
    // Gap at row 86: eff_y=390, tile_row=48, line_in_tile=6
    eprintln!("\n=== BG tile data at gap row (eff_y=390, tile_row=48) ===");
    let vram = emu.bus.vdc_vram();
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    eprintln!("Map dimensions: {}x{} tiles", map_w, map_h);
    // Display sx=432, so tile_col starts at 432/8 = 54
    let start_col = 432 / 8;
    // Show tiles at tile_row=48, cols around the visible area
    for col_off in 0..4 {
        let tile_col = (start_col + col_off + 10) % map_w; // +10 to get into the orb area
        let tile_row = 48usize;
        let map_addr = emu.bus.vdc_map_entry_address(tile_row, tile_col);
        let tile_entry = vram.get(map_addr).copied().unwrap_or(0);
        let tile_id = (tile_entry & 0x07FF) as usize;
        let palette_bank = ((tile_entry >> 12) & 0x0F) as usize;
        let tile_base = tile_id * 16;
        eprintln!(
            "  tile[{},{}] entry={:#06x} id={} pal={}",
            tile_row, tile_col, tile_entry, tile_id, palette_bank
        );
        // Dump all 8 rows of this tile
        for row in 0..8 {
            let chr0 = vram
                .get((tile_base + row) & (vram.len() - 1))
                .copied()
                .unwrap_or(0);
            let chr1 = vram
                .get((tile_base + row + 8) & (vram.len() - 1))
                .copied()
                .unwrap_or(0);
            let mut pixels = [0u8; 8];
            for bit in 0..8 {
                let shift = 7 - bit;
                let p0 = ((chr0 >> shift) & 1) as u8;
                let p1 = ((chr0 >> (shift + 8)) & 1) as u8;
                let p2 = ((chr1 >> shift) & 1) as u8;
                let p3 = ((chr1 >> (shift + 8)) & 1) as u8;
                pixels[bit] = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
            }
            let marker = if row == 6 { " <-- GAP ROW" } else { "" };
            eprintln!("    row {}: {:?}{}", row, pixels, marker);
        }
    }

    // Also render a debug "layer" image: BG pixels normal, sprite-only pixels green, gap pixels red
    // To do this, render frame twice: once with env DEBUG_BG_ONLY, once with env DEBUG_SPR_ONLY
    // Actually, use the existing framebuffer + a sprite-only render to identify layers
    // Simpler approach: scan for contiguous black pixels along the sprite-BG boundary
    eprintln!("\n=== Sprite/BG boundary analysis ===");
    // For each character sprite, check the row just below its bottom edge
    let char_sprites: Vec<(i32, i32, i32, &str)> = vec![
        (81, 113, 112, "spr[11] green"),
        (97, 129, 128, "spr[10] blue"),
        (113, 145, 112, "spr[9] pink"),
    ];
    for &(y_start, y_end, x_center, name) in &char_sprites {
        eprintln!("\n  {} (Y={}..{})", name, y_start, y_end);
        // Check rows around the bottom of the sprite
        let check_start = (y_end - 3).max(0) as usize;
        let check_end = (y_end + 3).min(h as i32) as usize;
        let x_range = ((x_center - 10).max(0) as usize)..((x_center + 26).min(w as i32) as usize);
        for row_y in check_start..check_end {
            eprint!(
                "    row {:3} ({}): ",
                row_y,
                if (row_y as i32) < y_end { "SPR" } else { "BG " }
            );
            for x in x_range.clone() {
                let pixel = frame_buf[row_y * w + x];
                if pixel == 0 {
                    eprint!(".");
                } else {
                    let r = ((pixel >> 16) & 0xFF) as u8;
                    let g = ((pixel >> 8) & 0xFF) as u8;
                    let b = (pixel & 0xFF) as u8;
                    if r > 200 && g < 50 && b < 50 {
                        eprint!("R");
                    } else if g > 200 && r < 50 && b < 50 {
                        eprint!("G");
                    } else if b > 200 && r < 50 && g < 50 {
                        eprint!("B");
                    } else if pixel == 0xFF000000 {
                        eprint!("X");
                    } else {
                        eprint!("#");
                    }
                }
            }
            eprintln!();
        }
    }

    Ok(())
}
