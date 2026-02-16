use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = "roms/Power League III (Japan).pce";
    let state_path = "states/Power League III (Japan).slot1.state";

    let rom = std::fs::read(rom_path)?;
    let mut emulator = Emulator::new();
    emulator.load_hucard(&rom)?;
    emulator.reset();
    emulator.load_state_from_file(state_path)?;
    eprintln!("loaded state");

    // Run 5 frames with no input to stabilize
    run_frames(&mut emulator, 5, 0xFF);

    // Key VRAM addresses to monitor
    let watch_addrs: &[(u16, &str)] = &[
        (0x3780, "pat 0xDE"),
        (0x3740, "pat 0xDD"),
    ];

    // Press LEFT for 4 frames, check VRAM state at frame production time
    let left_pressed: u8 = 0xFF & !(1 << 3); // 0xF7
    for frame_num in 0..4 {
        emulator.bus.set_joypad_input(left_pressed);
        let frame = run_one_frame(&mut emulator);
        if let Some(ref frame_data) = frame {
            let path = format!("pl3_left_{:03}.ppm", frame_num);
            write_ppm(frame_data, &path)?;

            println!("=== frame {} ===", frame_num);

            // VRAM state at render time (end of active display)
            print!("  VRAM@render:");
            for &(addr, label) in watch_addrs {
                let w = emulator.bus.vdc_vram_word(addr);
                print!("  {}={:04X}", label, w);
            }
            println!();

            // SAT for batter sprites
            for sprite in [6usize, 7, 8] {
                let base = sprite * 4;
                let y_w = emulator.bus.vdc_satb_word(base);
                let x_w = emulator.bus.vdc_satb_word(base + 1);
                let pat_w = emulator.bus.vdc_satb_word(base + 2);
                let attr_w = emulator.bus.vdc_satb_word(base + 3);
                let y = (y_w & 0x03FF) as i32 - 64;
                let x = (x_w & 0x03FF) as i32 - 32;
                let pat = (pat_w >> 1) & 0x03FF;
                let pal = attr_w & 0x000F;
                let cgx = (attr_w >> 8) & 1;
                let cgy = (attr_w >> 12) & 3;
                let width = if cgx == 0 { 16 } else { 32 };
                let height: i32 = match cgy { 0 => 16, 1 => 32, 3 => 64, _ => 16 };
                println!("  SPR#{:02} x={:4} y={:4} pat={:03X} pal={:X} {}x{} attr={:04X}",
                    sprite, x, y, pat, pal, width, height, attr_w);
            }

            // Count pixel diffs in batter area between consecutive frames
            if frame_num > 0 {
                // Compare with previous PPM (just check pixel data hash for simplicity)
                let prev_path = format!("pl3_left_{:03}.ppm", frame_num - 1);
                if let (Ok(prev_data), Ok(cur_data)) = (std::fs::read(&prev_path), std::fs::read(&path)) {
                    let diff_count = prev_data.iter().zip(cur_data.iter())
                        .filter(|(a, b)| a != b).count();
                    println!("  Byte diffs from frame {}: {}", frame_num - 1, diff_count);
                }
            }
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

fn run_one_frame(emulator: &mut Emulator) -> Option<Vec<u32>> {
    let mut budget: u64 = 500_000;
    while budget > 0 {
        let c = emulator.tick() as u64;
        budget = budget.saturating_sub(c.max(1));
        if let Some(frame) = emulator.take_frame() {
            return Some(frame);
        }
    }
    None
}

fn write_ppm(frame: &[u32], path: &str) -> Result<(), Box<dyn Error>> {
    const WIDTH: usize = 256;
    const HEIGHT: usize = 224;
    let mut file = File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", WIDTH, HEIGHT)?;
    for y in 0..HEIGHT {
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
