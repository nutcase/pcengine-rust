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

    // Check font tiles immediately after reset (before any game code runs)
    println!("=== Font tiles after reset (before game) ===");
    for &(ch, tid) in &[
        ('H', 0x148u16),
        ('I', 0x149),
        ('0', 0x130),
        ('U', 0x155),
        (' ', 0x140),
    ] {
        let base = tid as usize * 16;
        print!("  '{}' tile {:03X}: ", ch, tid);
        for row in 0..8usize {
            let w = emu.bus.vdc_vram_word((base + row) as u16);
            let p0 = (w & 0xFF) as u8;
            for bit in (0..8).rev() {
                if (p0 >> bit) & 1 != 0 {
                    print!("#");
                } else {
                    print!(".");
                }
            }
            print!("|");
        }
        println!();
    }

    // Run 10 frames (font should still be there)
    let mut frames = 0;
    while frames < 10 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    println!("\n=== Font tiles after 10 frames ===");
    for &(ch, tid) in &[
        ('H', 0x148u16),
        ('I', 0x149),
        ('0', 0x130),
        ('S', 0x153),
        ('P', 0x150),
    ] {
        let base = tid as usize * 16;
        print!("  '{}' tile {:03X}: ", ch, tid);
        for row in 0..8usize {
            let w = emu.bus.vdc_vram_word((base + row) as u16);
            let p0 = (w & 0xFF) as u8;
            for bit in (0..8).rev() {
                if (p0 >> bit) & 1 != 0 {
                    print!("#");
                } else {
                    print!(".");
                }
            }
            print!("|");
        }
        println!();
    }

    // Generate a frame image to visually verify
    while frames < 300 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    // Check if font survived (might be overwritten by background graphics)
    println!("\n=== Font tiles after 300 frames ===");
    for &(ch, tid) in &[('H', 0x148u16), ('I', 0x149), ('0', 0x130)] {
        let base = tid as usize * 16;
        print!("  '{}' tile {:03X}: ", ch, tid);
        for row in 0..8usize {
            let w = emu.bus.vdc_vram_word((base + row) as u16);
            let p0 = (w & 0xFF) as u8;
            for bit in (0..8).rev() {
                if (p0 >> bit) & 1 != 0 {
                    print!("#");
                } else {
                    print!(".");
                }
            }
            print!("|");
        }
        println!();
    }

    Ok(())
}
