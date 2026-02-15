/// Benchmark raw emulation speed (no diagnostics overhead).
use pce::emulator::Emulator;
use std::error::Error;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.set_audio_batch_size(128);

    let mut frames = 0u64;
    let mut total_samples = 0u64;

    let start = Instant::now();
    let mut sec_start = Instant::now();
    let mut sec_samples = 0u64;

    while frames < 600 {
        emu.tick();

        if let Some(chunk) = emu.take_audio_samples() {
            total_samples += chunk.len() as u64;
            sec_samples += chunk.len() as u64;
        }

        if emu.take_frame().is_some() {
            frames += 1;
            if frames % 60 == 0 {
                let wall_ms = sec_start.elapsed().as_secs_f64() * 1000.0;
                let speed = 1000.0 / wall_ms;
                println!(
                    "Frame {:3}: {:.1}ms/sec, {:.2}x realtime, {} samples/sec",
                    frames, wall_ms, speed, sec_samples
                );
                sec_start = Instant::now();
                sec_samples = 0;
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    let total_sec = frames as f64 / 60.0;
    println!(
        "\nTotal: {} frames in {:.1}ms ({:.2}x realtime)",
        frames,
        total_ms,
        total_sec * 1000.0 / total_ms
    );
    println!(
        "Total samples: {} ({:.1} samples/sec)",
        total_samples,
        total_samples as f64 / total_sec
    );

    Ok(())
}
