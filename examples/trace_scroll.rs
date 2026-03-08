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

    // Run 300 frames, trace RAM values at RCR handler entry
    println!("=== Tracing scroll RAM at RCR handler E31A ===");
    let mut samples = 0;
    while frames < 300 {
        let prev_pc = emu.cpu.pc;
        emu.tick();

        // Entry to RCR handler body
        if emu.cpu.pc == 0xE31A && prev_pc != 0xE31A {
            if samples < 10 || (samples % 50 == 0) {
                let bxr_lo = emu.bus.read(0x221B);
                let bxr_hi = emu.bus.read(0x221C);
                let byr_lo = emu.bus.read(0x221D);
                let byr_hi = emu.bus.read(0x221E);
                let scroll_x_offset = emu.bus.read(0x221F);
                let scroll_y_offset = emu.bus.read(0x2220);
                let frame_ctr = emu.bus.read(0x2218);
                let flag_2217 = emu.bus.read(0x2217);
                let rcr_ctr = emu.bus.read(0x2219);

                println!(
                    "Frame {:3} (ctr={:02X}): BXR_base={:02X}{:02X} BYR_base={:02X}{:02X} \
                          X_off={:02X} Y_off={:02X} $2217={:02X} RCR_ctr={:02X}",
                    frames,
                    frame_ctr,
                    bxr_hi,
                    bxr_lo,
                    byr_hi,
                    byr_lo,
                    scroll_x_offset,
                    scroll_y_offset,
                    flag_2217,
                    rcr_ctr
                );
            }
            samples += 1;
        }

        if emu.take_frame().is_some() {
            frames += 1;
        }
    }
    println!("Total RCR handler entries: {}", samples);

    // Also check the per-line scroll values for a few frames
    println!("\n=== Per-line scroll values (last frame) ===");
    for line in [0usize, 10, 20, 30, 35, 36, 40, 50, 100, 150, 200, 230, 240] {
        if emu.bus.vdc_scroll_line_valid(line) {
            let (sx, sy) = emu.bus.vdc_scroll_line(line);
            println!(
                "  Line {:3}: BXR={:04X}({:3}) BYR={:04X}({:3})",
                line, sx, sx, sy, sy
            );
        } else {
            println!("  Line {:3}: not latched", line);
        }
    }

    // Check what E14D subroutine does by looking at the code
    println!("\n=== E14D subroutine code ===");
    for i in 0..64u16 {
        let a = 0xE14D + i;
        let b = emu.bus.read(a);
        if i % 16 == 0 {
            print!("  {:04X}:", a);
        }
        print!(" {:02X}", b);
        if i % 16 == 15 || i == 63 {
            println!();
        }
    }

    // And the main loop code at E200-E220
    println!("\n=== Main loop E200-E220 ===");
    for i in 0..32u16 {
        let a = 0xE200 + i;
        let b = emu.bus.read(a);
        if i % 16 == 0 {
            print!("  {:04X}:", a);
        }
        print!(" {:02X}", b);
        if i % 16 == 15 || i == 31 {
            println!();
        }
    }

    // Check what $2201 (CR shadow) is
    let cr_shadow = emu.bus.read(0x2201);
    println!("\n$2201 (CR shadow) = {:02X}", cr_shadow);
    println!("  bit 4 (RCR IRQ enable) = {}", (cr_shadow & 0x10) != 0);

    // Check what RCR register R06 was set to
    // Dump VDC R06 writes
    if let Some(r06) = emu.bus.vdc_register(6) {
        println!(
            "VDC R06 (RCR) = {:04X} (target scanline = {})",
            r06,
            if r06 >= 0x40 {
                (r06 - 0x40) as i32
            } else {
                -(0x40i32 - r06 as i32)
            }
        );
    }

    // What's actually in BAT around the text area?
    println!("\n=== BAT around text area ===");
    // Page 0 row 24-26 (where text might appear)
    for row in [22, 24, 26] {
        print!("  Row {:2}, cols 0-15 (page0): ", row);
        for col in 0..16u16 {
            let addr = (row * 64 + col) as usize;
            if addr < 0x10000 {
                let entry = emu.bus.vdc_vram_word(addr as u16);
                let tile = entry & 0x07FF;
                if tile != 0x100 && tile != 0x000 {
                    print!("{:03X} ", tile);
                } else {
                    print!("... ");
                }
            }
        }
        println!();
    }

    // Page 1 row 24-26
    for row in [22, 24, 26] {
        print!("  Row {:2}, cols 32-47 (page1): ", row);
        for col in 32..48u16 {
            let addr = (row * 64 + col) as usize;
            if addr < 0x10000 {
                let entry = emu.bus.vdc_vram_word(addr as u16);
                let tile = entry & 0x07FF;
                if tile != 0x100 && tile != 0x000 {
                    print!("{:03X} ", tile);
                } else {
                    print!("... ");
                }
            }
        }
        println!();
    }

    Ok(())
}
