#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Kato-chan & Ken-chan (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    let mut frames = 0;
    let mut last_frame: Option<Vec<u32>> = None;
    while frames < 300 {
        emu.tick();
        if let Some(f) = emu.take_frame() {
            last_frame = Some(f);
            frames += 1;
        }
    }

    let pixels = last_frame.expect("should have frame");
    let width = 256;

    // Debug specific lines: check colors and what they correspond to
    println!("=== Y=141 (BAT row 24 'PUSH RUN BUTTON') pixel colors ===");
    for x in 0..256usize {
        let idx = 141 * width + x;
        let pixel = pixels[idx];
        let r = (pixel >> 16) & 0xFF;
        let g = (pixel >> 8) & 0xFF;
        let b = pixel & 0xFF;
        if x % 8 == 0 {
            print!("\n  X={:3} (col {:2}): ", x, x / 8);
        }
        print!("({:02X}{:02X}{:02X})", r, g, b);
    }
    println!();

    // Check Y=109 (BAT row 20 'HISCORE')
    println!("\n=== Y=109 (BAT row 20 'HISCORE') pixel colors at cols 8-16 ===");
    for x in 64..136 {
        let idx = 109 * width + x;
        let pixel = pixels[idx];
        let r = (pixel >> 16) & 0xFF;
        let g = (pixel >> 8) & 0xFF;
        let b = pixel & 0xFF;
        if x % 8 == 0 {
            print!("\n  X={:3} (col {:2}): ", x, x / 8);
        }
        print!("({:02X}{:02X}{:02X})", r, g, b);
    }
    println!();

    // Check line_state_index for various rows
    // This tells us which per-line scroll value is used
    // Since we can't call internal VDC methods, let's check the scroll lines directly
    println!("\n=== Per-line scroll values for text area ===");
    for y in [
        105, 109, 120, 130, 140, 141, 150, 157, 160, 170, 180, 200, 230, 239,
    ] {
        if emu.bus.vdc_scroll_line_valid(y) {
            let (sx, sy) = emu.bus.vdc_scroll_line(y);
            println!("  Line {:3}: BXR={:04X} BYR={:04X} valid=true", y, sx, sy);
        } else {
            println!("  Line {:3}: not latched (valid=false)", y);
        }
    }

    // Check if overscan color is blue
    let overscan_rgb = emu.bus.vce_palette_rgb(0x100);
    let r = (overscan_rgb >> 16) & 0xFF;
    let g = (overscan_rgb >> 8) & 0xFF;
    let b = overscan_rgb & 0xFF;
    println!(
        "\nOverscan color (palette 0x100): RGB({},{},{}) #{:02X}{:02X}{:02X}",
        r, g, b, r, g, b
    );

    // Check the exact color #0000FF - which palette entry produces it?
    println!("\n=== Searching for #0000FF in palette ===");
    for i in 0..512 {
        let rgb = emu.bus.vce_palette_rgb(i);
        if rgb == 0x0000FF || rgb == 0xFF0000FF {
            let raw = emu.bus.vce_palette_word(i);
            println!(
                "  Palette[{:3}] (0x{:03X}): raw={:04X} → #0000FF",
                i, i, raw
            );
        }
    }

    // Check what the control_line value is for the text area
    // This tells us if BG is enabled for each line
    println!("\n=== Control line values ===");
    for y in [0, 50, 100, 109, 120, 130, 140, 141, 157, 200, 239] {
        let ctrl = emu.bus.vdc_control_line(y);
        let bg_en = (ctrl & 0x80) != 0;
        let spr_en = (ctrl & 0x40) != 0;
        let rcr_en = (ctrl & 0x10) != 0;
        let vbl_en = (ctrl & 0x08) != 0;
        println!(
            "  Line {:3}: ctrl={:04X} BG={} SPR={} RCR_IRQ={} VBL_IRQ={}",
            y, ctrl, bg_en, spr_en, rcr_en, vbl_en
        );
    }

    Ok(())
}
