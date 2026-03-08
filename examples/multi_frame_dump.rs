#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Dump multiple frames from save state to find rendering issues.
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
    emu.load_state_from_file(&state_path)?;

    // Dump frames 0-19 (every frame) while holding right
    for phase in 0..20 {
        let pad = 0xFF & !(1u8 << 1); // Right pressed
        emu.bus.set_joypad_input(pad);

        let frame = loop {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                break f;
            }
        };

        let path = format!("mf_{:03}.ppm", phase);
        write_ppm(&frame, &path)?;

        // Print scroll info for this frame
        let mut scroll_changes = Vec::new();
        let mut prev = (0xFFFFu16, 0xFFFFu16);
        for row in 0..OUT_HEIGHT {
            let line = emu.bus.vdc_line_state_index_for_row(row);
            let (lx, ly) = emu.bus.vdc_scroll_line(line);
            if (lx, ly) != prev {
                scroll_changes.push((row, lx, ly));
                prev = (lx, ly);
            }
        }
        println!("Frame {:2}: scroll_changes={:?}", phase, scroll_changes);
    }

    // Also dump some frames with no input
    for phase in 20..30 {
        emu.bus.set_joypad_input(0xFF); // no buttons

        let frame = loop {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                break f;
            }
        };

        let path = format!("mf_{:03}.ppm", phase);
        write_ppm(&frame, &path)?;
    }

    println!("Dumped 30 frames to mf_000.ppm - mf_029.ppm");
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
