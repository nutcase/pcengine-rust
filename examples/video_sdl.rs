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
#[path = "common/hud_toast.rs"]
mod hud_toast;

use config::{AppConfig, ParsedBindings};
use hud_toast::{HudToast, draw_hud_toast, show_hud_toast};
use pce::emulator::Emulator;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn main() -> Result<(), String> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let (config_path, args) = config::parse_config_path(&raw_args);
    let mut args = args.into_iter();
    let rom_path = args
        .next()
        .ok_or_else(|| "usage: video_sdl <rom.[bin|pce]> [--config path.json]".to_string())?;
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
    if is_pce {
        emulator
            .load_hucard(&rom)
            .map_err(|err| format!("failed to load HuCard: {err}"))?;
    } else {
        emulator.load_program(0xC000, &rom);
    }
    emulator.set_audio_batch_size(perf.emu_audio_batch);
    emulator.reset();

    let mut current_width = emulator.display_width();
    let mut current_height = emulator.display_height();

    // PC Engine outputs to a 4:3 CRT regardless of dot clock / pixel count.
    // Window size is always 4:3, with height as reference.
    let win_h = (current_height as u32) * config.window_scale;
    let win_w = win_h * 4 / 3;

    let sdl = sdl2::init().map_err(|e| e.to_string())?;
    let audio = sdl.audio().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let window = video
        .window("PC Engine (preview)", win_w, win_h)
        .position_centered()
        .resizable()
        .opengl()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormatEnum::RGB24,
            current_width as u32,
            current_height as u32,
        )
        .map_err(|e| e.to_string())?;
    let desired_audio = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1),
        samples: Some(perf.audio_batch as u16),
    };
    let audio_device = audio
        .open_queue::<i16, _>(None, &desired_audio)
        .map_err(|e| e.to_string())?;
    audio_device.resume();

    let mut event_pump = sdl.event_pump().map_err(|e| e.to_string())?;
    let mut quit = false;
    let mut pressed: HashSet<Keycode> = HashSet::new();
    let mut latest_frame: Option<Vec<u32>> = None;
    let mut last_present = Instant::now();
    let mut hud_toast: Option<HudToast> = None;
    let auto_fire_epoch = Instant::now();
    let bound_keys = bindings.to_set();

    while !quit {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => quit = true,
                Event::KeyDown {
                    keycode: Some(code),
                    keymod,
                    repeat: false,
                    ..
                } => {
                    if code == Keycode::Escape {
                        quit = true;
                    } else if let Some(slot) = state_slot_from_keycode(code) {
                        let shift_pressed = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
                        let state_path = state_file_path(&rom_path, slot);
                        if shift_pressed {
                            if let Some(parent) = state_path.parent() {
                                if let Err(err) = std::fs::create_dir_all(parent) {
                                    eprintln!(
                                        "Failed to create state directory {}: {err}",
                                        parent.display()
                                    );
                                    show_hud_toast(&mut hud_toast, "STATE DIR ERR");
                                    continue;
                                }
                            }
                            match emulator.save_state_to_file(&state_path) {
                                Ok(()) => {
                                    eprintln!(
                                        "Saved state to slot {} ({})",
                                        slot,
                                        state_path.display()
                                    );
                                    show_hud_toast(&mut hud_toast, format!("SAVE {slot} OK"));
                                }
                                Err(err) => {
                                    eprintln!("Failed to save slot {}: {err}", slot);
                                    show_hud_toast(&mut hud_toast, format!("SAVE {slot} ERR"));
                                }
                            }
                        } else {
                            match emulator.load_state_from_file(&state_path) {
                                Ok(()) => {
                                    emulator.set_audio_batch_size(perf.emu_audio_batch);
                                    audio_device.clear();
                                    latest_frame = None;
                                    last_present = Instant::now();
                                    eprintln!(
                                        "Loaded state from slot {} ({})",
                                        slot,
                                        state_path.display()
                                    );
                                    show_hud_toast(&mut hud_toast, format!("LOAD {slot} OK"));
                                }
                                Err(err) => {
                                    eprintln!(
                                        "Failed to load slot {} ({}): {err}",
                                        slot,
                                        state_path.display()
                                    );
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
                    pressed.remove(&code);
                }
                _ => {}
            }
        }

        let auto_fire_on = auto_fire_phase_on(auto_fire_epoch, Instant::now(), auto_fire_period_ns);
        let button_i_pressed = pressed.contains(&bindings.button_i)
            || (pressed.contains(&bindings.rapid_i) && auto_fire_on);
        let button_ii_pressed = pressed.contains(&bindings.button_ii)
            || (pressed.contains(&bindings.rapid_ii) && auto_fire_on);
        let pad_state = build_pad_state(&pressed, &bindings, button_i_pressed, button_ii_pressed);
        emulator.bus.set_joypad_input(pad_state);

        let mut steps = 0usize;
        let mut frame_seen = false;
        while queued_samples(&audio_device) < perf.audio_queue_target
            && steps < perf.max_emu_steps_per_pump
        {
            emulator.tick();
            steps += 1;
            if let Some(samples) = emulator.take_audio_samples() {
                queue_audio_samples(&audio_device, &samples, perf.audio_queue_max)?;
            }
            if let Some(frame) = emulator.take_frame() {
                latest_frame = Some(frame);
                frame_seen = true;
            }
            if frame_seen && steps >= perf.max_steps_after_frame {
                // Keep window updates responsive even if queue size reporting is unstable.
                break;
            }
        }

        // Check if display dimensions changed and recreate texture if so.
        let new_width = emulator.display_width();
        let new_height = emulator.display_height();
        if new_width != current_width || new_height != current_height {
            current_width = new_width;
            current_height = new_height;
            let h = (current_height as u32) * config.window_scale;
            let w = h * 4 / 3;
            canvas
                .window_mut()
                .set_size(w, h)
                .map_err(|e| e.to_string())?;
            texture = texture_creator
                .create_texture_streaming(
                    PixelFormatEnum::RGB24,
                    current_width as u32,
                    current_height as u32,
                )
                .map_err(|e| e.to_string())?;
        }

        let queued = queued_samples(&audio_device);
        let should_present = queued >= perf.audio_queue_critical
            || last_present.elapsed() >= Duration::from_millis(perf.max_present_interval_ms);
        if should_present {
            if let Some(frame) = latest_frame.take() {
                let mut frame = frame;
                draw_hud_toast(&mut frame, current_width, current_height, &mut hud_toast);
                update_texture(&mut texture, &frame, current_width)?;
                canvas.clear();
                // Stretch texture to fill the window at 4:3 aspect ratio.
                // SDL handles non-integer scaling via the dest rect.
                let (win_w, win_h) = canvas.output_size().map_err(|e| e.to_string())?;
                canvas.copy(&texture, None, Some(Rect::new(0, 0, win_w, win_h)))?;
                canvas.present();
                last_present = Instant::now();
            }
        }
        if queued < perf.audio_queue_min {
            std::thread::yield_now();
        } else if queued > perf.audio_queue_target {
            // Audio is safely buffered; briefly yield CPU.
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

fn queue_audio_samples(
    device: &AudioQueue<i16>,
    samples: &[i16],
    audio_queue_max: usize,
) -> Result<(), String> {
    let available = audio_queue_max.saturating_sub(queued_samples(device));
    if available == 0 {
        return Ok(());
    }
    if samples.len() > available {
        // Keep stream continuity: enqueue the earliest portion, drop the newest tail.
        device
            .queue_audio(&samples[..available])
            .map_err(|e| e.to_string())
    } else {
        device.queue_audio(samples).map_err(|e| e.to_string())
    }
}

fn update_texture(
    texture: &mut sdl2::render::Texture,
    frame: &[u32],
    width: usize,
) -> Result<(), String> {
    texture.with_lock(None, |buffer, pitch| {
        for (y, row) in frame.chunks(width).enumerate() {
            let dest = &mut buffer[y * pitch..y * pitch + width * 3];
            for (pixel, chunk) in row.iter().zip(dest.chunks_mut(3)) {
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (*pixel & 0xFF) as u8;
                chunk[0] = r;
                chunk[1] = g;
                chunk[2] = b;
            }
        }
    })
}

fn build_pad_state(
    pressed: &HashSet<Keycode>,
    bindings: &ParsedBindings,
    button_i_pressed: bool,
    button_ii_pressed: bool,
) -> u8 {
    let mut state: u8 = 0xFF;
    // Active-low bits. Lower nibble = d-pad, upper nibble = buttons.
    let mut clear = |bit: u8| state &= !(1 << bit);
    // D-pad (lower nibble, returned when SEL=1)
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
    // Buttons (upper nibble, returned when SEL=0)
    if button_i_pressed {
        clear(4);
    } // I
    if button_ii_pressed {
        clear(5);
    } // II
    if pressed.contains(&bindings.select) {
        clear(6);
    } // Select
    if pressed.contains(&bindings.run) {
        clear(7);
    } // Run
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
