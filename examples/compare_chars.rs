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

    // Text is at Y=204 for PUSH RUN BUTTON!
    // BAT row 24, cols 7-24 → 18 chars
    // "P U S H _ _ R U N _ _ B U T T O N !"
    //  7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24
    // Each tile = 8px wide, screen X offset depends on BXR
    // BXR=0, HDW=32 tiles, HDS=2 → display start = (2+1)*8 = 24 px from left
    // But let me just find the exact X by looking for white pixels

    // Find the ! character - last non-space char in PUSH RUN BUTTON! row
    // Y=204-210 has white pixels
    println!("=== Row Y=204-210 (PUSH RUN BUTTON!) pixel dump ===");
    for y in 204..211 {
        print!("Y={}: ", y);
        for x in 40..220 {
            let p = pixels[y * width + x];
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r < 0x20 && g < 0x20 && b < 0x20 {
                print!(".");
            } else {
                print!("-");
            }
        }
        println!();
    }

    // Also dump HISCORE row for O comparison (in "SCORE")
    println!("\n=== Row Y=188-194 (SCORE) pixel dump ===");
    for y in 188..195 {
        print!("Y={}: ", y);
        for x in 40..220 {
            let p = pixels[y * width + x];
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            if r > 200 && g > 200 && b > 200 {
                print!("W");
            } else if r < 0x20 && g < 0x20 && b < 0x20 {
                print!(".");
            } else {
                print!("-");
            }
        }
        println!();
    }

    Ok(())
}
