#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
/// Find the first frame where VRAM[0x1400] gets corrupted (changes from non-zero to zero).
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

    let max_frames = 3000u64;
    let mut frame_count = 0u64;
    let mut prev_vram_1400 = 0u16;

    // Also track a few other tile-row-0 addresses
    let watch_addrs: Vec<u16> = vec![0x1000, 0x1040, 0x1080, 0x1400, 0x1440];

    for _ in 0..max_frames {
        let joypad = if frame_count % 120 < 5 {
            0xFF ^ 0x08
        } else if frame_count % 60 < 3 {
            0xFF ^ 0x01
        } else {
            0xFF
        };
        emu.bus.set_joypad_input(joypad);

        loop {
            emu.tick();
            if emu.take_frame().is_some() {
                break;
            }
        }
        frame_count += 1;

        let cur_vram_1400 = emu.bus.vdc_vram_word(0x1400);

        // Report transitions
        if cur_vram_1400 != prev_vram_1400 {
            println!(
                "Frame {:5}: VRAM[1400] changed {:04X} -> {:04X}  (in_vblank={}, scanline={})",
                frame_count,
                prev_vram_1400,
                cur_vram_1400,
                emu.bus.vdc_in_vblank(),
                emu.bus.vdc_current_scanline()
            );

            // If corrupted (went to 0 from non-zero), also show other addresses
            if cur_vram_1400 == 0 && prev_vram_1400 != 0 {
                println!("  *** CORRUPTION DETECTED ***");
                for &addr in &watch_addrs {
                    println!(
                        "    VRAM[{:04X}] = {:04X}",
                        addr,
                        emu.bus.vdc_vram_word(addr)
                    );
                }
                // Don't break - let's see if it recovers
            }
        }

        prev_vram_1400 = cur_vram_1400;

        // Show periodic status for first few frames and at load points
        if frame_count <= 5 || frame_count % 500 == 0 {
            println!("Frame {:5}: VRAM[1400]={:04X}", frame_count, cur_vram_1400);
        }
    }

    println!(
        "\nFinal VRAM[1400]={:04X} at frame {}",
        emu.bus.vdc_vram_word(0x1400),
        frame_count
    );
    Ok(())
}
