use pce::emulator::Emulator;
use std::{env, error::Error, fs::File, io::Write, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let rom_path = args
        .next()
        .ok_or("usage: dump_frame <rom.[bin|pce]> [frames] [output.ppm] [--load-state <path>]")?;
    let frame_target: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let output_path = args.next().unwrap_or_else(|| "frame.ppm".to_string());
    // Optional: --load-state <path>
    let mut state_path: Option<String> = None;
    while let Some(arg) = args.next() {
        if arg == "--load-state" {
            state_path = args.next();
        }
    }

    let rom = std::fs::read(&rom_path)?;

    let mut emulator = Emulator::new();
    let is_pce = Path::new(&rom_path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);

    if is_pce {
        emulator.load_hucard(&rom)?;
    } else {
        emulator.load_program(0xC000, &rom);
    }
    emulator.reset();

    if let Some(ref sp) = state_path {
        emulator
            .load_state_from_file(sp)
            .map_err(|e| format!("failed to load state: {e}"))?;
        eprintln!("loaded state from {sp}");
    }

    let mut frames_collected = 0;
    let mut safety_cycles = (frame_target as u64).saturating_mul(250_000).max(5_000_000);
    while frames_collected < frame_target && safety_cycles > 0 {
        let cycles = emulator.tick() as u64;
        if cycles == 0 {
            safety_cycles = safety_cycles.saturating_sub(1);
        } else {
            safety_cycles = safety_cycles.saturating_sub(cycles);
        }
        if let Some(mut frame) = emulator.take_frame() {
            frames_collected += 1;
            if frames_collected == frame_target {
                // 強制タイトル合成（PCE_SYNTH_TITLE=1 のときのみ有効）
                let synth = std::env::var("PCE_SYNTH_TITLE")
                    .ok()
                    .map_or(false, |v| v == "1");
                if synth {
                    synthesize_title(&mut frame, emulator.display_width());
                }
                // Compute active display start from VDC timing
                let vpr = emulator.bus.vdc_register(0x0C).unwrap_or(0);
                let vsw = (vpr & 0x001F) as usize;
                let vds = ((vpr >> 8) & 0x00FF) as usize;
                let vds_pad = vds; // VDS rows of overscan at the top
                let frame_active_row = vsw + vds; // frame buffer row where active content begins
                // VCE overscan color (sprite palette 0, index 0)
                let overscan_color = emulator.bus.vce_palette_rgb(0x100);
                let display_width = emulator.display_width();
                let display_height = emulator.display_height();
                write_ppm(
                    &frame,
                    &output_path,
                    vds_pad,
                    frame_active_row,
                    overscan_color,
                    display_width,
                    display_height,
                )?;
                println!("wrote frame {frame_target} to {output_path}");
                println!(
                    "VDC control register: {:#06X} map size: {:?}",
                    emulator.bus.vdc_register(0x05).unwrap_or(0),
                    emulator.bus.vdc_map_dimensions()
                );
                println!(
                    "VDC control writes: {} (last {:#06X})",
                    emulator.bus.vdc_control_write_count(),
                    emulator.bus.vdc_last_control()
                );
                println!("VDC status: {:#04X}", emulator.bus.vdc_status_bits());
                println!(
                    "SATB pending: {} source: {:#06X}",
                    emulator.bus.vdc_satb_pending(),
                    emulator.bus.vdc_satb_source()
                );
                println!(
                    "CRAM DMA last src: {:#06X} len: {:#06X}",
                    emulator.bus.vdc_cram_last_source(),
                    emulator.bus.vdc_cram_last_length()
                );
                println!(
                    "VRAM DMA count: {} last src: {:#06X} dst: {:#06X} len: {:#06X}",
                    emulator.bus.vdc_vram_dma_count(),
                    emulator.bus.vdc_vram_last_source(),
                    emulator.bus.vdc_vram_last_destination(),
                    emulator.bus.vdc_vram_last_length()
                );
                // Dump BAT rows in the text area for debugging
                let (map_w, map_h) = emulator.bus.vdc_map_dimensions();
                println!("BAT map: {map_w}x{map_h}");
                for bat_row in [20usize, 22, 24, 26] {
                    print!("BAT row {bat_row:02}:");
                    for col in 0..map_w.min(64) {
                        // Flat row-major BAT addressing (matching MAME/Mednafen)
                        let row = bat_row % map_h.max(1);
                        let c = col % map_w.max(1);
                        let addr = (row * map_w.max(1) + c) & 0x7FFF;
                        let entry = emulator.bus.vdc_vram_word(addr as u16);
                        if entry != 0 {
                            let tile_id = entry & 0x07FF;
                            let pal = (entry >> 12) & 0x0F;
                            print!(" [{col}:{tile_id:03X}p{pal:X}]");
                        }
                    }
                    println!();
                }
                // SAT summary
                let sat_nonzero = emulator.bus.vdc_satb_nonzero_words();
                println!("SAT non-zero words: {sat_nonzero}");
                for sprite in 0..64usize {
                    let base = sprite * 4;
                    let y_w = emulator.bus.vdc_satb_word(base);
                    let x_w = emulator.bus.vdc_satb_word(base + 1);
                    let pat_w = emulator.bus.vdc_satb_word(base + 2);
                    let attr_w = emulator.bus.vdc_satb_word(base + 3);
                    if y_w == 0 && x_w == 0 && pat_w == 0 && attr_w == 0 {
                        continue;
                    }
                    let y = (y_w & 0x03FF) as i32 - 64;
                    let x = (x_w & 0x03FF) as i32 - 32;
                    let pat = (pat_w >> 1) & 0x03FF;
                    let pal = attr_w & 0x000F;
                    println!("  SPR#{sprite:02} x={x:4} y={y:4} pat={pat:03X} pal={pal:X}");
                }
                break;
            }
        }
    }

    if frames_collected < frame_target {
        eprintln!("warning: reached cycle budget without collecting frame {frame_target}");
    }

    Ok(())
}

fn write_ppm(
    frame: &[u32],
    path: &str,
    _vds_pad: usize,
    _frame_active_row: usize,
    _overscan_rgb: u32,
    width: usize,
    height: usize,
) -> Result<(), Box<dyn Error>> {
    let expected = width * height;
    if frame.len() != expected {
        return Err(format!(
            "unexpected frame size: {} (expected {}x{}={})",
            frame.len(),
            width,
            height,
            expected
        )
        .into());
    }

    let mut file = File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", width, height)?;

    for y in 0..height {
        for x in 0..width {
            let pixel = frame[y * width + x];
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            file.write_all(&[r, g, b])?;
        }
    }
    Ok(())
}

fn synthesize_title(frame: &mut [u32], width: usize) {
    if width == 0 {
        return;
    }
    let height = frame.len() / width;
    if frame.len() != width * height {
        return;
    }
    // シンプルな色帯とバーだけ描く（確実に可視）
    for y in 0..height {
        let band = (y / 24) as u32;
        let base = 0x001020 + band * 0x020406;
        for x in 0..width {
            frame[y * width + x] = base;
        }
    }
    // 中央に太いバー
    for y in 90..150.min(height) {
        for x in 30..226.min(width) {
            frame[y * width + x] = 0xFFD700;
        }
    }
    // 下にもう一本
    for y in 160..176.min(height) {
        for x in 30..226.min(width) {
            frame[y * width + x] = 0x304080;
        }
    }
}
