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
        if let Some(_f) = emu.take_frame() {
            frames += 1;
        }
    }

    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let mwr = emu.bus.vdc_register(0x09).unwrap_or(0);
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();

    println!("BXR=0x{:04X} ({}) BYR=0x{:04X} ({})", bxr, bxr, byr, byr);
    println!("MWR=0x{:04X} map={}x{}", mwr, map_w, map_h);

    // Calculate what the current code does
    let effective_y_scroll = ((byr as i32 + 1) & 0x01FF) as i32;
    let y_origin_bias = -(0x40 as i32); // -64
    let step_y = 16; // default zoom

    println!("\nCurrent formula:");
    println!(
        "  effective_y_scroll = (BYR + 1) & 0x1FF = {}",
        effective_y_scroll
    );
    println!("  y_origin_bias = {}", y_origin_bias);
    println!(
        "  For row 0: sample_y = ({} + {}) * 16 + {} * 0 >> 4 = {}",
        effective_y_scroll,
        y_origin_bias,
        step_y,
        effective_y_scroll + y_origin_bias
    );
    println!(
        "  For row 63: sample_y = {}",
        effective_y_scroll + y_origin_bias + 63
    );
    println!(
        "  For row 64: sample_y = {}",
        effective_y_scroll + y_origin_bias + 64
    );

    println!("\nExpected (BYR + row):");
    println!("  For row 0: sample_y = BYR = {}", byr);
    println!("  For row 63: sample_y = {}", byr as i32 + 63);
    println!("  For row 64: sample_y = {}", byr as i32 + 64);

    println!(
        "\nDifference: current - expected = {}",
        (effective_y_scroll + y_origin_bias) - byr as i32
    );

    Ok(())
}
