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

    let mut frames = 0;
    while frames < 150 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Dump ALL VDC registers
    println!("VDC registers at frame 150:");
    let reg_names = [
        "R00 MAWR", "R01 MARR", "R02 VWR", "R03 ???", "R04 ???", "R05 CR", "R06 RCR", "R07 BXR",
        "R08 BYR", "R09 MWR", "R0A HSR", "R0B HDR", "R0C VPR", "R0D VDW", "R0E VCR", "R0F DCR",
        "R10 SOUR", "R11 DESR", "R12 LENR", "R13 SATB",
    ];
    for i in 0..20 {
        let val = emu.bus.vdc_register(i).unwrap_or(0);
        println!("  {}: {:#06X} ({})", reg_names[i], val, val);
    }

    // Parse timing registers
    let hsr = emu.bus.vdc_register(0x0A).unwrap_or(0);
    let hdr = emu.bus.vdc_register(0x0B).unwrap_or(0);
    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
    let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);

    let hsw = hsr & 0x1F;
    let hds = (hsr >> 8) & 0x7F;
    let hdw = hdr & 0x7F;
    let hde = (hdr >> 8) & 0x7F;
    let vsw = vpr & 0x1F;
    let vds = (vpr >> 8) & 0xFF;
    let vdw_val = vdw & 0x1FF;
    let vcr_val = vcr & 0xFF;

    println!("\nHorizontal timing:");
    println!("  HSW={} HDS={} HDW={} HDE={}", hsw, hds, hdw, hde);
    println!("  Active pixels: ({} + 1) * 8 = {}", hdw, (hdw + 1) * 8);

    println!("\nVertical timing:");
    println!("  VSW={} VDS={} VDW={} VCR={}", vsw, vds, vdw_val, vcr_val);
    println!(
        "  Total vertical: VSW+VDS+VDW+1+VCR = {}+{}+{}+1+{} = {}",
        vsw,
        vds,
        vdw_val,
        vcr_val,
        vsw as u32 + vds as u32 + vdw_val as u32 + 1 + vcr_val as u32
    );
    println!(
        "  Active start line: VSW+VDS = {} + {} = {}",
        vsw,
        vds,
        vsw + vds
    );
    println!("  Active lines: VDW+1 = {}", vdw_val + 1);
    println!(
        "  VBlank start: VSW+VDS+VDW+1 = {}",
        vsw + vds + vdw_val + 1
    );

    println!(
        "\nWith BYR={} and flat BAT addressing:",
        emu.bus.vdc_register(0x08).unwrap_or(0)
    );
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0) as usize;
    println!("  Display starts at sample_y = {}", byr);
    println!("  Display ends at sample_y = {}", byr + (vdw_val as usize));
    println!("  Title tiles (rows 4-9) at sample_y 32-79:");
    println!(
        "    First visible: sample_y {} = active_row {}",
        byr.max(32),
        if byr <= 32 { 32 - byr } else { 0 }
    );
    if byr > 32 {
        println!("    Top {} pixels of title are cut off", byr - 32);
    }

    // What does Mednafen do with these VDC settings?
    // In Mednafen, the output image starts from the first active line (no VDS padding)
    // The display shows VDW+1 active lines
    println!("\nOutput mapping:");
    println!("  If output starts at active_row 0 (no VDS padding): BYR=51 title at row 0-28");
    println!(
        "  If output includes VDS={} blank lines: title at row {}-{}",
        vds,
        vds + 0,
        vds as usize + 28
    );

    Ok(())
}
