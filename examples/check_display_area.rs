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
    let mut last_frame = None;

    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    let pixels = last_frame.as_ref().unwrap();
    let width = 256;
    let height = pixels.len() / width;

    // Check VDC display registers
    for reg in [0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E] {
        let val = emu.bus.vdc_register(reg).unwrap_or(0);
        let name = match reg {
            0x05 => "CR ",
            0x06 => "RCR",
            0x07 => "BXR",
            0x08 => "BYR",
            0x09 => "MWR",
            0x0A => "HSR",
            0x0B => "HDR",
            0x0C => "VPR",
            0x0D => "VDW",
            0x0E => "VCR",
            _ => "???",
        };
        println!("  Reg {:02X} ({}): {:04X}", reg, name, val);
    }

    // HSR: horizontal sync register (HDS = bits[6:0], HSW = bits[12:8])
    let hsr = emu.bus.vdc_register(0x0A).unwrap_or(0);
    let hsw = (hsr >> 8) & 0x1F;
    let hds = hsr & 0x7F;
    println!("\n  HSW={} HDS={}", hsw, hds);

    // HDR: horizontal display register (HDW = bits[6:0], HDE = bits[14:8])
    let hdr = emu.bus.vdc_register(0x0B).unwrap_or(0);
    let hdw = (hdr & 0x7F) + 1;
    let hde = (hdr >> 8) & 0x7F;
    println!("  HDW={} tiles = {} pixels, HDE={}", hdw, hdw * 8, hde);

    // VPR: vertical position register (VDS = bits[7:0], VSW = bits[12:8])
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vsw = (vpr >> 8) & 0x1F;
    let vds = vpr & 0xFF;
    println!("  VSW={} VDS={}", vsw, vds);

    // VDW: vertical display width (lines - 1)
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let display_height = (vdw & 0x1FF) + 1;
    println!("  VDW={:04X} → {} active scanlines", vdw, display_height);

    // VCR: vertical display end
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);
    println!("  VCR={:04X}", vcr);

    println!(
        "\n  Active display: {} lines starting after VDS={} + VSW={}",
        display_height, vds, vsw
    );
    let first_active = (vsw + vds + 2) as usize; // +2 for sync
    println!("  First active line in frame: ~{}", first_active);

    // BYR determines the tile map Y offset at the first active scanline
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    println!(
        "\n  BYR={:04X} ({}) → first active scanline shows tile row {}",
        byr,
        byr,
        byr / 8
    );

    // Text positions in tile map: rows 20, 22, 24, 26
    // Map pixel Y = row * 8
    // Map Y relative to BYR = row * 8 - BYR
    // If this is negative, it wraps (map is 64 tiles = 512 pixels tall)
    let map_h_pixels = 64 * 8; // 512 for 64-tile map
    for &(tile_row, desc) in &[
        (20, "HISCORE"),
        (22, "SCORE"),
        (24, "PUSH RUN"),
        (26, "COPYRIGHT"),
    ] {
        let map_y = tile_row * 8;
        let mut rel_y = (map_y as i32 - byr as i32 + map_h_pixels as i32) % map_h_pixels as i32;
        if rel_y >= map_h_pixels as i32 / 2 {
            rel_y -= map_h_pixels as i32;
        }
        // Check if this row is within the display height
        let visible = rel_y >= 0 && rel_y < display_height as i32;
        // The frame row where this text appears
        let frame_y = if visible {
            first_active as i32 + rel_y
        } else {
            -1
        };
        println!(
            "  {} at tile row {}: mapY={} relY={} visible={} frameY={}",
            desc, tile_row, map_y, rel_y, visible, frame_y
        );
    }

    // Show what's at the bottom of the frame
    println!("\n=== Bottom of frame scan ===");
    let bg_color = pixels[0] & 0xFFFFFF; // assume top-left is background
    for y in (height - 30)..height {
        let mut non_bg = 0;
        let mut has_white = false;
        for x in 0..width {
            let p = pixels[y * width + x] & 0xFFFFFF;
            if p != bg_color && p != 0 {
                non_bg += 1;
            }
            if ((p >> 16) & 0xFF) > 200 && ((p >> 8) & 0xFF) > 200 && (p & 0xFF) > 200 {
                has_white = true;
            }
        }
        if non_bg > 0 || has_white {
            println!("  Y={:3}: {} non-bg pixels, white={}", y, non_bg, has_white);
        }
    }

    // Scan entire frame for white text-like patterns
    println!("\n=== Scanning for white pixel clusters (potential text) ===");
    for y in 0..height {
        let mut white_count = 0;
        let mut white_start = 0;
        for x in 0..width {
            let p = pixels[y * width + x];
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            if r > 200 && g > 200 && b > 200 {
                if white_count == 0 {
                    white_start = x;
                }
                white_count += 1;
            }
        }
        if white_count > 10 {
            println!(
                "  Y={:3}: {} white pixels (first at X={})",
                y, white_count, white_start
            );
        }
    }

    // Save PNG-compatible PPM
    let mut data = Vec::with_capacity(width * height * 3);
    for &p in pixels {
        data.push(((p >> 16) & 0xFF) as u8);
        data.push(((p >> 8) & 0xFF) as u8);
        data.push((p & 0xFF) as u8);
    }
    let header = format!("P6\n{} {}\n255\n", width, height);
    let mut file = std::fs::File::create("frame_300_restored.ppm")?;
    file.write_all(header.as_bytes())?;
    file.write_all(&data)?;

    Ok(())
}
