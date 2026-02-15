/// Measure how many tick() iterations per frame and WAI vs active cycles.
/// If WAI dominates, the emulator might be too slow for real-time playback.
use pce::emulator::Emulator;
use std::error::Error;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args().nth(1).unwrap_or_else(|| {
        "roms/Kato-chan & Ken-chan (Japan).pce".to_string()
    });
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();
    emu.set_audio_batch_size(1);

    let mut frames = 0u64;
    let mut ticks_this_frame = 0u64;
    let mut wai_ticks = 0u64;
    let mut active_ticks = 0u64;
    let mut samples_this_sec = 0u64;

    println!("Frame | Ticks/frame |  WAI%  | Samples | WallTime(ms) | RealTime?");
    println!("------+-------------+--------+---------+--------------+----------");

    let mut sec_start = Instant::now();
    let mut sec_ticks = 0u64;
    let mut sec_wai = 0u64;
    let mut sec_samples = 0u64;

    while frames < 600 {
        let is_waiting = emu.cpu.is_waiting();

        let cycles = emu.tick();

        if is_waiting {
            wai_ticks += 1;
            sec_wai += 1;
        } else {
            active_ticks += 1;
        }
        ticks_this_frame += 1;
        sec_ticks += 1;

        if let Some(chunk) = emu.take_audio_samples() {
            samples_this_sec += chunk.len() as u64;
            sec_samples += chunk.len() as u64;
        }

        if emu.take_frame().is_some() {
            frames += 1;

            if frames % 60 == 0 {
                let elapsed = sec_start.elapsed();
                let wall_ms = elapsed.as_secs_f64() * 1000.0;
                let wai_pct = if sec_ticks > 0 {
                    sec_wai as f64 / sec_ticks as f64 * 100.0
                } else { 0.0 };
                let avg_ticks_per_frame = sec_ticks / 60;
                let realtime = if wall_ms < 1050.0 { "YES" } else { "SLOW" };
                println!(
                    "{:5} | {:11} | {:5.1}% | {:7} | {:12.1} | {}",
                    frames, avg_ticks_per_frame, wai_pct, sec_samples, wall_ms, realtime
                );

                sec_start = Instant::now();
                sec_ticks = 0;
                sec_wai = 0;
                sec_samples = 0;
            }
        }

        if emu.cpu.halted { break; }
    }

    let total = wai_ticks + active_ticks;
    println!("\n=== Summary ===");
    println!("Total ticks: {} (WAI: {}, active: {})", total, wai_ticks, active_ticks);
    println!("WAI fraction: {:.1}%", wai_ticks as f64 / total as f64 * 100.0);
    println!("Avg ticks/frame: {}", total / frames.max(1));

    Ok(())
}
