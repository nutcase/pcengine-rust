#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::bus::{VDC_STATUS_BUSY, VDC_STATUS_DS, VDC_STATUS_RCR, VDC_STATUS_VBL};
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0;
    while frames < 150 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Check RCR register (0x06) value
    let rcr_reg = emu.bus.vdc_register(0x06).unwrap_or(0);
    let cr_reg = emu.bus.vdc_register(0x05).unwrap_or(0);
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);
    let status = emu.bus.vdc_status_bits();

    println!("RCR register (0x06): {:#06X} ({})", rcr_reg, rcr_reg);
    println!("CR register (0x05): {:#06X}", cr_reg);
    println!("VPR (0x0C): {:#06X}", vpr);
    println!("VDW (0x0D): {:#06X}", vdw);
    println!("VCR (0x0E): {:#06X}", vcr);
    println!("VDC status: {:#04X}", status);
    println!(
        "  VBL={} DS={} RCR={} BUSY={}",
        (status & VDC_STATUS_VBL) != 0,
        (status & VDC_STATUS_DS) != 0,
        (status & VDC_STATUS_RCR) != 0,
        (status & VDC_STATUS_BUSY) != 0
    );

    // Compute active window
    let vsw = vpr & 0x001F;
    let vds = (vpr >> 8) & 0x00FF;
    let active_start = vsw + vds;
    let vdw_lines = (vdw & 0x01FF) + 1;
    println!(
        "VSW={} VDS={} active_start={} VDW_lines={}",
        vsw, vds, active_start, vdw_lines
    );

    // Check what scanline RCR target maps to
    let rcr_target = rcr_reg & 0x03FF;
    println!("RCR target: {:#06X} ({})", rcr_target, rcr_target);
    if rcr_target >= 0x40 && rcr_target <= 0x0146 {
        let relative = rcr_target - 0x40;
        let line = (active_start as u16 + relative) % 263;
        println!("RCR maps to scanline {} (active counter base 0x40)", line);
    } else if rcr_target < 263 {
        println!("RCR maps to absolute scanline {}", rcr_target);
    } else {
        println!("RCR target out of range - no match");
    }

    Ok(())
}
