#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    let mut frames = 0;
    let mut last_frame = None;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }
    let frame = last_frame.unwrap();

    let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
    let vsw = (vpr & 0x001F) as usize;
    let vds = ((vpr >> 8) & 0x00FF) as usize;
    let frame_active_row = vsw + vds;
    println!("VSW={vsw} VDS={vds} frame_active_row={frame_active_row}");

    // Option A: No padding, start from frame_active_row, show 224 active lines
    let w = 256usize;
    let mut file = File::create("katoken_nopad.ppm")?;
    writeln!(file, "P6\n{w} 224\n255")?;
    for y in 0..224usize {
        let fy = frame_active_row + y;
        for x in 0..w {
            let p = if fy < 240 { frame[fy * w + x] } else { 0 };
            let r = ((p >> 16) & 0xFF) as u8;
            let g = ((p >> 8) & 0xFF) as u8;
            let b = (p & 0xFF) as u8;
            file.write_all(&[r, g, b])?;
        }
    }
    println!("wrote katoken_nopad.ppm (224 active lines from row {frame_active_row})");
    Ok(())
}
