#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Dump VDC register state after loading save state.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let state_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "states/Kato-chan & Ken-chan (Japan).slot1.state".to_string());

    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.load_state_from_file(&state_path)?;

    // Run a few frames
    for _ in 0..3 {
        emu.bus.set_joypad_input(0xFF);
        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
    }

    println!("VDC Registers:");
    for idx in 0..0x14 {
        if let Some(val) = emu.bus.vdc_register(idx) {
            println!("  R{:02X} = 0x{:04X} ({})", idx, val, val);
        }
    }

    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vsw = vpr & 0x001F;
    let vds = (vpr >> 8) & 0x00FF;
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0) & 0x01FF;
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0) & 0x00FF;
    println!("\nVertical timing:");
    println!("  VSW = {} (V sync width)", vsw);
    println!("  VDS = {} (V display start)", vds);
    println!("  VDW = {} (V display width -> {} lines)", vdw, vdw + 1);
    println!("  VCR = {} (V display end)", vcr);
    println!("  active_start_line = VSW + VDS = {}", vsw + vds);

    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    let rcr = emu.bus.vdc_register(0x06).unwrap_or(0);
    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
    println!("\nControl/Scroll:");
    println!(
        "  CR  = 0x{:04X} (bits: spr={} bg={} rcr_ie={} vbl_ie={})",
        cr,
        (cr >> 6) & 1, // sprite enable
        (cr >> 7) & 1, // BG enable
        (cr >> 2) & 1, // RCR interrupt enable
        (cr >> 3) & 1
    ); // VBlank interrupt enable
    println!("  RCR = 0x{:04X} ({})", rcr, rcr);
    println!("  BXR = {} BYR = {}", bxr, byr);
    println!("  MWR = 0x{:04X}", mwr);

    let hsr = emu.bus.vdc_register(0x0A).unwrap_or(0);
    let hdr = emu.bus.vdc_register(0x0B).unwrap_or(0);
    let hsw = hsr & 0x1F;
    let hds = (hsr >> 8) & 0x7F;
    let hdw = hdr & 0x7F;
    let hde = (hdr >> 8) & 0x7F;
    println!("\nHorizontal timing:");
    println!("  HSW={} HDS={} HDW={} HDE={}", hsw, hds, hdw, hde);

    // Show which output rows are in the active window
    println!("\nActive window rows:");
    let mut first_active = None;
    let mut last_active = None;
    for row in 0..240 {
        let line = emu.bus.vdc_line_state_index_for_row(row);
        let (bxr, byr) = emu.bus.vdc_scroll_line(line);
        let valid = emu.bus.vdc_scroll_line_valid(line);
        if valid {
            if first_active.is_none() {
                first_active = Some(row);
            }
            last_active = Some(row);
        }
        if row < 5 || (row >= 33 && row <= 38) || row >= 236 {
            println!(
                "  row {:3} -> line {:3} BXR={:4} BYR={:4} valid={}",
                row, line, bxr, byr, valid
            );
        }
    }
    println!("  First active row: {:?}", first_active);
    println!("  Last active row: {:?}", last_active);

    // Check HUD tile data in VRAM
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();
    println!("\nBAT map: {}x{}", map_w, map_h);

    // Check what tiles are at the HUD rows
    // With BXR=0 and BYR=0, tile_row=0, tile_col starts from 0
    println!("\nBAT row 0 (HUD first tile row):");
    for col in 0..map_w.min(16) {
        let addr = col; // simplified - row 0, col
        let entry = emu.bus.vdc_vram_word(addr as u16);
        let tile_id = entry & 0x07FF;
        let pal = (entry >> 12) & 0x0F;
        print!(" {:03X}p{:X}", tile_id, pal);
    }
    println!();

    println!("BAT row 1:");
    for col in 0..map_w.min(16) {
        let addr = map_w + col;
        let entry = emu.bus.vdc_vram_word(addr as u16);
        let tile_id = entry & 0x07FF;
        let pal = (entry >> 12) & 0x0F;
        print!(" {:03X}p{:X}", tile_id, pal);
    }
    println!();

    // Dump the tile pattern for tile ID at BAT(0,0)
    let entry0 = emu.bus.vdc_vram_word(0);
    let tile_id0 = (entry0 & 0x07FF) as u16;
    println!("\nTile #{:03X} pattern data:", tile_id0);
    let tile_base = tile_id0 * 16;
    for row in 0..8u16 {
        let chr0 = emu.bus.vdc_vram_word(tile_base + row);
        let chr1 = emu.bus.vdc_vram_word(tile_base + 8 + row);
        // Decode to pixel values
        let mut pixels = [0u8; 8];
        for bit in 0..8 {
            let shift = 7 - bit;
            let p0 = ((chr0 >> shift) & 1) as u8;
            let p1 = ((chr0 >> (shift + 8)) & 1) as u8;
            let p2 = ((chr1 >> shift) & 1) as u8;
            let p3 = ((chr1 >> (shift + 8)) & 1) as u8;
            pixels[bit] = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
        }
        println!(
            "  row {}: chr0={:04X} chr1={:04X} -> pixels={:?}",
            row, chr0, chr1, pixels
        );
    }

    Ok(())
}
