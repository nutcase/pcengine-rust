use pce::emulator::Emulator;
use std::error::Error;
use std::io::Write;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let target_frames = 600u64; // 10 seconds
    let max_ticks = 100_000_000u64;
    let mut total_ticks = 0u64;
    let mut frames = 0u64;
    let mut all_samples: Vec<i16> = Vec::new();

    // Track note changes per channel
    let mut prev_freq = [0u16; 6];
    let mut note_changes = [0u32; 6];
    let mut prev_ctrl = [0u8; 6];
    let mut ctrl_changes = [0u32; 6];

    while frames < target_frames && total_ticks < max_ticks {
        emu.tick();
        total_ticks += 1;

        if let Some(samples) = emu.take_audio_samples() {
            all_samples.extend_from_slice(&samples);
        }

        if emu.take_frame().is_some() {
            frames += 1;

            // Count note changes every frame
            for ch in 0..6 {
                let (freq, ctrl, _, _) = emu.bus.psg_channel_info(ch);
                if freq != prev_freq[ch] {
                    note_changes[ch] += 1;
                    prev_freq[ch] = freq;
                }
                if ctrl != prev_ctrl[ch] {
                    ctrl_changes[ch] += 1;
                    prev_ctrl[ch] = ctrl;
                }
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    // Drain remaining audio
    emu.set_audio_batch_size(1);
    loop {
        match emu.take_audio_samples() {
            Some(s) => all_samples.extend_from_slice(&s),
            None => break,
        }
    }

    // Write WAV file
    let sample_rate = 44100u32;
    let path = "pl3_title.wav";
    write_wav(path, sample_rate, &all_samples)?;
    println!(
        "Wrote {} samples ({:.2} sec) to {}",
        all_samples.len(),
        all_samples.len() as f64 / sample_rate as f64,
        path
    );
    println!("Frames: {}", frames);

    // Report note change frequency
    let duration_sec = frames as f64 / 60.0;
    println!("\nNote changes over {:.1} seconds:", duration_sec);
    for ch in 0..6 {
        println!(
            "  CH{}: {} freq changes ({:.1}/sec), {} ctrl changes ({:.1}/sec)",
            ch,
            note_changes[ch],
            note_changes[ch] as f64 / duration_sec,
            ctrl_changes[ch],
            ctrl_changes[ch] as f64 / duration_sec
        );
    }

    Ok(())
}

fn write_wav(path: &str, sample_rate: u32, samples: &[i16]) -> Result<(), Box<dyn Error>> {
    let mut f = std::fs::File::create(path)?;
    let data_size = (samples.len() * 2) as u32;
    let file_size = 36 + data_size;

    // RIFF header
    f.write_all(b"RIFF")?;
    f.write_all(&file_size.to_le_bytes())?;
    f.write_all(b"WAVE")?;

    // fmt chunk
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?; // chunk size
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&(sample_rate * 2).to_le_bytes())?; // byte rate
    f.write_all(&2u16.to_le_bytes())?; // block align
    f.write_all(&16u16.to_le_bytes())?; // bits per sample

    // data chunk
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;
    for &s in samples {
        f.write_all(&s.to_le_bytes())?;
    }

    Ok(())
}
