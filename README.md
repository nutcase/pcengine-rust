# PC Engine Emulator (Rust)

Rust implementation of a PC Engine / TurboGrafx-16 emulator (HuC6280 + HuC6270 + HuC6260 + PSG path).

Current focus is correctness-first bring-up using `roms/Kato-chan & Ken-chan (Japan).pce`, with SDL preview tooling for rapid iteration.

## Implemented
- HuC6280 core with broad opcode coverage, interrupts (`IRQ1/IRQ2/TIMER/NMI`), `WAI/RTI`, block transfer instructions, and MPR banking.
- Hardware page decoding for VDC/VCE/PSG/timer/IRQ on `$FF` mapped I/O segments.
- HuC6270 VDC background + sprite rendering pipeline, per-line control latching, scroll/zoom handling, SATB, and DMA status behaviour.
- HuC6260 VCE palette register path with indexed access and RGB conversion.
- HuC6280 PSG register model and sample generation path, plus SDL audio playback examples.
- HuCard loader (`.pce`) with optional header handling and initial bank mapping.
- Backup RAM load/save flow for HuCard runs.

## Quick Start
Preferred launcher:
```bash
./run.sh
./run.sh "roms/Kato-chan & Ken-chan (Japan).pce"
./run.sh --debug "roms/Kato-chan & Ken-chan (Japan).pce"
```

Direct CLI:
```bash
cargo run -- roms/<game>.pce
cargo run -- path/to/program.bin
```

Useful options:
```bash
cargo run -- roms/<game>.pce --load-backup saves/game.sav --save-backup saves/game.sav --frame-limit 2
```

- `.bin` programs load at `$C000`.
- `.pce` HuCards auto-load/save `ROM_NAME.sav` unless explicitly overridden.

## SDL Front-Ends
```bash
cargo run --example video_sdl --features video-sdl -- roms/<game>.pce
cargo run --example audio_sdl --features audio-sdl -- roms/<game>.pce
```

Controls in `video_sdl`:
- D-pad: arrow keys
- Button I / II: `Z` / `X`
- Select: `Shift`
- Run: `Enter` or `Space`
- Save state: `Ctrl + 0..9`
- Load state: `0..9`
- Quit: `Esc`
- Save/Load result: HUD toast (`SAVE n OK` / `LOAD n ERR` など)

State files are persisted under `states/<rom_name>.slotN.state`.
`video_sdl` 起動中のみ有効で、スロットは `0` から `9` です。

`pc_engine`（ゲーム画面+チートパネル）では、ゲーム操作中は SDL テキスト入力を停止し、
チートパネル入力は ASCII のみ受け付けます（IME 合成入力は無効）。

## Build Notes
- `sdl2` is built with the `bundled` feature.
- This repo includes `.cargo/config.toml` with:
  - `CMAKE_POLICY_VERSION_MINIMUM=3.5`
- That setting is required for newer CMake versions that reject old policy defaults used by `sdl2-sys`.

## Development Commands
```bash
cargo fmt
cargo clippy --all-targets --all-features -D warnings
cargo test
cargo run --example dump_frame -- roms/<game>.pce
cargo run --example trace_boot -- roms/<game>.pce
```

## Known Limitations
- Audio timing/mixing is still being tuned; BGM tempo stability and residual noise are under active investigation.
- Some VDC edge cases (exact per-line behaviour and game-specific quirks) are still being refined.
- CD-ROM subsystem and debugger UI are not implemented yet.

See `TODO.md` for the detailed roadmap and current priorities.

## License

This project is for educational purposes.
