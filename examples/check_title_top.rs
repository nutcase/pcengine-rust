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

    let mut last_frame = None;
    let mut frames = 0;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }
    let frame = last_frame.unwrap();

    // Write top 50 rows at 4x scale for close inspection
    let scale = 4;
    let rows = 50;
    let mut file = File::create("katoken_top_zoom.ppm")?;
    writeln!(file, "P6\n{} {}\n255", 256 * scale, rows * scale)?;
    for y in 0..rows {
        for _sy in 0..scale {
            for x in 0..256 {
                let pixel = frame[y * 256 + x];
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                for _sx in 0..scale {
                    file.write_all(&[r, g, b])?;
                }
            }
        }
    }

    // Also analyze first 30 rows
    let bg = frame[0];
    println!("BG color: #{:06X}", bg);
    for y in 0..30 {
        let mut colors: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
        for x in 0..256 {
            *colors.entry(frame[y * 256 + x]).or_insert(0) += 1;
        }
        let non_bg: usize = colors
            .iter()
            .filter(|&(&c, _)| c != bg)
            .map(|(_, &n)| n)
            .sum();
        if non_bg > 0 {
            let top_colors: Vec<String> = colors
                .iter()
                .filter(|&(&c, _)| c != bg)
                .take(4)
                .map(|(&c, &n)| format!("#{:06X}x{}", c, n))
                .collect();
            println!(
                "Row {:2}: {:3} non-bg - {}",
                y,
                non_bg,
                top_colors.join(", ")
            );
        } else {
            println!("Row {:2}: all bg", y);
        }
    }

    Ok(())
}
