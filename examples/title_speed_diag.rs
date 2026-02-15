/// Diagnose why the title screen music is slow.
/// Track CPU speed mode (CSL/CSH), timer rate, and music tempo counter.
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Kato-chan & Ken-chan (Japan).pce".to_string());
    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0u64;
    let mut timer_fires = 0u64;
    let mut prev_timer_counter: u8 = 0;
    let mut low_speed_ticks = 0u64;
    let mut high_speed_ticks = 0u64;
    let mut tempo_counter_resets = 0u64;
    let mut prev_tempo = 0u8;

    // Track per-frame stats
    let mut frame_timer_fires = 0u64;
    let mut frame_low_ticks = 0u64;
    let mut frame_high_ticks = 0u64;

    println!("Frame | HighSpeed% | TimerFires | TempoResets | TimerReload | ClockMode");
    println!("------+------------+------------+-------------+-------------+----------");

    while frames < 600 {
        let was_high_speed = emu.cpu.clock_high_speed;

        emu.tick();

        // Track speed mode
        if emu.cpu.clock_high_speed {
            high_speed_ticks += 1;
            frame_high_ticks += 1;
        } else {
            low_speed_ticks += 1;
            frame_low_ticks += 1;
        }

        // Track timer fires by watching counter transitions
        let (reload, counter, enabled, _prescaler) = emu.bus.timer_info();
        if enabled && counter == reload && prev_timer_counter == 0 && reload > 0 {
            timer_fires += 1;
            frame_timer_fires += 1;
        }
        prev_timer_counter = counter;

        // Track tempo counter at $3E01
        let tempo = emu.bus.read(0x3E01);
        if tempo == 0 && prev_tempo > 0 {
            tempo_counter_resets += 1;
        }
        prev_tempo = tempo;

        if emu.take_frame().is_some() {
            frames += 1;

            // Print per-frame stats every 60 frames (1 second)
            if frames % 60 == 0 {
                let total_ticks = frame_high_ticks + frame_low_ticks;
                let high_pct = if total_ticks > 0 {
                    frame_high_ticks as f64 / total_ticks as f64 * 100.0
                } else {
                    0.0
                };
                let mode = if frame_low_ticks > frame_high_ticks {
                    "LOW"
                } else {
                    "HIGH"
                };
                println!(
                    "{:5} | {:9.1}% | {:10} | {:11} | {:11} | {}",
                    frames, high_pct, frame_timer_fires, tempo_counter_resets, reload, mode
                );

                // Reset per-second counters
                frame_timer_fires = 0;
                frame_high_ticks = 0;
                frame_low_ticks = 0;
                tempo_counter_resets = 0;
            }
        }

        if emu.cpu.halted {
            break;
        }
    }

    let total = high_speed_ticks + low_speed_ticks;
    println!("\n=== Summary ===");
    println!(
        "Total ticks: {} (high: {}, low: {})",
        total, high_speed_ticks, low_speed_ticks
    );
    println!(
        "High speed: {:.1}%",
        high_speed_ticks as f64 / total as f64 * 100.0
    );
    println!("Total timer fires: {}", timer_fires);

    Ok(())
}
