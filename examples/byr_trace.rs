#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Trace BYR/BXR writes during frame execution.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let state_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "states/Kato-chan & Ken-chan (Japan).slot1.state".to_string());

    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.load_state_from_file(&state_path)?;

    for _ in 0..3 {
        emu.bus.set_joypad_input(0xFF);
        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
    }

    let mut prev_bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let mut prev_byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let mut tick_count = 0u64;
    let mut writes = Vec::new();

    emu.bus.set_joypad_input(0xFF);
    loop {
        emu.tick();
        tick_count += 1;
        let cur_bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
        let cur_byr = emu.bus.vdc_register(0x08).unwrap_or(0);
        if cur_bxr != prev_bxr || cur_byr != prev_byr {
            writes.push((tick_count, prev_bxr, prev_byr, cur_bxr, cur_byr, emu.cpu.pc));
            prev_bxr = cur_bxr;
            prev_byr = cur_byr;
        }
        if emu.take_frame().is_some() {
            break;
        }
    }

    println!("BXR/BYR changes during frame ({} ticks):", tick_count);
    for (tick, old_bxr, old_byr, new_bxr, new_byr, pc) in &writes {
        println!(
            "  tick {:6}: BXR {:4}->{:4} BYR {:4}->{:4} (PC={:04X})",
            tick, old_bxr, old_byr, new_bxr, new_byr, pc
        );
    }
    println!("Total changes: {}", writes.len());
    Ok(())
}
