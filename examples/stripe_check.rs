#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0;
    while frames < 150 {
        emu.tick();
        if let Some(frame) = emu.take_frame() {
            frames += 1;
            if frames == 150 {
                // Dump raw frame to PPM (full 240 lines, no VDS padding)
                let path = "/tmp/katoken_raw240.ppm";
                let mut file = std::fs::File::create(path)?;
                use std::io::Write;
                writeln!(file, "P6\n256 240\n255")?;
                for y in 0..240 {
                    for x in 0..256 {
                        let pixel = frame[y * 256 + x];
                        let r = ((pixel >> 16) & 0xFF) as u8;
                        let g = ((pixel >> 8) & 0xFF) as u8;
                        let b = (pixel & 0xFF) as u8;
                        file.write_all(&[r, g, b])?;
                    }
                }
                println!("Wrote raw 240-line frame to {}", path);

                // Check for stripe pattern: compare adjacent pixels in a few rows
                println!("\nChecking for vertical stripe patterns:");
                for check_row in [30, 60, 90, 120, 150, 200] {
                    let mut changes = 0;
                    let mut last_pixel = frame[check_row * 256];
                    for x in 1..256 {
                        let pixel = frame[check_row * 256 + x];
                        if pixel != last_pixel {
                            changes += 1;
                        }
                        last_pixel = pixel;
                    }
                    println!(
                        "  Row {}: {} color changes in 256 pixels",
                        check_row, changes
                    );
                }

                // Dump detailed pixel pattern for rows that show stripes
                for check_row in [60, 120] {
                    println!("\nRow {} pixels 0-31:", check_row);
                    for x in 0..32 {
                        let pixel = frame[check_row * 256 + x];
                        print!("{:06X} ", pixel);
                        if (x + 1) % 8 == 0 {
                            print!("| ");
                        }
                    }
                    println!();
                }
            }
        }
    }
    Ok(())
}
