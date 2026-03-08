#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Verify that the RCR split boundary latches post-ISR scroll values.
use pce::emulator::Emulator;
use std::error::Error;
use std::fs::File;
use std::io::Write;

const WIDTH: usize = 256;
const HEIGHT: usize = 240;
const OUT_HEIGHT: usize = 224;

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

    println!("Loading state from {}", state_path);
    emu.load_state_from_file(&state_path)?;
    println!("State loaded");

    // Run one frame
    let frame = loop {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            break f;
        }
    };

    // Dump per-line scroll values around the split boundary
    println!("\nPer-line scroll values (rows 30-45):");
    println!("Row  | Line | BXR  | BYR  | Y_Offset | Valid");
    println!("-----|------|------|------|----------|------");
    for row in 30..46 {
        let line = emu.bus.vdc_line_state_index_for_row(row);
        let (bxr, byr) = emu.bus.vdc_scroll_line(line);
        let valid = emu.bus.vdc_scroll_line_valid(line);
        let y_offset = emu.bus.vdc_scroll_line_y_offset(line);
        println!(
            "{:4} | {:4} | {:4} | {:4} | {:8} | {}",
            row, line, bxr, byr, y_offset, valid
        );
    }

    // Check that row 36 has gameplay scroll (not HUD scroll)
    let line_35 = emu.bus.vdc_line_state_index_for_row(35);
    let line_36 = emu.bus.vdc_line_state_index_for_row(36);
    let (bxr_35, byr_35) = emu.bus.vdc_scroll_line(line_35);
    let (bxr_36, byr_36) = emu.bus.vdc_scroll_line(line_36);
    println!("\nRow 35 (HUD):      BXR={} BYR={}", bxr_35, byr_35);
    println!("Row 36 (gameplay): BXR={} BYR={}", bxr_36, byr_36);

    if byr_35 != byr_36 {
        println!("OK: Scroll split detected at row 36");
    } else {
        println!("WARNING: No scroll split at row 36!");
    }

    // Dump the frame
    write_ppm(&frame, "split_verify.ppm")?;
    println!("Frame dumped to split_verify.ppm");

    Ok(())
}

fn write_ppm(frame: &[u32], path: &str) -> Result<(), Box<dyn Error>> {
    if frame.len() != WIDTH * HEIGHT {
        return Err(format!(
            "unexpected frame size: {} (expected {})",
            frame.len(),
            WIDTH * HEIGHT
        )
        .into());
    }
    let mut file = File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", WIDTH, OUT_HEIGHT)?;
    for y in 0..OUT_HEIGHT {
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
