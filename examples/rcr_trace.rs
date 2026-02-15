/// Trace RCR interrupts and per-scanline scroll values to diagnose split-screen.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0u64;

    // Get to gameplay
    while frames < 2000 {
        emu.tick();
        if let Some(_f) = emu.take_frame() {
            frames += 1;
            let press_run = matches!(
                frames,
                100..=110
                    | 200..=210
                    | 300..=310
                    | 400..=410
                    | 500..=510
                    | 600..=610
                    | 700..=710
                    | 800..=810
            );
            if press_run {
                emu.bus.set_joypad_input(0x7F);
            } else {
                emu.bus.set_joypad_input(0xFF);
            }
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Run to frame 3000, but on last frame dump per-scanline scroll
    while frames < 2999 {
        emu.tick();
        if let Some(_f) = emu.take_frame() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Get VDC state
    let rcr_raw = emu.bus.vdc_register(0x06).unwrap_or(0);
    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    let cr = emu.bus.vdc_register(0x05).unwrap_or(0);
    let (map_w, map_h) = emu.bus.vdc_map_dimensions();

    println!("Before frame render:");
    println!("  RCR=0x{:04X} ({})", rcr_raw, rcr_raw);
    println!("  BXR={} BYR={}", bxr, byr);
    println!("  CR=0x{:04X} (RCR IRQ enabled: {})", cr, cr & 0x0004 != 0);
    println!("  Map={}x{}", map_w, map_h);

    // Now run ONE more frame and capture per-line scroll
    let frame = loop {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            frames += 1;
            break f;
        }
    };

    println!("\nAfter frame {}:", frames);
    let rcr_raw = emu.bus.vdc_register(0x06).unwrap_or(0);
    let bxr = emu.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emu.bus.vdc_register(0x08).unwrap_or(0);
    println!("  RCR=0x{:04X} BXR={} BYR={}", rcr_raw, bxr, byr);

    // Dump per-scanline scroll values used during rendering
    println!("\nPer-line scroll values for rendered frame:");
    let mut prev_bxr = 0xFFFFu16;
    let mut prev_byr = 0xFFFFu16;
    for row in 0..224 {
        let line = emu.bus.vdc_line_state_index_for_row(row);
        let (lx, ly) = emu.bus.vdc_scroll_line(line);
        if lx != prev_bxr || ly != prev_byr {
            println!(
                "  Row {:3} (line {:3}): BXR={:4} BYR={:4}",
                row, line, lx, ly
            );
            prev_bxr = lx;
            prev_byr = ly;
        }
    }

    // Also trace what the VDC IRQ state looks like
    println!("\nVDC status bits: 0x{:02X}", emu.bus.vdc_status_bits());

    Ok(())
}
