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

    let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    println!("CR  (R05) = {:#06X}", cr);
    println!("MWR (R09) = {:#06X}", mwr);
    println!("MWR bits [5:4] = {} → width code", (mwr >> 4) & 3);
    println!("MWR bit [6] = {} → height code", (mwr >> 6) & 1);
    println!("Map dimensions = {:?}", emu.bus.vdc_map_dimensions());

    // Check VRAM at flat address vs page address for BAT row 8, col 5
    let (map_w, _) = emu.bus.vdc_map_dimensions();
    let flat_addr = 8 * map_w + 5;
    println!("\nBAT row 8, col 5:");
    println!(
        "  Flat addr (row*width+col) = {:#06X}: entry = {:#06X}",
        flat_addr,
        emu.bus.vdc_vram_word(flat_addr as u16)
    );

    // Page-based for comparison
    let page_cols = (map_w / 32).max(1);
    let page_x = 5 / 32;
    let page_y = 8 / 32;
    let in_page_x = 5 % 32;
    let in_page_y = 8 % 32;
    let page_index = page_y * page_cols + page_x;
    let page_addr = page_index * 0x400 + in_page_y * 32 + in_page_x;
    println!(
        "  Page addr = {:#06X}: entry = {:#06X}",
        page_addr,
        emu.bus.vdc_vram_word(page_addr as u16)
    );

    Ok(())
}
