#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Pixel-level diagnostic: zoom into HUD rows and sprite area.
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

    // Run 5 frames to settle
    for _ in 0..5 {
        emu.bus.set_joypad_input(0xFF);
        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
    }

    // Get a frame
    let frame = loop {
        emu.bus.set_joypad_input(0xFF);
        emu.tick();
        if let Some(f) = emu.take_frame() {
            break f;
        }
    };

    // Dump full 240-line frame as PPM
    {
        let mut file = File::create("diag_full240.ppm")?;
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
    }

    // Dump zoomed HUD area (rows 0-40, 4x height)
    {
        let zoom = 4;
        let rows = 42;
        let mut file = File::create("diag_hud_zoom.ppm")?;
        writeln!(file, "P6\n{} {}\n255", WIDTH * zoom, rows * zoom)?;
        for y in 0..rows {
            for _rep in 0..zoom {
                for x in 0..WIDTH {
                    let pixel = frame[y * WIDTH + x];
                    let r = ((pixel >> 16) & 0xFF) as u8;
                    let g = ((pixel >> 8) & 0xFF) as u8;
                    let b = (pixel & 0xFF) as u8;
                    for _xrep in 0..zoom {
                        file.write_all(&[r, g, b])?;
                    }
                }
            }
        }
    }

    // Dump zoomed character area (find sprite around the character)
    // Character is usually around x=50-90, y=90-160
    {
        let zoom = 4;
        let x_start = 30;
        let x_end = 120;
        let y_start = 80;
        let y_end = 170;
        let w = x_end - x_start;
        let h = y_end - y_start;
        let mut file = File::create("diag_char_zoom.ppm")?;
        writeln!(file, "P6\n{} {}\n255", w * zoom, h * zoom)?;
        for y in y_start..y_end {
            for _rep in 0..zoom {
                for x in x_start..x_end {
                    let pixel = frame[y * WIDTH + x];
                    let r = ((pixel >> 16) & 0xFF) as u8;
                    let g = ((pixel >> 8) & 0xFF) as u8;
                    let b = (pixel & 0xFF) as u8;
                    for _xrep in 0..zoom {
                        file.write_all(&[r, g, b])?;
                    }
                }
            }
        }
    }

    // Analyze HUD for horizontal discontinuities
    println!("=== HUD row analysis (rows 0-40) ===");
    for y in 0..41 {
        let mut avg_r = 0u64;
        let mut avg_g = 0u64;
        let mut avg_b = 0u64;
        for x in 0..WIDTH {
            let pixel = frame[y * WIDTH + x];
            avg_r += ((pixel >> 16) & 0xFF) as u64;
            avg_g += ((pixel >> 8) & 0xFF) as u64;
            avg_b += (pixel & 0xFF) as u64;
        }
        avg_r /= WIDTH as u64;
        avg_g /= WIDTH as u64;
        avg_b /= WIDTH as u64;

        // Check if this row differs significantly from the one above
        let diff = if y > 0 {
            let mut d = 0u64;
            for x in 0..WIDTH {
                let p0 = frame[(y - 1) * WIDTH + x];
                let p1 = frame[y * WIDTH + x];
                let dr = ((p0 >> 16) & 0xFF) as i64 - ((p1 >> 16) & 0xFF) as i64;
                let dg = ((p0 >> 8) & 0xFF) as i64 - ((p1 >> 8) & 0xFF) as i64;
                let db = (p0 & 0xFF) as i64 - (p1 & 0xFF) as i64;
                d += (dr.unsigned_abs() + dg.unsigned_abs() + db.unsigned_abs()) as u64;
            }
            d / WIDTH as u64
        } else {
            0
        };
        let marker = if diff > 50 { " <<< DIFF" } else { "" };
        println!(
            "Row {:3}: avg=({:3},{:3},{:3}) diff_from_prev={:4}{}",
            y, avg_r, avg_g, avg_b, diff, marker
        );
    }

    // Show per-line scroll + y_offset details around split
    println!("\n=== Scroll details rows 32-40 ===");
    for row in 32..41 {
        let line = emu.bus.vdc_line_state_index_for_row(row);
        let (bxr, byr) = emu.bus.vdc_scroll_line(line);
        let y_off = emu.bus.vdc_scroll_line_y_offset(line);
        let valid = emu.bus.vdc_scroll_line_valid(line);
        println!(
            "Row {:3} (line {:3}): BXR={:4} BYR={:4} y_offset={:3} valid={}",
            row, line, bxr, byr, y_off, valid
        );
    }

    // Show sprite SATB info
    println!("\n=== Active sprites ===");
    for sprite in 0..64usize {
        let base = sprite * 4;
        let y_word = emu.bus.vdc_satb_word(base);
        let x_word = emu.bus.vdc_satb_word(base + 1);
        let pattern_word = emu.bus.vdc_satb_word(base + 2);
        let attr_word = emu.bus.vdc_satb_word(base + 3);
        if y_word == 0 && x_word == 0 && pattern_word == 0 && attr_word == 0 {
            continue;
        }
        let y = (y_word & 0x03FF) as i32 - 64;
        let x = (x_word & 0x03FF) as i32 - 32;
        let width = if (attr_word & 0x0100) != 0 { 32 } else { 16 };
        let height = match (attr_word >> 12) & 0x03 {
            0 => 16,
            1 => 32,
            _ => 64,
        };
        let pattern = (pattern_word >> 1) & 0x03FF;
        let pal = attr_word & 0x000F;
        let pri = if (attr_word & 0x0080) != 0 {
            "hi"
        } else {
            "lo"
        };
        let hf = if (attr_word & 0x0800) != 0 { "H" } else { "." };
        let vf = if (attr_word & 0x8000) != 0 { "V" } else { "." };
        println!(
            "  #{:02} x={:4} y={:4} {}x{} pat={:03X} pal={:X} pri={} {}{}",
            sprite, x, y, width, height, pattern, pal, pri, hf, vf
        );
    }

    println!("\nDumped: diag_full240.ppm, diag_hud_zoom.ppm, diag_char_zoom.ppm");
    Ok(())
}
