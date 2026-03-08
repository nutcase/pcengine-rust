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

    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    let hsr = emu.bus.vdc_register(0x0A).unwrap_or(0);
    let hdr = emu.bus.vdc_register(0x0B).unwrap_or(0);
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);

    println!("BXR (R07) = {:#06X}", bxr);
    println!("BYR (R08) = {:#06X}", byr);
    println!("MWR (R09) = {:#06X}", mwr);
    println!("CR  (R05) = {:#06X}", cr);
    println!("HSR (R0A) = {:#06X}", hsr);
    println!("HDR (R0B) = {:#06X}", hdr);
    println!("VPR (R0C) = {:#06X}", vpr);
    println!("VDW (R0D) = {:#06X}", vdw);
    println!("VCR (R0E) = {:#06X}", vcr);

    let hsw = hsr & 0x1F;
    let hds = (hsr >> 8) & 0x7F;
    let hdw = hdr & 0x7F;
    let hde = (hdr >> 8) & 0x7F;
    println!(
        "\nHorizontal: HSW={} HDS={} HDW={} HDE={}",
        hsw, hds, hdw, hde
    );
    println!("  Display width = (HDW+1)*8 = {} pixels", (hdw + 1) * 8);

    let vsw = vpr & 0x1F;
    let vds = (vpr >> 8) & 0xFF;
    let active_h = (vdw & 0x1FF) + 1;
    let vcr_val = vcr & 0xFF;
    println!(
        "Vertical: VSW={} VDS={} VDW={} VCR={}",
        vsw, vds, active_h, vcr_val
    );

    println!("\nMap dimensions: {:?}", emu.bus.vdc_map_dimensions());

    // Check frame pixel data at known positions
    // First generate one more frame to check
    while frames < 151 {
        emu.tick();
        if let Some(frame) = emu.take_frame() {
            frames += 1;
            // Check a few pixels at the title area
            // Active row 30 should be in the title area
            let frame_row = 17 + 13; // vsw+vds + active_row 13 (BYR=51, tile_row=8 starts here)
            println!("\nFrame row {} (active_row 13) pixels 32-47:", frame_row);
            for x in 32..48 {
                let pixel = frame[frame_row * 256 + x];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                print!("{:02X}{:02X}{:02X} ", r, g, b);
            }
            println!();

            // Also check a few rows of the lower half
            let frame_row2 = 17 + 100; // active_row 100
            println!("Frame row {} (active_row 100) pixels 0-15:", frame_row2);
            for x in 0..16 {
                let pixel = frame[frame_row2 * 256 + x];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                print!("{:02X}{:02X}{:02X} ", r, g, b);
            }
            println!();
        }
    }

    Ok(())
}
