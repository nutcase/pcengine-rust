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

    // Check specific tiles that the user reports as wrong: P, B, !
    let tiles_to_check: &[(u16, &str)] = &[
        (0x150, "P"),
        (0x142, "B"),
        (0x15C, "! (game uses \\ position)"),
        (0x148, "H"), // reference: should look correct
        (0x121, "! (ASCII position)"),
    ];

    for &(tile_id, label) in tiles_to_check {
        let base = tile_id * 16;
        println!(
            "\nTile 0x{:03X} '{}' (VRAM base 0x{:04X}):",
            tile_id, label, base
        );
        for row in 0..8u16 {
            let w01 = emu.bus.vdc_vram_word(base + row);
            let w23 = emu.bus.vdc_vram_word(base + 8 + row);
            let p0 = w01 & 0xFF;
            let p1 = (w01 >> 8) & 0xFF;
            let p2 = w23 & 0xFF;
            let p3 = (w23 >> 8) & 0xFF;

            // Show plane 0 pattern as visual
            let mut line = String::new();
            for bit in (0..8).rev() {
                let b0 = (p0 >> bit) & 1;
                let b1 = (p1 >> bit) & 1;
                let b2 = (p2 >> bit) & 1;
                let b3 = (p3 >> bit) & 1;
                let pixel = b0 | (b1 << 1) | (b2 << 2) | (b3 << 3);
                if pixel == 0 {
                    line.push('.');
                } else if pixel == 0xF {
                    line.push('#');
                } else {
                    line.push_str(&format!("{:X}", pixel));
                }
            }
            println!("  row {}: {} (w01={:04X} w23={:04X})", row, line, w01, w23);
        }
    }

    Ok(())
}
