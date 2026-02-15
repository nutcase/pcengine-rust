use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Verify noise frequency matches Mednafen formula:
    // period = (31-NF)*128 for NF<31, 64 for NF==31
    // LFSR rate = PSG_CLOCK / period = 3,579,545 / period
    let mut emu = Emulator::new();
    let program = [0x00]; // BRK
    emu.load_program(0xC000, &program);
    emu.reset();

    println!("=== Noise Rate Verification (Mednafen formula) ===");
    println!(
        "{:>4} {:>8} {:>12} {:>12} {:>8}",
        "NF", "period", "expected_Hz", "measured_Hz", "error%"
    );

    for nf in [0u8, 5, 10, 15, 20, 25, 30, 31] {
        // Set up CH4 with noise enabled, all other channels off
        for ch in 0..6u8 {
            emu.bus.write_io(0x0800, ch);
            emu.bus.write_io(0x0804, 0x00); // KEY OFF
        }
        emu.bus.write_io(0x0800, 4); // select ch4
        emu.bus.write_io(0x0804, 0x80 | 0x1F); // KEY_ON + max volume
        emu.bus.write_io(0x0805, 0xFF); // max balance
        emu.bus.write_io(0x0807, 0x80 | nf); // noise enable + frequency

        // Compute expected based on Mednafen formula
        let raw = 31u32.saturating_sub(nf as u32);
        let period = if raw == 0 { 64u64 } else { raw as u64 * 128 };
        let expected_lfsr_rate = 3_579_545.0 / period as f64;

        // Generate samples and count output transitions
        let num_samples = 44_100; // 1 second of audio
        let mut transitions = 0u64;
        let mut prev_sample = 0i16;
        emu.set_audio_batch_size(1);

        for _ in 0..num_samples {
            let sample = emu.bus.psg_sample();
            if (sample > 0) != (prev_sample > 0) && prev_sample != 0 {
                transitions += 1;
            }
            prev_sample = sample;
        }

        // Output frequency = transitions / 2 (each "cycle" has 2 transitions)
        // LFSR transition probability ~0.5 per step for pseudo-random sequence
        // So: measured_transitions ≈ lfsr_steps * 0.5
        // And: measured_freq ≈ measured_transitions / 2 ≈ lfsr_steps * 0.25
        // To estimate lfsr_rate: lfsr_rate ≈ transitions * 2
        let estimated_lfsr_rate = transitions as f64 * 2.0;
        let error_pct = if expected_lfsr_rate > 0.0 {
            (estimated_lfsr_rate - expected_lfsr_rate) / expected_lfsr_rate * 100.0
        } else {
            0.0
        };

        println!(
            "{:4} {:8} {:12.1} {:12.1} {:8.1}",
            nf, period, expected_lfsr_rate, estimated_lfsr_rate, error_pct
        );
    }

    Ok(())
}
