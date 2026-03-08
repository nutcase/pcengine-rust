#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::emulator::Emulator;

fn main() {
    let mut emu = Emulator::new();
    emu.reset();

    loop {
        emu.tick();
        if let Some(samples) = emu.take_audio_samples() {
            println!("{} audio samples ready", samples.len());
        }

        if emu.cpu.halted {
            break;
        }
    }
}
