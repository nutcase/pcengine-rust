# PC Engine Emulator Roadmap

Goal: boot and run `roms/Kato-chan & Ken-chan (Japan).pce` with accurate CPU, memory, and video/audio behaviour.

## Stage 1 – CPU Fidelity & Core Timing
- [x] Implement `BIT` instruction variants with correct flag behaviour.
- [x] Implement shift/rotate instructions (`ASL`, `LSR`, `ROL`, `ROR`) for accumulator and memory modes.
- [x] Implement `STZ`, `TSB`, and `TRB` across addressing modes.
- [x] Implement `RTI`/`WAI` and interrupt request handling scaffold.
- [x] Implement HuC6280 block move instructions (`TII`, `TIN`, `TDD`, `TIA`, `TAI`) with stack usage and alternation semantics.
- [x] Implement HuC6280 memory page register transfers (`TMA`, `TAM`).
- [x] Implement `SMB`/`RMB` bit set/reset instructions.
- [x] Implement `BBS`/`BBR` branch-on-bit instructions.
- [x] Implement `CSH`/`CSL`, `SET`, `CLA/CLX/CLY`, `SAX/SAY/SXY`, and `BSR`.
- [x] Allow CPU to service any pending bus IRQ automatically without manual flag poking.
- [x] Validate block move cycle timing and hardware edge cases (e.g., I/O register accesses, zero-length transfers).
- [x] Add full interrupt handling for NMI, IRQ1/IRQ2, and timer sources; wire status register pushes/pops and vector fetch.
- [x] Introduce instruction cycle tables for all currently implemented HuC6280 opcodes.
- [x] Verify cycle timing against diagnostics (reference subset tests and timing examples).

## Stage 2 – Memory Map & Devices
- [x] Wire `$FF80`–`$FF87` memory-mapped MPR registers and basic I/O page backing.
- [x] Add stub storage for `$FF00`–`$FF7F` HuC6280 I/O registers to unblock device wiring.
- [x] Capture VDC register writes via `ST0`/`ST1`/`ST2` into an emulated register file.
- [x] Surface VDC status flags and route IRQ1 through the shared interrupt request register.
- [x] Provide an initial VBlank/DS status handshake so boot code observes ready VDC state.
- [x] Respect VDC auto-increment configuration when accessing VRAM through CPU data ports.
- [x] Emit stubbed VBlank IRQs on a cycle accumulator so CPU polling sees recurring frame events.
- [x] Trigger IRQ1 automatically for VBlank/RCR events with priority-preserving acknowledgement.
- [x] Map MPR value `$FF` to the hardware register page so memory accesses hit the emulated VDC/PSG/timer.
- [x] Model HuC6270 VRAM↔VRAM DMA and SATB DMA, including busy timing, status flags, and auto-transfer.
- [x] Stub HuC6260 VCE palette registers with indexed reads/writes and auto-increment behaviour.
- [x] Flesh out HuC6280 address decoding for VDC/VCE/PSG/timer registers and external peripherals (with real side-effects).
- [x] Implement bank registers and mirror behaviour for the MPR0–MPR7 memory page registers.
- [x] Interpret HuC6280 MPR values using hardware encodings (0xF8–0xFD for work RAM, direct ROM page indices, 0xFF for I/O) so TAM/TMA and memory-mapped writes match console behaviour.
- [x] Support `.pce` image header parsing to configure initial bank layout and detect save RAM requirements.

## Stage 3 – Video & Audio
- [x] Implement VDC (HuC6270) tile/sprite rendering pipeline with line timing, proper priority, and VCE colour output.
  - [x] Add sprite priority/background-blend rules.
  - [x] Implement sprite size encoding and clipping to act like hardware.
  - [x] Honour VDC scroll registers per scanline.
  - [x] Honour background zoom registers (per-axis scaling with tests).
  - [x] Model VDC DMA handshakes (SATB auto-transfer DS flag, DCR-driven CRAM uploads with DV/busy timing).
- [x] Track per-scanline sprite density and raise the OR status flag when more than 16 sprites overlap one scanline (with test coverage).
- [ ] Integrate framebuffer presentation with SDL, keep video preview toggles in README up to date.
- [x] Implement PSG (HuC6280 APU) channels and timers; provide audio mixing interface.
- [ ] Integrate a host front-end (e.g., SDL2) for video/audio/output and controller input.

## Stage 4 – Tooling & UX
- [ ] Add debugger hooks (breakpoints, register/memory inspection, VRAM viewer).
- [ ] Provide config handling for input bindings, scaling, and performance tuning.
- [ ] Ship basic CI with ROM-independent tests and linting.

### Current Sprint TODO
- [x] Add `BIT` instruction variants with flag behaviour matching HuC6280, including tests covering zero, negative, and overflow flag updates.
- [x] Fill out shift/rotate and stack push/pull instructions with coverage.
- [x] Implement `STZ`/`TSB`/`TRB` plus `RTI`/`WAI` with interrupt scaffolding.
- [x] Implement HuC6280 block moves with unit coverage for each addressing pattern.
- [x] Validate block move cycle timing against reference hardware behaviour.
