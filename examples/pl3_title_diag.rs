use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "roms/Power League III (Japan).pce".to_string());
    let seconds = std::env::args()
        .nth(2)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(12);

    let rom = std::fs::read(&rom_path)?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.set_audio_batch_size(256);
    emu.reset();
    emu.bus.reset_audio_diagnostics();

    let mut frame_buf = Vec::new();
    let mut last_diag = emu.bus.audio_diagnostics();
    let mut total_frames = 0u64;
    let mut last_frames = 0u64;
    let mut second = 0u64;

    println!("PL3 title diag: rom={} seconds={}", rom_path, seconds);

    loop {
        emu.tick();
        if emu.take_frame_into(&mut frame_buf) {
            total_frames += 1;
        }
        let _ = emu.take_audio_samples();

        let diag = emu.bus.audio_diagnostics();
        let next_report_phi = (second + 1) * diag.master_clock_hz as u64;
        if diag.total_phi_cycles >= next_report_phi {
            let delta_phi = diag
                .total_phi_cycles
                .saturating_sub(last_diag.total_phi_cycles);
            let delta_frames = total_frames.saturating_sub(last_frames);
            let fps = if delta_phi == 0 {
                0.0
            } else {
                delta_frames as f64 * diag.master_clock_hz as f64 / delta_phi as f64
            };
            let ctrl = emu.bus.vdc_control_register();
            let rcr = emu.bus.vdc_register(0x06).unwrap_or(0);
            let vpr = emu.bus.vdc_register(0x0C).unwrap_or(0);
            let vdw = emu.bus.vdc_register(0x0D).unwrap_or(0);
            let vcr = emu.bus.vdc_register(0x0E).unwrap_or(0);
            println!(
                "[{:2}s] frames={} (+{}) fps={:.2} scanline={} vblank={} status=${:02X} cr=${:04X} rcr=${:04X} vpr=${:04X} vdw=${:04X} vcr=${:04X}",
                second + 1,
                total_frames,
                delta_frames,
                fps,
                emu.bus.vdc_current_scanline(),
                emu.bus.vdc_in_vblank(),
                emu.bus.vdc_status_bits(),
                ctrl,
                rcr,
                vpr,
                vdw,
                vcr
            );
            last_diag = diag;
            last_frames = total_frames;
            second += 1;
            if second >= seconds {
                break;
            }
        }
    }

    Ok(())
}
