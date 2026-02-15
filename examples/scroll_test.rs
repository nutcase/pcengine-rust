/// Run game while scrolling to detect artifacts at different scroll positions.
use pce::emulator::Emulator;
use std::collections::HashMap;
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
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0u64;

    // Get to gameplay
    while frames < 2000 {
        emu.tick();
        if let Some(_f) = emu.take_frame() {
            frames += 1;
            let press_run = matches!(frames,
                100..=110 | 200..=210 | 300..=310 | 400..=410 |
                500..=510 | 600..=610 | 700..=710 | 800..=810
            );
            if press_run {
                emu.bus.set_joypad_input(0x7F);
            } else {
                emu.bus.set_joypad_input(0xFF);
            }
        }
        if emu.cpu.halted {
            break;
        }
    }

    println!("Starting gameplay scroll test at frame {}", frames);

    // Now walk right for 600 frames (10 seconds), dumping frames every 30
    let mut anomaly_frames = Vec::new();

    for phase in 0..600 {
        // Hold right + occasionally I button (attack/action)
        // Right = bit 1 clear, active-low
        let pad = 0xFF & !(1u8 << 1); // Right pressed
        emu.bus.set_joypad_input(pad);

        // Run one frame
        loop {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                frames += 1;

                // Check for brown/yellow pixels in sky area (y=32..70, away from HUD and cloud)
                let mut sky_anomalies = 0;
                for y in 32..70 {
                    for x in 0..WIDTH {
                        let pixel = f[y * WIDTH + x];
                        let r = (pixel >> 16) & 0xFF;
                        let g = (pixel >> 8) & 0xFF;
                        let b = pixel & 0xFF;
                        // Detect non-blue, non-white, non-black pixels in sky
                        // Sky should be shades of blue, white (cloud), or black
                        let is_blue = b > g && b > r;
                        let is_white = r > 200 && g > 200 && b > 200;
                        let is_black = r < 20 && g < 20 && b < 20;
                        let is_light_blue = r > 100 && g > 180 && b > 220;
                        if !is_blue && !is_white && !is_black && !is_light_blue && pixel != 0 {
                            sky_anomalies += 1;
                        }
                    }
                }

                if sky_anomalies > 0 {
                    anomaly_frames.push((frames, sky_anomalies));
                    if anomaly_frames.len() <= 5 {
                        // Dump this frame
                        let path = format!("frame_anomaly_{}.ppm", frames);
                        write_ppm(&f, &path)?;
                        println!(
                            "Frame {} has {} sky anomaly pixels, dumped to {}",
                            frames, sky_anomalies, path
                        );

                        // Show specific anomaly pixels
                        for y in 32..70 {
                            for x in 0..WIDTH {
                                let pixel = f[y * WIDTH + x];
                                let r = (pixel >> 16) & 0xFF;
                                let g = (pixel >> 8) & 0xFF;
                                let b = pixel & 0xFF;
                                let is_blue = b > g && b > r;
                                let is_white = r > 200 && g > 200 && b > 200;
                                let is_black = r < 20 && g < 20 && b < 20;
                                let is_light_blue = r > 100 && g > 180 && b > 220;
                                if !is_blue
                                    && !is_white
                                    && !is_black
                                    && !is_light_blue
                                    && pixel != 0
                                {
                                    println!("  Anomaly at ({},{}) = RGB({},{},{})", x, y, r, g, b);
                                }
                            }
                        }
                    }
                }

                if phase % 60 == 0 {
                    println!("Phase {}, frame {}, scroll phase", phase, frames);
                }
                break;
            }
        }
    }

    println!("\n=== Summary ===");
    println!(
        "Total frames with sky anomalies: {}/{}",
        anomaly_frames.len(),
        600
    );
    if !anomaly_frames.is_empty() {
        println!("First anomaly at frame {}", anomaly_frames[0].0);
        println!(
            "Max anomaly pixels: {}",
            anomaly_frames.iter().map(|a| a.1).max().unwrap_or(0)
        );
    } else {
        println!("No sky anomalies detected during scroll test");
    }

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
