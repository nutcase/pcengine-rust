#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::io::Write;

fn save_ppm(path: &str, pixels: &[u32], width: usize, height: usize) {
    let mut f = std::fs::File::create(path).unwrap();
    write!(f, "P6\n{} {}\n255\n", width, height).unwrap();
    for y in 0..height {
        for x in 0..width {
            let p = pixels[y * width + x];
            let r = ((p >> 16) & 0xFF) as u8;
            let g = ((p >> 8) & 0xFF) as u8;
            let b = (p & 0xFF) as u8;
            f.write_all(&[r, g, b]).unwrap();
        }
    }
}

fn main() {
    let rom_path = "roms/Bikkuriman World (Japan).pce";
    let state_path = "states/Bikkuriman World (Japan).slot3.state";

    let rom = std::fs::read(rom_path).expect("failed to read ROM");
    let mut emu = Emulator::new();
    emu.load_hucard(&rom).expect("failed to load HuCard");
    emu.set_audio_batch_size(128);
    emu.reset();
    emu.load_state_from_file(state_path)
        .expect("failed to load state");
    emu.set_audio_batch_size(128);

    let _ = std::fs::create_dir_all("debug_frames_bik");

    let mut frame_count = 0u32;

    for _step in 0..200_000_000u64 {
        emu.bus.set_joypad_input(0xFF);
        emu.tick();
        let _ = emu.take_audio_samples();

        if let Some(frame) = emu.take_frame() {
            let cr = emu.bus.vdc_control_register();
            let bg_on = cr & 0x80 != 0;
            let spr_on = cr & 0x40 != 0;
            let mode = match (bg_on, spr_on) {
                (true, true) => "BG+SPR",
                (true, false) => "BG    ",
                (false, true) => "SPR   ",
                (false, false) => "BURST ",
            };
            let vce0 = emu.bus.vce_palette_word(0);
            let non_black = frame.iter().filter(|&&p| p != 0 && p != 0xFF000000).count();

            if frame_count % 50 == 0
                || (frame_count >= 140 && frame_count <= 180)
                || (frame_count >= 765 && frame_count <= 780)
            {
                println!(
                    "Frame {:5}: CR={:04X} {} VCE0={:03X} pixels={}",
                    frame_count, cr, mode, vce0, non_black
                );
            }

            // Save frames during and after SPR-only transition
            if (frame_count >= 144 && frame_count <= 160)
                || frame_count == 770
                || frame_count == 771
                || frame_count == 800
            {
                let w = emu.display_width();
                let h = emu.display_height();
                save_ppm(
                    &format!("debug_frames_bik/frame_{:04}.ppm", frame_count),
                    &frame,
                    w,
                    h,
                );
            }

            frame_count += 1;
            if frame_count > 850 {
                break;
            }
        }
    }
}
