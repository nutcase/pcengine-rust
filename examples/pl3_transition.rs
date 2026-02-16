use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = "roms/Power League III (Japan).pce";
    let rom = std::fs::read(rom_path)?;
    let mut emulator = Emulator::new();
    emulator.load_hucard(&rom)?;
    emulator.reset();

    // Capture ALL frames from boot, log VCE[0] changes and display state
    let mut prev_vce0: u32 = 0xFFFFFFFF;
    let mut prev_cr: u16 = 0xFFFF;
    let mut frame_count = 0usize;
    let mut budget: u64 = 1500 * 250_000;
    let run_pressed: u8 = 0xFF & !(1 << 7);

    // Press RUN at certain frames to advance menus
    let press_at: &[usize] = &[65, 130, 200, 270, 340, 410, 480, 550];
    let press_dur = 5;

    while frame_count < 1500 && budget > 0 {
        // Set joypad based on frame
        let pad = if press_at.iter().any(|&f| frame_count >= f && frame_count < f + press_dur) {
            run_pressed
        } else {
            0xFF
        };
        emulator.bus.set_joypad_input(pad);

        let c = emulator.tick() as u64;
        budget = budget.saturating_sub(c.max(1));
        if let Some(frame) = emulator.take_frame() {
            frame_count += 1;
            let vce0 = emulator.bus.vce_palette_rgb(0);
            let vce0_raw = emulator.bus.vce_palette_word(0);
            let cr = emulator.bus.vdc_register(0x05).unwrap_or(0);
            let bg_en = (cr & 0x80) != 0;
            let spr_en = (cr & 0x40) != 0;

            if vce0 != prev_vce0 || cr != prev_cr || frame_count <= 5
                || frame_count % 50 == 0
            {
                eprintln!(
                    "frame {:4}: VCE[0]={:06X} (raw={:03X}) CR={:04X} BG={} SPR={}",
                    frame_count, vce0, vce0_raw, cr, bg_en, spr_en
                );

                // If display is disabled or VCE[0] changed, dump frame
                if !bg_en && !spr_en || vce0 != prev_vce0 {
                    let path = format!("pl3_trans_{:04}.ppm", frame_count);
                    write_ppm(&frame, &path)?;
                    eprintln!("  -> wrote {}", path);

                    // Check what colors dominate the frame
                    let mut black = 0u32;
                    let mut green_ish = 0u32;
                    let mut other = 0u32;
                    for &px in frame.iter() {
                        if px == 0x000000 {
                            black += 1;
                        } else if (px >> 8) & 0xFF > ((px >> 16) & 0xFF) + 20
                            && (px >> 8) & 0xFF > (px & 0xFF) + 20
                        {
                            green_ish += 1;
                        } else {
                            other += 1;
                        }
                    }
                    eprintln!(
                        "  pixels: black={} green={} other={}",
                        black, green_ish, other
                    );

                    // Sample some pixels
                    eprintln!(
                        "  pixel[0]={:06X} pixel[128,112]={:06X}",
                        frame[0],
                        frame.get(112 * 256 + 128).copied().unwrap_or(0)
                    );
                }
            }
            prev_vce0 = vce0;
            prev_cr = cr;
        }
    }

    Ok(())
}

fn run_frames(emulator: &mut Emulator, count: usize, pad: u8) {
    let mut collected = 0;
    let mut budget = count as u64 * 250_000;
    while collected < count && budget > 0 {
        emulator.bus.set_joypad_input(pad);
        let c = emulator.tick() as u64;
        budget = budget.saturating_sub(c.max(1));
        if emulator.take_frame().is_some() {
            collected += 1;
        }
    }
}

fn write_ppm(frame: &[u32], path: &str) -> Result<(), Box<dyn Error>> {
    const WIDTH: usize = 256;
    const HEIGHT: usize = 240;
    let out_h = HEIGHT.min(frame.len() / WIDTH);
    let mut file = File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", WIDTH, out_h)?;
    for y in 0..out_h {
        for x in 0..WIDTH {
            let pixel = frame.get(y * WIDTH + x).copied().unwrap_or(0);
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            file.write_all(&[r, g, b])?;
        }
    }
    Ok(())
}
