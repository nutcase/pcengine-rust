#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Deep test: load save state, run many frames with varied input, check every frame.
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

    let mut total_frames = 0u64;
    let mut anomaly_frames = 0u64;
    let mut dumped = 0;

    // Run 2000 frames with varied input
    for phase in 0..2000 {
        // Vary input: walk right, stop, walk left, jump, etc.
        let pad = match phase % 200 {
            0..=49 => 0xFF & !(1u8 << 1),    // Right
            50..=59 => 0xFF,                 // Stop
            60..=109 => 0xFF & !(1u8 << 3),  // Left
            110..=119 => 0xFF,               // Stop
            120..=139 => 0xFF & !(1u8 << 4), // Button I (action)
            140..=149 => 0xFF & !(1u8 << 0), // Up
            150..=169 => 0xFF & !(1u8 << 1), // Right
            170..=179 => 0xFF & !(1u8 << 5), // Button II
            _ => 0xFF,                       // Stop
        };
        emu.bus.set_joypad_input(pad);

        let frame = loop {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                break f;
            }
        };
        total_frames += 1;

        // Check per-line scroll consistency
        let mut scroll_changes = Vec::new();
        let mut prev_bxr = 0xFFFFu16;
        let mut prev_byr = 0xFFFFu16;
        for row in 0..OUT_HEIGHT {
            let line = emu.bus.vdc_line_state_index_for_row(row);
            let (lx, ly) = emu.bus.vdc_scroll_line(line);
            if lx != prev_bxr || ly != prev_byr {
                scroll_changes.push((row, lx, ly));
                prev_bxr = lx;
                prev_byr = ly;
            }
        }

        // Check for anomalies:
        // 1. No scroll split (HUD and gameplay should have different BYR)
        let has_split = scroll_changes.len() >= 2;

        // 2. HUD area showing gameplay content (check for non-HUD colors in rows 0-8)
        let mut hud_anomaly = false;
        for y in 0..8 {
            for x in 0..WIDTH {
                let pixel = frame[y * WIDTH + x];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                // HUD background should be blue (like the HUD bar) or black/white text
                // If we see green/brown (scenery colors) in the very top rows, that's a glitch
                if g > 150 && b < 100 && r < 100 {
                    // Green in top of HUD - likely scenery leaking
                    hud_anomaly = true;
                }
            }
        }

        // 3. Gameplay area (rows 36+) - check for HUD text artifacts
        // HUD text is white on blue. If we see "text-like" white pixels in the
        // sky area that shouldn't have them, that's a glitch.
        let mut gameplay_text_leak = 0;
        for y in 38..60 {
            let mut consecutive_white = 0;
            for x in 0..WIDTH {
                let pixel = frame[y * WIDTH + x];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                if r > 240 && g > 240 && b > 240 {
                    consecutive_white += 1;
                    if consecutive_white >= 3 {
                        gameplay_text_leak += 1;
                    }
                } else {
                    consecutive_white = 0;
                }
            }
        }

        // 4. Wrong BG Y position - gameplay rows showing wrong part of tilemap
        // If BYR for gameplay is not around 51 (expected value), something is wrong
        let gameplay_byr_wrong = if scroll_changes.len() >= 2 {
            let (_, _, byr) = scroll_changes[1];
            // BYR should be around 51 for this game (may vary slightly)
            byr > 200 || (byr > 0 && byr < 20)
        } else {
            false
        };

        let is_anomaly = !has_split || hud_anomaly || gameplay_text_leak > 10 || gameplay_byr_wrong;

        if is_anomaly {
            anomaly_frames += 1;
            println!(
                "Frame {:4} (phase {:4}): split={} hud_anom={} text_leak={} byr_wrong={} scroll_changes={:?}",
                total_frames,
                phase,
                has_split,
                hud_anomaly,
                gameplay_text_leak,
                gameplay_byr_wrong,
                scroll_changes
            );
            if dumped < 10 {
                let path = format!("deep_anomaly_{}.ppm", phase);
                write_ppm(&frame, &path)?;
                dumped += 1;
            }
        }
    }

    println!(
        "\nSummary: {} anomaly frames out of {} total",
        anomaly_frames, total_frames
    );

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
