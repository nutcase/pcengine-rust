#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::bus::{
    VDC_STATUS_BUSY, VDC_STATUS_CR, VDC_STATUS_DS, VDC_STATUS_DV, VDC_STATUS_OR, VDC_STATUS_RCR,
    VDC_STATUS_VBL,
};
use pce::emulator::Emulator;
use std::{env, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let path = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("Usage: vdc_inspect <program.[bin|pce]> [cycles]");
            return Ok(());
        }
    };
    let cycle_limit: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(200_000);

    let rom = fs::read(&path)?;
    let mut emulator = Emulator::new();
    let is_pce = Path::new(&path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);

    if is_pce {
        emulator.load_hucard(&rom)?;
    } else {
        emulator.load_program(0xC000, &rom);
    }

    emulator.reset();
    emulator.run_until_halt(Some(cycle_limit));

    println!("Cycles executed: {}", emulator.cycles());
    println!(
        "Status: halted={} waiting={} pending_irq={:#04X}",
        emulator.cpu.halted,
        emulator.cpu.is_waiting(),
        emulator.bus.pending_interrupts()
    );

    let vdc_status = emulator.bus.read_io(0x00);
    println!(
        "VDC status {:02X}: {}",
        vdc_status,
        describe_status(vdc_status)
    );

    println!("\nVDC registers:");
    for index in 0..32 {
        if let Some(value) = emulator.bus.vdc_register(index) {
            println!("R{:02X}: {:04X}", index, value);
        }
    }

    let (map_w, map_h) = emulator.bus.vdc_map_dimensions();
    println!("\nDerived state:");
    println!("Map size: {}x{} tiles", map_w, map_h);
    println!(
        "Scroll X: {:04X}",
        emulator.bus.vdc_register(0x07).unwrap_or(0)
    );
    println!(
        "Scroll Y: {:04X}",
        emulator.bus.vdc_register(0x08).unwrap_or(0)
    );
    if let Some(ctrl) = emulator.bus.vdc_register(0x05) {
        println!("Control: {:04X} ({})", ctrl, describe_control(ctrl));
    }
    if let Some(mwr) = emulator.bus.vdc_register(0x09) {
        println!("MWR: {:04X} ({})", mwr, describe_mwr(mwr, map_w, map_h));
    }
    if let Some(increment) = emulator.bus.vdc_register(0x05) {
        let step = vram_increment(increment);
        println!("VRAM increment step: {}", step);
    }

    Ok(())
}

fn describe_status(status: u8) -> String {
    let mut flags = Vec::new();
    if status & VDC_STATUS_CR != 0 {
        flags.push("CR");
    }
    if status & VDC_STATUS_OR != 0 {
        flags.push("OR");
    }
    if status & VDC_STATUS_RCR != 0 {
        flags.push("RCR");
    }
    if status & VDC_STATUS_DS != 0 {
        flags.push("DS");
    }
    if status & VDC_STATUS_DV != 0 {
        flags.push("DV");
    }
    if status & VDC_STATUS_VBL != 0 {
        flags.push("VBL");
    }
    if status & VDC_STATUS_BUSY != 0 {
        flags.push("BUSY");
    }
    if flags.is_empty() {
        "none".to_string()
    } else {
        flags.join("|")
    }
}

fn describe_control(control: u16) -> String {
    let mut parts: Vec<String> = Vec::new();
    if control & 0x0001 != 0 {
        parts.push("IRQ-CR".to_string());
    }
    if control & 0x0002 != 0 {
        parts.push("IRQ-OR".to_string());
    }
    if control & 0x0004 != 0 {
        parts.push("IRQ-RCR".to_string());
    }
    if control & 0x0008 != 0 {
        parts.push("IRQ-VBL".to_string());
    }
    parts.push(if control & 0x0040 != 0 {
        "SPR=on".to_string()
    } else {
        "SPR=off".to_string()
    });
    parts.push(if control & 0x0080 != 0 {
        "BG=on".to_string()
    } else {
        "BG=off".to_string()
    });
    let increment = match (control >> 11) & 0x03 {
        0 => "+1",
        1 => "+32",
        2 => "+64",
        _ => "+128",
    };
    parts.push(format!("INC={}", increment));
    parts.join(" ")
}

fn describe_mwr(mwr: u16, map_w: usize, map_h: usize) -> String {
    let pixel_mode = match mwr & 0x03 {
        0 => "CPU-friendly",
        1 => "Balanced",
        2 => "Pattern-heavy",
        _ => "Single-plane",
    };
    let sprite_mode = match (mwr >> 2) & 0x03 {
        0 => "2 sprites/cycle",
        1 => "dual-cycle",
        2 => "1 sprite/cycle",
        _ => "1 sprite/cycle (half planes)",
    };
    let cg_mode = if (mwr & 0x80) != 0 { "CG1" } else { "CG0" };
    format!(
        "map={}x{} tiles, pixel={}, sprite={}, CG={}",
        map_w, map_h, pixel_mode, sprite_mode, cg_mode
    )
}

fn vram_increment(control: u16) -> u16 {
    match (control >> 11) & 0x03 {
        0 => 1,
        1 => 32,
        2 => 64,
        _ => 128,
    }
}
