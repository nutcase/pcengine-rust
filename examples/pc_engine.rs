#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
#[path = "common/config.rs"]
mod config;
mod egui_ui;
#[path = "common/hud_toast.rs"]
mod hud_toast;

use config::{AppConfig, ParsedBindings};
use egui_ui::debugger::{CpuSnapshot, DebuggerAction, VdcSnapshot, describe_break};
use egui_ui::gl_game::GlGameRenderer;
use egui_ui::{CheatToolUi, DebuggerPanelData};
use hud_toast::{HudToast, draw_hud_toast, show_hud_toast};
use pce::bus::AudioDiagnostics;
use pce::debugger::{DebugTick, Debugger};
use pce::emulator::Emulator;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui_sdl2_gl::DpiScaling;
use egui_sdl2_gl::ShaderVersion;
use egui_sdl2_gl::gl;

const PANEL_WIDTH_MIN: f32 = 300.0;

#[derive(Clone, Copy, Default)]
struct QueueWriteStats {
    queued: usize,
    dropped: usize,
}

struct AudioQueueDiag {
    enabled: bool,
    start: Instant,
    last_report: Instant,
    last_bus_diag: AudioDiagnostics,
    queued_samples_interval: u64,
    dropped_samples_interval: u64,
    queue_low_hits: u64,
    queue_zero_hits: u64,
    queue_min: usize,
    queue_max: usize,
}

impl AudioQueueDiag {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            start: Instant::now(),
            last_report: Instant::now(),
            last_bus_diag: AudioDiagnostics::default(),
            queued_samples_interval: 0,
            dropped_samples_interval: 0,
            queue_low_hits: 0,
            queue_zero_hits: 0,
            queue_min: usize::MAX,
            queue_max: 0,
        }
    }

    fn record_queue_level(&mut self, queued: usize, low_threshold: usize) {
        if !self.enabled {
            return;
        }
        self.queue_min = self.queue_min.min(queued);
        self.queue_max = self.queue_max.max(queued);
        if queued < low_threshold {
            self.queue_low_hits = self.queue_low_hits.saturating_add(1);
        }
        if queued == 0 {
            self.queue_zero_hits = self.queue_zero_hits.saturating_add(1);
        }
    }

    fn record_write(&mut self, stats: QueueWriteStats) {
        if !self.enabled {
            return;
        }
        self.queued_samples_interval = self
            .queued_samples_interval
            .saturating_add(stats.queued as u64);
        self.dropped_samples_interval = self
            .dropped_samples_interval
            .saturating_add(stats.dropped as u64);
    }

    fn maybe_report(&mut self, emulator: &Emulator, queued: usize) {
        if !self.enabled {
            return;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_report);
        if elapsed < Duration::from_secs(1) {
            return;
        }

        let dt = elapsed.as_secs_f64().max(f64::EPSILON);
        let current_bus = emulator.bus.audio_diagnostics();
        let delta_generated = current_bus
            .generated_samples
            .saturating_sub(self.last_bus_diag.generated_samples);
        let delta_drained = current_bus
            .drained_samples
            .saturating_sub(self.last_bus_diag.drained_samples);

        eprintln!(
            "[pc_engine {:.1}s] q={} min={} max={} bus_gen={:.1}/s bus_drain={:.1}/s queued={:.1}/s dropped={:.1}/s low_hits={} zero_hits={} bus_pending={} emu_pending={} phi_rem={}",
            now.duration_since(self.start).as_secs_f64(),
            queued,
            if self.queue_min == usize::MAX {
                queued
            } else {
                self.queue_min
            },
            self.queue_max,
            delta_generated as f64 / dt,
            delta_drained as f64 / dt,
            self.queued_samples_interval as f64 / dt,
            self.dropped_samples_interval as f64 / dt,
            self.queue_low_hits,
            self.queue_zero_hits,
            current_bus.pending_bus_samples,
            emulator.pending_audio_samples(),
            current_bus.phi_remainder,
        );

        self.last_report = now;
        self.last_bus_diag = current_bus;
        self.queued_samples_interval = 0;
        self.dropped_samples_interval = 0;
        self.queue_low_hits = 0;
        self.queue_zero_hits = 0;
        self.queue_min = usize::MAX;
        self.queue_max = queued;
    }
}

fn main() -> Result<(), String> {
    const AUDIO_SAMPLES_PER_FRAME: usize = 44_100 / 60;

    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let (config_path, args) = config::parse_config_path(&raw_args);
    let mut args = args.into_iter();
    let rom_path = args
        .next()
        .ok_or_else(|| "usage: video_sdl_egui <rom.[bin|pce]> [--config path.json]".to_string())?;
    let rom = std::fs::read(&rom_path)
        .map_err(|err| format!("failed to read ROM {}: {err}", rom_path))?;

    let config_path = config_path.map(std::path::PathBuf::from).or_else(|| {
        let path = std::path::PathBuf::from("pce_config.json");
        path.exists().then_some(path)
    });
    let config = AppConfig::load(config_path.as_deref());
    let perf = &config.performance;
    let bindings = ParsedBindings::from_input(&config.input)
        .ok_or_else(|| "invalid key binding in config".to_string())?;
    let auto_fire_period_ns = 1_000_000_000u128 / perf.auto_fire_hz.max(1);

    let mut emulator = Emulator::new();
    let is_pce = Path::new(&rom_path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);
    let backup_path = Path::new(&rom_path).with_extension("sav");
    let bram_path = Path::new(&rom_path).with_extension("brm");
    if is_pce {
        emulator
            .load_hucard(&rom)
            .map_err(|err| format!("failed to load HuCard: {err}"))?;
        if backup_path.exists() {
            match std::fs::read(&backup_path) {
                Ok(bytes) => {
                    if let Err(err) = emulator.load_backup_ram(&bytes) {
                        eprintln!(
                            "warning: failed to load backup RAM from {}: {err}",
                            backup_path.display()
                        );
                    }
                }
                Err(err) => eprintln!(
                    "warning: could not read backup RAM file {}: {err}",
                    backup_path.display()
                ),
            }
        }
        if bram_path.exists() {
            match std::fs::read(&bram_path) {
                Ok(bytes) => {
                    if let Err(err) = emulator.load_bram(&bytes) {
                        eprintln!(
                            "warning: failed to load BRAM from {}: {err}",
                            bram_path.display()
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "warning: could not read BRAM file {}: {err}",
                        bram_path.display()
                    )
                }
            }
        }
    } else {
        emulator.load_program(0xC000, &rom);
    }
    emulator.set_audio_batch_size(perf.emu_audio_batch);
    emulator.reset();

    let mut current_width = emulator.display_width();
    let mut current_height = emulator.display_height();
    let game_h = (current_height as u32) * config.window_scale;
    let game_w = game_h * 4 / 3;

    let sdl = sdl2::init().map_err(|e| e.to_string())?;
    let audio_subsystem = sdl.audio().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;

    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attr.set_context_version(3, 2);
    gl_attr.set_double_buffer(true);
    gl_attr.set_multisample_samples(0);

    let mut window = video
        .window("PC Engine + Cheat Tool", game_w, game_h)
        .position_centered()
        .resizable()
        .opengl()
        .build()
        .map_err(|e| e.to_string())?;

    let _gl_context = window.gl_create_context().map_err(|e| e.to_string())?;
    window
        .gl_make_current(&_gl_context)
        .map_err(|e| e.to_string())?;

    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);

    // Disable VSync — emulator is audio-driven, VSync would block the tick loop.
    let _ = video.gl_set_swap_interval(sdl2::video::SwapInterval::Immediate);

    let (mut painter, mut egui_state) =
        egui_sdl2_gl::with_sdl2(&window, ShaderVersion::Default, DpiScaling::Default);
    let egui_ctx = egui::Context::default();

    // Audio
    let desired_audio = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1),
        samples: Some(perf.audio_batch as u16),
    };
    let audio_device = audio_subsystem
        .open_queue::<i16, _>(None, &desired_audio)
        .map_err(|e| e.to_string())?;
    audio_device.resume();
    let mut audio_diag = AudioQueueDiag::new(std::env::var_os("PCE_AUDIO_DIAG").is_some());
    if audio_diag.enabled {
        eprintln!("PCE_AUDIO_DIAG enabled for pc_engine");
    }

    let mut event_pump = sdl.event_pump().map_err(|e| e.to_string())?;
    let mut quit = false;
    let mut pressed: HashSet<Keycode> = HashSet::new();
    let mut frame_buf: Vec<u32> = Vec::new();
    let mut frame_buf_ready = false;
    let mut last_present = Instant::now();
    let mut last_loop_tick = Instant::now();
    let mut emu_frame_budget = Duration::ZERO;
    let mut hud_toast: Option<HudToast> = None;
    let auto_fire_epoch = Instant::now();
    let bound_keys = bindings.to_set();

    let mut game_renderer = GlGameRenderer::new();
    let mut cheat_ui = CheatToolUi::new();
    let mut debugger = Debugger::new();
    let mut prev_panel_visible = cheat_ui.panel_visible;
    let mut panel_width_px: u32 = config.panel_width.max(PANEL_WIDTH_MIN as u32);
    let text_input = video.text_input();
    let mut text_input_active = false;
    let mut audio_prefill = true;
    text_input.stop();

    let cheat_path = cheat_file_path(&rom_path);

    while !quit {
        let should_enable_text_input = cheat_ui.panel_visible;
        if should_enable_text_input != text_input_active {
            if should_enable_text_input {
                text_input.start();
            } else {
                text_input.stop();
            }
            text_input_active = should_enable_text_input;
        }

        egui_state.input.time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        );

        let egui_wants_kb = cheat_ui.panel_visible && egui_ctx.wants_keyboard_input();

        for event in event_pump.poll_iter() {
            // Forward to egui first so it can capture text input
            if cheat_ui.panel_visible {
                if let Some(filtered) = filter_event_for_ascii_text_input(&event) {
                    egui_state.process_input(&window, filtered, &mut painter);
                }
            }

            match &event {
                Event::Quit { .. } => quit = true,
                Event::KeyDown {
                    keycode: Some(code),
                    keymod,
                    repeat: false,
                    ..
                } => {
                    let code = *code;
                    let keymod = *keymod;

                    if code == Keycode::Tab {
                        cheat_ui.panel_visible = !cheat_ui.panel_visible;
                        continue;
                    }

                    // Skip game hotkeys when egui text fields have focus
                    if egui_wants_kb {
                        continue;
                    }

                    if let Some(slot) = state_slot_from_keycode(code) {
                        let ctrl_pressed = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);
                        let state_path = state_file_path(&rom_path, slot);
                        if ctrl_pressed {
                            if let Some(parent) = state_path.parent() {
                                if let Err(err) = std::fs::create_dir_all(parent) {
                                    eprintln!("Save failed: {err}");
                                    show_hud_toast(&mut hud_toast, "STATE DIR ERR");
                                    continue;
                                }
                            }
                            match emulator.save_state_to_file(&state_path) {
                                Ok(()) => {
                                    eprintln!("Saved slot {}", slot);
                                    show_hud_toast(&mut hud_toast, format!("SAVE {slot} OK"));
                                }
                                Err(err) => {
                                    eprintln!("Save failed: {err}");
                                    show_hud_toast(&mut hud_toast, format!("SAVE {slot} ERR"));
                                }
                            }
                        } else {
                            match emulator.load_state_from_file(&state_path) {
                                Ok(()) => {
                                    emulator.set_audio_batch_size(perf.emu_audio_batch);
                                    audio_device.clear();
                                    frame_buf_ready = false;
                                    last_present = Instant::now();
                                    last_loop_tick = Instant::now();
                                    emu_frame_budget = Duration::ZERO;
                                    audio_prefill = true;
                                    eprintln!("Loaded slot {}", slot);
                                    show_hud_toast(&mut hud_toast, format!("LOAD {slot} OK"));
                                }
                                Err(err) => {
                                    eprintln!("Load failed: {err}");
                                    show_hud_toast(&mut hud_toast, format!("LOAD {slot} ERR"));
                                }
                            }
                        }
                    } else if bound_keys.contains(&code) {
                        pressed.insert(code);
                    }
                }
                Event::KeyUp {
                    keycode: Some(code),
                    repeat: false,
                    ..
                } => {
                    if !egui_wants_kb {
                        pressed.remove(code);
                    }
                }
                _ => {}
            }
        }

        // Resize window on panel toggle
        if cheat_ui.panel_visible != prev_panel_visible {
            if cheat_ui.panel_visible {
                cheat_ui.refresh(emulator.work_ram());
            }
            let new_w = if cheat_ui.panel_visible {
                game_w + panel_width_px
            } else {
                game_w
            };
            let _ = window.set_size(new_w, game_h);
            prev_panel_visible = cheat_ui.panel_visible;
        }

        // Emulation tick (audio-driven) — skip when paused
        let auto_fire_on = auto_fire_phase_on(auto_fire_epoch, Instant::now(), auto_fire_period_ns);
        let button_i_pressed = pressed.contains(&bindings.button_i)
            || (pressed.contains(&bindings.rapid_i) && auto_fire_on);
        let button_ii_pressed = pressed.contains(&bindings.button_ii)
            || (pressed.contains(&bindings.rapid_ii) && auto_fire_on);
        let pad_state = build_pad_state(&pressed, &bindings, button_i_pressed, button_ii_pressed);
        emulator.bus.set_joypad_input(pad_state);

        let now = Instant::now();
        emu_frame_budget = (emu_frame_budget + now.saturating_duration_since(last_loop_tick))
            .min(Duration::from_millis(100));
        last_loop_tick = now;

        let mut steps = 0usize;
        let mut frame_seen = false;
        let allow_step = debugger.step_pending();
        let paused = cheat_ui.paused || (debugger.paused && !allow_step);
        if !paused {
            let mut queued_now = queued_samples(&audio_device);
            let frame_interval = Duration::from_micros(16_667);
            let mut frames_due =
                (emu_frame_budget.as_micros() / frame_interval.as_micros()) as usize;
            if queued_now <= perf.audio_queue_critical {
                audio_prefill = true;
            }
            if audio_prefill {
                let target_frames = perf.audio_queue_target.div_ceil(AUDIO_SAMPLES_PER_FRAME);
                let queued_frames = queued_now / AUDIO_SAMPLES_PER_FRAME;
                frames_due = frames_due.max(target_frames.saturating_sub(queued_frames));
            } else if queued_now < perf.audio_queue_min {
                frames_due = frames_due.max(1);
            }
            frames_due = frames_due.min(4);

            let mut completed_frames = 0usize;
            while completed_frames < frames_due && steps < perf.max_emu_steps_per_pump {
                match emulator.tick_debugger(&mut debugger) {
                    DebugTick::Ran(_) => {}
                    DebugTick::Paused => break,
                    DebugTick::Break(reason) => {
                        let label = describe_break(reason);
                        show_hud_toast(&mut hud_toast, label);
                        break;
                    }
                }
                steps += 1;
                if let Some(samples) = emulator.take_audio_samples() {
                    let write_stats =
                        queue_audio_samples(&audio_device, &samples, perf.audio_queue_max)?;
                    audio_diag.record_write(write_stats);
                    queued_now = queued_samples(&audio_device);
                }
                if emulator.take_frame_into(&mut frame_buf) {
                    frame_buf_ready = true;
                    frame_seen = true;
                    completed_frames = completed_frames.saturating_add(1);
                    emu_frame_budget = emu_frame_budget.saturating_sub(frame_interval);
                    if queued_now >= perf.audio_queue_target {
                        audio_prefill = false;
                    }
                }
                if frame_seen && steps >= perf.max_steps_after_frame && completed_frames > 0 {
                    break;
                }
            }
        }

        // Track display dimension changes
        let new_width = emulator.display_width();
        let new_height = emulator.display_height();
        if new_width != current_width || new_height != current_height {
            current_width = new_width;
            current_height = new_height;
        }

        // Apply cheats every iteration (to both work RAM and cart RAM)
        {
            let wram_len = emulator.work_ram().len();
            let mgr = &cheat_ui.cheat_search_ui.manager;
            for entry in &mgr.entries {
                if !entry.enabled {
                    continue;
                }
                let addr = entry.address as usize;
                if addr < wram_len {
                    emulator.work_ram_mut()[addr] = entry.value;
                } else if let Some(cram) = emulator.backup_ram_mut() {
                    let cram_addr = addr - wram_len;
                    if cram_addr < cram.len() {
                        cram[cram_addr] = entry.value;
                    }
                }
            }
        }

        // Upload game frame to GL texture
        if frame_buf_ready {
            draw_hud_toast(
                &mut frame_buf,
                current_width,
                current_height,
                &mut hud_toast,
            );
            game_renderer.upload_frame(&frame_buf, current_width, current_height);
            frame_buf_ready = false;
        }

        let queued = queued_samples(&audio_device);
        audio_diag.record_queue_level(queued, perf.audio_queue_critical);
        audio_diag.maybe_report(&emulator, queued);
        let present_interval_ms = if queued < perf.audio_queue_min {
            perf.max_present_interval_ms.saturating_mul(3)
        } else {
            perf.max_present_interval_ms
        };
        let should_present =
            frame_seen || last_present.elapsed() >= Duration::from_millis(present_interval_ms);

        if should_present {
            let (win_w, win_h) = window.size();

            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }

            // Draw game quad on the left; panel occupies the right
            let panel_px = if cheat_ui.panel_visible {
                panel_width_px
            } else {
                0
            };
            let game_vp_w = win_w.saturating_sub(panel_px);
            // GL viewport: game on left, full height
            game_renderer.draw(0, 0, game_vp_w as i32, win_h as i32);

            // Draw panel when visible — egui renders directly to the screen
            if cheat_ui.panel_visible {
                // Use full window for screen_rect so mouse coordinates map correctly
                painter.update_screen_rect((win_w, win_h));
                egui_state.input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(win_w as f32, win_h as f32),
                ));

                let mut ram_writes: Vec<(usize, u8)> = Vec::new();
                let wram = emulator.work_ram();
                let cram = emulator.backup_ram();

                let mut debug_action = DebuggerAction::None;
                let full_output = egui_ctx.run(egui_state.input.take(), |ctx| {
                    let panel_resp = egui::SidePanel::right("cheat_panel")
                        .resizable(true)
                        .min_width(PANEL_WIDTH_MIN)
                        .default_width(config.panel_width as f32)
                        .show(ctx, |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let debug_data = DebuggerPanelData {
                                        debugger: &debugger,
                                        cpu: CpuSnapshot {
                                            pc: emulator.cpu.pc,
                                            a: emulator.cpu.a,
                                            x: emulator.cpu.x,
                                            y: emulator.cpu.y,
                                            sp: emulator.cpu.sp,
                                            status: emulator.cpu.status,
                                            last_opcode: emulator.cpu.last_opcode(),
                                        },
                                        vdc: VdcSnapshot {
                                            status: emulator.bus.vdc_status_bits(),
                                            scanline: emulator.bus.vdc_current_scanline(),
                                            in_vblank: emulator.bus.vdc_in_vblank(),
                                            vram_dma_busy: emulator.bus.vdc_busy_cycles() > 0,
                                        },
                                        vram: emulator.bus.vdc_vram(),
                                        palette_rgb: &|idx| emulator.bus.vce_palette_rgb(idx),
                                        egui_ctx: ctx,
                                    };
                                    debug_action = cheat_ui.show_panel(
                                        ui,
                                        &mut ram_writes,
                                        wram,
                                        cram,
                                        Some(&cheat_path),
                                        Some(debug_data),
                                    );
                                });
                        });
                    // Track actual panel width for GL viewport
                    let actual_w = panel_resp.response.rect.width() as u32;
                    if actual_w != panel_width_px {
                        panel_width_px = actual_w;
                        // Resize window to match new panel width
                        let new_w = game_w + panel_width_px;
                        let _ = window.set_size(new_w, game_h);
                    }
                });

                if cheat_ui.refresh_requested {
                    cheat_ui.refresh(emulator.work_ram());
                    cheat_ui.refresh_requested = false;
                }

                let prims = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                painter.paint_jobs(None, full_output.textures_delta, prims);
                egui_state.process_output(&window, &full_output.platform_output);

                for (addr, val) in ram_writes {
                    let wram = emulator.work_ram_mut();
                    if addr < wram.len() {
                        wram[addr] = val;
                    }
                }

                match debug_action {
                    DebuggerAction::None => {}
                    DebuggerAction::TogglePause => {
                        debugger.paused = !debugger.paused;
                        show_hud_toast(
                            &mut hud_toast,
                            if debugger.paused { "PAUSE" } else { "RUN" },
                        );
                    }
                    DebuggerAction::Step => {
                        debugger.request_step();
                    }
                    DebuggerAction::ClearBreak => {
                        debugger.clear_break();
                    }
                    DebuggerAction::AddBreakpoint(pc) => {
                        debugger.add_breakpoint(pc);
                        show_hud_toast(&mut hud_toast, format!("BP ${pc:04X}"));
                    }
                    DebuggerAction::RemoveBreakpoint(pc) => {
                        debugger.remove_breakpoint(pc);
                        show_hud_toast(&mut hud_toast, format!("BP- ${pc:04X}"));
                    }
                }
            }

            window.gl_swap_window();
            last_present = Instant::now();
        }

        if queued < perf.audio_queue_min {
            std::thread::yield_now();
        } else if queued > perf.audio_queue_target {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            std::thread::yield_now();
        }
    }

    if is_pce {
        if let Some(snapshot) = emulator.save_backup_ram() {
            if let Err(err) = std::fs::write(&backup_path, snapshot) {
                eprintln!(
                    "warning: failed to write backup RAM to {}: {err}",
                    backup_path.display()
                );
            }
        }
        if let Err(err) = std::fs::write(&bram_path, emulator.save_bram()) {
            eprintln!(
                "warning: failed to write BRAM to {}: {err}",
                bram_path.display()
            );
        }
    }

    Ok(())
}

fn queued_samples(device: &AudioQueue<i16>) -> usize {
    device.size() as usize / std::mem::size_of::<i16>()
}

fn queue_audio_samples(
    device: &AudioQueue<i16>,
    samples: &[i16],
    audio_queue_max: usize,
) -> Result<QueueWriteStats, String> {
    let available = audio_queue_max.saturating_sub(queued_samples(device));
    if available == 0 {
        return Ok(QueueWriteStats {
            queued: 0,
            dropped: samples.len(),
        });
    }
    if samples.len() > available {
        device
            .queue_audio(&samples[..available])
            .map_err(|e| e.to_string())?;
        Ok(QueueWriteStats {
            queued: available,
            dropped: samples.len() - available,
        })
    } else {
        device.queue_audio(samples).map_err(|e| e.to_string())?;
        Ok(QueueWriteStats {
            queued: samples.len(),
            dropped: 0,
        })
    }
}

fn build_pad_state(
    pressed: &HashSet<Keycode>,
    bindings: &ParsedBindings,
    button_i_pressed: bool,
    button_ii_pressed: bool,
) -> u8 {
    let mut state: u8 = 0xFF;
    let mut clear = |bit: u8| state &= !(1 << bit);
    if pressed.contains(&bindings.up) {
        clear(0);
    }
    if pressed.contains(&bindings.right) {
        clear(1);
    }
    if pressed.contains(&bindings.down) {
        clear(2);
    }
    if pressed.contains(&bindings.left) {
        clear(3);
    }
    if button_i_pressed {
        clear(4);
    }
    if button_ii_pressed {
        clear(5);
    }
    if pressed.contains(&bindings.select) {
        clear(6);
    }
    if pressed.contains(&bindings.run) {
        clear(7);
    }
    state
}

fn auto_fire_phase_on(epoch: Instant, now: Instant, period_ns: u128) -> bool {
    let elapsed_ns = now.duration_since(epoch).as_nanos();
    let phase = elapsed_ns % period_ns;
    phase < (period_ns / 2)
}

fn state_slot_from_keycode(code: Keycode) -> Option<usize> {
    match code {
        Keycode::Num0 | Keycode::Kp0 => Some(0),
        Keycode::Num1 | Keycode::Kp1 => Some(1),
        Keycode::Num2 | Keycode::Kp2 => Some(2),
        Keycode::Num3 | Keycode::Kp3 => Some(3),
        Keycode::Num4 | Keycode::Kp4 => Some(4),
        Keycode::Num5 | Keycode::Kp5 => Some(5),
        Keycode::Num6 | Keycode::Kp6 => Some(6),
        Keycode::Num7 | Keycode::Kp7 => Some(7),
        Keycode::Num8 | Keycode::Kp8 => Some(8),
        Keycode::Num9 | Keycode::Kp9 => Some(9),
        _ => None,
    }
}

fn state_file_path(rom_path: &str, slot: usize) -> PathBuf {
    let stem = Path::new(rom_path)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("game");
    PathBuf::from("states").join(format!("{stem}.slot{slot}.state"))
}

fn cheat_file_path(rom_path: &str) -> PathBuf {
    let stem = Path::new(rom_path)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("game");
    PathBuf::from("cheats").join(format!("{stem}.json"))
}

fn filter_event_for_ascii_text_input(event: &Event) -> Option<Event> {
    match event {
        // Drop IME composition events so non-ASCII conversion is not used.
        Event::TextEditing { .. } => None,
        Event::TextInput {
            timestamp,
            window_id,
            text,
        } => {
            let ascii_text: String = text.chars().filter(|ch| ch.is_ascii()).collect();
            if ascii_text.is_empty() {
                None
            } else {
                Some(Event::TextInput {
                    timestamp: *timestamp,
                    window_id: *window_id,
                    text: ascii_text,
                })
            }
        }
        _ => Some(event.clone()),
    }
}
