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

    // Dump raw VRAM words for key text tiles
    for &(ch, tid) in &[
        ('H', 0x148u16),
        ('I', 0x149),
        ('0', 0x130),
        ('U', 0x155),
        (' ', 0x140),
    ] {
        let base = tid as usize * 16;
        println!("Tile {:03X} '{}' raw VRAM words:", tid, ch);
        for row in 0..8usize {
            let w0 = emu.bus.vdc_vram_word((base + row) as u16);
            let w1 = emu.bus.vdc_vram_word((base + row + 8) as u16);
            println!(
                "  Row {}: plane01={:04X} plane23={:04X}  (p0={:02X} p1={:02X} p2={:02X} p3={:02X})",
                row,
                w0,
                w1,
                w0 & 0xFF,
                (w0 >> 8) & 0xFF,
                w1 & 0xFF,
                (w1 >> 8) & 0xFF
            );
        }
        println!();
    }

    // Check if there's a consistent pattern - are all planes identical?
    println!("=== Checking if font data is in wrong byte order ===");
    for &(ch, tid) in &[('H', 0x148u16), ('I', 0x149), ('S', 0x153)] {
        let base = tid as usize * 16;
        println!(
            "Tile {:03X} '{}' bit patterns (each plane separately):",
            tid, ch
        );
        for plane in 0..4 {
            print!("  Plane {}: ", plane);
            for row in 0..8usize {
                let word_offset = if plane < 2 { row } else { row + 8 };
                let w = emu.bus.vdc_vram_word((base + word_offset) as u16);
                let byte_val = if plane % 2 == 0 {
                    (w & 0xFF) as u8
                } else {
                    ((w >> 8) & 0xFF) as u8
                };
                for bit in (0..8).rev() {
                    if (byte_val >> bit) & 1 != 0 {
                        print!("#");
                    } else {
                        print!(".");
                    }
                }
                print!("|");
            }
            println!();
        }
    }

    // Also check: what does the font look like if we read only plane 0?
    // If it's a real font, plane 0 alone might show the character shape
    println!("\n=== Font using only plane 0 ===");
    for &(ch, tid) in &[
        ('H', 0x148u16),
        ('I', 0x149),
        ('S', 0x153),
        ('P', 0x150),
        ('0', 0x130),
        ('1', 0x131),
        ('R', 0x152),
        ('N', 0x14E),
    ] {
        let base = tid as usize * 16;
        print!("  '{}'({:03X}): ", ch, tid);
        for row in 0..8usize {
            let w0 = emu.bus.vdc_vram_word((base + row) as u16);
            let p0 = (w0 & 0xFF) as u8;
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

    // And what if the font data byte order is: plane0 in HIGH byte?
    println!("\n=== Font using high byte of word 0 (plane 1 position) ===");
    for &(ch, tid) in &[
        ('H', 0x148u16),
        ('I', 0x149),
        ('S', 0x153),
        ('P', 0x150),
        ('0', 0x130),
        ('1', 0x131),
        ('R', 0x152),
        ('N', 0x14E),
    ] {
        let base = tid as usize * 16;
        print!("  '{}'({:03X}): ", ch, tid);
        for row in 0..8usize {
            let w0 = emu.bus.vdc_vram_word((base + row) as u16);
            let p1 = ((w0 >> 8) & 0xFF) as u8;
            for bit in (0..8).rev() {
                if (p1 >> bit) & 1 != 0 {
                    print!("#");
                } else {
                    print!(".");
                }
            }
            print!("|");
        }
        println!();
    }

    // What if font data is loaded one byte per VRAM word?
    // (wrong: loading 8-bit font data as if each byte is a 16-bit word)
    // Check if we interpret the low bytes of consecutive words as font rows
    println!("\n=== Interpreting low bytes of words 0-7 as font (std 8-row) ===");
    for &(ch, tid) in &[('H', 0x148u16), ('P', 0x150), ('0', 0x130)] {
        let base = tid as usize * 16;
        print!("  '{}'({:03X}): ", ch, tid);
        for row in 0..8usize {
            let w = emu.bus.vdc_vram_word((base + row) as u16);
            let byte_val = (w & 0xFF) as u8;
            for bit in (0..8).rev() {
                if (byte_val >> bit) & 1 != 0 {
                    print!("#");
                } else {
                    print!(".");
                }
            }
            print!("|");
        }
        println!();
    }

    // What if the font data is loaded with TIA but byte order swapped?
    // TIA writes: byte0 → port $0002 (low), byte1 → port $0003 (high)
    // If font ROM layout is: row0, row1, row2... but TIA puts them as (row0=low, row1=high) of word0
    // Then plane0 = even bytes, plane1 = odd bytes
    // Let's check if plane0 (even bytes from TIA) forms recognizable characters
    println!("\n=== MAWR value at current state ===");
    let mawr = emu.bus.vdc_register(0x00).unwrap_or(0);
    println!("MAWR = {:04X}", mawr);

    // Check increment step (from control register)
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    let inc = match (cr >> 11) & 0x03 {
        0 => 1,
        1 => 32,
        2 => 64,
        _ => 128,
    };
    println!("Control Register = {:04X}, increment step = {}", cr, inc);

    Ok(())
}
