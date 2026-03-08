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
    while frames < 150 {
        emu.tick();
        if let Some(_) = emu.take_frame() {
            frames += 1;
        }
    }

    // Read timer control via I/O port
    // Timer counter: $0C00, Timer control: $0C01
    let timer_counter = emu.bus.read(0x0C00);
    let timer_control = emu.bus.read(0x0C01);
    println!("Timer counter: {:#04X} ({})", timer_counter, timer_counter);
    println!(
        "Timer control: {:#04X} (enabled={})",
        timer_control,
        timer_control & 1 != 0
    );

    // Check interrupt disable register
    // $1402 = interrupt disable register
    let irq_disable = emu.bus.read(0x1402);
    println!("IRQ disable register: {:#04X}", irq_disable);
    println!("  IRQ2 (VDC) disabled: {}", irq_disable & 1 != 0);
    println!("  IRQ1 (VDC) disabled: {}", irq_disable & 2 != 0);
    println!("  Timer disabled: {}", irq_disable & 4 != 0);

    Ok(())
}
