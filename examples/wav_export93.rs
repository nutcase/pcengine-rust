use pce::emulator::Emulator;
use std::error::Error;
use std::io::Write;

const SAMPLE_RATE: u32 = 44_100;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League '93 (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.set_audio_batch_size(1); // get every sample

    let target_frames = 600u64; // 10 seconds at 60 Hz
    let mut frames = 0u64;
    let mut total_ticks = 0u64;
    let mut all_samples: Vec<i16> = Vec::new();
    let mut min_sample: i16 = 0;
    let mut max_sample: i16 = 0;
    let mut zero_count = 0u64;
    let mut clip_count = 0u64;

    while frames < target_frames && total_ticks < 100_000_000 {
        emu.tick();
        total_ticks += 1;

        if let Some(samples) = emu.take_audio_samples() {
            for &s in &samples {
                if s < min_sample {
                    min_sample = s;
                }
                if s > max_sample {
                    max_sample = s;
                }
                if s == 0 {
                    zero_count += 1;
                }
                if s == i16::MIN || s == i16::MAX {
                    clip_count += 1;
                }
                all_samples.push(s);
            }
        }

        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    let num_samples = all_samples.len() as u32;
    let duration_secs = num_samples as f64 / SAMPLE_RATE as f64;
    eprintln!("=== WAV Export Summary ===");
    eprintln!("Frames: {}", frames);
    eprintln!("Total ticks: {}", total_ticks);
    eprintln!(
        "Samples: {} ({:.3}s at {} Hz)",
        num_samples, duration_secs, SAMPLE_RATE
    );
    eprintln!(
        "Expected: {:.3}s for {} frames at 60 Hz",
        frames as f64 / 60.0,
        frames
    );
    eprintln!(
        "Sample range: {} to {} (i16: {} to {})",
        min_sample,
        max_sample,
        i16::MIN,
        i16::MAX
    );
    eprintln!(
        "Zero samples: {} ({:.1}%)",
        zero_count,
        zero_count as f64 / num_samples as f64 * 100.0
    );
    eprintln!(
        "Clipped samples: {} ({:.1}%)",
        clip_count,
        clip_count as f64 / num_samples as f64 * 100.0
    );

    // Analyze first few seconds for silence
    let samples_per_frame = (SAMPLE_RATE as f64 / 60.0) as usize; // ~735
    let mut silent_frames = 0u64;
    for f in 0..frames.min(300) as usize {
        let start = f * samples_per_frame;
        let end = (start + samples_per_frame).min(all_samples.len());
        if start >= all_samples.len() {
            break;
        }
        let chunk = &all_samples[start..end];
        let max_abs = chunk.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        if max_abs < 100 {
            silent_frames += 1;
        }
    }
    eprintln!(
        "Silent frames (first 300): {} ({:.1}%)",
        silent_frames,
        silent_frames as f64 / 300.0 * 100.0
    );

    // RMS analysis by sections
    let section_size = SAMPLE_RATE as usize; // 1 second
    eprintln!("\n=== RMS per second ===");
    for sec in 0..(num_samples as usize / section_size).min(10) {
        let start = sec * section_size;
        let end = start + section_size;
        let rms: f64 = (all_samples[start..end]
            .iter()
            .map(|&s| (s as f64) * (s as f64))
            .sum::<f64>()
            / section_size as f64)
            .sqrt();
        eprintln!("  Second {}: RMS={:.1}", sec, rms);
    }

    // Write WAV file
    let file_path = "pl93_audio.wav";
    let mut f = std::fs::File::create(file_path)?;
    let data_size = num_samples * 2; // 16-bit mono
    let file_size = 36 + data_size;

    // WAV header
    f.write_all(b"RIFF")?;
    f.write_all(&file_size.to_le_bytes())?;
    f.write_all(b"WAVE")?;
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?; // chunk size
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&SAMPLE_RATE.to_le_bytes())?;
    f.write_all(&(SAMPLE_RATE * 2).to_le_bytes())?; // bytes/sec
    f.write_all(&2u16.to_le_bytes())?; // block align
    f.write_all(&16u16.to_le_bytes())?; // bits/sample
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;
    for &s in &all_samples {
        f.write_all(&s.to_le_bytes())?;
    }

    eprintln!("\nWrote: {}", file_path);
    Ok(())
}
