/// Trace music engine: count PSG writes per second and dump music state.
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

    // Run 200 frames
    let mut frames = 0u64;
    while frames < 200 {
        emu.tick();
        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    // Dump work RAM at $2200-$2230 (music driver state)
    println!("=== Music State at $2200 (after {} frames) ===", frames);
    for row in 0..4 {
        let base: u16 = 0x2200 + row * 16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    // Dump zero-page (actually at $2000-$20FF in work RAM)
    println!("\n=== Work RAM Zero Page ($2000-$203F) ===");
    for row in 0..4 {
        let base: u16 = 0x2000 + row * 16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    // Now run 120 more frames and track ALL PSG register writes
    // (We need to hook into I/O writes. Since we can't directly, we'll
    //  track PSG state changes per frame instead.)
    println!("\n=== Tracking PSG state over 120 frames ===");
    let measure_frames = 120u64;
    let start_frame = frames;

    let mut prev_psg_state = [(0u16, 0u8, 0u8, 0u8); 6];
    for ch in 0..6 {
        prev_psg_state[ch] = emu.bus.psg_channel_info(ch);
    }
    let mut prev_main_bal = emu.bus.psg_main_balance();

    let mut psg_changes = [0u64; 6];
    let mut psg_freq_writes = [0u64; 6];
    let mut psg_ctrl_writes = [0u64; 6];
    let mut psg_bal_writes = [0u64; 6];
    let mut total_psg_events = 0u64;

    while frames < start_frame + measure_frames {
        emu.tick();

        // Check PSG state changes (approximation of writes)
        for ch in 0..6 {
            let info = emu.bus.psg_channel_info(ch);
            if info.0 != prev_psg_state[ch].0 {
                // freq changed
                psg_freq_writes[ch] += 1;
                total_psg_events += 1;
            }
            if info.1 != prev_psg_state[ch].1 {
                // control changed
                psg_ctrl_writes[ch] += 1;
                total_psg_events += 1;
            }
            if info.2 != prev_psg_state[ch].2 {
                // balance changed
                psg_bal_writes[ch] += 1;
                total_psg_events += 1;
            }
            if info != prev_psg_state[ch] {
                psg_changes[ch] += 1;
            }
            prev_psg_state[ch] = info;
        }

        if emu.take_frame().is_some() {
            frames += 1;
        }
        if emu.cpu.halted {
            break;
        }
    }

    let elapsed = (frames - start_frame) as f64 / 60.0;
    println!(
        "Total PSG state changes: {} ({:.1}/sec)",
        total_psg_events,
        total_psg_events as f64 / elapsed
    );
    println!("\nPer-channel stats (over {:.2}s):", elapsed);
    println!("CH  FreqChg  CtrlChg  BalChg   Total   /sec");
    for ch in 0..6 {
        let total = psg_changes[ch];
        println!(
            "{}   {:5}    {:5}    {:5}   {:5}   {:.1}",
            ch,
            psg_freq_writes[ch],
            psg_ctrl_writes[ch],
            psg_bal_writes[ch],
            total,
            total as f64 / elapsed
        );
    }

    // Dump final music state
    println!("\n=== Music State at $2200 (after {} frames) ===", frames);
    for row in 0..4 {
        let base: u16 = 0x2200 + row * 16;
        print!("${:04X}: ", base);
        for col in 0..16 {
            print!("{:02X} ", emu.bus.read(base + col));
        }
        println!();
    }

    // Also check the music engine function area
    println!("\n=== Music Engine $EDAF area ===");
    let mut pc: u16 = 0xEDAF;
    for _ in 0..20 {
        let b = emu.bus.read(pc);
        print!("{:02X} ", b);
        pc = pc.wrapping_add(1);
    }
    println!();

    // Check $ED00-$ED10 too (music data area?)
    println!("\n=== $ED90-$EDB0 ===");
    for addr in (0xED90u16..0xEDB0).step_by(16) {
        print!("${:04X}: ", addr);
        for i in 0..16 {
            print!("{:02X} ", emu.bus.read(addr + i));
        }
        println!();
    }

    Ok(())
}
