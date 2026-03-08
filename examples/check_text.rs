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
    while frames < 300 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    // Full BAT dump for text rows on page 0
    println!("=== BAT text rows (page 0, cols 0-31) ===");
    for row in 20..28u16 {
        print!("  Row {:2}: ", row);
        for col in 0..32u16 {
            let addr = row * 64 + col;
            let entry = emu.bus.vdc_vram_word(addr);
            let tile = entry & 0x07FF;
            let pal = (entry >> 12) & 0x0F;
            if tile != 0x100 && tile != 0x000 {
                print!("{:03X}p{:X} ", tile, pal);
            } else {
                print!("..... ");
            }
        }
        println!();
    }

    // Check if tile patterns for text characters exist in VRAM
    // Tile 0x131 ('1'), 0x150 ('P'), etc.
    println!("\n=== Checking text tile patterns ===");
    for &(label, tile_id) in &[
        ("tile 0x131 '1'", 0x131u16),
        ("tile 0x150 'P'", 0x150u16),
        ("tile 0x148 'H'", 0x148u16),
        ("tile 0x100 (blank?)", 0x100u16),
        ("tile 0x13D '('", 0x13Du16),
    ] {
        let base = tile_id as usize * 16;
        let mut all_zero = true;
        print!("  {} at VRAM {:04X}: ", label, base);
        for row in 0..8 {
            let w0 = emu.bus.vdc_vram_word((base + row) as u16);
            let w1 = emu.bus.vdc_vram_word((base + row + 8) as u16);
            if w0 != 0 || w1 != 0 {
                all_zero = false;
            }
        }
        println!(
            "{}",
            if all_zero {
                "ALL ZERO (no pattern!)"
            } else {
                "has data"
            }
        );
    }

    // Dump the actual pattern for tile 0x150 'P'
    println!("\n=== Tile 0x150 'P' pattern ===");
    let base = 0x150 * 16;
    for row in 0..8 {
        let w01 = emu.bus.vdc_vram_word((base + row) as u16);
        let w23 = emu.bus.vdc_vram_word((base + row + 8) as u16);
        let plane0 = (w01 & 0xFF) as u8;
        let plane1 = ((w01 >> 8) & 0xFF) as u8;
        let plane2 = (w23 & 0xFF) as u8;
        let plane3 = ((w23 >> 8) & 0xFF) as u8;

        print!("  Row {}: ", row);
        for bit in (0..8).rev() {
            let p0 = (plane0 >> bit) & 1;
            let p1 = (plane1 >> bit) & 1;
            let p2 = (plane2 >> bit) & 1;
            let p3 = (plane3 >> bit) & 1;
            let pixel = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
            if pixel == 0 {
                print!(".");
            } else {
                print!("{:X}", pixel);
            }
        }
        println!("  ({:04X} {:04X})", w01, w23);
    }

    // Check visible area and rendering
    let byr = emu.bus.vdc_scroll_line(0).1;
    println!("\n=== Display layout ===");
    println!(
        "BYR = {:04X} ({} pixels, tile offset {})",
        byr,
        byr,
        byr / 8
    );
    println!("Text rows in BAT: 20-26");
    println!(
        "Expected display Y for row 22: {} pixels",
        (22u16.wrapping_sub(byr / 8)) * 8
    );
    println!(
        "Expected display Y for row 24: {} pixels",
        (24u16.wrapping_sub(byr / 8)) * 8
    );
    println!(
        "Expected display Y for row 26: {} pixels",
        (26u16.wrapping_sub(byr / 8)) * 8
    );

    // Check palette for text (palette 5)
    println!("\n=== Palette 5 (text palette) ===");
    // CRM palette data - check if palette 5 has non-black colors
    // Palette 5 starts at BG palette entry 5*16 = 80
    // The palette is in CRAM (Color RAM)
    // We can check via the rendered frame
    // But let's check CRAM directly
    // CRAM is 512 entries (256 BG + 256 sprite), each 9-bit
    // Actually, CRAM stores up to 512 entries at 16-bit each

    // Check VDC visible lines
    if let Some(r12) = emu.bus.vdc_register(0x0C) {
        let vds = (r12 >> 8) as u8;
        let vsw = (r12 & 0xFF) as u8;
        println!("VDC R12 (VDW/VSR): raw={:04X} VDS={} VSW={}", r12, vds, vsw);
    }
    if let Some(r0d) = emu.bus.vdc_register(0x0D) {
        let vdw = r0d & 0x01FF;
        let vcr = (r0d >> 8) as u8;
        println!("VDC R13 (VDE/VCR): raw={:04X} VDW={} VCR={}", r0d, vdw, vcr);
    }

    // Check what render_frame_from_vram() sees
    // Actually, let's check visible_lines computation
    if let Some(r0d) = emu.bus.vdc_register(0x0D) {
        let vdw = r0d & 0x01FF;
        println!("\nVisible lines (VDW+1) = {}", vdw + 1);
    }

    Ok(())
}
