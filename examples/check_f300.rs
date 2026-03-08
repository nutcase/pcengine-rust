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

    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let vsw = vpr & 0x001F;
    let vds = (vpr >> 8) & 0x00FF;
    println!(
        "BXR={} BYR={} VSW={} VDS={} VDW={}",
        bxr,
        byr,
        vsw,
        vds,
        vdw & 0x01FF
    );

    // Check where "HISCORE" text is in framebuffer
    let frame = {
        let mut emu2 = Emulator::new();
        emu2.load_hucard(&rom)?;
        emu2.reset();
        let mut frames = 0;
        let mut last_frame = None;
        while frames < 300 {
            emu2.tick();
            if let Some(f) = emu2.take_frame() {
                last_frame = Some(f);
                frames += 1;
            }
        }
        last_frame.unwrap()
    };

    // "HISCORE" text: look for rows with font tiles (palette 5)
    // These would have specific colors. Let me find the first row that
    // has many scattered non-bg, non-building pixels in the lower half
    let bg = 0x242491u32;
    println!("\nRow analysis (looking for text rows):");
    for y in 90..180 {
        let mut non_bg = 0;
        for x in 80..200 {
            if frame[y * 256 + x] != bg {
                non_bg += 1;
            }
        }
        if non_bg > 5 && non_bg < 100 {
            println!("  Row {:3}: {:3} non-bg pixels (center region)", y, non_bg);
        }
    }

    // Find "© 1987" row
    println!("\nLooking for © 1987 text:");
    for y in 150..224 {
        let mut non_bg = 0;
        for x in 60..200 {
            if frame[y * 256 + x] != bg {
                non_bg += 1;
            }
        }
        if non_bg > 5 && non_bg < 80 {
            println!("  Row {:3}: {:3} non-bg pixels", y, non_bg);
        }
    }

    Ok(())
}
