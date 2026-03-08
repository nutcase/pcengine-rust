#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Trace VCE palette entry 0 changes during a frame.
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

    // Settle for 3 frames
    for _ in 0..3 {
        emu.bus.set_joypad_input(0xFF);
        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
    }

    // Now trace one frame, recording VCE palette 0 changes
    emu.bus.set_joypad_input(0xFF);
    let mut prev_pal0 = emu.bus.vce_palette_rgb(0x00);
    let mut tick_count = 0u64;
    let mut changes = Vec::new();

    // Also track a few other palette entries
    let mut prev_pal_f0 = emu.bus.vce_palette_rgb(0xF0); // palette bank 15 entry 0

    loop {
        emu.tick();
        tick_count += 1;

        let cur_pal0 = emu.bus.vce_palette_rgb(0x00);
        let cur_pal_f0 = emu.bus.vce_palette_rgb(0xF0);

        if cur_pal0 != prev_pal0 {
            changes.push((tick_count, "pal[0]", prev_pal0, cur_pal0, emu.cpu.pc));
            prev_pal0 = cur_pal0;
        }
        if cur_pal_f0 != prev_pal_f0 {
            changes.push((tick_count, "pal[F0]", prev_pal_f0, cur_pal_f0, emu.cpu.pc));
            prev_pal_f0 = cur_pal_f0;
        }

        if emu.take_frame().is_some() {
            break;
        }
    }

    println!("VCE palette changes during frame ({} ticks):", tick_count);
    for (tick, name, old, new, pc) in &changes {
        println!(
            "  tick {:6}: {} RGB({},{},{}) -> RGB({},{},{})  PC={:04X}",
            tick,
            name,
            (old >> 16) & 0xFF,
            (old >> 8) & 0xFF,
            old & 0xFF,
            (new >> 16) & 0xFF,
            (new >> 8) & 0xFF,
            new & 0xFF,
            pc
        );
    }
    if changes.is_empty() {
        println!("  (no changes)");
    }

    Ok(())
}
