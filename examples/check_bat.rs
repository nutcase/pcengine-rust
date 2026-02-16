use pce::emulator::Emulator;
use std::{env, error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let rom_path = args
        .next()
        .ok_or("usage: check_bat <rom.[bin|pce]> [frames] [output] [--load-state <path>]")?;
    let frame_target: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let _output_path = args.next().unwrap_or_else(|| "unused".to_string());
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

    // Run to the target frame
    let mut frames_collected = 0;
    let mut safety_cycles = (frame_target as u64).saturating_mul(250_000).max(5_000_000);
    while frames_collected < frame_target && safety_cycles > 0 {
        let cycles = emulator.tick() as u64;
        if cycles == 0 {
            safety_cycles = safety_cycles.saturating_sub(1);
        } else {
            safety_cycles = safety_cycles.saturating_sub(cycles);
        }
        if emulator.take_frame().is_some() {
            frames_collected += 1;
        }
    }

    if frames_collected < frame_target {
        eprintln!("warning: reached cycle budget without collecting frame {frame_target}");
    }

    // --- Dump VDC registers ---
    let bxr = emulator.bus.vdc_register(0x07).unwrap_or(0);
    let byr = emulator.bus.vdc_register(0x08).unwrap_or(0);
    let mwr = emulator.bus.vdc_register(0x09).unwrap_or(0);
    let cr = emulator.bus.vdc_register(0x05).unwrap_or(0);
    let (map_w, map_h) = emulator.bus.vdc_map_dimensions();

    println!("=== VDC Registers ===");
    println!("BXR (0x07) = {:#06X}  (scroll X = {})", bxr, bxr & 0x03FF);
    println!("BYR (0x08) = {:#06X}  (scroll Y = {})", byr, byr & 0x01FF);
    println!("MWR (0x09) = {:#06X}", mwr);
    println!("CR  (0x05) = {:#06X}", cr);
    println!("Map dimensions: {}x{} tiles", map_w, map_h);
    println!();

    // --- Dump specific BAT entries ---
    let specific_addrs: &[(u16, &str)] = &[
        (0x0270, "row 9 col 48"),
        (0x0238, "row 8 col 56"),
    ];

    for &(bat_addr, label) in specific_addrs {
        println!("=== BAT @ {:#06X} ({}) ===", bat_addr, label);
        let entry = emulator.bus.vdc_vram_word(bat_addr);
        let tile_id = entry & 0x07FF;
        let palette_bank = (entry >> 12) & 0x0F;
        println!(
            "Entry word: {:#06X}  tile_id={:#05X} ({})  palette_bank={}",
            entry, tile_id, tile_id, palette_bank
        );

        // Dump 4-bitplane tile data (8 rows)
        let tile_base = (tile_id as u16) * 16;
        println!("Tile data (base={:#06X}):", tile_base);
        println!("  row  chr_a(p0p1) chr_b(p2p3)  pixels");
        for r in 0u16..8 {
            let chr_a = emulator.bus.vdc_vram_word(tile_base + r);
            let chr_b = emulator.bus.vdc_vram_word(tile_base + r + 8);
            // Decode pixel values for this row
            let plane0 = (chr_a & 0x00FF) as u8;
            let plane1 = ((chr_a >> 8) & 0x00FF) as u8;
            let plane2 = (chr_b & 0x00FF) as u8;
            let plane3 = ((chr_b >> 8) & 0x00FF) as u8;
            let mut pixels = String::new();
            for bit in (0..8).rev() {
                let p0 = (plane0 >> bit) & 1;
                let p1 = (plane1 >> bit) & 1;
                let p2 = (plane2 >> bit) & 1;
                let p3 = (plane3 >> bit) & 1;
                let val = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);
                pixels.push_str(&format!("{:X}", val));
            }
            println!(
                "  [{}]  {:#06X}      {:#06X}      {}",
                r, chr_a, chr_b, pixels
            );
        }

        // Dump palette entries for this palette bank
        let pal_start = (palette_bank as usize) * 16;
        print!("Palette bank {} (VCE indices {}-{}):", palette_bank, pal_start, pal_start + 15);
        for i in 0..16 {
            let rgb = emulator.bus.vce_palette_rgb(pal_start + i);
            let r = (rgb >> 16) & 0xFF;
            let g = (rgb >> 8) & 0xFF;
            let b = rgb & 0xFF;
            if i % 8 == 0 {
                println!();
                print!(" ");
            }
            print!(" [{:2}]#{:02X}{:02X}{:02X}", i, r, g, b);
        }
        println!();
        println!();
    }

    // --- Dump sprite palettes 3 and 4 ---
    for spr_pal in [3usize, 4] {
        let pal_start = 256 + spr_pal * 16;
        print!("Sprite palette {} (VCE indices {}-{}):", spr_pal, pal_start, pal_start + 15);
        for i in 0..16 {
            let rgb = emulator.bus.vce_palette_rgb(pal_start + i);
            let r = (rgb >> 16) & 0xFF;
            let g = (rgb >> 8) & 0xFF;
            let b = rgb & 0xFF;
            if i % 8 == 0 {
                println!();
                print!(" ");
            }
            print!(" [{:2}]#{:02X}{:02X}{:02X}", i, r, g, b);
        }
        println!();
    }
    println!();

    // --- Dump SAT entries with full attribute word ---
    println!("=== SAT entries (with size bits) ===");
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
        let cgx = (attr_w >> 8) & 1;  // 0=16px, 1=32px wide
        let cgy = (attr_w >> 12) & 3; // 0=16, 1=32, 2=invalid, 3=64 tall
        let width = if cgx == 0 { 16 } else { 32 };
        let height = match cgy {
            0 => 16, 1 => 32, 3 => 64, _ => 16,
        };
        let pri = (attr_w >> 7) & 1;
        let vflip = (attr_w >> 15) & 1;
        let hflip = (attr_w >> 11) & 1;
        println!(
            "  SPR#{:02} x={:4} y={:4} pat={:03X} pal={:X} {}x{} pri={} hf={} vf={} attr={:04X}",
            sprite, x, y, pat, pal, width, height, pri, hflip, vflip, attr_w
        );
    }
    println!();

    // --- Dump BAT rows 8 and 9, cols 32-63 ---
    println!("=== BAT Row 8, cols 32-63 ===");
    print_bat_row(&emulator, 8, 32, 63, map_w);
    println!();
    println!("=== BAT Row 9, cols 32-63 ===");
    print_bat_row(&emulator, 9, 32, 63, map_w);

    Ok(())
}

fn print_bat_row(emulator: &Emulator, row: usize, col_start: usize, col_end: usize, map_w: usize) {
    for col in col_start..=col_end {
        let addr = (row * map_w + col) as u16;
        let entry = emulator.bus.vdc_vram_word(addr);
        let tile_id = entry & 0x07FF;
        let palette_bank = (entry >> 12) & 0x0F;
        if entry != 0 {
            print!("[c{:02}:{:04X} t{:03X}p{:X}] ", col, entry, tile_id, palette_bank);
        } else {
            print!("[c{:02}:----] ", col);
        }
        if (col - col_start + 1) % 8 == 0 {
            println!();
        }
    }
    println!();
}
