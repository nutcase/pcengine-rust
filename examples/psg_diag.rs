use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 300u64;
    let max_ticks = 50_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut total_samples = 0u64;
    let mut nonzero_samples = 0u64;
    let mut sample_rms_sum = 0.0f64;

    while frames < target_frames && total_ticks < max_ticks {
        emu.tick();
        total_ticks += 1;

        if let Some(samples) = emu.take_audio_samples() {
            for &s in &samples {
                total_samples += 1;
                if s != 0 {
                    nonzero_samples += 1;
                }
                sample_rms_sum += (s as f64) * (s as f64);
            }
        }

        if emu.take_frame().is_some() {
            frames += 1;

            if frames % 30 == 0 || frames == 1 || frames == 5 || frames == 10 || frames == 20 {
                println!("=== Frame {} (tick {}) ===", frames, total_ticks);
                for ch in 0..6 {
                    let (freq, ctrl, bal, noise) = emu.bus.psg_channel_info(ch);
                    let key_on = ctrl & 0x80 != 0;
                    let dda = ctrl & 0x40 != 0;
                    let volume = ctrl & 0x1F;

                    let freq_hz = if freq > 0 {
                        3_579_545.0 / (32.0 * freq as f64)
                    } else {
                        0.0
                    };
                    println!(
                        "  CH{}: freq={:4} ({:7.1}Hz) vol={:2} key={} dda={} bal=${:02X} noise=${:02X}",
                        ch, freq, freq_hz, volume, key_on, dda, bal, noise
                    );
                }
                let rms = if total_samples > 0 {
                    (sample_rms_sum / total_samples as f64).sqrt()
                } else {
                    0.0
                };
                println!(
                    "  Audio: {} samples, {} nonzero ({:.1}%), RMS={:.1}",
                    total_samples,
                    nonzero_samples,
                    if total_samples > 0 {
                        nonzero_samples as f64 / total_samples as f64 * 100.0
                    } else {
                        0.0
                    },
                    rms
                );
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    println!("\n=== Final State ({} frames) ===", frames);
    for ch in 0..6 {
        let (freq, ctrl, bal, noise) = emu.bus.psg_channel_info(ch);
        let key_on = ctrl & 0x80 != 0;
        let dda = ctrl & 0x40 != 0;
        let volume = ctrl & 0x1F;
        let freq_hz = if freq > 0 {
            3_579_545.0 / (32.0 * freq as f64)
        } else {
            0.0
        };
        println!(
            "  CH{}: freq={:4} ({:7.1}Hz) ctrl=${:02X} vol={:2} key={:<5} dda={:<5} bal=${:02X} noise=${:02X}",
            ch, freq, freq_hz, ctrl, volume, key_on, dda, bal, noise
        );
    }

    let rms = if total_samples > 0 {
        (sample_rms_sum / total_samples as f64).sqrt()
    } else {
        0.0
    };
    println!(
        "\nTotal audio: {} samples, {} nonzero ({:.1}%), RMS={:.1}",
        total_samples,
        nonzero_samples,
        if total_samples > 0 {
            nonzero_samples as f64 / total_samples as f64 * 100.0
        } else {
            0.0
        },
        rms
    );

    Ok(())
}
