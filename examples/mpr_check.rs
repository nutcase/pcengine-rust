use pce::emulator::Emulator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let rom = std::fs::read("roms/Power League III (Japan).pce")?;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom)?;
    emu.reset();

    // Check MPR values after reset
    println!("MPR values after reset:");
    for i in 0..8 {
        println!("  MPR[{}] = ${:02X}", i, emu.bus.mpr(i));
    }

    // Run a few frames
    let mut frames = 0;
    let max_ticks = 200_000u64;
    for _ in 0..max_ticks {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
            if frames == 1 {
                println!("\nMPR values after frame 1:");
                for i in 0..8 {
                    println!("  MPR[{}] = ${:02X}", i, emu.bus.mpr(i));
                }
            }
            if frames == 5 {
                println!("\nMPR values after frame 5:");
                for i in 0..8 {
                    println!("  MPR[{}] = ${:02X}", i, emu.bus.mpr(i));
                }

                // Read VDC status through direct I/O vs through $0000
                let vdc_io = emu.bus.read_io(0x00);
                let mem_0000 = emu.bus.read(0x0000);
                println!("\nVDC status via read_io(0x00): ${:02X}", vdc_io);
                println!("Memory at $0000 via read(): ${:02X}", mem_0000);
                println!("VDC status match: {}", vdc_io == mem_0000);
                break;
            }
        }
        if emu.cpu.halted {
            break;
        }
    }

    Ok(())
}
