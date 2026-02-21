mod egui_ui;
#[path = "common/hud_toast.rs"]
mod hud_toast;

use egui_ui::CheatToolUi;
use egui_ui::gl_game::GlGameRenderer;
use hud_toast::{HudToast, draw_hud_toast, show_hud_toast};
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

const SCALE: u32 = 3;
const AUDIO_BATCH: usize = 512;
const EMU_AUDIO_BATCH: usize = 128;
const AUDIO_QUEUE_MIN: usize = AUDIO_BATCH * 2;
const AUDIO_QUEUE_TARGET: usize = AUDIO_BATCH * 4;
const AUDIO_QUEUE_MAX: usize = AUDIO_BATCH * 6;
const AUDIO_QUEUE_CRITICAL: usize = AUDIO_BATCH;
const MAX_EMU_STEPS_PER_PUMP: usize = 120_000;
const MAX_STEPS_AFTER_FRAME: usize = 30_000;
const MAX_PRESENT_INTERVAL: Duration = Duration::from_millis(33);
const PANEL_WIDTH_DEFAULT: f32 = 420.0;
const PANEL_WIDTH_MIN: f32 = 300.0;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let rom_path = args
        .next()
        .ok_or_else(|| "usage: video_sdl_egui <rom.[bin|pce]>".to_string())?;
    let rom = std::fs::read(&rom_path)
        .map_err(|err| format!("failed to read ROM {}: {err}", rom_path))?;

    let mut emulator = Emulator::new();
    let is_pce = Path::new(&rom_path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);
    if is_pce {
        emulator
            .load_hucard(&rom)
            .map_err(|err| format!("failed to load HuCard: {err}"))?;
    } else {
        emulator.load_program(0xC000, &rom);
    }
    emulator.set_audio_batch_size(EMU_AUDIO_BATCH);
    emulator.reset();

    let mut current_width = emulator.display_width();
    let mut current_height = emulator.display_height();
    let game_h = (current_height as u32) * SCALE;
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
        samples: Some(AUDIO_BATCH as u16),
    };
    let audio_device = audio_subsystem
        .open_queue::<i16, _>(None, &desired_audio)
        .map_err(|e| e.to_string())?;
    audio_device.resume();

    let mut event_pump = sdl.event_pump().map_err(|e| e.to_string())?;
    let mut quit = false;
    let mut pressed: HashSet<Keycode> = HashSet::new();
    let mut frame_buf: Vec<u32> = Vec::new();
    let mut frame_buf_ready = false;
    let mut last_present = Instant::now();
    let mut hud_toast: Option<HudToast> = None;

    let mut game_renderer = GlGameRenderer::new();
    let mut cheat_ui = CheatToolUi::new();
    let mut prev_panel_visible = cheat_ui.panel_visible;
    let mut panel_width_px: u32 = PANEL_WIDTH_DEFAULT as u32;
    let text_input = video.text_input();
    let mut text_input_active = false;
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
                                    emulator.set_audio_batch_size(EMU_AUDIO_BATCH);
                                    audio_device.clear();
                                    frame_buf_ready = false;
                                    last_present = Instant::now();
                                    eprintln!("Loaded slot {}", slot);
                                    show_hud_toast(&mut hud_toast, format!("LOAD {slot} OK"));
                                }
                                Err(err) => {
                                    eprintln!("Load failed: {err}");
                                    show_hud_toast(&mut hud_toast, format!("LOAD {slot} ERR"));
                                }
                            }
                        }
                    } else {
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
        let pad_state = build_pad_state(&pressed);
        emulator.bus.set_joypad_input(pad_state);

        let mut steps = 0usize;
        let mut frame_seen = false;
        if !cheat_ui.paused {
            while queued_samples(&audio_device) < AUDIO_QUEUE_TARGET
                && steps < MAX_EMU_STEPS_PER_PUMP
            {
                emulator.tick();
                steps += 1;
                if let Some(samples) = emulator.take_audio_samples() {
                    queue_audio_samples(&audio_device, &samples)?;
                }
                if emulator.take_frame_into(&mut frame_buf) {
                    frame_buf_ready = true;
                    frame_seen = true;
                }
                if frame_seen && steps >= MAX_STEPS_AFTER_FRAME {
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
        let should_present =
            queued >= AUDIO_QUEUE_CRITICAL || last_present.elapsed() >= MAX_PRESENT_INTERVAL;

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

                let full_output = egui_ctx.run(egui_state.input.take(), |ctx| {
                    let panel_resp = egui::SidePanel::right("cheat_panel")
                        .resizable(true)
                        .min_width(PANEL_WIDTH_MIN)
                        .default_width(PANEL_WIDTH_DEFAULT)
                        .show(ctx, |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    cheat_ui.show_panel(
                                        ui,
                                        &mut ram_writes,
                                        wram,
                                        cram,
                                        Some(&cheat_path),
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
            }

            window.gl_swap_window();
            last_present = Instant::now();
        }

        if queued < AUDIO_QUEUE_MIN {
            std::thread::yield_now();
        } else if queued > AUDIO_QUEUE_TARGET {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            std::thread::yield_now();
        }
    }

    Ok(())
}

fn queued_samples(device: &AudioQueue<i16>) -> usize {
    device.size() as usize / std::mem::size_of::<i16>()
}

fn queue_audio_samples(device: &AudioQueue<i16>, samples: &[i16]) -> Result<(), String> {
    let available = AUDIO_QUEUE_MAX.saturating_sub(queued_samples(device));
    if available == 0 {
        return Ok(());
    }
    if samples.len() > available {
        device
            .queue_audio(&samples[..available])
            .map_err(|e| e.to_string())
    } else {
        device.queue_audio(samples).map_err(|e| e.to_string())
    }
}

fn build_pad_state(pressed: &HashSet<Keycode>) -> u8 {
    let mut state: u8 = 0xFF;
    let mut clear = |bit: u8| state &= !(1 << bit);
    if pressed.contains(&Keycode::Up) {
        clear(0);
    }
    if pressed.contains(&Keycode::Right) {
        clear(1);
    }
    if pressed.contains(&Keycode::Down) {
        clear(2);
    }
    if pressed.contains(&Keycode::Left) {
        clear(3);
    }
    if pressed.contains(&Keycode::Z) {
        clear(4);
    }
    if pressed.contains(&Keycode::X) {
        clear(5);
    }
    if pressed.contains(&Keycode::LShift) || pressed.contains(&Keycode::RShift) {
        clear(6);
    }
    if pressed.contains(&Keycode::Return) {
        clear(7);
    }
    state
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
