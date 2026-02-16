use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/R-Type I (Japan) (En).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0;
    let mut last_frame: Option<Vec<u32>> = None;

    // Run 180 frames to reach title screen
    while frames < 180 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            frames += 1;
            last_frame = Some(f);
        }
    }
    eprintln!("Title at frame {}", frames);

    // Press RUN (bit 7 active-low) to start
    emu.bus.set_joypad_input(0xFF & !(1 << 7));
    while frames < 184 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            frames += 1;
            last_frame = Some(f);
        }
    }
    emu.bus.set_joypad_input(0xFF);

    // Wait for gameplay (run ~599 frames total)
    while frames < 599 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            frames += 1;
            last_frame = Some(f);
        }
    }

    // Enable register trace for frame 600
    emu.bus.enable_vdc_reg_trace();
    while frames < 600 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            frames += 1;
            last_frame = Some(f);
        }
    }
    let trace = emu.bus.take_vdc_reg_trace();
    eprintln!("\n=== Register writes during frame 600 ===");
    for (sl, reg, val) in &trace {
        let name = match reg {
            0x05 => "CR ",
            0x06 => "RCR",
            0x07 => "BXR",
            0x08 => "BYR",
            _ => "???",
        };
        eprintln!("  sl {:3}: {} (R{:02X}) = {:#06X} ({})", sl, name, reg, val, val);
    }

    let w = emu.display_width();
    let h = emu.display_height();
    let y_off = emu.display_y_offset();
    eprintln!("Gameplay frame {}: {}x{} y_offset={}", frames, w, h, y_off);
    eprintln!("  PPM row 0 = fb_row {} = scanline {}", y_off, y_off + 20);
    eprintln!("  PPM row {} = fb_row {} = scanline {}", h-1, y_off + h - 1, y_off + h - 1 + 20);
    eprintln!("VDC registers: BYR(R08)={:#06X} BXR(R07)={:#06X} CR(R05)={:#06X} RCR(R06)={:#06X}",
        emu.bus.vdc_register(0x08).unwrap_or(0),
        emu.bus.vdc_register(0x07).unwrap_or(0),
        emu.bus.vdc_register(0x05).unwrap_or(0),
        emu.bus.vdc_register(0x06).unwrap_or(0));
    eprintln!("VDW(R0D)={:#06X} VPR(R0C)={:#06X} VCR(R0E)={:#06X} HDR(R0B)={:#06X} HSR(R0A)={:#06X}",
        emu.bus.vdc_register(0x0D).unwrap_or(0),
        emu.bus.vdc_register(0x0C).unwrap_or(0),
        emu.bus.vdc_register(0x0E).unwrap_or(0),
        emu.bus.vdc_register(0x0B).unwrap_or(0),
        emu.bus.vdc_register(0x0A).unwrap_or(0));

    if let Some(ref frame) = last_frame {
        // Detailed row analysis - show first 20 and last 20 rows
        eprintln!("\n=== Top rows ===");
        for y in 0..20.min(h) {
            let mut nonblack = 0;
            let mut first_color = 0u32;
            for x in 0..w {
                let px = frame[y * w + x];
                if px != 0x000000 {
                    nonblack += 1;
                    if first_color == 0 { first_color = px; }
                }
            }
            eprintln!("  row {:3}: {:4} non-black  first={:06X}  sample=[{:06X} {:06X} {:06X} {:06X}]",
                y, nonblack, first_color,
                frame[y * w], frame[y * w + w/4], frame[y * w + w/2], frame[y * w + 3*w/4]);
        }

        eprintln!("\n=== Bottom rows ===");
        for y in h.saturating_sub(20)..h {
            let mut nonblack = 0;
            let mut first_color = 0u32;
            for x in 0..w {
                let px = frame[y * w + x];
                if px != 0x000000 {
                    nonblack += 1;
                    if first_color == 0 { first_color = px; }
                }
            }
            eprintln!("  row {:3}: {:4} non-black  first={:06X}  sample=[{:06X} {:06X} {:06X} {:06X}]",
                y, nonblack, first_color,
                frame[y * w], frame[y * w + w/4], frame[y * w + w/2], frame[y * w + 3*w/4]);
        }

        // Write full PPM
        write_ppm(frame, "/tmp/rtype_gameplay.ppm", w, h)?;
        eprintln!("\nWrote /tmp/rtype_gameplay.ppm");
    }

    // Dump BAT entries for tile rows around the score bar
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    eprintln!("\n=== BAT tile rows 27-31 (map {}x{}) ===", map_w, map_h);
    for tile_row in 27..32 {
        let row = tile_row % map_h.max(1);
        let mut entries = Vec::new();
        for col in 0..map_w.min(64) {
            let c = col % map_w.max(1);
            let addr = (row * map_w.max(1) + c) & 0x7FFF;
            let entry = emu.bus.vdc_vram_word(addr as u16);
            if entry != 0 {
                let tile_id = entry & 0x07FF;
                let pal = (entry >> 12) & 0x0F;
                entries.push(format!("[{col}:{tile_id:03X}p{pal:X}]"));
            }
        }
        eprintln!("  tile_row {:2} (BAT Y {:3}-{:3}): {} entries: {}",
            tile_row, tile_row * 8, tile_row * 8 + 7,
            entries.len(),
            if entries.is_empty() { "EMPTY".to_string() } else { entries.join(" ") });
    }

    // Dump tile 0x0BF VRAM data (used for HUD BAT entries)
    // HuC6270 tile format: 16 VRAM words per 8x8 tile
    //   Words 0-7: row N planes 0-1 (low=plane0, high=plane1)
    //   Words 8-15: row N planes 2-3 (low=plane2, high=plane3)
    let tile_base = 0x0BF * 16;
    eprintln!("\n=== Tile 0x0BF VRAM data (HUD border tile) ===");
    for line in 0..8usize {
        let w0 = emu.bus.vdc_vram_word((tile_base + line) as u16);       // planes 0-1
        let w1 = emu.bus.vdc_vram_word((tile_base + 8 + line) as u16);   // planes 2-3
        let bp0 = (w0 & 0xFF) as u8;
        let bp1 = ((w0 >> 8) & 0xFF) as u8;
        let bp2 = (w1 & 0xFF) as u8;
        let bp3 = ((w1 >> 8) & 0xFF) as u8;
        let mut pixels = [0u8; 8];
        for bit in 0..8 {
            let shift = 7 - bit;
            pixels[bit] = ((bp0 >> shift) & 1)
                | (((bp1 >> shift) & 1) << 1)
                | (((bp2 >> shift) & 1) << 2)
                | (((bp3 >> shift) & 1) << 3);
        }
        let pix_str: Vec<String> = pixels.iter().map(|p| format!("{:X}", p)).collect();
        let nonzero = pixels.iter().filter(|&&p| p != 0).count();
        eprintln!("  line {}: w0={:04X} w1={:04X}  bp[{:02X} {:02X} {:02X} {:02X}] pixels=[{}] nonzero={}",
            line, w0, w1, bp0, bp1, bp2, bp3, pix_str.join(""), nonzero);
    }

    // Check VCE[0] (background color)
    let vce0 = emu.bus.vce_palette_word(0);
    let vce0_rgb = emu.bus.vce_palette_rgb(0);
    eprintln!("  VCE[0] = {:04X} -> RGB {:06X}", vce0, vce0_rgb);
    let vce_spr0 = emu.bus.vce_palette_word(0x100);
    eprintln!("  VCE[0x100] (sprite pal 0) = {:04X} -> RGB {:06X}", vce_spr0, emu.bus.vce_palette_rgb(0x100));

    // Detailed analysis of HUD transition (fb_row 198=sl218 game last, fb_row 199=sl219 HUD start)
    let fw = 512; // FRAME_WIDTH internal stride
    let raw_fb = emu.framebuffer();
    for check_row in [197, 198, 199, 200, 201] {
        eprintln!("\n=== Detailed fb_row {} (sl {}) first 48 pixels ===", check_row, check_row + 20);
        for chunk_start in (0..48).step_by(8) {
            let mut pixels = Vec::new();
            for x in chunk_start..chunk_start+8 {
                let px = raw_fb[check_row * fw + x];
                pixels.push(format!("{:06X}", px));
            }
            eprintln!("  x {:3}-{:3}: {}", chunk_start, chunk_start+7, pixels.join(" "));
        }
    }
    // Check game tile at tile_row 26 col 0 (BYR=9 yoff=200 → Y=209 → tile_row 26 line 1)
    eprintln!("\n=== Tile 0x610 line 1 (game terrain at tile_row 26 col 0) ===");
    let game_tile_base = 0x610 * 16;
    for line in [0usize, 1, 7] {
        let w0 = emu.bus.vdc_vram_word((game_tile_base + line) as u16);
        let w1 = emu.bus.vdc_vram_word((game_tile_base + 8 + line) as u16);
        let bp0 = (w0 & 0xFF) as u8;
        let bp1 = ((w0 >> 8) & 0xFF) as u8;
        let bp2 = (w1 & 0xFF) as u8;
        let bp3 = ((w1 >> 8) & 0xFF) as u8;
        let mut pix = [0u8; 8];
        for bit in 0..8 {
            let shift = 7 - bit;
            pix[bit] = ((bp0 >> shift) & 1)
                | (((bp1 >> shift) & 1) << 1)
                | (((bp2 >> shift) & 1) << 2)
                | (((bp3 >> shift) & 1) << 3);
        }
        let pix_str: Vec<String> = pix.iter().map(|p| format!("{:X}", p)).collect();
        eprintln!("  line {}: pixels=[{}]", line, pix_str.join(""));
    }
    // Map a few fb_row 216 pixel colors to palette entries
    eprintln!("\n=== Palette mapping for fb_row 216 pixel colors ===");
    for &px in &[0x482400u32, 0xB69191u32, 0x6D6D48u32] {
        // Reverse the palette_rgb mapping
        let r = ((px >> 16) & 0xFF) as u8;
        let g = ((px >> 8) & 0xFF) as u8;
        let b = (px & 0xFF) as u8;
        // 3-bit values: 0→0x00, 1→0x24, 2→0x48, 3→0x6D, 4→0x91, 5→0xB6, 6→0xDA, 7→0xFF
        let lookup = |v: u8| -> u8 {
            match v { 0x00=>0, 0x24=>1, 0x48=>2, 0x6D=>3, 0x91=>4, 0xB6=>5, 0xDA=>6, 0xFF=>7, _=>0xFF }
        };
        let rb = lookup(r); let gb = lookup(g); let bb = lookup(b);
        if rb <= 7 && gb <= 7 && bb <= 7 {
            let vce_val = (gb as u16) << 6 | (rb as u16) << 3 | bb as u16;
            // Search all palette entries for this value
            let mut found = Vec::new();
            for i in 0..512 {
                if emu.bus.vce_palette_word(i) == vce_val {
                    found.push(format!("{:03X}", i));
                }
            }
            eprintln!("  {:06X} → VCE {:03X} found at: {}", px, vce_val, found.join(", "));
        }
    }

    // Also check tile at game area BAT (tile_row 26, col 0) for comparison
    let game_bat_addr = (26 * map_w) & 0x7FFF;
    let game_entry = emu.bus.vdc_vram_word(game_bat_addr as u16);
    let game_tile = game_entry & 0x07FF;
    let game_pal = (game_entry >> 12) & 0x0F;
    eprintln!("\n=== Game area tile at tile_row 26 col 0: tile={:03X} pal={:X} ===", game_tile, game_pal);

    // Dump per-line scroll state for scanlines around the HUD transitions
    // R-Type: active_start = (VSW+1)+(VDS+2) = 3+17 = 20
    // fb_row 0 -> scanline 20, fb_row 8 -> scanline 28, etc.
    eprintln!("\n=== Per-line scroll state (scanlines 20-55, fb_rows 0-35) ===");
    for sl in 20..55 {
        let (bxr, byr) = emu.bus.vdc_scroll_line(sl);
        let yoff = emu.bus.vdc_scroll_line_y_offset(sl);
        let valid = emu.bus.vdc_scroll_line_valid(sl);
        let ctrl = emu.bus.vdc_control_line(sl);
        let fb_row = sl - 20;
        eprintln!("  sl {:3} (fb {:3}): BXR={:04X} BYR={:04X} yoff={:3} ctrl={:04X} v={}",
            sl, fb_row, bxr, byr, yoff, ctrl, valid);
    }
    eprintln!("\n=== Per-line scroll state (scanlines 210-235, around HUD RCR at sl 219) ===");
    for sl in 210..236 {
        let (bxr, byr) = emu.bus.vdc_scroll_line(sl);
        let yoff = emu.bus.vdc_scroll_line_y_offset(sl);
        let valid = emu.bus.vdc_scroll_line_valid(sl);
        let ctrl = emu.bus.vdc_control_line(sl);
        let fb_row = sl as isize - 20;
        eprintln!("  sl {:3} (fb {:3}): BXR={:04X} BYR={:04X} yoff={:3} ctrl={:04X} v={}",
            sl, fb_row, bxr, byr, yoff, ctrl, valid);
    }

    // Also dump the raw 240-line framebuffer before crop
    eprintln!("\n=== Raw framebuffer (240 rows) top 25 ===");
    let raw_fb = emu.framebuffer();
    let fw = 512; // FRAME_WIDTH internal stride
    for y in 0..25 {
        let mut nonblack = 0;
        for x in 0..w.min(fw) {
            if raw_fb[y * fw + x] != 0x000000 {
                nonblack += 1;
            }
        }
        let in_active = if nonblack > 0 { "CONTENT" } else { "black" };
        eprintln!("  fb_row {:3}: {:4} non-black  [{}]  sample=[{:06X} {:06X} {:06X}]",
            y, nonblack, in_active,
            raw_fb[y * fw], raw_fb[y * fw + w/4], raw_fb[y * fw + w/2]);
    }

    eprintln!("\n=== Raw framebuffer around HUD transition (fb 190-215) ===");
    for y in 190..216 {
        let mut nonblack = 0;
        for x in 0..w.min(fw) {
            if raw_fb[y * fw + x] != 0x000000 {
                nonblack += 1;
            }
        }
        eprintln!("  fb_row {:3}: {:4} non-black  sample=[{:06X} {:06X} {:06X}]",
            y, nonblack,
            raw_fb[y * fw], raw_fb[y * fw + w/4], raw_fb[y * fw + w/2]);
    }

    Ok(())
}

fn write_ppm(frame: &[u32], path: &str, width: usize, height: usize) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", width, height)?;
    for y in 0..height {
        for x in 0..width {
            let pixel = frame[y * width + x];
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            file.write_all(&[r, g, b])?;
        }
    }
    Ok(())
}
