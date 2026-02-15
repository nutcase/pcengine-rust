use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let max_ticks = 500_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut prev_pc = 0u16;

    // Track MPR changes via TAM instruction
    let mut mpr_changes: Vec<(u64, u16, [u8; 8])> = Vec::new();
    let mut prev_mpr = emu.bus.mpr_array();

    // Track when PC enters suspicious ranges
    let mut first_io_exec = false;

    // Track JSR/JMP targets
    let mut call_trace: Vec<(u64, u16, u16, &'static str)> = Vec::new(); // (tick, from_pc, to_pc, type)

    // Track subroutine depth
    let mut jsr_stack: Vec<(u64, u16)> = Vec::new(); // (tick, return_addr)

    while frames < 3 && total_ticks < max_ticks {
        let current_pc = emu.cpu.pc;
        let current_opcode = emu.bus.read(current_pc);
        emu.tick();
        total_ticks += 1;
        let new_pc = emu.cpu.pc;

        // Check for MPR changes
        let new_mpr = emu.bus.mpr_array();
        if new_mpr != prev_mpr {
            mpr_changes.push((total_ticks, current_pc, new_mpr));
            // Log MPR changes involving high page numbers (>= 64)
            for i in 0..8 {
                if new_mpr[i] != prev_mpr[i] {
                    let page = new_mpr[i];
                    if page < 0xF8 {
                        // Skip RAM/hardware pages
                        println!(
                            "[tick {:6} PC=${:04X}] MPR{}: ${:02X} -> ${:02X} (ROM page {})",
                            total_ticks, current_pc, i, prev_mpr[i], page, page
                        );
                    }
                }
            }
            prev_mpr = new_mpr;
        }

        // Track when PC enters IO page ($0000-$1FFF with MPR0=$FF)
        if !first_io_exec && new_pc < 0x2000 && emu.bus.mpr_array()[0] == 0xFF && frames > 0 {
            first_io_exec = true;
            println!("\n!!! CPU executing from IO page!");
            println!("  tick: {}", total_ticks);
            println!(
                "  previous PC: ${:04X} (opcode ${:02X})",
                current_pc, current_opcode
            );
            println!("  new PC: ${:04X}", new_pc);
            println!(
                "  Status: ${:02X} (I={})",
                emu.cpu.status,
                emu.cpu.status & 0x04 != 0
            );
            println!("  SP: ${:02X}", emu.cpu.sp);
            println!(
                "  A=${:02X} X=${:02X} Y=${:02X}",
                emu.cpu.a, emu.cpu.x, emu.cpu.y
            );

            // Dump stack
            print!("  Stack:");
            for i in 0..16 {
                let sp_addr = 0x2100u16 + (emu.cpu.sp.wrapping_add(1).wrapping_add(i as u8)) as u16;
                print!(" {:02X}", emu.bus.read(sp_addr));
            }
            println!();

            // Show the last 20 call trace entries
            println!("\n  Last 20 PC transitions before IO exec:");
            let start = if call_trace.len() > 20 {
                call_trace.len() - 20
            } else {
                0
            };
            for &(tick, from, to, kind) in &call_trace[start..] {
                println!(
                    "    [tick {:6}] ${:04X} → ${:04X} ({})",
                    tick, from, to, kind
                );
            }
        }

        // Track significant PC transitions (JSR, JMP, RTS, RTI, BRK, interrupts)
        if new_pc != current_pc.wrapping_add(1)
            && new_pc != current_pc.wrapping_add(2)
            && new_pc != current_pc.wrapping_add(3)
            && new_pc != current_pc
        {
            let kind = match current_opcode {
                0x20 => "JSR",
                0x4C | 0x6C | 0x7C => "JMP",
                0x60 => "RTS",
                0x40 => "RTI",
                0x00 => "BRK",
                0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0 | 0x80 | 0x0F | 0x1F
                | 0x2F | 0x3F | 0x4F | 0x5F | 0x6F | 0x7F | 0x8F | 0x9F | 0xAF | 0xBF | 0xCF
                | 0xDF | 0xEF | 0xFF => "Bxx",
                0x44 | 0x54 => "BSR",
                _ => "???",
            };

            // Only track non-branch transitions (JSR, JMP, RTS, RTI, BRK, ???)
            if !kind.starts_with('B') || kind == "BRK" || kind == "BSR" {
                call_trace.push((total_ticks, current_pc, new_pc, kind));
            }
        }

        prev_pc = current_pc;

        if emu.take_frame().is_some() {
            frames += 1;
            println!("=== Frame {} at tick {} ===", frames, total_ticks);
        }

        if emu.cpu.halted {
            break;
        }
    }

    // Summary
    println!("\n=== MPR changes during boot ===");
    let mut high_page_used = false;
    for &(tick, pc, mpr) in &mpr_changes {
        for i in 0..8 {
            if mpr[i] >= 64 && mpr[i] < 0xF8 {
                high_page_used = true;
                println!(
                    "  [tick {:6} PC=${:04X}] MPR{} = ${:02X} (page {})",
                    tick, pc, i, mpr[i], mpr[i]
                );
            }
        }
    }
    if !high_page_used {
        println!("  No ROM pages >= 64 used during boot");
    }

    Ok(())
}
