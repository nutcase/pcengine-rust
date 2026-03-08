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
    let rom_path = "roms/Kato-chan & Ken-chan (Japan).pce";
    let state_path = "states/Kato-chan & Ken-chan (Japan).slot3.state";

    let rom = std::fs::read(rom_path).expect("failed to read ROM");
    let mut emu = Emulator::new();
    emu.load_hucard(&rom).expect("failed to load HuCard");
    emu.set_audio_batch_size(128);
    emu.reset();
    emu.load_state_from_file(state_path)
        .expect("failed to load state");
    emu.set_audio_batch_size(128);

    let _ = std::fs::create_dir_all("debug_frames");

    let mut frame_count = 0u32;
    let mut up_pressed = false;
    let mut frames_since_up = 0u32;

    // Track previous SATB state for change detection
    let mut prev_satb0_y: u16 = 0xFFFF;
    let mut prev_dma_ran = false;

    println!("=== Kato-chan Ken-chan Transition Deep Trace ===");
    println!(
        "Columns: Frame | CR(hex) | Mode | VCE[0] | DCR | SATB_written | SATB_pending | Sprite0 Y/X | Non-black pixels"
    );
    println!();

    for _step in 0..10_000_000u64 {
        if frame_count == 30 && !up_pressed {
            up_pressed = true;
            frames_since_up = 0;
        }

        let pad = if up_pressed { 0xFF & !(1 << 0) } else { 0xFF };
        emu.bus.set_joypad_input(pad);

        emu.tick();
        let _ = emu.take_audio_samples();

        if let Some(frame) = emu.take_frame() {
            let cr = emu.bus.vdc_control_register();
            let bg_on = cr & 0x80 != 0;
            let spr_on = cr & 0x40 != 0;
            let mode = match (bg_on, spr_on) {
                (true, true) => "BG+SPR ",
                (true, false) => "BG     ",
                (false, true) => "SPR    ",
                (false, false) => "BURST  ",
            };

            let vce0 = emu.bus.vce_palette_word(0);
            let satb_written = emu.bus.vdc_satb_written();
            let satb_pending = emu.bus.vdc_satb_pending();

            // Read first 4 sprites from SATB
            let s0_y = emu.bus.vdc_satb_word(0) & 0x03FF;
            let s0_x = emu.bus.vdc_satb_word(1) & 0x03FF;
            let s0_pat = emu.bus.vdc_satb_word(2);
            let s0_attr = emu.bus.vdc_satb_word(3);

            let s1_y = emu.bus.vdc_satb_word(4) & 0x03FF;
            let s1_x = emu.bus.vdc_satb_word(5) & 0x03FF;

            let s2_y = emu.bus.vdc_satb_word(8) & 0x03FF;
            let s2_x = emu.bus.vdc_satb_word(9) & 0x03FF;

            // Count non-black pixels
            let non_black = frame.iter().filter(|&&p| p != 0 && p != 0xFF000000).count();

            // Print for transition region (frames 80-140) or when interesting state changes
            let in_region = frame_count >= 80 && frame_count <= 140;
            let satb_changed = s0_y != prev_satb0_y;

            if in_region || satb_changed {
                println!(
                    "F{:4} CR={:04X} {} VCE0={:03X} written={} pending={} S0=({:3},{:3}) pat={:04X} attr={:04X} S1=({:3},{:3}) S2=({:3},{:3}) px={}",
                    frame_count,
                    cr,
                    mode,
                    vce0,
                    if satb_written { "Y" } else { "n" },
                    if satb_pending { "Y" } else { "n" },
                    s0_y as i32 - 64,
                    s0_x as i32 - 32,
                    s0_pat,
                    s0_attr,
                    s1_y as i32 - 64,
                    s1_x as i32 - 32,
                    s2_y as i32 - 64,
                    s2_x as i32 - 32,
                    non_black,
                );
            }
            prev_satb0_y = s0_y;

            // Also dump SATB source VRAM area for comparison
            if in_region && !bg_on && spr_on {
                // When in SPR-only mode, show what the SATB source in VRAM looks like
                let satb_src = emu.bus.vdc_satb_source();
                println!(
                    "      SATB_SRC={:04X}  non-zero SATB words: {}",
                    satb_src,
                    emu.bus.vdc_satb_nonzero_words(),
                );

                // Show first 4 sprites from VRAM source (for comparison with SATB copy)
                // Read via bus accessors
                let vram_s0_y = emu.bus.vdc_vram_word(satb_src);
                let vram_s0_x = emu.bus.vdc_vram_word(satb_src.wrapping_add(1));
                println!(
                    "      VRAM[SATB_SRC]: S0 Y={:04X} X={:04X}",
                    vram_s0_y, vram_s0_x,
                );
            }

            // Dump PPM for transition frames
            if frame_count >= 90 && frame_count <= 130 {
                let w = emu.display_width();
                let h = emu.display_height();
                let path = format!("debug_frames/frame_{:04}.ppm", frame_count);
                save_ppm(&path, &frame, w, h);
            }

            frame_count += 1;

            if up_pressed {
                frames_since_up += 1;
                if frames_since_up >= 10 {
                    up_pressed = false;
                }
            }

            if !up_pressed && frame_count % 60 == 30 {
                up_pressed = true;
                frames_since_up = 0;
            }

            if frame_count > 350 {
                break;
            }
        }
    }
    println!("\nDone. PPM frames saved to debug_frames/");
}
