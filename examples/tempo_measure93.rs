use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0u64;
    let mut total_ticks = 0u64;

    // Skip to frame 100
    while frames < 100 && total_ticks < 100_000_000 {
        emu.tick();
        total_ticks += 1;
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Track PSG channel frequency changes over 500 frames (~8.3s)
    let mut prev_freqs = [0u16; 6];
    let mut freq_changes = [0u64; 6];
    let mut active_frames = [0u64; 6];

    println!(
        "=== Tracking PSG changes from frame {} to {} ===",
        frames,
        frames + 500
    );
    println!("Frame-by-frame channel activity:\n");

    let start_frame = frames;
    let mut last_dump_frame = frames;

    while frames < start_frame + 500 && total_ticks < 200_000_000 {
        emu.tick();
        total_ticks += 1;

        if emu.take_frame().is_some() {
            frames += 1;

            // Check channel states every frame
            let mut any_change = false;
            for ch in 0..6 {
                let (freq, control, _, noise_ctrl) = emu.bus.psg_channel_info(ch);
                let key_on = (control & 0x80) != 0;
                let dda = (control & 0x40) != 0;

                if key_on {
                    active_frames[ch] += 1;
                }

                if freq != prev_freqs[ch] && key_on {
                    freq_changes[ch] += 1;
                    any_change = true;
                }
                prev_freqs[ch] = freq;
            }

            // Print summary every 30 frames (~0.5s) or on changes
            if frames - last_dump_frame >= 30 || (any_change && frames - last_dump_frame >= 5) {
                print!("F{:4}: ", frames);
                for ch in 0..6 {
                    let (freq, control, balance, noise_ctrl) = emu.bus.psg_channel_info(ch);
                    let key_on = (control & 0x80) != 0;
                    let dda = (control & 0x40) != 0;
                    let vol = control & 0x1F;
                    let noise_en = (noise_ctrl & 0x80) != 0;
                    if key_on {
                        if dda {
                            print!("CH{}:DDA(v{:02}) ", ch, vol);
                        } else if ch >= 4 && noise_en {
                            print!("CH{}:N{:02}(v{:02}) ", ch, noise_ctrl & 0x1F, vol);
                        } else {
                            let hz = if freq == 0 {
                                0.0
                            } else {
                                3_579_545.0 / (32.0 * freq as f64)
                            };
                            print!("CH{}:{:.0}Hz(v{:02}) ", ch, hz, vol);
                        }
                    }
                }
                println!();
                last_dump_frame = frames;
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    println!("\n=== Summary over {} frames ===", frames - start_frame);
    println!("Channel  Active  FreqChanges  Changes/sec");
    let elapsed = (frames - start_frame) as f64 / 60.0;
    for ch in 0..6 {
        println!(
            "  CH{}     {:4}      {:4}         {:.1}",
            ch,
            active_frames[ch],
            freq_changes[ch],
            freq_changes[ch] as f64 / elapsed
        );
    }

    // Also measure audio timing
    println!("\nCPU high_speed: {}", emu.cpu.clock_high_speed);
    let (reload, _counter, enabled, _) = emu.bus.timer_info();
    println!("Timer: reload={} enabled={}", reload, enabled);

    Ok(())
}
