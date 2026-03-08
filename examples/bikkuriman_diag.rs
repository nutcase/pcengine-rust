#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = "roms/Bikkuriman World (Japan).pce";
    let rom = std::fs::read(rom_path)?;

    let mut emulator = Emulator::new();
    emulator.load_hucard(&rom)?;
    emulator.reset();

    let run_pressed: u8 = 0xFF & !(1 << 7);

    // Get to game screen
    let mut frame_count = 0;
    run_frames(&mut emulator, 600, 0xFF, &mut frame_count);
    run_frames(&mut emulator, 10, run_pressed, &mut frame_count);
    run_frames(&mut emulator, 120, 0xFF, &mut frame_count);
    run_frames(&mut emulator, 10, run_pressed, &mut frame_count);

    // Run a few more frames and save
    let mut last_frame = None;
    run_frames_save(&mut emulator, 300, 0xFF, &mut frame_count, &mut last_frame);

    eprintln!("=== Game screen at frame {} ===", frame_count);

    // Check state
    let cr = emulator.bus.vdc_register(0x05).unwrap_or(0);
    let dcr = emulator.bus.vdc_register(0x0F).unwrap_or(0);
    let dvssr = emulator.bus.vdc_register(0x13).unwrap_or(0);
    eprintln!(
        "CR={:04X} (BG={} SPR={}) DCR={:04X} (auto_satb={}) DVSSR={:04X}",
        cr,
        (cr & 0x80) != 0,
        (cr & 0x40) != 0,
        dcr,
        (dcr & 0x10) != 0,
        dvssr
    );
    eprintln!(
        "satb_written={} satb_pending={} satb_source={:04X}",
        emulator.bus.vdc_satb_written(),
        emulator.bus.vdc_satb_pending(),
        emulator.bus.vdc_satb_source()
    );

    // Count sprites
    let mut sat_count = 0;
    for sprite in 0..64usize {
        let base = sprite * 4;
        let y = emulator.bus.vdc_satb_word(base);
        let x = emulator.bus.vdc_satb_word(base + 1);
        let pat = emulator.bus.vdc_satb_word(base + 2);
        let attr = emulator.bus.vdc_satb_word(base + 3);
        if y != 0 || x != 0 || pat != 0 || attr != 0 {
            sat_count += 1;
            if sat_count <= 12 {
                let sy = (y & 0x03FF) as i32 - 64;
                let sx = (x & 0x03FF) as i32 - 32;
                let sp = (pat >> 1) & 0x03FF;
                eprintln!(
                    "  SPR#{:02}: x={:4} y={:4} pat={:03X} pal={:X} attr={:04X}",
                    sprite,
                    sx,
                    sy,
                    sp,
                    attr & 0xF,
                    attr
                );
            }
        }
    }
    if sat_count > 12 {
        eprintln!("  ...and {} more sprites", sat_count - 12);
    }
    eprintln!("Total sprites in SAT: {}", sat_count);

    // Scroll registers
    let bxr = emulator.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emulator.bus.vdc_register(0x08).unwrap_or(0);
    let mwr = emulator.bus.vdc_register(0x09).unwrap_or(0);
    eprintln!("BXR={:04X} BYR={:04X} MWR={:04X}", bxr, byr, mwr);
    let (map_w, map_h) = emulator.bus.vdc_map_dimensions();
    eprintln!("BAT map: {}x{}", map_w, map_h);

    // Save last frame
    if let Some(ref frame) = last_frame {
        write_ppm(frame, "bikkuriman_game_fixed.ppm")?;
        eprintln!("Wrote bikkuriman_game_fixed.ppm");

        // Scan for black pixel clusters in sky area (y=40..120, x=64..224)
        eprintln!("\n=== Scanning for black pixels in sky area ===");
        for sy in (40..120).step_by(8) {
            for sx in (64..224).step_by(8) {
                let mut black_count = 0;
                for dy in 0..8 {
                    for dx in 0..8 {
                        let px = frame.get((sy + dy) * 256 + (sx + dx)).copied().unwrap_or(0);
                        if px == 0x000000 {
                            black_count += 1;
                        }
                    }
                }
                if black_count > 16 {
                    eprintln!(
                        "  Black cluster at screen ({},{}): {} black pixels",
                        sx, sy, black_count
                    );
                    // Show what BAT tile this maps to
                    let bg_x = (sx as u16).wrapping_add(bxr) & 0x1FF;
                    let bg_y = (sy as u16).wrapping_add(byr) & 0x1FF;
                    let tile_col = (bg_x / 8) as usize % map_w;
                    let tile_row = (bg_y / 8) as usize % map_h;
                    let bat_addr = (tile_row * map_w + tile_col) as u16;
                    let bat_entry = emulator.bus.vdc_vram_word(bat_addr);
                    let tile_id = bat_entry & 0x07FF;
                    let pal = (bat_entry >> 12) & 0x0F;
                    eprintln!(
                        "    BAT[{},{}] addr={:04X} entry={:04X} tile={:03X} pal={:X}",
                        tile_col, tile_row, bat_addr, bat_entry, tile_id, pal
                    );
                    // Dump tile pixel data from VRAM
                    let tile_vram_base = tile_id as u16 * 16;
                    eprintln!("    Tile VRAM base={:04X}", tile_vram_base);
                    for row in 0..8u16 {
                        let plane0 = emulator.bus.vdc_vram_word(tile_vram_base + row);
                        let plane1 = emulator.bus.vdc_vram_word(tile_vram_base + row + 8);
                        let lo0 = (plane0 & 0xFF) as u8;
                        let hi0 = ((plane0 >> 8) & 0xFF) as u8;
                        let lo1 = (plane1 & 0xFF) as u8;
                        let hi1 = ((plane1 >> 8) & 0xFF) as u8;
                        let mut pixels = [0u8; 8];
                        for bit in 0..8 {
                            let mask = 1 << (7 - bit);
                            pixels[bit] = ((lo0 & mask != 0) as u8)
                                | (((hi0 & mask != 0) as u8) << 1)
                                | (((lo1 & mask != 0) as u8) << 2)
                                | (((hi1 & mask != 0) as u8) << 3);
                        }
                        eprintln!("    row{}: {:?}", row, pixels);
                    }
                }
            }
        }

        // Check VCE palette 1 (the palette used by the black tile)
        eprintln!("\n=== VCE BG palette 1 ===");
        for i in 0..16u16 {
            let rgb = emulator.bus.vce_palette_rgb((16 + i) as usize);
            let r = (rgb >> 16) & 0xFF;
            let g = (rgb >> 8) & 0xFF;
            let b = rgb & 0xFF;
            eprintln!("  pal1[{:2}] = {:06X} (R={} G={} B={})", i, rgb, r, g, b);
        }

        // Dump BAT grid around the black square (BAT col 10-14, row 11-15)
        eprintln!(
            "\n=== BAT around black square (BXR={:04X} BYR={:04X}) ===",
            bxr, byr
        );
        // The black square is at screen (128,64). With BXR=0xE0, BYR=0x2E:
        // bg_x = (128+0xE0)&0x1FF = 352, tile_col = 352/8 = 44 mod 32 = 12
        // bg_y = (64+0x2E)&0x1FF = 110, tile_row = 110/8 = 13
        for row in 10..18usize {
            let r = row % map_h;
            eprint!("  row {:2}:", row);
            for col in 8..20usize {
                let c = col % map_w;
                let addr = (r * map_w + c) as u16;
                let entry = emulator.bus.vdc_vram_word(addr);
                let tile_id = entry & 0x07FF;
                let pal = (entry >> 12) & 0x0F;
                if tile_id != 0 || pal != 0 {
                    eprint!(" [{:2}:{:03X}p{:X}]", col, tile_id, pal);
                } else {
                    eprint!(" [{:2}:---  ]", col);
                }
            }
            eprintln!();
        }

        // Dump raw VRAM words for tile 0x050
        eprintln!("\n=== Raw VRAM for tile 0x050 (addr 0x0500-0x050F) ===");
        for i in 0..16u16 {
            let w = emulator.bus.vdc_vram_word(0x0500 + i);
            eprintln!("  VRAM[{:04X}] = {:04X}", 0x0500 + i, w);
        }

        // Check tile 0 VRAM data
        eprintln!("\n=== Raw VRAM for tile 0x000 (addr 0x0000-0x000F) ===");
        for i in 0..16u16 {
            let w = emulator.bus.vdc_vram_word(i);
            eprintln!("  VRAM[{:04X}] = {:04X}", i, w);
        }

        // Check VCE[0] (background color)
        let bg_color = emulator.bus.vce_palette_rgb(0);
        eprintln!("\nVCE[0] (BG color) = {:06X}", bg_color);

        // Dump actual rendered pixels at the black square area
        eprintln!("\n=== Rendered pixels at screen (124..140, 60..80) ===");
        for y in 60..80 {
            eprint!("  y={:3}: ", y);
            for x in 124..140 {
                let px = frame.get(y * 256 + x).copied().unwrap_or(0);
                if px == 0x000000 {
                    eprint!("BK ");
                } else if px == 0x00B6FF {
                    eprint!("SK ");
                } else {
                    eprint!("{:02X} ", px & 0xFF);
                }
            }
            eprintln!();
        }

        // Scan entire BAT for 0x0000 entries (unwritten)
        eprintln!("\n=== BAT entries that are 0x0000 (32x32 BAT) ===");
        let mut zero_count = 0;
        for row in 0..32usize {
            for col in 0..32usize {
                let addr = (row * 32 + col) as u16;
                let entry = emulator.bus.vdc_vram_word(addr);
                if entry == 0x0000 {
                    zero_count += 1;
                    eprintln!("  BAT[{:2},{:2}] VRAM[{:04X}] = 0x0000", col, row, addr);
                }
            }
        }
        eprintln!("Total zero BAT entries: {}", zero_count);

        // Check CR increment settings
        let cr = emulator.bus.vdc_register(0x05).unwrap_or(0);
        let inc_mode = (cr >> 11) & 0x03;
        let inc_amount = match inc_mode {
            0 => 1,
            1 => 32,
            2 => 64,
            _ => 128,
        };
        eprintln!("\nCR increment mode: {} (+{} words)", inc_mode, inc_amount);

        // Check MAWR
        eprintln!(
            "MAWR (write addr): {:04X}",
            emulator.bus.vdc_register(0x00).unwrap_or(0)
        );
    } else {
        eprintln!("No frame captured!");
    }

    Ok(())
}

fn run_frames(emulator: &mut Emulator, count: usize, pad: u8, frame_count: &mut usize) {
    let mut collected = 0;
    let mut budget = count as u64 * 250_000;
    while collected < count && budget > 0 {
        emulator.bus.set_joypad_input(pad);
        let c = emulator.tick() as u64;
        budget = budget.saturating_sub(c.max(1));
        if emulator.take_frame().is_some() {
            collected += 1;
            *frame_count += 1;
        }
    }
}

fn run_frames_save(
    emulator: &mut Emulator,
    count: usize,
    pad: u8,
    frame_count: &mut usize,
    last: &mut Option<Vec<u32>>,
) {
    let mut collected = 0;
    let mut budget = count as u64 * 250_000;
    while collected < count && budget > 0 {
        emulator.bus.set_joypad_input(pad);
        let c = emulator.tick() as u64;
        budget = budget.saturating_sub(c.max(1));
        if let Some(frame) = emulator.take_frame() {
            collected += 1;
            *frame_count += 1;
            *last = Some(frame);
        }
    }
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
