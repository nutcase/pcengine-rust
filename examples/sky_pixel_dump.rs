/// Analyze pixel data in the sky area to detect artifacts.
use pce::emulator::Emulator;
use std::collections::HashMap;
use std::error::Error;

const WIDTH: usize = 256;

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

    // Capture a gameplay frame
    let frame = loop {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            frames += 1;
            break f;
        }
    };

    println!(
        "=== Frame {} pixel analysis (sky area rows 16-80) ===",
        frames
    );

    // Find the background color (most common in sky area)
    let mut color_freq: HashMap<u32, usize> = HashMap::new();
    for y in 16..80 {
        for x in 0..WIDTH {
            let pixel = frame[y * WIDTH + x];
            *color_freq.entry(pixel).or_insert(0) += 1;
        }
    }
    let mut freq_list: Vec<_> = color_freq.iter().collect();
    freq_list.sort_by(|a, b| b.1.cmp(a.1));

    println!("Color frequency in sky area (rows 16-80):");
    for (color, count) in freq_list.iter().take(10) {
        let r = (*color >> 16) & 0xFF;
        let g = (*color >> 8) & 0xFF;
        let b = *color & 0xFF;
        println!(
            "  RGB({:3},{:3},{:3}) = #{:06X}: {} pixels",
            r, g, b, color, count
        );
    }

    let bg_color = *freq_list[0].0;
    println!("\nBackground color: #{:06X}", bg_color);

    // Find non-background pixels in the sky area
    println!("\nNon-background pixels in sky area:");
    for y in 16..80 {
        let mut has_anomaly = false;
        let mut anomalies = Vec::new();
        for x in 0..WIDTH {
            let pixel = frame[y * WIDTH + x];
            if pixel != bg_color {
                anomalies.push((x, pixel));
                has_anomaly = true;
            }
        }
        if has_anomaly {
            print!("  Row {:3}: ", y);
            for (x, pixel) in &anomalies {
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                print!("x={} ({},{},{}) ", x, r, g, b);
            }
            println!();
        }
    }

    // Also check bg_opaque for the sky area by running with debug
    // Use the emulator's internal state to check BG opacity
    println!("\n=== Checking BG opacity in sky area ===");
    let bg_opaques: Vec<bool> = (0..WIDTH * 240)
        .map(|i| {
            // We can't directly access bg_opaque, but we can infer it
            // A pixel matching bg_color with no sprite contribution = transparent BG
            false
        })
        .collect();

    // Check if there are any sprites that would appear in the sky
    println!("\n=== Sprites potentially in sky (y < 80 after offset) ===");
    for sprite in 0..64usize {
        let base = sprite * 4;
        let y_w = emu.bus.vdc_satb_word(base);
        let x_w = emu.bus.vdc_satb_word(base + 1);
        let pat_w = emu.bus.vdc_satb_word(base + 2);
        let attr_w = emu.bus.vdc_satb_word(base + 3);

        let y = (y_w & 0x03FF) as i32 - 64;
        let x = (x_w & 0x03FF) as i32 - 32;
        let h_code = ((attr_w >> 12) & 0x03) as usize;
        let h = match h_code {
            0 => 16,
            1 => 32,
            _ => 64,
        } as i32;

        if y < 80 && y + h > 16 && x > -32 && x < 256 {
            let pat = (pat_w >> 1) & 0x03FF;
            let pal = attr_w & 0x000F;
            let pri = if (attr_w & 0x0080) != 0 { "HI" } else { "LO" };
            println!(
                "  SPR#{:02} y={:4} x={:4} pat={:03X} pal={:X} {} attr={:04X}",
                sprite, y, x, pat, pal, pri, attr_w
            );
        }
    }

    Ok(())
}
