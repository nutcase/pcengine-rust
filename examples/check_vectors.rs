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

    // Read interrupt vectors
    for (name, addr) in [
        ("NMI", 0xFFFCu16),
        ("RESET", 0xFFFE),
        ("IRQ2/VDC", 0xFFF6),
        ("IRQ1", 0xFFF8),
        ("Timer", 0xFFFA),
    ] {
        let lo = emu.bus.read(addr);
        let hi = emu.bus.read(addr + 1);
        let vec = u16::from_le_bytes([lo, hi]);
        println!("{:10}: ${:04X} -> ${:04X}", name, addr, vec);
    }

    Ok(())
}
