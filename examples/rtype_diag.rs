use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = "roms/R-Type I (Japan) (En).pce";
    let rom = std::fs::read(rom_path)?;

    let mut emulator = Emulator::new();
    emulator.load_hucard(&rom)?;
    emulator.reset();

    // Run to title screen
    let mut frame_count = 0;
    let mut last_frame = None;
    let mut budget: u64 = 300 * 250_000;
    while frame_count < 300 && budget > 0 {
        let c = emulator.tick() as u64;
        budget = budget.saturating_sub(c.max(1));
        if let Some(frame) = emulator.take_frame() {
            frame_count += 1;
            last_frame = Some(frame);
        }
    }

    eprintln!("=== R-Type title screen at frame {} ===", frame_count);

    // VDC registers
    for reg in [0x05u8, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E] {
        let val = emulator.bus.vdc_register(reg as usize).unwrap_or(0);
        let name = match reg {
            0x05 => "CR  ",
            0x07 => "BXR ",
            0x08 => "BYR ",
            0x09 => "MWR ",
            0x0A => "HSR ",
            0x0B => "HDR ",
            0x0C => "VPR ",
            0x0D => "VDW ",
            0x0E => "VCR ",
            _ => "??? ",
        };
        eprintln!("R{:02X} ({}) = {:04X}", reg, name, val);
    }

    // Parse horizontal display settings
    let hsr = emulator.bus.vdc_register(0x0A).unwrap_or(0);
    let hdr = emulator.bus.vdc_register(0x0B).unwrap_or(0);
    let hsw = hsr & 0x1F;
    let hds = (hsr >> 8) & 0x7F;
    let hdw = hdr & 0x7F;
    let hde = (hdr >> 8) & 0x7F;
    eprintln!("\nHorizontal: HSW={} HDS={} HDW={} HDE={}", hsw, hds, hdw, hde);
    let display_width = (hdw + 1) * 8;
    eprintln!("Display width = ({} + 1) * 8 = {} pixels", hdw, display_width);

    // Parse vertical display settings
    let vpr = emulator.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emulator.bus.vdc_register(0x0D).unwrap_or(0);
    let vcr = emulator.bus.vdc_register(0x0E).unwrap_or(0);
    let vsw = vpr & 0x1F;
    let vds = (vpr >> 8) & 0xFF;
    eprintln!("Vertical: VSW={} VDS={} VDW={} VCR={}", vsw, vds, vdw & 0x1FF, vcr & 0xFF);
    let display_height = (vdw & 0x1FF) + 1;
    eprintln!("Display height = {} pixels", display_height);

    // MWR details
    let mwr = emulator.bus.vdc_register(0x09).unwrap_or(0);
    let screen_w = match (mwr >> 4) & 0x07 {
        0b000 => 32,
        0b001 => 64,
        0b010 => 128,
        0b011 => 128,
        0b100 => 32,
        0b101 => 64,
        0b110 => 128,
        0b111 => 128,
        _ => 32,
    };
    let screen_h = if (mwr & 0x40) != 0 { 64 } else { 32 };
    eprintln!("Virtual screen: {}x{} tiles ({}x{} pixels)", screen_w, screen_h, screen_w * 8, screen_h * 8);

    // VCE dot clock
    // Check the VCE control register for dot clock
    let emu_w = emulator.display_width();
    eprintln!("\nemulator.display_width() = {}", emu_w);

    eprintln!("\n=== Frame buffer analysis ===");
    if let Some(ref frame) = last_frame {
        eprintln!("Frame size: {} pixels ({}x{})", frame.len(), emu_w, frame.len() / emu_w);

        // Count non-black pixels per column
        let height = frame.len() / emu_w;
        let mut col_activity = vec![0u32; emu_w];
        for y in 0..height {
            for x in 0..emu_w {
                if frame[y * emu_w + x] != 0x000000 {
                    col_activity[x] += 1;
                }
            }
        }
        // Show last 32 columns
        let start_col = emu_w.saturating_sub(32);
        eprint!("Columns {}-{} activity: ", start_col, emu_w - 1);
        for x in start_col..emu_w {
            eprint!("{} ", col_activity[x]);
        }
        eprintln!();

        // Row activity analysis (non-black pixel count per row)
        eprintln!("\n=== Row activity (top/bottom) ===");
        let mut row_activity = vec![0u32; height];
        for y in 0..height {
            for x in 0..emu_w {
                if frame[y * emu_w + x] != 0x000000 {
                    row_activity[y] += 1;
                }
            }
        }
        // Show first 20 and last 20 rows
        for y in 0..20.min(height) {
            eprintln!("  row {:3}: {} non-black pixels (first pixel: {:06X})",
                y, row_activity[y], frame[y * emu_w]);
        }
        if height > 40 {
            eprintln!("  ...");
        }
        for y in height.saturating_sub(20)..height {
            eprintln!("  row {:3}: {} non-black pixels (first pixel: {:06X})",
                y, row_activity[y], frame[y * emu_w]);
        }

        // Check sprite positions
        eprintln!("\n=== Sprites beyond x={} ===", emu_w);
        for sprite in 0..64usize {
            let base = sprite * 4;
            let y_w = emulator.bus.vdc_satb_word(base);
            let x_w = emulator.bus.vdc_satb_word(base + 1);
            let pat_w = emulator.bus.vdc_satb_word(base + 2);
            let attr_w = emulator.bus.vdc_satb_word(base + 3);
            if y_w == 0 && x_w == 0 && pat_w == 0 && attr_w == 0 {
                continue;
            }
            let y = (y_w & 0x03FF) as i32 - 64;
            let x = (x_w & 0x03FF) as i32 - 32;
            let pat = (pat_w >> 1) & 0x03FF;
            let pal = attr_w & 0x000F;
            let cgx = (attr_w >> 8) & 1;
            let cgy = (attr_w >> 12) & 3;
            let w = if cgx == 0 { 16 } else { 32 };
            let h: i32 = match cgy { 0 => 16, 1 => 32, 3 => 64, _ => 16 };
            if x + w > emu_w as i32 - 32 {
                eprintln!("  SPR#{:02} x={:4} y={:4} pat={:03X} pal={:X} {}x{} (right edge: {})",
                    sprite, x, y, pat, pal, w, h, x + w);
            }
        }

        // Write full frame
        write_ppm(frame, "rtype_title_full.ppm", emu_w)?;
        eprintln!("Wrote rtype_title_full.ppm ({}x{})", emu_w, height.min(224));
    }

    Ok(())
}

fn write_ppm(frame: &[u32], path: &str, width: usize) -> Result<(), Box<dyn Error>> {
    let height = 224.min(frame.len() / width);
    let mut file = File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", width, height)?;
    for y in 0..height {
        for x in 0..width {
            let pixel = frame.get(y * width + x).copied().unwrap_or(0);
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            file.write_all(&[r, g, b])?;
        }
    }
    Ok(())
}
