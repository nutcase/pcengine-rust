/// Load a save state and walk right to find rendering corruption.
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
    println!("State loaded successfully");

    // Dump the initial frame after loading
    let frame = loop {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            break f;
        }
    };
    write_ppm(&frame, "state_frame_0.ppm")?;
    println!("Dumped initial frame");

    // Show VDC state
    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let rcr = emu.bus.vdc_register(0x06).unwrap_or(0);
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    println!("VDC: BXR={} BYR={} RCR=0x{:04X} CR=0x{:04X}", bxr, byr, rcr, cr);

    // Walk right for 600 frames, checking per-line scroll and dumping periodically
    let mut anomaly_count = 0;
    for phase in 0..600 {
        // Hold right
        let pad = 0xFF & !(1u8 << 1); // Right pressed
        emu.bus.set_joypad_input(pad);

        let frame = loop {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                break f;
            }
        };

        // Check for per-line scroll anomalies
        // The HUD area (rows 0-35) should have different scroll from gameplay (rows 36+)
        let hud_line = emu.bus.vdc_line_state_index_for_row(0);
        let game_line = emu.bus.vdc_line_state_index_for_row(36);
        let (hud_bxr, hud_byr) = emu.bus.vdc_scroll_line(hud_line);
        let (game_bxr, game_byr) = emu.bus.vdc_scroll_line(game_line);

        // Check for "glitch" patterns in the sky/background area
        // Look for unexpected pixel patterns in the gameplay area
        let mut sky_anomalies = 0;
        for y in 40..80 {
            for x in 0..WIDTH {
                let pixel = frame[y * WIDTH + x];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                // Sky should be blue-ish or have normal scenery colors
                // Detect obviously wrong colors (like HUD colors in gameplay area)
                // Pure white text in sky area would be a glitch
                if r > 240 && g > 240 && b > 240 && pixel != 0 {
                    sky_anomalies += 1;
                }
            }
        }

        // Check if scroll split is wrong (HUD and game should differ)
        let split_ok = hud_bxr != game_bxr || hud_byr != game_byr;

        if phase % 30 == 0 || !split_ok || sky_anomalies > 50 {
            println!(
                "Phase {:3}: HUD scroll=({},{}) Game scroll=({},{}) split_ok={} sky_anom={}",
                phase, hud_bxr, hud_byr, game_bxr, game_byr, split_ok, sky_anomalies
            );
        }

        if !split_ok {
            anomaly_count += 1;
            if anomaly_count <= 5 {
                let path = format!("state_anomaly_{}.ppm", phase);
                write_ppm(&frame, &path)?;
                println!("  ** SCROLL SPLIT ANOMALY - dumped to {}", path);
            }
        }

        if sky_anomalies > 50 {
            anomaly_count += 1;
            if anomaly_count <= 5 {
                let path = format!("state_sky_anomaly_{}.ppm", phase);
                write_ppm(&frame, &path)?;
                println!("  ** SKY ANOMALY ({} white pixels) - dumped to {}", sky_anomalies, path);
            }
        }

        // Dump every 100 frames for comparison
        if phase % 100 == 0 && phase > 0 {
            let path = format!("state_frame_{}.ppm", phase);
            write_ppm(&frame, &path)?;
        }
    }

    println!("\nTotal anomalies: {}", anomaly_count);

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
