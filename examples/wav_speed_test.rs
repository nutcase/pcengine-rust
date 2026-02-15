/// Create WAV files at different speeds for Kato-chan Ken-chan
/// to help identify the correct tempo.
use pce::emulator::Emulator;
use std::error::Error;
use std::io::Write;

fn write_wav(filename: &str, samples: &[i16], sample_rate: u32) -> Result<(), Box<dyn Error>> {
    let data_size = (samples.len() * 2) as u32;
    let mut f = std::fs::File::create(filename)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_size).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&(sample_rate * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;
    for s in samples {
        f.write_all(&s.to_le_bytes())?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.set_audio_batch_size(1);

    let target_frames = 600u64;
    let mut frames = 0u64;
    let mut all_samples: Vec<i16> = Vec::new();

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

    println!(
        "Generated {} samples ({:.2}s at 44100 Hz)",
        all_samples.len(),
        all_samples.len() as f64 / 44100.0
    );

    // Normal speed (44100 Hz)
    write_wav("kk_1x.wav", &all_samples, 44100)?;
    println!("Wrote kk_1x.wav (normal speed)");

    // 2x speed: write the same data but declare it as 22050 Hz
    // (player will play at 44100, effectively 2x speed)
    // Actually, to make it play faster, we set a HIGHER sample rate in the header
    write_wav("kk_2x.wav", &all_samples, 88200)?;
    println!("Wrote kk_2x.wav (2x speed)");

    // 4x speed
    write_wav("kk_4x.wav", &all_samples, 176400)?;
    println!("Wrote kk_4x.wav (4x speed)");

    // 3x speed
    write_wav("kk_3x.wav", &all_samples, 132300)?;
    println!("Wrote kk_3x.wav (3x speed)");

    Ok(())
}
