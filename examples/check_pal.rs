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
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Dump BG palettes 0-3
    println!("BG Palettes (VCE colors 0x000-0x03F):");
    for pal in 0..4 {
        print!("  Palette {:X}:", pal);
        for idx in 0..16 {
            let vce_idx = pal * 16 + idx;
            let rgb = emu.bus.vce_palette_rgb(vce_idx);
            print!(" {:06X}", rgb);
        }
        println!();
    }

    // Also dump sprite palette 0 (VCE 0x100-0x10F)
    println!("Sprite Palette 0 (VCE 0x100-0x10F):");
    for idx in 0..16 {
        let vce_idx = 0x100 + idx;
        let rgb = emu.bus.vce_palette_rgb(vce_idx);
        print!(" {:06X}", rgb);
    }
    println!();

    // Check specific tile data at VRAM for title tiles
    // Tile 0x201 at word address 0x201*16 = 0x2010
    println!("\nTitle tile 0x201 pattern data:");
    for row in 0..8 {
        let w_a = emu.bus.vdc_vram_word((0x2010 + row) as u16);
        let w_b = emu.bus.vdc_vram_word((0x2018 + row) as u16);
        // Decode pixels
        let mut pixels = [0u8; 8];
        for bit in 0..8 {
            let shift = 7 - bit;
            let p0 = ((w_a >> shift) & 1) as u8;
            let p1 = ((w_a >> (shift + 8)) & 1) as u8;
            let p2 = ((w_b >> shift) & 1) as u8;
            let p3 = ((w_b >> (shift + 8)) & 1) as u8;
            pixels[bit] = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
        }
        println!("  Row {}: {:X?} (A={:04X} B={:04X})", row, pixels, w_a, w_b);
    }

    // Tile 0x202
    println!("\nTitle tile 0x202 pattern data:");
    for row in 0..8 {
        let base = 0x202 * 16;
        let w_a = emu.bus.vdc_vram_word((base + row) as u16);
        let w_b = emu.bus.vdc_vram_word((base + 8 + row) as u16);
        let mut pixels = [0u8; 8];
        for bit in 0..8 {
            let shift = 7 - bit;
            let p0 = ((w_a >> shift) & 1) as u8;
            let p1 = ((w_a >> (shift + 8)) & 1) as u8;
            let p2 = ((w_b >> shift) & 1) as u8;
            let p3 = ((w_b >> (shift + 8)) & 1) as u8;
            pixels[bit] = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
        }
        println!("  Row {}: {:X?} (A={:04X} B={:04X})", row, pixels, w_a, w_b);
    }

    Ok(())
}
