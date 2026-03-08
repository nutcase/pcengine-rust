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

    // Check what's mapped at the BIOS area ($E000-$FFFF)
    println!("=== Memory map at BIOS area ===");
    for i in 0..8 {
        let mpr = emu.bus.mpr(i);
        let start = i * 0x2000;
        println!(
            "  MPR[{}] = {:02X} → ${:04X}-${:04X}",
            i,
            mpr,
            start,
            start + 0x1FFF
        );
    }

    // Check what's at common BIOS entry points
    let bios_addrs: &[(u16, &str)] = &[
        (0xE000, "BIOS start"),
        (0xE003, "VDC init?"),
        (0xE009, "ex_colorcmd?"),
        (0xE00C, "ex_bgon?"),
        (0xE00F, "ex_bgoff?"),
        (0xE018, "set_font?"),
        (0xE01B, "font_load?"),
        (0xE03C, "ex_getfnt?"),
        (0xE03F, "ex_setfnt?"),
        (0xE042, "ex_vsync?"),
        (0xE048, "ex_scrwin?"),
    ];

    println!("\n=== BIOS entry point bytes ===");
    for &(addr, name) in bios_addrs {
        let b0 = emu.bus.read(addr);
        let b1 = emu.bus.read(addr + 1);
        let b2 = emu.bus.read(addr + 2);
        let opcode_name = match b0 {
            0x20 => "JSR",
            0x4C => "JMP",
            0x60 => "RTS",
            0x40 => "RTI",
            0x6C => "JMP()",
            0xEA => "NOP",
            0x00 => "BRK",
            _ => "???",
        };
        println!(
            "  ${:04X} ({:12}): {:02X} {:02X} {:02X} [{}]",
            addr, name, b0, b1, b2, opcode_name
        );
    }

    // Run to frame 130 (just before graphics loading) and check PC
    let mut frames = 0;
    let mut last_frame = None;

    // Track execution in the $E000-$FFFF range
    let mut bios_call_count = 0u64;
    let mut last_bios_pc = 0u16;
    let mut ticks = 0u64;

    while frames < 300 {
        let pc = emu.cpu.pc;
        if pc >= 0xE000 {
            bios_call_count += 1;
            if bios_call_count <= 20 || (bios_call_count % 1000 == 0) {
                last_bios_pc = pc;
                println!(
                    "Frame {}: PC in BIOS area: ${:04X} (opcode {:02X}, call #{})",
                    frames,
                    pc,
                    emu.bus.read(pc),
                    bios_call_count
                );
            }
        }

        emu.tick();
        ticks += 1;

        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;

            if frames == 130 || frames == 140 || frames == 150 || frames == 300 {
                println!("\n--- Frame {} (tick {}) ---", frames, ticks);
                println!("  PC=${:04X} SP=${:02X}", emu.cpu.pc, emu.cpu.sp);
                println!(
                    "  BIOS calls so far: {}, last at ${:04X}",
                    bios_call_count, last_bios_pc
                );

                // Check font H
                let base = 0x148u16 * 16;
                let w0 = emu.bus.vdc_vram_word(base);
                let is_font = (w0 & 0xFF) == 0x66 && (w0 >> 8) == 0x00;
                println!("  Font 'H' w0={:04X} (is_font={})", w0, is_font);
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Total BIOS area executions: {}", bios_call_count);

    Ok(())
}
