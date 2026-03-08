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
- Ten no Koe 2 BRAM (2KB) emulation: `MPR=$F7`, lock/unlock via `$1803/$1807`, file persistence.

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
cargo run -- roms/<game>.pce --load-backup saves/game.sav --save-backup saves/game.sav --load-bram saves/game.brm --save-bram saves/game.brm --frame-limit 2
```

- `.bin` programs load at `$C000`.
- `.pce` HuCards auto-load/save `ROM_NAME.sav` (HuCard backup RAM) and `ROM_NAME.brm` (Ten no Koe 2 BRAM) unless explicitly overridden.

## SDL Front-Ends
```bash
cargo run --example video_sdl --features video-sdl -- roms/<game>.pce
cargo run --example audio_sdl --features audio-sdl -- roms/<game>.pce
```

Controls in `video_sdl`:
- D-pad: arrow keys
- Button I / II: `Z` / `X`
- Rapid fire (about 22 shots/sec): `A` (I), `S` (II)
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

`pc_engine` のデバッガータブ:
- Pause/Run: 実行の停止/再開
- Step: 1命令ステップ実行
- Breakpoints: PC アドレス (`$C000` 形式) で追加/削除
- VRAM Viewer: 8x8 タイルの先頭 512 枚を表示（Refresh/Auto）

### Config (`pce_config.json`)
`video_sdl` と `pc_engine` は `--config path.json` で JSON 設定を読み込みます。
カレントディレクトリに `pce_config.json` があれば自動で読み込みます。

```json
{
  "window_scale": 3,
  "panel_width": 420,
  "input": {
    "up": "Up",
    "down": "Down",
    "left": "Left",
    "right": "Right",
    "button_i": "Z",
    "button_ii": "X",
    "rapid_i": "A",
    "rapid_ii": "S",
    "select": "LShift",
    "run": "Return"
  },
  "performance": {
    "audio_batch": 512,
    "emu_audio_batch": 128,
    "audio_queue_min": 1024,
    "audio_queue_target": 2048,
    "audio_queue_max": 3072,
    "audio_queue_critical": 512,
    "max_emu_steps_per_pump": 120000,
    "max_steps_after_frame": 30000,
    "max_present_interval_ms": 33,
    "auto_fire_hz": 22
  }
}
```

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
- CD-ROM subsystem is not implemented yet.


## License

This project is for educational purposes.
