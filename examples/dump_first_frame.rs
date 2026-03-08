#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Dump the first frame after loading a save state (no warmup frames).
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

    // Get frame without any warmup
    for frame_idx in 0..5 {
        emu.bus.set_joypad_input(0xFF);
        let frame = loop {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                break f;
            }
        };

        let path = format!("first_frame_{}.ppm", frame_idx);
        let mut file = File::create(&path)?;
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

        // Print sprite info for this frame
        if frame_idx < 3 {
            println!("Frame {}: Sprites:", frame_idx);
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
                println!("  #{:02} x={:4} y={:4} {}x{}", sprite, x, y, width, height);
            }
        }
    }

    println!("Dumped first_frame_0.ppm through first_frame_4.ppm");
    Ok(())
}
