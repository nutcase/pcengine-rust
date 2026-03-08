#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

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

    let frame_active_row = 17; // VSW+VDS

    // Zoom 4x into active rows 10-70 (where title should be)
    let start_active = 10usize;
    let end_active = 70usize;
    let scale = 4;
    let w = 256 * scale;
    let h = (end_active - start_active) * scale;
    let mut file = File::create("katoken_title_zoom.ppm")?;
    writeln!(file, "P6\n{w} {h}\n255")?;
    for ar in start_active..end_active {
        let fy = frame_active_row + ar;
        for _ in 0..scale {
            for x in 0..256usize {
                let p = if fy < 240 { frame[fy * 256 + x] } else { 0 };
                let r = ((p >> 16) & 0xFF) as u8;
                let g = ((p >> 8) & 0xFF) as u8;
                let b = (p & 0xFF) as u8;
                for _ in 0..scale {
                    file.write_all(&[r, g, b])?;
                }
            }
        }
    }

    // Also print row content for title area
    println!("Title area content (active rows 10-60):");
    for ar in 10..60 {
        let fy = frame_active_row + ar;
        let tile_y = 51 + ar; // BYR=51
        let mut nonzero = 0;
        let mut unique = std::collections::HashSet::new();
        for x in 0..256 {
            let p = frame[fy * 256 + x];
            if p != 0x242491 {
                nonzero += 1;
            } // not background
            unique.insert(p);
        }
        let bat_row = tile_y / 8;
        let line_in_tile = tile_y % 8;
        println!(
            "  AR{ar:3} → Y={tile_y:3} (BAT row {bat_row:2} line {line_in_tile}): non-bg={nonzero:3} colors={:2}",
            unique.len()
        );
    }

    println!("wrote katoken_title_zoom.ppm");
    Ok(())
}
