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
use std::io::Write;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0;
    let save_frames = [5, 10, 50, 100, 120, 130];
    let mut sf_idx = 0;
    let mut last_frame: Option<Vec<u32>> = None;

    while sf_idx < save_frames.len() {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
            if frames == save_frames[sf_idx] {
                let pixels = last_frame.as_ref().unwrap();
                let width = 256;
                let height = pixels.len() / width;

                // Save PPM
                let filename = format!("frame_{:03}.ppm", frames);
                let mut data = Vec::with_capacity(width * height * 3);
                for &p in pixels {
                    data.push(((p >> 16) & 0xFF) as u8);
                    data.push(((p >> 8) & 0xFF) as u8);
                    data.push((p & 0xFF) as u8);
                }
                let header = format!("P6\n{} {}\n255\n", width, height);
                let mut file = std::fs::File::create(&filename)?;
                file.write_all(header.as_bytes())?;
                file.write_all(&data)?;

                // Check text row
                let mut has_nonblack = false;
                for x in 48..210 {
                    let idx = 172.min(height - 1) * width + x;
                    let p = pixels[idx];
                    if p != 0 {
                        has_nonblack = true;
                        break;
                    }
                }

                // Check font tile H (0x148)
                let base = 0x148 * 16;
                let w0 = emu.bus.vdc_vram_word(base as u16);
                let font_ok = (w0 & 0xFF) == 0x66; // 'H' first row = 0x66

                // Check palette
                let bg_color = pixels[100 * width + 128]; // center of screen

                println!(
                    "Frame {:3}: saved {}, text_row_nonblack={}, font_H_ok={}, bg=#{:06X}",
                    frames,
                    filename,
                    has_nonblack,
                    font_ok,
                    bg_color & 0xFFFFFF
                );

                sf_idx += 1;
            }
        }
    }

    // Also render frame 130 with text annotation
    let pixels = last_frame.as_ref().unwrap();
    let width = 256;
    let height = pixels.len() / width;
    println!("\n=== Pixel analysis at frame 130 ===");
    println!("Frame dimensions: {}x{}", width, height);

    // Check if display is active
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    println!(
        "CR (control): {:04X} (BG enable: {}, sprite enable: {})",
        cr,
        (cr >> 7) & 1,
        (cr >> 6) & 1
    );

    // Check palette entry for font (palette bank 5, color 1)
    let pal_idx = 5 * 16 + 1; // = 81
    println!("Palette index 81 (bank 5, color 1): TODO - need accessor");

    // Check what color the text would be
    // With palette 5 and font color 1, the VCE color lookup gives us the final RGB
    // Let me check a broader Y range for any visible content
    for y_check in [160, 170, 172, 175, 180, 200, 204, 210, 220] {
        if y_check >= height {
            continue;
        }
        let mut nonblack_count = 0;
        let mut sample_color = 0u32;
        for x in 0..width {
            let p = pixels[y_check * width + x];
            if p != 0 {
                nonblack_count += 1;
                if sample_color == 0 {
                    sample_color = p;
                }
            }
        }
        if nonblack_count > 0 {
            println!(
                "  Y={}: {} non-black pixels (sample: #{:06X})",
                y_check,
                nonblack_count,
                sample_color & 0xFFFFFF
            );
        } else {
            println!("  Y={}: all black", y_check);
        }
    }

    // Check entire frame for any non-black pixels
    let mut total_nonblack = 0;
    for &p in pixels {
        if p != 0 {
            total_nonblack += 1;
        }
    }
    println!(
        "\nTotal non-black pixels: {} / {}",
        total_nonblack,
        pixels.len()
    );

    Ok(())
}
