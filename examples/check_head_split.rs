#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Check for character head split across 100 frames.
/// Dumps any frame where the character sprite (#00) appears to have
/// pixel anomalies in the head region.
use pce::emulator::Emulator;
use std::error::Error;
use std::fs::File;
use std::io::Write;

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

    let mut anomaly_frames = Vec::new();

    for frame_idx in 0..100 {
        emu.bus.set_joypad_input(0xFF);
        let frame = loop {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                break f;
            }
        };

        // Check sprite #00 position
        let y_word = emu.bus.vdc_satb_word(0);
        let x_word = emu.bus.vdc_satb_word(1);
        let char_y = (y_word & 0x03FF) as i32 - 64;
        let char_x = (x_word & 0x03FF) as i32 - 32;

        // Check the scroll split position
        let mut split_row = None;
        let mut prev_bxr = 0xFFFFu16;
        for row in 0..HEIGHT {
            let line = emu.bus.vdc_line_state_index_for_row(row);
            let (bxr, _byr) = emu.bus.vdc_scroll_line(line);
            if bxr != prev_bxr && prev_bxr != 0xFFFF {
                split_row = Some(row);
            }
            prev_bxr = bxr;
        }

        // Check if the split row intersects the character sprite
        let char_top = char_y;
        let char_bottom = char_y + 32;
        let intersects = if let Some(sr) = split_row {
            sr as i32 >= char_top && (sr as i32) < char_bottom
        } else {
            false
        };

        if intersects || frame_idx < 3 {
            println!(
                "Frame {:3}: sprite#0 at ({},{}) to ({},{}), split_row={:?} {}",
                frame_idx,
                char_x,
                char_y,
                char_x + 32,
                char_bottom,
                split_row,
                if intersects { "*** INTERSECTS ***" } else { "" }
            );
        }

        // Check for horizontal discontinuity in the character head area
        if char_y > 0 && char_y < HEIGHT as i32 - 32 && char_x >= 0 && char_x < WIDTH as i32 - 32 {
            let head_row = char_y as usize;
            // Check rows within the sprite for any horizontal color discontinuity
            // that might indicate a "split head"
            let mut max_diff = 0u64;
            let mut max_diff_row = 0;
            for dy in 1..16 {
                let row_a = head_row + dy - 1;
                let row_b = head_row + dy;
                if row_b >= HEIGHT {
                    break;
                }
                let mut diff = 0u64;
                for dx in 0..32 {
                    let x = (char_x as usize + dx).min(WIDTH - 1);
                    let pa = frame[row_a * WIDTH + x];
                    let pb = frame[row_b * WIDTH + x];
                    let dr = ((pa >> 16) & 0xFF) as i64 - ((pb >> 16) & 0xFF) as i64;
                    let dg = ((pa >> 8) & 0xFF) as i64 - ((pb >> 8) & 0xFF) as i64;
                    let db = (pa & 0xFF) as i64 - (pb & 0xFF) as i64;
                    diff += (dr.unsigned_abs() + dg.unsigned_abs() + db.unsigned_abs()) as u64;
                }
                if diff > max_diff {
                    max_diff = diff;
                    max_diff_row = dy;
                }
            }

            if max_diff > 3000 {
                anomaly_frames.push(frame_idx);
                println!(
                    "  HIGH pixel diff at head row +{}: diff={} (possible split)",
                    max_diff_row, max_diff
                );
                let path = format!("head_split_{:03}.ppm", frame_idx);
                write_ppm(&frame, &path)?;
                println!("  Dumped to {}", path);
            }
        }
    }

    println!(
        "\nTotal frames with anomalies: {} out of 100",
        anomaly_frames.len()
    );
    Ok(())
}

fn write_ppm(frame: &[u32], path: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", WIDTH, HEIGHT)?;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let pixel = frame[y * WIDTH + x];
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            file.write_all(&[r, g, b])?;
        }
    }
    Ok(())
}
