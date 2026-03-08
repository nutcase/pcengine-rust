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

    let mut last_frame = None;
    let mut frames = 0;
    while frames < 150 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    let frame = last_frame.unwrap();

    // VDC timing
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);
    let vsw = vpr & 0x001F;
    let vds = (vpr >> 8) & 0x00FF;
    let active_lines = (vdw & 0x01FF) + 1;
    println!(
        "VSW={} VDS={} active_start={} VDW={} active_lines={} VCR={}",
        vsw,
        vds,
        vsw + vds,
        vdw,
        active_lines,
        vcr & 0xFF
    );

    // Analyze each row
    let bg_color = frame[0]; // assume row 0 is background
    println!("\nBackground color (row 0, x=0): #{:06X}", bg_color);

    println!("\nRow analysis (256x240 framebuffer):");
    let mut first_content = None;
    let mut last_content = None;
    for y in 0..240 {
        let mut non_bg = 0;
        let mut unique_colors = std::collections::HashSet::new();
        for x in 0..256 {
            let pixel = frame[y * 256 + x];
            if pixel != bg_color {
                non_bg += 1;
            }
            unique_colors.insert(pixel);
        }
        if non_bg > 0 {
            if first_content.is_none() {
                first_content = Some(y);
            }
            last_content = Some(y);
        }
        if y < 20 || y >= 220 || (non_bg > 0 && non_bg < 20) {
            println!(
                "  Row {:3}: {:4} non-bg pixels, {:3} unique colors",
                y,
                non_bg,
                unique_colors.len()
            );
        }
    }
    println!("\nFirst content row: {:?}", first_content);
    println!("Last content row: {:?}", last_content);
    println!(
        "Content height: {} rows",
        last_content.unwrap_or(0) - first_content.unwrap_or(0) + 1
    );

    // Check what a reference 224-line image would look like
    // Typical NTSC visible area: lines 22-245 of 263, or about 224 lines
    // Active display of 240 lines starts at line 17
    // So visible portion of active display starts at line 22-17=5 of the active area
    // and ends at line 245-17=228, but capped at 240 → visible = rows 5-228 of framebuffer
    println!("\nFor reference: if visible area = rows 8..232 (224 lines), content coverage:");
    let mut has_content_in_crop = false;
    for y in 8..232 {
        let mut non_bg = 0;
        for x in 0..256 {
            let pixel = frame[y * 256 + x];
            if pixel != bg_color {
                non_bg += 1;
            }
        }
        if non_bg > 0 {
            has_content_in_crop = true;
        }
    }
    println!("  Has content: {}", has_content_in_crop);

    // Try different crop ranges
    for (top, name) in [
        (0, "0..224"),
        (8, "8..232"),
        (14, "14..238"),
        (16, "16..240"),
    ] {
        let bottom = top + 224;
        let bottom = bottom.min(240);
        let mut first = None;
        let mut last_r = None;
        for y in top..bottom {
            let mut non_bg = 0;
            for x in 0..256 {
                if frame[y * 256 + x] != bg_color {
                    non_bg += 1;
                }
            }
            if non_bg > 0 {
                if first.is_none() {
                    first = Some(y - top);
                }
                last_r = Some(y - top);
            }
        }
        println!(
            "  Crop {}: first_content={:?} last_content={:?}",
            name, first, last_r
        );
    }

    Ok(())
}
