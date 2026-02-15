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

    // Track exact timing of PSG frequency changes per channel
    let mut prev_freq = [0u16; 6];
    let mut freq_change_frames: Vec<(u64, usize, u16, u16)> = Vec::new(); // (frame, ch, old_freq, new_freq)

    // Track ZP state at sound driver entry
    let mut driver_call_count = 0u32;
    let mut prev_zp_snapshot = [0u8; 256];
    let mut zp_change_count = [0u32; 256]; // How often each ZP byte changes between driver calls

    // Track code bytes at $D094
    let mut driver_code_dumped = false;

    // MPR state at driver entry
    let mut mpr_at_driver = [0u8; 8];

    while frames < target_frames && total_ticks < max_ticks {
        let current_pc = emu.cpu.pc;
        emu.tick();
        total_ticks += 1;

        // Detect sound driver entry at $D094
        if emu.cpu.pc == 0xD094 && prev_pc != 0xD094 {
            driver_call_count += 1;

            // Capture MPR state
            mpr_at_driver = emu.bus.mpr_array();

            // Capture ZP snapshot and diff with previous
            if driver_call_count > 1 {
                for i in 0..256 {
                    let val = emu.bus.read_zero_page(i as u8);
                    if val != prev_zp_snapshot[i] {
                        zp_change_count[i] += 1;
                    }
                    prev_zp_snapshot[i] = val;
                }
            } else {
                for i in 0..256 {
                    prev_zp_snapshot[i] = emu.bus.read_zero_page(i as u8);
                }
            }

            // Dump code bytes at $D094 (first time only)
            if !driver_code_dumped && driver_call_count == 1 {
                driver_code_dumped = true;
                println!("=== Sound driver code at $D094 ===");
                println!(
                    "MPR: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
                    mpr_at_driver[0],
                    mpr_at_driver[1],
                    mpr_at_driver[2],
                    mpr_at_driver[3],
                    mpr_at_driver[4],
                    mpr_at_driver[5],
                    mpr_at_driver[6],
                    mpr_at_driver[7]
                );
                print!("Code bytes: ");
                for offset in 0..64 {
                    let byte = emu.bus.read(0xD094 + offset);
                    print!("{:02X} ", byte);
                }
                println!();
            }

            // Log first 5 driver calls with ZP state
            if driver_call_count <= 5 {
                println!(
                    "\n--- Driver call #{} at frame {} tick {} ---",
                    driver_call_count, frames, total_ticks
                );
                println!(
                    "MPR: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
                    mpr_at_driver[0],
                    mpr_at_driver[1],
                    mpr_at_driver[2],
                    mpr_at_driver[3],
                    mpr_at_driver[4],
                    mpr_at_driver[5],
                    mpr_at_driver[6],
                    mpr_at_driver[7]
                );

                // Dump interesting ZP ranges (potential tempo counters)
                print!("ZP $00-$1F: ");
                for i in 0..0x20 {
                    print!("{:02X} ", emu.bus.read_zero_page(i));
                }
                println!();
                print!("ZP $20-$3F: ");
                for i in 0x20..0x40 {
                    print!("{:02X} ", emu.bus.read_zero_page(i));
                }
                println!();
                print!("ZP $40-$5F: ");
                for i in 0x40..0x60 {
                    print!("{:02X} ", emu.bus.read_zero_page(i));
                }
                println!();
                print!("ZP $60-$7F: ");
                for i in 0x60..0x80 {
                    print!("{:02X} ", emu.bus.read_zero_page(i));
                }
                println!();
            }
        }

        prev_pc = current_pc;

        if emu.take_frame().is_some() {
            frames += 1;

            // Check for frequency changes every frame
            for ch in 0..6 {
                let (freq, _ctrl, _bal, _noise) = emu.bus.psg_channel_info(ch);
                if freq != prev_freq[ch] && frames > 1 {
                    freq_change_frames.push((frames, ch, prev_freq[ch], freq));
                }
                prev_freq[ch] = freq;
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    // Report frequency change timeline
    println!("\n=== Frequency change timeline (frames 290-400) ===");
    for &(frame, ch, old_f, new_f) in &freq_change_frames {
        if frame >= 290 {
            let old_hz = if old_f > 0 {
                3_579_545.0 / (32.0 * old_f as f64)
            } else {
                0.0
            };
            let new_hz = if new_f > 0 {
                3_579_545.0 / (32.0 * new_f as f64)
            } else {
                0.0
            };
            println!(
                "  Frame {:4}: CH{} freq {:4} ({:7.1}Hz) -> {:4} ({:7.1}Hz)",
                frame, ch, old_f, old_hz, new_f, new_hz
            );
        }
    }

    // Find frequency change intervals for each channel
    println!("\n=== Note change intervals per channel (frames >= 300) ===");
    for ch in 0..6 {
        let changes: Vec<u64> = freq_change_frames
            .iter()
            .filter(|&&(f, c, _, _)| c == ch && f >= 300)
            .map(|&(f, _, _, _)| f)
            .collect();
        if changes.len() >= 2 {
            let intervals: Vec<u64> = changes.windows(2).map(|w| w[1] - w[0]).collect();
            let avg = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
            println!(
                "  CH{}: {} changes, avg interval = {:.1} frames ({:.1} Hz)",
                ch,
                changes.len(),
                avg,
                60.0 / avg
            );
            if intervals.len() <= 20 {
                println!("    Intervals: {:?}", intervals);
            } else {
                println!("    First 20 intervals: {:?}", &intervals[..20]);
            }
        } else {
            println!("  CH{}: {} changes (not enough data)", ch, changes.len());
        }
    }

    // Report most-changed ZP locations
    println!(
        "\n=== ZP locations that change between driver calls ({} calls) ===",
        driver_call_count
    );
    let mut zp_sorted: Vec<(usize, u32)> = zp_change_count
        .iter()
        .enumerate()
        .filter(|&(_, &count)| count > 0)
        .map(|(addr, &count)| (addr, count))
        .collect();
    zp_sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for &(addr, count) in zp_sorted.iter().take(30) {
        let rate = count as f64 / driver_call_count as f64;
        println!(
            "  ZP ${:02X}: changed {} times ({:.1}% of calls), current = ${:02X}",
            addr,
            count,
            rate * 100.0,
            emu.bus.read_zero_page(addr as u8)
        );
    }

    // Dump RAM at likely sound driver work area
    println!("\n=== RAM $2000-$2100 (work RAM page 1) ===");
    for row in 0..16 {
        let base = 0x2000 + row * 16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    Ok(())
}
