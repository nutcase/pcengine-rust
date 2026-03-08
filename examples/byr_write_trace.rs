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
    while frames < 20 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
            if frames <= 3 || frames == 10 || frames == 15 || frames == 20 {
                let byr_reg = emu.bus.vdc_register(0x08).unwrap_or(0);
                let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
                let vsw = (vpr & 0x001F) as usize;
                let vds = ((vpr >> 8) & 0x00FF) as usize;
                let active_start = vsw + vds;
                println!(
                    "Frame {:3}: BYR_reg={} vsw={} vds={} active_start={}",
                    frames, byr_reg, vsw, vds, active_start
                );

                // Read per-line scroll_y values around the active area
                println!("  Per-line scroll_y (around active start):");
                for line in active_start.saturating_sub(2)..=(active_start + 10).min(262) {
                    let (sx, sy) = emu.bus.vdc_scroll_line(line);
                    let valid = emu.bus.vdc_scroll_line_valid(line);
                    let active_row = if line >= active_start {
                        line - active_start
                    } else {
                        999
                    };
                    println!(
                        "    line {:3} (active_row {:3}): scroll_y={:3} valid={} {}",
                        line,
                        active_row,
                        sy,
                        valid,
                        if line == active_start {
                            "<-- active start"
                        } else {
                            ""
                        }
                    );
                }
                // Also check a few lines in the middle
                println!("  Key active lines:");
                for active_row in [0, 10, 28, 29, 30, 50, 100, 200] {
                    let line = active_start + active_row;
                    if line < 263 {
                        let (_, sy) = emu.bus.vdc_scroll_line(line);
                        let valid = emu.bus.vdc_scroll_line_valid(line);
                        // With flat addressing, title at rows 4-9 = sample_y 32-79
                        // sample_y = scroll_y + active_row
                        let sample_y = sy as usize + active_row;
                        let in_title = sample_y >= 32 && sample_y < 80;
                        println!(
                            "    active_row {:3} (line {:3}): scroll_y={:3} sample_y={:3} {}",
                            active_row,
                            line,
                            sy,
                            sample_y,
                            if in_title { "TITLE" } else { "" }
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
