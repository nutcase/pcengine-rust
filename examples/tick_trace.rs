use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 420u64;
    let max_ticks = 75_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;

    // Track tick counters at $2A19+X for channels 0-11
    // These are in RAM page, accessed as $2A19 through MPR1
    // RAM offset = $2A19 - $2000 = $0A19
    let tick_counter_base = 0x2A19u16;
    let duration_base = 0x2A01u16; // Duration/next value base

    // Also track the PSG frequency for correlation
    let mut prev_freq = [0u16; 6];

    // Capture tick counter at each frame (after music starts ~frame 300)
    let mut tick_snapshots: Vec<(u64, [u8; 12], [u8; 12])> = Vec::new();

    while frames < target_frames && total_ticks < max_ticks {
        emu.tick();
        total_ticks += 1;

        if emu.take_frame().is_some() {
            frames += 1;

            // Capture tick counters and durations starting from frame 290
            if frames >= 290 {
                let mut ticks = [0u8; 12];
                let mut durs = [0u8; 12];
                for ch in 0..12 {
                    ticks[ch] = emu.bus.read(tick_counter_base + ch as u16);
                    durs[ch] = emu.bus.read(duration_base + ch as u16);
                }
                tick_snapshots.push((frames, ticks, durs));

                // Check for frequency changes
                for ch in 0..6 {
                    let (freq, ctrl, _, _) = emu.bus.psg_channel_info(ch);
                    if freq != prev_freq[ch] {
                        let hz = if freq > 0 {
                            3_579_545.0 / (32.0 * freq as f64)
                        } else {
                            0.0
                        };
                        println!(
                            "Frame {:3}: CH{} freq {:4} ({:6.1}Hz) tick_ctr={} dur={}",
                            frames, ch, freq, hz, ticks[ch], durs[ch]
                        );
                        prev_freq[ch] = freq;
                    }
                }
            }
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Print tick counter evolution for channels 0, 4, 5
    println!("\n=== Tick counter per frame (CH0=melody, CH4=drums, CH5=bass) ===");
    println!(
        "{:>5} | {:>4} {:>4} {:>4} {:>4} {:>4} {:>4} | {:>4} {:>4} {:>4} {:>4} {:>4} {:>4}",
        "Frame", "T0", "T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10", "T11"
    );
    for &(frame, ref ticks, _) in &tick_snapshots {
        if frame >= 340 && frame <= 365 {
            println!(
                "{:5} | {:4} {:4} {:4} {:4} {:4} {:4} | {:4} {:4} {:4} {:4} {:4} {:4}",
                frame,
                ticks[0],
                ticks[1],
                ticks[2],
                ticks[3],
                ticks[4],
                ticks[5],
                ticks[6],
                ticks[7],
                ticks[8],
                ticks[9],
                ticks[10],
                ticks[11]
            );
        }
    }

    // Print duration values for each channel
    println!("\n=== Duration values ($2A01+X) at last frame ===");
    if let Some(&(_, _, ref durs)) = tick_snapshots.last() {
        for ch in 0..12 {
            println!(
                "  Channel {:2}: duration = {} (${:02X})",
                ch, durs[ch], durs[ch]
            );
        }
    }

    // Dump the full work area $2A00-$2A80
    println!("\n=== Sound driver work area $2A00-$2A80 ===");
    for row in 0..8 {
        let base = 0x2A00u16 + (row * 16) as u16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col as u16));
        }
        println!();
    }

    // Also dump $2A80-$2B00
    println!("\n=== Sound driver work area $2A80-$2B00 ===");
    for row in 0..8 {
        let base = 0x2A80u16 + (row * 16) as u16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col as u16));
        }
        println!();
    }

    Ok(())
}
