#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Trace all CR (Control Register) value changes from boot.
/// Find when the high byte first becomes 0x10 (increment=64).
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());

    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frame_count = 0u64;
    let mut prev_cr = emu.bus.vdc_control_register();
    let mut cr_changes = 0u64;
    let mut first_high_10 = false;

    println!(
        "Initial CR={:04X} (incr={})",
        prev_cr,
        match (prev_cr >> 11) & 3 {
            0 => 1,
            1 => 32,
            2 => 64,
            _ => 128,
        }
    );

    for _ in 0..200 {
        let joypad = if frame_count % 120 < 5 {
            0xFF ^ 0x08
        } else if frame_count % 60 < 3 {
            0xFF ^ 0x01
        } else {
            0xFF
        };
        emu.bus.set_joypad_input(joypad);

        let mut tick_count = 0u64;
        loop {
            emu.tick();
            tick_count += 1;

            let cur_cr = emu.bus.vdc_control_register();
            if cur_cr != prev_cr {
                cr_changes += 1;
                let hi_byte = (cur_cr >> 8) as u8;
                let was_hi_10 = (prev_cr >> 8) as u8 == 0x10;
                let now_hi_10 = hi_byte == 0x10;
                let marker = if now_hi_10 && !was_hi_10 {
                    first_high_10 = true;
                    " <<< HIGH BYTE SET TO 0x10"
                } else if was_hi_10 && !now_hi_10 {
                    " <<< HIGH BYTE CLEARED FROM 0x10"
                } else {
                    ""
                };

                // Show all changes for the first 20 frames, then only significant ones
                if frame_count < 20 || marker.len() > 0 || cr_changes <= 50 {
                    let incr = match (cur_cr >> 11) & 3 {
                        0 => 1,
                        1 => 32,
                        2 => 64,
                        _ => 128,
                    };
                    println!(
                        "Frame {:5} tick {:6}: CR {:04X} -> {:04X} (incr={}){}",
                        frame_count, tick_count, prev_cr, cur_cr, incr, marker
                    );
                }
                prev_cr = cur_cr;
            }

            if emu.take_frame().is_some() {
                break;
            }
        }
        frame_count += 1;
    }

    println!(
        "\nTotal CR changes in {} frames: {}",
        frame_count, cr_changes
    );
    println!("Final CR={:04X}", prev_cr);
    Ok(())
}
