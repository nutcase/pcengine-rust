use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 300u64;
    let max_ticks = 50_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;

    // Track sound driver calls at $D094
    let mut sound_driver_calls = 0u64;
    let mut last_sound_calls = 0u64;
    let mut prev_pc = 0u16;

    // Track VBlank ISR entries at $FB83
    let mut vbl_isr_entries = 0u64;
    let mut last_vbl_entries = 0u64;

    // Track the actual VBL processing (when bit 5 of VDC status IS set)
    // The ISR at $FB83 does LDA $0000, AND #$20, BEQ skip
    // If it falls through (VBL bit set), the next instruction is at ~$FB92
    let mut vbl_processed = 0u64;
    let mut last_vbl_processed = 0u64;

    // Watch for PC at $D094 (sound driver entry) and $FB92 (VBL processing)
    let irq1_vector = 0xFB83u16;

    while frames < target_frames && total_ticks < max_ticks {
        let current_pc = emu.cpu.pc;
        emu.tick();
        total_ticks += 1;

        // Detect sound driver entry
        if emu.cpu.pc == 0xD094 && prev_pc != 0xD094 {
            sound_driver_calls += 1;
        }

        // Detect VBlank ISR entry
        if emu.cpu.pc == irq1_vector && prev_pc != irq1_vector {
            vbl_isr_entries += 1;
        }

        // Detect VBL processing (PC reaches $FB92 which is after the BEQ skip)
        if emu.cpu.pc == 0xFB92 && prev_pc != 0xFB92 {
            vbl_processed += 1;
        }

        prev_pc = current_pc;

        if emu.take_frame().is_some() {
            frames += 1;

            if frames % 60 == 0 || frames == 1 {
                let sd_calls = sound_driver_calls - last_sound_calls;
                let vbl_entries = vbl_isr_entries - last_vbl_entries;
                let vbl_proc = vbl_processed - last_vbl_processed;

                println!(
                    "Frame {}: sound_driver={} ({}/frame), VBL_ISR={}, VBL_processed={}",
                    frames,
                    sd_calls,
                    if frames > 1 {
                        sd_calls as f64 / 60.0
                    } else {
                        sd_calls as f64
                    },
                    vbl_entries,
                    vbl_proc
                );

                last_sound_calls = sound_driver_calls;
                last_vbl_entries = vbl_isr_entries;
                last_vbl_processed = vbl_processed;
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    println!("\n=== Summary ({} frames) ===", frames);
    println!(
        "Sound driver ($D094) calls: {} ({:.2}/frame)",
        sound_driver_calls,
        sound_driver_calls as f64 / frames as f64
    );
    println!(
        "VBL ISR ($FB83) entries: {} ({:.2}/frame)",
        vbl_isr_entries,
        vbl_isr_entries as f64 / frames as f64
    );
    println!(
        "VBL processing ($FB92): {} ({:.2}/frame)",
        vbl_processed,
        vbl_processed as f64 / frames as f64
    );

    Ok(())
}
