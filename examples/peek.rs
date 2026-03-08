#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::{env, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let path = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("Usage: peek <image.pce> <address_hex>");
            return Ok(());
        }
    };
    let addr = args
        .next()
        .map(|arg| u16::from_str_radix(arg.trim_start_matches("0x"), 16).unwrap_or(0))
        .unwrap_or(0xE000);

    let rom = fs::read(&path)?;
    let mut emulator = Emulator::new();
    let is_pce = Path::new(&path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);
    if is_pce {
        emulator.load_hucard(&rom)?;
    } else {
        emulator.load_program(0xC000, &rom);
    }
    emulator.reset();

    let start = addr & !0x000F;
    for offset in 0..0x40 {
        let cur = start.wrapping_add(offset);
        if offset % 0x10 == 0 {
            print!("\n{cur:04X}: ");
        }
        let byte = emulator.bus.read(cur);
        print!("{byte:02X} ");
    }
    println!();

    println!(
        "MPR: {:?}",
        (0..8).map(|i| emulator.bus.mpr(i)).collect::<Vec<_>>()
    );
    Ok(())
}
