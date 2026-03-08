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

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0;
    while frames < 150 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // VDC register dump
    println!("=== VDC Registers ===");
    for reg in 0..0x14 {
        let val = emu.bus.vdc_register(reg).unwrap_or(0);
        let name = match reg {
            0x00 => "MAWR (write addr)",
            0x01 => "MARR (read addr)",
            0x02 => "VWR (data write)",
            0x03 => "--- (unused)",
            0x04 => "--- (alias CR)",
            0x05 => "CR (control)",
            0x06 => "RCR (raster counter)",
            0x07 => "BXR (bg X scroll)",
            0x08 => "BYR (bg Y scroll)",
            0x09 => "MWR (memory width)",
            0x0A => "HSR (h sync)",
            0x0B => "HDR (h display)",
            0x0C => "VPR (v sync period)",
            0x0D => "VDW (v display width)",
            0x0E => "VCR (v display end)",
            0x0F => "DCR (DMA control)",
            0x10 => "SOUR (DMA source)",
            0x11 => "DESR (DMA dest)",
            0x12 => "LENR (DMA length)",
            0x13 => "DVSSR (SATB source)",
            _ => "???",
        };
        println!("  R{:02X} ({}) = 0x{:04X}", reg, name, val);
    }

    // Decode vertical timing
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);
    let vsw = vpr & 0x001F;
    let vds = (vpr >> 8) & 0x00FF;
    let active_lines = (vdw & 0x01FF) + 1;

    println!("\n=== Vertical Timing ===");
    println!("  VSW (sync width) = {}", vsw);
    println!("  VDS (display start) = {}", vds);
    println!("  VDW (display width) = {} lines", active_lines);
    println!("  VCR (display end margin) = {}", vcr & 0xFF);
    println!("  Active start line = {} (VSW+VDS)", vsw + vds);
    println!(
        "  Total per frame ~ {} lines",
        (vsw + vds) as u32 + active_lines as u32 + 3 + (vcr & 0xFF) as u32
    );

    // Decode horizontal timing
    let hsr = emu.bus.vdc_register(0x0A).unwrap_or(0);
    let hdr = emu.bus.vdc_register(0x0B).unwrap_or(0);
    let hsw = hsr & 0x001F;
    let hds = (hsr >> 8) & 0x007F;
    let hdw = hdr & 0x007F;
    let hde = (hdr >> 8) & 0x007F;
    println!("\n=== Horizontal Timing ===");
    println!("  HSW = {}, HDS = {}", hsw, hds);
    println!(
        "  HDW = {} (display {} tiles = {} px)",
        hdw,
        hdw + 1,
        (hdw + 1) * 8
    );
    println!("  HDE = {}", hde);

    // Check top-left corner pixels in the framebuffer
    println!("\n=== Top 16 rows pixel data (first 8 pixels) ===");
    // We need to get the last frame
    // Re-run to capture the frame
    let mut emu2 = Emulator::new();
    emu2.load_hucard(&rom)?;
    emu2.reset();
    let mut last_frame = None;
    let mut frames = 0;
    while frames < 150 {
        emu2.tick();
        if let Some(f) = emu2.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    if let Some(frame) = &last_frame {
        for y in 0..16 {
            print!("  Row {:3}: ", y);
            for x in 0..8 {
                let pixel = frame[y * 256 + x];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                print!("({:3},{:3},{:3}) ", r, g, b);
            }
            println!();
        }

        // Check if rows 0-7 differ from rows 8-15
        let row0_sample: Vec<u32> = (0..256).map(|x| frame[x]).collect();
        let row8_sample: Vec<u32> = (0..256).map(|x| frame[8 * 256 + x]).collect();
        let differs = row0_sample
            .iter()
            .zip(row8_sample.iter())
            .any(|(a, b)| a != b);
        println!("\n  Rows 0 and 8 differ: {}", differs);

        // Count non-black pixels in first 8 rows
        let mut non_black_top = 0;
        for y in 0..8 {
            for x in 0..256 {
                if frame[y * 256 + x] != 0 {
                    non_black_top += 1;
                }
            }
        }
        println!("  Non-black pixels in top 8 rows: {}", non_black_top);

        // Count non-black pixels in last 8 rows
        let mut non_black_bottom = 0;
        for y in 232..240 {
            for x in 0..256 {
                if frame[y * 256 + x] != 0 {
                    non_black_bottom += 1;
                }
            }
        }
        println!("  Non-black pixels in bottom 8 rows: {}", non_black_bottom);

        // Check for red vertical lines at top
        println!("\n=== Checking for red pixels in top rows ===");
        for y in 0..8 {
            let mut red_count = 0;
            let mut first_red_x = None;
            for x in 0..256 {
                let pixel = frame[y * 256 + x];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                if r > 100 && g < 50 && b < 50 {
                    red_count += 1;
                    if first_red_x.is_none() {
                        first_red_x = Some(x);
                    }
                }
            }
            if red_count > 0 {
                println!(
                    "  Row {}: {} red pixels, first at x={}",
                    y,
                    red_count,
                    first_red_x.unwrap_or(0)
                );
            }
        }
    }

    Ok(())
}
