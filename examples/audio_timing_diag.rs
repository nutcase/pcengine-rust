use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 60u64;
    let max_ticks = 50_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut total_audio_samples = 0u64;

    while frames < target_frames && total_ticks < max_ticks {
        emu.tick();
        total_ticks += 1;

        if emu.take_frame().is_some() {
            frames += 1;
        }

        if let Some(samples) = emu.take_audio_samples() {
            total_audio_samples += samples.len() as u64;
        }

        if emu.cpu.halted {
            break;
        }
    }

    // Drain remaining buffered audio
    emu.set_audio_batch_size(1);
    loop {
        match emu.take_audio_samples() {
            Some(s) => total_audio_samples += s.len() as u64,
            None => break,
        }
    }

    let expected_samples_per_frame = 44100.0 / 60.0;
    let expected_total = expected_samples_per_frame * frames as f64;
    let actual_per_frame = if frames > 0 {
        total_audio_samples as f64 / frames as f64
    } else {
        0.0
    };

    println!("=== Audio Timing Diagnostic ===");
    println!("Frames rendered: {}", frames);
    println!("Total ticks (emu.tick() calls): {}", total_ticks);
    println!("Total audio samples: {}", total_audio_samples);
    println!(
        "Samples per frame: {:.2} (expected ~{:.2})",
        actual_per_frame, expected_samples_per_frame
    );
    println!(
        "Expected total for {} frames: {:.0}",
        frames, expected_total
    );
    println!(
        "Ratio actual/expected: {:.4}",
        total_audio_samples as f64 / expected_total
    );
    println!(
        "Playback duration at 44100Hz: {:.3} sec",
        total_audio_samples as f64 / 44100.0
    );
    println!(
        "Expected duration at 60fps: {:.3} sec",
        frames as f64 / 60.0
    );
    println!(
        "Speed ratio: {:.4}",
        (total_audio_samples as f64 / 44100.0) / (frames as f64 / 60.0)
    );
    println!();
    println!("CPU cycles: {}", emu.cycles());
    println!(
        "Cycles per frame: {:.0}",
        emu.cycles() as f64 / frames.max(1) as f64
    );
    println!(
        "Expected cycles/frame (7159090/60): {:.0}",
        7_159_090.0 / 60.0
    );

    Ok(())
}
