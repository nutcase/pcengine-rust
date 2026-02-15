/// Dump sprite attributes during gameplay to identify rendering artifacts.
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

    // Run to frame 300 (should be in gameplay after pressing start)
    let target_frame: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);

    let mut frames = 0u64;
    while frames < target_frame {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Dump all 64 sprite attributes from SATB
    println!("=== Sprite Attributes at frame {} ===", frames);
    println!("  #  |    Y   |    X   | Pattern | Attr   | Size    | Priority | Flips");
    println!("-----+--------+--------+---------+--------+---------+----------+------");
    for i in 0..64 {
        let y_word = emu.bus.vdc_satb_word(i * 4);
        let x_word = emu.bus.vdc_satb_word(i * 4 + 1);
        let pattern = emu.bus.vdc_satb_word(i * 4 + 2);
        let attr = emu.bus.vdc_satb_word(i * 4 + 3);

        let y = (y_word & 0x03FF) as i32 - 64;
        let x = (x_word & 0x03FF) as i32 - 32;
        let w = if (attr & 0x0100) != 0 { 32 } else { 16 };
        let h_code = ((attr >> 12) & 0x03) as usize;
        let h = match h_code {
            0 => 16,
            1 => 32,
            _ => 64,
        };
        let pri = if (attr & 0x0080) != 0 { "HI" } else { "LO" };
        let h_flip = if (attr & 0x0800) != 0 { "H" } else { "-" };
        let v_flip = if (attr & 0x8000) != 0 { "V" } else { "-" };
        let pal = attr & 0x000F;
        let pat_idx = (pattern >> 1) & 0x03FF;

        // Only show sprites that are on-screen
        if y > -64 && y < 240 && x > -32 && x < 256 {
            println!(
                " {:2}  | {:5}  | {:5}  |  {:04X}   | P{:<2} {}  | {:2}x{:<2}   | {:2}       | {}{}",
                i, y, x, pat_idx, pal, pri, w, h, pri, h_flip, v_flip
            );
        }
    }

    // Check for sprites in the sky area (y < 80 approximately)
    println!("\n=== Sprites visible in sky area (y < 80) ===");
    for i in 0..64 {
        let y_word = emu.bus.vdc_satb_word(i * 4);
        let x_word = emu.bus.vdc_satb_word(i * 4 + 1);
        let pattern = emu.bus.vdc_satb_word(i * 4 + 2);
        let attr = emu.bus.vdc_satb_word(i * 4 + 3);

        let y = (y_word & 0x03FF) as i32 - 64;
        let x = (x_word & 0x03FF) as i32 - 32;
        let h_code = ((attr >> 12) & 0x03) as usize;
        let h = match h_code {
            0 => 16,
            1 => 32,
            _ => 64,
        };

        if y < 80 && y + h as i32 > 0 && x > -32 && x < 256 {
            let pat_idx = (pattern >> 1) & 0x03FF;
            println!(
                "  Sprite {:2}: Y={:4} X={:4} pat={:04X} attr={:04X} (raw: Y={:04X} X={:04X} P={:04X} A={:04X})",
                i, y, x, pat_idx, attr, y_word, x_word, pattern, attr
            );
        }
    }

    Ok(())
}
