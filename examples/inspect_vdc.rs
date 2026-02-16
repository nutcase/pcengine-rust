/// Inspect VDC registers and VRAM tile data after loading a Power League III save state.
/// Designed to help diagnose visual noise on the baseball field.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Power League III (Japan).pce".to_string());
    let state_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "states/Power League III (Japan).slot1.state".to_string());

    // --- Load ROM and save state ---
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.load_state_from_file(&state_path)?;

    // --- Run 1 frame ---
    emu.bus.set_joypad_input(0xFF);
    loop {
        emu.tick();
        if emu.take_frame().is_some() {
            break;
        }
    }

    // =========================================================
    // 1) Print ALL VDC registers $00-$13
    // =========================================================
    println!("=== VDC Registers $00-$13 ===");
    let reg_names = [
        "MAWR", "MARR", "VRR/VWR", "???", "???", "CR", "RCR", "BXR",
        "BYR", "MWR", "HSR", "HDR", "VPR", "VDW", "VCR", "DCR",
        "SOUR", "DESR", "LENR", "DVSSR",
    ];
    for i in 0..0x14usize {
        if let Some(val) = emu.bus.vdc_register(i) {
            let name = if i < reg_names.len() {
                reg_names[i as usize]
            } else {
                "???"
            };
            println!("  R${:02X} ({:6}): ${:04X}  ({})", i, name, val, val);
        }
    }

    // =========================================================
    // 2) Decode MWR ($09) in detail
    // =========================================================
    let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
    println!("\n=== MWR Register ($09) = ${:04X} Detailed Decode ===", mwr);

    let vram_access = mwr & 0x03;
    let sprite_dot = (mwr >> 2) & 0x03;
    let bat_width_code = (mwr >> 4) & 0x03;
    let bat_height_code = (mwr >> 6) & 0x01;
    let cg_mode_bit = (mwr >> 7) & 0x01;

    let bat_width = match bat_width_code {
        0 => 32,
        1 => 64,
        2 => 128,
        3 => 128,
        _ => unreachable!(),
    };
    let bat_height = if bat_height_code == 0 { 32 } else { 64 };

    println!("  Bits 1-0 (VRAM access width): {} -> {}", vram_access, match vram_access {
        0 => "2 words",
        1 => "4 words",
        2 => "8 words",
        3 => "8 words",
        _ => "?",
    });
    println!("  Bits 3-2 (Sprite dot period):  {} -> {}", sprite_dot, match sprite_dot {
        0 => "2-cycle / normal",
        1 => "2-cycle / CG1",
        2 => "undocumented",
        3 => "undocumented",
        _ => "?",
    });
    println!(
        "  Bits 5-4 (BAT width code):    {} -> {} tiles",
        bat_width_code, bat_width
    );
    println!(
        "  Bit    6 (BAT height code):   {} -> {} tiles",
        bat_height_code, bat_height
    );
    println!(
        "  Bit    7 (CG mode):           {} -> {}",
        cg_mode_bit,
        if cg_mode_bit == 0 { "CG0 (normal)" } else { "CG1" }
    );
    println!(
        "  => BAT dimensions: {}x{} tiles ({}x{} pixels)",
        bat_width,
        bat_height,
        bat_width * 8,
        bat_height * 8
    );

    let (api_w, api_h) = emu.bus.vdc_map_dimensions();
    println!("  => vdc_map_dimensions() reports: {}x{}", api_w, api_h);

    // =========================================================
    // 3) Decode CR ($05) for context
    // =========================================================
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    println!("\n=== CR Register ($05) = ${:04X} ===", cr);
    println!("  Collision IRQ enable: {}", (cr & 0x01) != 0);
    println!("  Overflow IRQ enable:  {}", (cr & 0x02) != 0);
    println!("  RCR IRQ enable:       {}", (cr & 0x04) != 0);
    println!("  VBlank IRQ enable:    {}", (cr & 0x08) != 0);
    println!("  EX bits (4-5):        {}", (cr >> 4) & 0x03);
    println!("  Sprite enable:        {}", (cr & 0x40) != 0);
    println!("  BG enable:            {}", (cr & 0x80) != 0);
    let inc_code = (cr >> 11) & 0x03;
    let inc_step = match inc_code { 0 => 1, 1 => 32, 2 => 64, _ => 128 };
    println!("  VRAM increment (11-12): {} -> +{}", inc_code, inc_step);

    // =========================================================
    // 4) Sample BAT entries to check tile data encoding
    // =========================================================
    println!("\n=== Sample BAT Entries ===");
    println!("  BAT layout: each entry is a 16-bit word at VRAM[row * bat_width + col]");
    println!("  Encoding: bits 11-0 = tile index, bits 15-12 = palette");

    // Sample a grid of BAT entries across the visible area
    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    println!("  Scroll: BXR=${:04X} ({}), BYR=${:04X} ({})", bxr, bxr, byr, byr);

    let scroll_tile_x = (bxr / 8) as usize;
    let scroll_tile_y = (byr / 8) as usize;
    println!(
        "  Scroll tile offset: col={}, row={}",
        scroll_tile_x, scroll_tile_y
    );

    println!("\n  BAT entries around the visible area (row, col -> VRAM addr: raw entry -> tile/pal):");
    for row_off in 0..30usize {
        let bat_row = (scroll_tile_y + row_off) % (bat_height as usize);
        // Print a few columns across the width
        for col_off in [0, 4, 8, 12, 16, 20, 24, 28, 31].iter() {
            let bat_col = (scroll_tile_x + col_off) % (bat_width as usize);
            let addr = (bat_row * bat_width as usize + bat_col) as u16;
            let entry = emu.bus.vdc_vram_word(addr);
            let tile = entry & 0x0FFF;
            let pal = (entry >> 12) & 0x0F;
            if *col_off == 0 {
                print!("  row {:2}: ", bat_row);
            }
            print!("[c{:2}]={:04X}(t{:03X}/p{:X}) ", bat_col, entry, tile, pal);
        }
        println!();
    }

    // =========================================================
    // 5) Identify commonly used tiles in the field area
    // =========================================================
    println!("\n=== Tile Frequency in Visible BAT ===");
    let mut tile_counts: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
    for row_off in 0..30usize {
        let bat_row = (scroll_tile_y + row_off) % (bat_height as usize);
        for col_off in 0..32usize {
            let bat_col = (scroll_tile_x + col_off) % (bat_width as usize);
            let addr = (bat_row * bat_width as usize + bat_col) as u16;
            let entry = emu.bus.vdc_vram_word(addr);
            *tile_counts.entry(entry).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<_> = tile_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    println!("  Top 20 most frequent BAT entries:");
    for (i, (entry, count)) in sorted.iter().take(20).enumerate() {
        let tile = entry & 0x0FFF;
        let pal = (entry >> 12) & 0x0F;
        println!(
            "    #{:2}: entry=${:04X} (tile ${:03X}, pal {}) - {} occurrences",
            i + 1,
            entry,
            tile,
            pal,
            count
        );
    }

    // =========================================================
    // 6) Dump tile bitplanes for tile $200 (palette 1, suspected green field tile)
    //    and also the top 3 most common tiles
    // =========================================================
    let tiles_to_dump: Vec<u16> = {
        let mut v = vec![0x200u16];
        for (entry, _) in sorted.iter().take(5) {
            let tile = entry & 0x0FFF;
            if !v.contains(&tile) {
                v.push(tile);
            }
            if v.len() >= 4 {
                break;
            }
        }
        v
    };

    for &tile_id in &tiles_to_dump {
        println!("\n=== Tile ${:03X} VRAM Data (4 bitplanes) ===", tile_id);
        let base = tile_id as usize * 16;
        println!("  VRAM base address: ${:04X}", base);
        println!("  Raw 16 words:");
        for w in 0..16usize {
            let word = emu.bus.vdc_vram_word((base + w) as u16);
            println!("    VRAM[${:04X}] = ${:04X}", base + w, word);
        }

        // Decode the 4 bitplanes
        // PCE tile format: 16 words per tile
        //   Words 0-7: plane 0 (low byte), plane 1 (high byte)
        //   Words 8-15: plane 2 (low byte), plane 3 (high byte)
        println!("\n  Bitplane decode (8 rows, 8 pixels each):");
        println!("  Row  Plane0   Plane1   Plane2   Plane3   Combined");
        for row in 0..8usize {
            let w01 = emu.bus.vdc_vram_word((base + row) as u16);
            let w23 = emu.bus.vdc_vram_word((base + row + 8) as u16);
            let p0 = (w01 & 0xFF) as u8;
            let p1 = ((w01 >> 8) & 0xFF) as u8;
            let p2 = (w23 & 0xFF) as u8;
            let p3 = ((w23 >> 8) & 0xFF) as u8;

            let plane_str = |byte: u8| -> String {
                (0..8)
                    .rev()
                    .map(|bit| if (byte >> bit) & 1 != 0 { '#' } else { '.' })
                    .collect()
            };

            // Combined 4bpp pixel values
            let combined: String = (0..8)
                .rev()
                .map(|bit| {
                    let val = ((p0 >> bit) & 1)
                        | (((p1 >> bit) & 1) << 1)
                        | (((p2 >> bit) & 1) << 2)
                        | (((p3 >> bit) & 1) << 3);
                    format!("{:X}", val)
                })
                .collect();

            println!(
                "   {:1}:  {}  {}  {}  {}  {}",
                row,
                plane_str(p0),
                plane_str(p1),
                plane_str(p2),
                plane_str(p3),
                combined
            );
        }
    }

    // =========================================================
    // 7) Quick palette dump for the field-relevant palettes
    // =========================================================
    println!("\n=== BG Palette Colors (palettes 0-3) ===");
    println!("  (VCE color format: GRB 9-bit, $000-$1FF)");
    // Note: We don't have a direct palette read API in all builds,
    // but we can check if the bus exposes it.
    // For now, just report what we know from registers.

    println!("\n=== Scroll Registers ===");
    println!("  BXR (R$07): ${:04X}", emu.bus.vdc_register(0x07).unwrap_or(0));
    println!("  BYR (R$08): ${:04X}", emu.bus.vdc_register(0x08).unwrap_or(0));
    println!("  RCR (R$06): ${:04X}", emu.bus.vdc_register(0x06).unwrap_or(0));

    println!("\n=== VDC Timing Registers ===");
    let hsr = emu.bus.vdc_register(0x0A).unwrap_or(0);
    let hdr = emu.bus.vdc_register(0x0B).unwrap_or(0);
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);

    let hsw = hsr & 0x1F;
    let hds = (hsr >> 8) & 0x7F;
    let hdw = hdr & 0x7F;
    let hde = (hdr >> 8) & 0x7F;
    println!("  HSR=${:04X}: HSW={}, HDS={}", hsr, hsw, hds);
    println!("  HDR=${:04X}: HDW={} ({}px), HDE={}", hdr, hdw, (hdw + 1) * 8, hde);

    let vsw = vpr & 0x1F;
    let vds = (vpr >> 8) & 0xFF;
    let vdw_val = vdw & 0x01FF;
    let vcr_val = vcr & 0xFF;
    println!("  VPR=${:04X}: VSW={}, VDS={}", vpr, vsw, vds);
    println!("  VDW=${:04X}: VDW={} (active lines = {})", vdw, vdw_val, vdw_val + 1);
    println!("  VCR=${:04X}: VCR={}", vcr, vcr_val);

    println!("\nDone.");
    Ok(())
}
