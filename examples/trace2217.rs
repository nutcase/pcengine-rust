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

    let mut frames = 0u64;
    let mut last_val = emu.bus.read(0x2217);
    let mut step_count = 0u64;

    while frames < 60 {
        let prev_val = emu.bus.read(0x2217);
        let prev_pc = emu.cpu.pc;
        emu.tick();
        step_count += 1;
        let new_val = emu.bus.read(0x2217);
        if new_val != prev_val {
            println!(
                "step {:8}: $2217 changed {:02X}->{:02X}  PC was {:04X}  frame {}",
                step_count, prev_val, new_val, prev_pc, frames
            );
        }
        if emu.take_frame().is_some() {
            frames += 1;
        }
    }

    println!("Final $2217 = {:02X}", emu.bus.read(0x2217));

    Ok(())
}
