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
    let mut last_frame = None;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }
    let frame = last_frame.unwrap();

    // Tile 0x201 is at BAT row 8, col 5
    // BYR=51, so tile map Y=64 (BAT row 8 line 0) = active row 13 = frame row 30
    // Column 5 → pixel x = 40
    // Expected pixels for row 0: [0, 0, D, 4, 4, D, D, 1]
    // With palette 3: [bg, bg, 2424B6, FF6D00, FF6D00, 2424B6, 2424B6, FFFF91]

    println!("Frame row 30 (active row 13, tile map Y=64), x=40-47:");
    println!("Expected: bg bg 2424B6 FF6D00 FF6D00 2424B6 2424B6 FFFF91");
    print!("Actual:  ");
    for x in 40..48 {
        print!(" {:06X}", frame[30 * 256 + x]);
    }
    println!();

    // Check more rows of tile 0x201
    for row_in_tile in 0..8 {
        let fy = 30 + row_in_tile;
        print!("Row {} (frame row {}): ", row_in_tile, fy);
        for x in 40..48 {
            let p = frame[fy * 256 + x];
            print!(" {:06X}", p);
        }
        println!();
    }

    // Also check BXR (X scroll) which affects column positioning
    println!(
        "\nBXR={} BYR={}",
        emu.bus.vdc_register(0x07).unwrap_or(0),
        emu.bus.vdc_register(0x08).unwrap_or(0)
    );

    // Check the full row of pixel data at frame row 30, x=0-80
    println!("\nFrame row 30, x=0-80 (hex RGB):");
    for x in 0..80 {
        if x % 8 == 0 {
            print!("\n  x={:3}: ", x);
        }
        print!("{:06X} ", frame[30 * 256 + x]);
    }
    println!();

    Ok(())
}
