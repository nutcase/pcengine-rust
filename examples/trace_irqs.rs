#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::bus::{VDC_STATUS_DS, VDC_STATUS_RCR, VDC_STATUS_VBL};
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Run to frame 148
    let mut frames = 0;
    while frames < 148 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Now trace detailed IRQ/status for frames 149-150
    let mut prev_status = emu.bus.vdc_status_bits();
    let mut tick_count = 0u64;
    let mut isr_entries = 0;

    while frames < 150 {
        let pc = emu.cpu.pc;
        let old_status = emu.bus.vdc_status_bits();
        let scanline = emu.bus.vdc_current_scanline();

        // Detect ISR entry: PC at $E2AB (start of ISR)
        if pc == 0xE2AB {
            let status = emu.bus.vdc_status_bits();
            isr_entries += 1;
            println!(
                "  ISR entry #{} at tick {} scanline {} VDC_status={:02X} (VBL={} DS={} RCR={})",
                isr_entries,
                tick_count,
                scanline,
                status,
                (status & VDC_STATUS_VBL) != 0,
                (status & VDC_STATUS_DS) != 0,
                (status & VDC_STATUS_RCR) != 0
            );
        }

        // Detect VDC status read (PC at $E2AF, which is LDA $0000)
        if pc == 0xE2AF {
            let status = emu.bus.vdc_status_bits();
            println!(
                "    Status READ at ${:04X} tick {} scanline {} status={:02X} (VBL={} DS={} RCR={})",
                pc,
                tick_count,
                scanline,
                status,
                (status & VDC_STATUS_VBL) != 0,
                (status & VDC_STATUS_DS) != 0,
                (status & VDC_STATUS_RCR) != 0
            );
        }

        let cycles = emu.tick();
        tick_count += cycles as u64;

        let new_status = emu.bus.vdc_status_bits();
        // Detect new status bits being raised
        let raised = new_status & !old_status;
        if raised & VDC_STATUS_RCR != 0 {
            let sl = emu.bus.vdc_current_scanline();
            println!(
                "  RCR raised at tick {} scanline {} PC=${:04X}",
                tick_count, sl, pc
            );
        }
        if raised & VDC_STATUS_VBL != 0 {
            let sl = emu.bus.vdc_current_scanline();
            println!(
                "  VBL raised at tick {} scanline {} PC=${:04X}",
                tick_count, sl, pc
            );
        }

        if let Some(_) = emu.take_frame() {
            frames += 1;
            println!("=== Frame {} ===", frames);
            tick_count = 0;
            isr_entries = 0;
        }
    }

    // Timer info not available via public API

    Ok(())
}
