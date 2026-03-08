#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Zoom into the character's head area to diagnose the split.
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

    // Run 3 frames to settle
    for _ in 0..3 {
        emu.bus.set_joypad_input(0xFF);
        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
    }

    // Get frame
    emu.bus.set_joypad_input(0xFF);
    let frame = loop {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            break f;
        }
    };

    // Sprite #00 at (60, 145) 32x32
    // Zoom into character area: x=55-95, y=140-185 (8x zoom)
    let zoom = 8;
    let x_start = 55usize;
    let x_end = 95usize;
    let y_start = 140usize;
    let y_end = 185usize;
    let w = x_end - x_start;
    let h = y_end - y_start;

    let mut file = File::create("char_head_zoom.ppm")?;
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

    // Also zoom the signpost area (sprites #04-#09 around x=125, y=145)
    let x_start2 = 120usize;
    let x_end2 = 160usize;
    let y_start2 = 140usize;
    let y_end2 = 200usize;
    let w2 = x_end2 - x_start2;
    let h2 = y_end2 - y_start2;

    let mut file2 = File::create("signpost_zoom.ppm")?;
    writeln!(file2, "P6\n{} {}\n255", w2 * zoom, h2 * zoom)?;
    for y in y_start2..y_end2 {
        for _rep in 0..zoom {
            for x in x_start2..x_end2 {
                let pixel = frame[y * WIDTH + x];
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                for _xrep in 0..zoom {
                    file2.write_all(&[r, g, b])?;
                }
            }
        }
    }

    // Print per-row pixel info for the character head area
    println!("Character head area (rows 145-165, x=60-92):");
    for y in 145..165 {
        let mut non_bg = 0;
        let mut colors = Vec::new();
        for x in 60..92 {
            let p = frame[y * WIDTH + x];
            if p != frame[y_start * WIDTH + 0] {
                // not background
                non_bg += 1;
            }
            if x == 70 || x == 75 || x == 80 {
                colors.push(format!(
                    "({},{},{})",
                    (p >> 16) & 0xFF,
                    (p >> 8) & 0xFF,
                    p & 0xFF
                ));
            }
        }
        // Check if this is a BG tile boundary row (every 8 pixels from BYR offset)
        let line = emu.bus.vdc_line_state_index_for_row(y);
        let (_bxr, byr) = emu.bus.vdc_scroll_line(line);
        let y_off = emu.bus.vdc_scroll_line_y_offset(line);
        let sample_y = byr as usize + y_off as usize;
        let tile_pixel_row = sample_y % 8;
        let marker = if tile_pixel_row == 0 {
            " <<< TILE ROW 0"
        } else {
            ""
        };
        println!(
            "  row {:3}: non_bg_px={:2} sample_y={:3} tile_px_row={} samples=[{}]{}",
            y,
            non_bg,
            sample_y,
            tile_pixel_row,
            colors.join(", "),
            marker
        );
    }

    println!("\nDumped: char_head_zoom.ppm, signpost_zoom.ppm");
    Ok(())
}
