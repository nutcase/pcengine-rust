/// Dump BAT and tile data for the visible area.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0u64;
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

    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    println!("Scroll: BXR={} BYR={} Map={}x{}", bxr, byr, map_w, map_h);

    let start_tile_x = (bxr as usize) / 8;
    let start_tile_y = (byr as usize) / 8;

    println!("\n=== BAT entries (visible) ===");
    for ty in 0..12 {
        let bat_row = (start_tile_y + ty) % map_h;
        print!("R{:02}: ", bat_row);
        for tx in 0..33 {
            let bat_col = (start_tile_x + tx) % map_w;
            let bat_addr = bat_row * map_w + bat_col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            print!("{:03X}{} ", tile_id, pal);
        }
        println!();
    }

    // Dump unique tile patterns in sky area
    println!("\n=== Unique tiles in sky ===");
    let mut seen = std::collections::HashSet::new();
    for ty in 0..6 {
        let bat_row = (start_tile_y + ty) % map_h;
        for tx in 0..33 {
            let bat_col = (start_tile_x + tx) % map_w;
            let bat_addr = bat_row * map_w + bat_col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = (entry & 0x07FF) as usize;
            if seen.contains(&tile_id) {
                continue;
            }
            seen.insert(tile_id);
            let tile_base = tile_id * 16;
            println!("Tile {:03X}:", tile_id);
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
                println!();
            }
        }
    }

    // Palette
    println!("\n=== BG palettes 0-5 ===");
    for bank in 0..6 {
        for i in 0..16 {
            let idx = bank * 16 + i;
            let rgb = emu.bus.vce_palette_rgb(idx);
            let r = (rgb >> 16) & 0xFF;
            let g = (rgb >> 8) & 0xFF;
            let b = rgb & 0xFF;
            if r != 0 || g != 0 || b != 0 {
                println!("  [{:03X}] = ({:3},{:3},{:3})", idx, r, g, b);
            }
        }
    }
    Ok(())
}
