/// Export WAV from Kato-chan Ken-chan.
use pce::emulator::Emulator;
use std::error::Error;
use std::io::Write;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 600u64;
    let sample_rate = 44100u32;
    let mut frames = 0u64;
    let mut all_samples: Vec<i16> = Vec::new();
    emu.set_audio_batch_size(1);

    // Collect all audio
    while frames < target_frames {
        emu.tick();
        if let Some(chunk) = emu.take_audio_samples() {
            all_samples.extend_from_slice(&chunk);
        }
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    println!("Frames: {}", frames);
    println!(
        "Samples: {} ({:.3}s at {} Hz)",
        all_samples.len(),
        all_samples.len() as f64 / sample_rate as f64,
        sample_rate
    );

    // Write WAV
    let filename = "kk_audio.wav";
    let data_size = (all_samples.len() * 2) as u32;
    let mut f = std::fs::File::create(filename)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_size).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&(sample_rate * 2).to_le_bytes())?; // byte rate
    f.write_all(&2u16.to_le_bytes())?; // block align
    f.write_all(&16u16.to_le_bytes())?; // bits per sample
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;
    for s in &all_samples {
        f.write_all(&s.to_le_bytes())?;
    }

    println!("Wrote: {}", filename);

    // Analyze: find note boundaries by detecting significant amplitude changes
    let window = 441; // 10ms
    let mut rms_values: Vec<f64> = Vec::new();
    for i in (0..all_samples.len()).step_by(window) {
        let end = (i + window).min(all_samples.len());
        let rms: f64 = (all_samples[i..end]
            .iter()
            .map(|&s| (s as f64) * (s as f64))
            .sum::<f64>()
            / (end - i) as f64)
            .sqrt();
        rms_values.push(rms);
    }

    // Print RMS per 100ms
    println!("\nRMS per 100ms (first 5 seconds):");
    for sec_10th in 0..50 {
        let start_idx = sec_10th * 10;
        let end_idx = ((sec_10th + 1) * 10).min(rms_values.len());
        if start_idx >= rms_values.len() {
            break;
        }
        let avg: f64 =
            rms_values[start_idx..end_idx].iter().sum::<f64>() / (end_idx - start_idx) as f64;
        let time = sec_10th as f64 * 0.1;
        print!("{:.1}s:{:.0} ", time, avg);
        if (sec_10th + 1) % 10 == 0 {
            println!();
        }
    }
    println!();

    // Sample range
    let min = all_samples.iter().min().unwrap();
    let max = all_samples.iter().max().unwrap();
    println!("Sample range: {} to {}", min, max);

    Ok(())
}
