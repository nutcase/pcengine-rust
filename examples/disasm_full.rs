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

    // Dump raw bytes from $E200 to $E3A0
    println!("Raw bytes $E200-$E3A0:");
    for base in (0xE200u16..=0xE3A0).step_by(16) {
        print!("${:04X}:", base);
        for i in 0..16 {
            let addr = base + i;
            let byte = emu.bus.read(addr);
            print!(" {:02X}", byte);
        }
        println!();
    }

    // Also dump the RAM variables the ISR uses
    println!("\nRAM variables:");
    for addr in [
        0x2200u16, 0x2201, 0x2204, 0x2205, 0x2206, 0x2207, 0x2208, 0x2209, 0x220A, 0x220B, 0x221A,
        0x221B, 0x221C, 0x221D, 0x221E, 0x2218, 0x2219, 0x2220,
    ] {
        let val = emu.bus.read(addr);
        println!("  ${:04X} = {:02X} ({})", addr, val, val);
    }

    // Check VDC register 8 (BYR) value
    println!(
        "\nVDC BYR (reg 8): {:04X}",
        emu.bus.vdc_register(0x08).unwrap_or(0)
    );

    Ok(())
}
