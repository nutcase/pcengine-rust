use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 400u64;
    let max_ticks = 70_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut prev_pc = 0u16;

    // Track specific function calls
    let mut start_music_calls = 0u32; // $D0E8 (start music wrapper)
    let mut stop_music_calls = 0u32; // $D0BF (stop/command wrapper)
    let mut snd_4000_calls = 0u32; // $4000 (banked start entry)
    let mut snd_400c_calls = 0u32; // $400C (init routine after JMP)
    let mut snd_4006_calls = 0u32; // $4006 (vblank handler)

    // Track timer port writes
    let mut prev_timer_counter = emu.bus.read_io(0x0C00);
    let mut prev_timer_control = emu.bus.read_io(0x0C01);

    // Track what A register value is at key entry points
    let mut a_at_d0e8: Vec<(u64, u64, u8)> = Vec::new(); // (frame, tick, A)
    let mut a_at_4000: Vec<(u64, u64, u8)> = Vec::new();
    let mut a_at_400c: Vec<(u64, u64, u8)> = Vec::new();

    // Track ZP $47 (timer reload value used by sound driver)
    let mut prev_zp47 = 0u8;

    // Monitor IRQ mask register changes
    let mut prev_irq_mask = 0u8;

    while frames < target_frames && total_ticks < max_ticks {
        let current_pc = emu.cpu.pc;
        emu.tick();
        total_ticks += 1;

        let new_pc = emu.cpu.pc;

        // Detect function entries (when PC transitions TO the address)
        if new_pc != prev_pc {
            match new_pc {
                0xD0E8 if prev_pc != 0xD0E8 => {
                    start_music_calls += 1;
                    a_at_d0e8.push((frames, total_ticks, emu.cpu.a));
                    println!(
                        "[tick {:8} frame {:3}] $D0E8 start_music called, A=${:02X} (song #{})",
                        total_ticks, frames, emu.cpu.a, emu.cpu.a
                    );
                }
                0xD0BF if prev_pc != 0xD0BF => {
                    stop_music_calls += 1;
                    println!(
                        "[tick {:8} frame {:3}] $D0BF stop/cmd called",
                        total_ticks, frames
                    );
                }
                0x4000 if prev_pc != 0x4000 => {
                    snd_4000_calls += 1;
                    a_at_4000.push((frames, total_ticks, emu.cpu.a));
                    if snd_4000_calls <= 10 {
                        println!(
                            "[tick {:8} frame {:3}] $4000 banked start entry, A=${:02X}",
                            total_ticks, frames, emu.cpu.a
                        );
                    }
                }
                0x400C if prev_pc != 0x400C => {
                    snd_400c_calls += 1;
                    a_at_400c.push((frames, total_ticks, emu.cpu.a));
                    if snd_400c_calls <= 10 {
                        let zp47 = emu.bus.read_zero_page(0x47);
                        let zpaa = emu.bus.read_zero_page(0xAA);
                        println!(
                            "[tick {:8} frame {:3}] $400C init routine, A=${:02X}, ZP$47=${:02X}, ZP$AA=${:02X}",
                            total_ticks, frames, emu.cpu.a, zp47, zpaa
                        );
                    }
                }
                0x4006 if prev_pc != 0x4006 => {
                    snd_4006_calls += 1;
                }
                _ => {}
            }
        }

        // Monitor timer port changes (check every 100 ticks for performance)
        if total_ticks % 100 == 0 {
            let tc = emu.bus.read_io(0x0C00);
            let tctrl = emu.bus.read_io(0x0C01);
            if tctrl != prev_timer_control {
                println!(
                    "[tick {:8} frame {:3}] Timer control changed: ${:02X} -> ${:02X} (enabled={})",
                    total_ticks,
                    frames,
                    prev_timer_control,
                    tctrl,
                    tctrl & 1 != 0
                );
                prev_timer_control = tctrl;
            }
            if tc != prev_timer_counter && (tctrl & 1 == 0) {
                // Only log counter changes when timer is disabled (otherwise it auto-decrements)
                println!(
                    "[tick {:8} frame {:3}] Timer counter set: {} -> {}",
                    total_ticks, frames, prev_timer_counter, tc
                );
                prev_timer_counter = tc;
            } else {
                prev_timer_counter = tc;
            }
        }

        // Monitor ZP $47 changes
        if total_ticks % 1000 == 0 {
            let zp47 = emu.bus.read_zero_page(0x47);
            if zp47 != prev_zp47 {
                println!(
                    "[tick {:8} frame {:3}] ZP $47 (timer value) changed: ${:02X} -> ${:02X}",
                    total_ticks, frames, prev_zp47, zp47
                );
                prev_zp47 = zp47;
            }
        }

        // Monitor IRQ mask changes
        if total_ticks % 100 == 0 {
            let irq_mask = emu.bus.read_io(0x1402);
            if irq_mask != prev_irq_mask {
                println!(
                    "[tick {:8} frame {:3}] IRQ mask changed: ${:02X} -> ${:02X} (timer_disabled={}, irq1_disabled={}, irq2_disabled={})",
                    total_ticks,
                    frames,
                    prev_irq_mask,
                    irq_mask,
                    irq_mask & 0x04 != 0,
                    irq_mask & 0x02 != 0,
                    irq_mask & 0x01 != 0
                );
                prev_irq_mask = irq_mask;
            }
        }

        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    println!("\n=== Summary ({} frames) ===", frames);
    println!("$D0E8 (start music wrapper): {} calls", start_music_calls);
    println!("$D0BF (stop/command wrapper): {} calls", stop_music_calls);
    println!("$4000 (banked start entry):   {} calls", snd_4000_calls);
    println!("$400C (init routine):         {} calls", snd_400c_calls);
    println!("$4006 (vblank handler):       {} calls", snd_4006_calls);

    println!("\nA values at $D0E8 calls:");
    for &(frame, tick, a) in &a_at_d0e8 {
        println!(
            "  frame {} tick {}: A=${:02X} (>= $40: {})",
            frame,
            tick,
            a,
            a >= 0x40
        );
    }

    println!("\nA values at $400C calls:");
    for &(frame, tick, a) in &a_at_400c {
        println!(
            "  frame {} tick {}: A=${:02X} (>= $40: {})",
            frame,
            tick,
            a,
            a >= 0x40
        );
    }

    println!("\nFinal timer state:");
    println!("  Counter: {}", emu.bus.read_io(0x0C00));
    println!(
        "  Control: ${:02X} (enabled={})",
        emu.bus.read_io(0x0C01),
        emu.bus.read_io(0x0C01) & 1 != 0
    );
    println!("  IRQ mask: ${:02X}", emu.bus.read_io(0x1402));
    println!("  ZP $47: ${:02X}", emu.bus.read_zero_page(0x47));

    Ok(())
}
