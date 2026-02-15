/// Analyze the vertical stripes in the lower-right area of gameplay screen.
use pce::emulator::Emulator;
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

    // Continue to frame 3000 (same as the frame the user pointed to)
    while frames < 3000 {
        emu.tick();
        if let Some(_f) = emu.take_frame() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    println!("Scroll: BXR={} BYR={} Map={}x{}", bxr, byr, map_w, map_h);

    let start_tile_x = (bxr as usize) / 8;
    let start_tile_y = (byr as usize) / 8;

    // Focus on the lower portion of the screen (tile rows 16-28 from scroll start)
    // which corresponds to the area with vertical stripes
    println!("\n=== BAT entries for lower screen area ===");
    for ty in 12..30 {
        let bat_row = (start_tile_y + ty) % map_h;
        print!("R{:02} (sty{:02}): ", bat_row, ty);
        for tx in 0..33 {
            let bat_col = (start_tile_x + tx) % map_w;
            let bat_addr = bat_row * map_w + bat_col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile_id != 0x200 {
                print!("{:03X}{:X} ", tile_id, pal);
            } else {
                print!(".... ");
            }
        }
        println!();
    }

    // Find and dump unique tiles in the stripe area (right side, lower portion)
    println!("\n=== Unique non-sky tiles in stripe area ===");
    let mut seen = std::collections::HashSet::new();
    for ty in 16..28 {
        let bat_row = (start_tile_y + ty) % map_h;
        for tx in 24..33 {
            // Right side of screen
            let bat_col = (start_tile_x + tx) % map_w;
            let bat_addr = bat_row * map_w + bat_col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = (entry & 0x07FF) as usize;
            let pal = ((entry >> 12) & 0x0F) as usize;
            if tile_id == 0x200 {
                continue;
            }
            let key = (tile_id, pal);
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);

            let tile_base = tile_id * 16;
            println!(
                "Tile {:03X} palette {} (at BAT col {}, row {}):",
                tile_id, pal, bat_col, bat_row
            );
            for row in 0..8 {
                let chr_a = emu.bus.vdc_vram_word((tile_base + row) as u16);
                let chr_b = emu.bus.vdc_vram_word((tile_base + 8 + row) as u16);
                print!("  R{}: ", row);
                for col in 0..8 {
                    let shift = 7 - col;
                    let p0 = ((chr_a >> shift) & 1) as u8;
                    let p1 = ((chr_a >> (shift + 8)) & 1) as u8;
                    let p2 = ((chr_b >> shift) & 1) as u8;
                    let p3 = ((chr_b >> (shift + 8)) & 1) as u8;
                    print!("{:X}", p0 | (p1 << 1) | (p2 << 2) | (p3 << 3));
                }
                // Also show raw words for debugging
                print!("  (chr_a={:04X} chr_b={:04X})", chr_a, chr_b);
                println!();
            }

            // Show what colors this maps to
            println!("  Palette {} colors:", pal);
            for i in 0..16 {
                let idx = pal * 16 + i;
                let rgb = emu.bus.vce_palette_rgb(idx);
                let r = (rgb >> 16) & 0xFF;
                let g = (rgb >> 8) & 0xFF;
                let b = rgb & 0xFF;
                if r != 0 || g != 0 || b != 0 {
                    println!("    [{:X}] = ({:3},{:3},{:3})", i, r, g, b);
                }
            }
        }
    }

    // Dump actual pixel colors in the stripe area
    println!("\n=== Pixel colors at vertical stripe area (x=200-240, y=130-160) ===");
    // Get the last frame
    let frame = loop {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            break f;
        }
    };

    for y in [130usize, 135, 140, 145, 150, 155, 160] {
        print!("Y{:3}: ", y);
        for x in 200..240 {
            let pixel = frame[y * WIDTH + x];
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;
            // Compact representation: just show if it's blue, dark, or other
            if r < 20 && g < 20 && b < 20 {
                print!("."); // black
            } else if b > r && b > g && b > 150 {
                print!("B"); // blue
            } else if b > r && b > g {
                print!("b"); // dark blue
            } else if r > 150 && g > 150 && b > 150 {
                print!("W"); // white
            } else if g > r && g > b {
                print!("G"); // green
            } else if r > g && r > b {
                print!("R"); // red
            } else {
                print!("?"); // other
            }
        }
        println!();
    }

    // More detailed dump for a few rows
    println!("\n=== Detailed pixels at y=150, x=200-256 ===");
    for x in 200..256 {
        let pixel = frame[150 * WIDTH + x];
        let r = (pixel >> 16) & 0xFF;
        let g = (pixel >> 8) & 0xFF;
        let b = pixel & 0xFF;
        println!("  x={:3}: RGB({:3},{:3},{:3}) #{:06X}", x, r, g, b, pixel);
    }

    Ok(())
}
