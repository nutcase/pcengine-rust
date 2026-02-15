/// Dump a gameplay frame by simulating button presses to get past title screen.
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
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0u64;
    let mut last_frame_data: Option<Vec<u32>> = None;

    // Run for 2000 frames, pressing Run periodically to advance
    while frames < 2000 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            frames += 1;
            last_frame_data = Some(f);

            // Press Run for 10-frame bursts at various intervals
            let press_run = matches!(frames,
                100..=110 | 200..=210 | 300..=310 | 400..=410 |
                500..=510 | 600..=610 | 700..=710 | 800..=810
            );
            if press_run {
                // Run button pressed (bit 7 clear, active-low)
                emu.bus.set_joypad_input(0x7F);
            } else {
                emu.bus.set_joypad_input(0xFF);
            }
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Dump frame every 100 frames for debugging
    println!("Reached frame {}", frames);

    // Now dump multiple frames at different points
    for target in [2000u64, 2100, 2200, 2400, 2600, 3000] {
        while frames < target {
            emu.tick();
            if let Some(f) = emu.take_frame() {
                frames += 1;
                last_frame_data = Some(f);
            }
            if emu.cpu.halted {
                break;
            }
        }
        if let Some(ref frame) = last_frame_data {
            let path = format!("frame_at_{}.ppm", frames);
            write_ppm(frame, &path)?;
            println!("Dumped frame at {}", frames);
        }
    }

    // Also dump sprite pattern data
    println!("\n=== Sprite pattern pixel dump (pat 0xB9) ===");
    dump_sprite_pattern(&emu, 0xB9);
    println!("\n=== Sprite pattern pixel dump (pat 0xBB) ===");
    dump_sprite_pattern(&emu, 0xBB);

    // Dump sprite palette 0
    println!("\n=== Sprite palette 0 (indices 0x100-0x10F) ===");
    for i in 0x100..0x110 {
        let rgb = emu.bus.vce_palette_rgb(i);
        let r = (rgb >> 16) & 0xFF;
        let g = (rgb >> 8) & 0xFF;
        let b = rgb & 0xFF;
        println!("  [{:03X}] RGB = ({:3}, {:3}, {:3})", i, r, g, b);
    }

    // Dump SATB at this point
    println!("\n=== SATB at frame {} ===", frames);
    for sprite in 0..64usize {
        let base = sprite * 4;
        let y_w = emu.bus.vdc_satb_word(base);
        let x_w = emu.bus.vdc_satb_word(base + 1);
        let pat_w = emu.bus.vdc_satb_word(base + 2);
        let attr_w = emu.bus.vdc_satb_word(base + 3);
        if y_w == 0 && x_w == 0 && pat_w == 0 && attr_w == 0 {
            continue;
        }
        let y = (y_w & 0x03FF) as i32 - 64;
        let x = (x_w & 0x03FF) as i32 - 32;
        let pat = (pat_w >> 1) & 0x03FF;
        let pal = attr_w & 0x000F;
        let pri = if (attr_w & 0x0080) != 0 { "HI" } else { "LO" };
        let w = if (attr_w & 0x0100) != 0 { 32 } else { 16 };
        let h_code = ((attr_w >> 12) & 0x03) as usize;
        let h = match h_code {
            0 => 16,
            1 => 32,
            _ => 64,
        };
        println!(
            "  SPR#{:02} x={:4} y={:4} pat={:03X} pal={:X} {} {}x{} attr={:04X}",
            sprite, x, y, pat, pal, pri, w, h, attr_w
        );
    }

    Ok(())
}

fn dump_sprite_pattern(emu: &Emulator, pat_index: usize) {
    let pat_base = pat_index * 64;
    for row in 0..16 {
        let p0 = emu.bus.vdc_vram_word((pat_base + row) as u16);
        let p1 = emu.bus.vdc_vram_word((pat_base + 16 + row) as u16);
        let p2 = emu.bus.vdc_vram_word((pat_base + 32 + row) as u16);
        let p3 = emu.bus.vdc_vram_word((pat_base + 48 + row) as u16);
        print!("Row {:2}: ", row);
        for col in 0..16 {
            let shift = 15 - col;
            let pixel = ((p0 >> shift) & 1)
                | (((p1 >> shift) & 1) << 1)
                | (((p2 >> shift) & 1) << 2)
                | (((p3 >> shift) & 1) << 3);
            print!("{:X}", pixel);
        }
        println!();
    }
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
