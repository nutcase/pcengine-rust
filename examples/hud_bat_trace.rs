/// Trace BAT content for HUD area vs gameplay area to diagnose layout
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
            let press_run = matches!(
                frames,
                100..=110
                    | 200..=210
                    | 300..=310
                    | 400..=410
                    | 500..=510
                    | 600..=610
                    | 700..=710
                    | 800..=810
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

    // Run to frame 3000
    while frames < 3000 {
        emu.tick();
        if let Some(_f) = emu.take_frame() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    println!("Map dimensions: {}x{} tiles", map_w, map_h);

    // HUD area: BXR=136 BYR=0
    println!("\n=== HUD area BAT (BXR=136 BYR=0, rows 0-35) ===");
    let hud_bxr = 136usize;
    let hud_byr = 0usize;
    let hud_tile_x = hud_bxr / 8;
    let hud_tile_y = hud_byr / 8;
    for ty in 0..5 {
        let bat_row = (hud_tile_y + ty) % map_h;
        print!("ty{:02} (batR{:02}): ", ty, bat_row);
        for tx in 0..33 {
            let bat_col = (hud_tile_x + tx) % map_w;
            let bat_addr = bat_row * map_w + bat_col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile_id != 0 {
                print!("{:03X}{:X} ", tile_id, pal);
            } else {
                print!(".... ");
            }
        }
        println!();
    }

    // Now show what BXR=0 BYR=0 would look like (full tilemap start)
    println!("\n=== Full tilemap start BAT (BXR=0 BYR=0, rows 0-5) ===");
    for ty in 0..5 {
        let bat_row = ty % map_h;
        print!("ty{:02} (batR{:02}): ", ty, bat_row);
        for tx in 0..64.min(map_w) {
            let bat_col = tx % map_w;
            let bat_addr = bat_row * map_w + bat_col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile_id != 0 {
                print!("{:03X}{:X} ", tile_id, pal);
            } else {
                print!(".... ");
            }
        }
        println!();
    }

    // Gameplay area: BXR=256 BYR=51
    println!("\n=== Gameplay area BAT (BXR=256 BYR=51) ===");
    let gp_bxr = 256usize;
    let gp_byr = 51usize;
    let gp_tile_x = gp_bxr / 8;
    let gp_tile_y = gp_byr / 8;
    for ty in 0..5 {
        let bat_row = (gp_tile_y + ty) % map_h;
        print!("ty{:02} (batR{:02}): ", ty, bat_row);
        for tx in 0..33 {
            let bat_col = (gp_tile_x + tx) % map_w;
            let bat_addr = bat_row * map_w + bat_col;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile_id != 0 {
                print!("{:03X}{:X} ", tile_id, pal);
            } else {
                print!(".... ");
            }
        }
        println!();
    }

    // Show what the reference image has: "FIELD 1-1 VITALITY"
    // Try BXR=0 BYR=0 for HUD area
    println!("\n=== HUD at BXR=0, BYR=0 ===");
    for ty in 0..5 {
        print!("ty{:02}: ", ty);
        for tx in 0..33 {
            let bat_addr = ty * map_w + tx;
            let entry = emu.bus.vdc_vram_word(bat_addr as u16);
            let tile_id = entry & 0x07FF;
            if tile_id != 0 {
                print!("{:03X} ", tile_id);
            } else {
                print!("... ");
            }
        }
        println!();
    }

    Ok(())
}
