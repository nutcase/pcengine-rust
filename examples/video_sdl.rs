use pce::emulator::Emulator;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

const FRAME_WIDTH: usize = 256;
const FRAME_HEIGHT: usize = 240;
const SCALE: u32 = 3;
const AUDIO_BATCH: usize = 1024;
const EMU_AUDIO_BATCH: usize = 256;
const AUDIO_QUEUE_MIN: usize = AUDIO_BATCH * 4;
const AUDIO_QUEUE_TARGET: usize = AUDIO_BATCH * 12;
const AUDIO_QUEUE_MAX: usize = AUDIO_BATCH * 16;
const AUDIO_QUEUE_CRITICAL: usize = AUDIO_BATCH * 2;
const MAX_EMU_STEPS_PER_PUMP: usize = 120_000;
const MAX_STEPS_AFTER_FRAME: usize = 30_000;
const MAX_PRESENT_INTERVAL: Duration = Duration::from_millis(33);

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let rom_path = args
        .next()
        .ok_or_else(|| "usage: video_sdl <rom.[bin|pce]>".to_string())?;
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

    let sdl = sdl2::init().map_err(|e| e.to_string())?;
    let audio = sdl.audio().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let window = video
        .window(
            "PC Engine (preview)",
            (FRAME_WIDTH as u32) * SCALE,
            (FRAME_HEIGHT as u32) * SCALE,
        )
        .position_centered()
        .opengl()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;
    canvas
        .set_scale(SCALE as f32, SCALE as f32)
        .map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormatEnum::RGB24,
            FRAME_WIDTH as u32,
            FRAME_HEIGHT as u32,
        )
        .map_err(|e| e.to_string())?;
    let desired_audio = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1),
        samples: Some(AUDIO_BATCH as u16),
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

    while !quit {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => quit = true,
                Event::KeyDown {
                    keycode: Some(code),
                    repeat: false,
                    ..
                } => {
                    if code == Keycode::Escape {
                        quit = true;
                    } else {
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

        let pad_state = build_pad_state(&pressed);
        emulator.bus.set_joypad_input(pad_state);

        let mut steps = 0usize;
        let mut frame_seen = false;
        while queued_samples(&audio_device) < AUDIO_QUEUE_TARGET && steps < MAX_EMU_STEPS_PER_PUMP {
            emulator.tick();
            steps += 1;
            if let Some(samples) = emulator.take_audio_samples() {
                queue_audio_samples(&audio_device, &samples)?;
            }
            if let Some(frame) = emulator.take_frame() {
                latest_frame = Some(frame);
                frame_seen = true;
            }
            if frame_seen && steps >= MAX_STEPS_AFTER_FRAME {
                // Keep window updates responsive even if queue size reporting is unstable.
                break;
            }
        }

        let queued = queued_samples(&audio_device);
        let should_present =
            queued >= AUDIO_QUEUE_CRITICAL || last_present.elapsed() >= MAX_PRESENT_INTERVAL;
        if should_present {
            if let Some(frame) = latest_frame.take() {
                update_texture(&mut texture, &frame)?;
                canvas.clear();
                canvas.copy(
                    &texture,
                    None,
                    Some(Rect::new(0, 0, FRAME_WIDTH as u32, FRAME_HEIGHT as u32)),
                )?;
                canvas.present();
                last_present = Instant::now();
            }
        }
        if queued < AUDIO_QUEUE_MIN {
            std::thread::yield_now();
        } else if queued > AUDIO_QUEUE_TARGET {
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

fn queue_audio_samples(device: &AudioQueue<i16>, samples: &[i16]) -> Result<(), String> {
    let available = AUDIO_QUEUE_MAX.saturating_sub(queued_samples(device));
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

fn update_texture(texture: &mut sdl2::render::Texture, frame: &[u32]) -> Result<(), String> {
    texture.with_lock(None, |buffer, pitch| {
        for (y, row) in frame.chunks(FRAME_WIDTH).enumerate() {
            let dest = &mut buffer[y * pitch..y * pitch + FRAME_WIDTH * 3];
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

fn build_pad_state(pressed: &HashSet<Keycode>) -> u8 {
    let mut state: u8 = 0xFF;
    // Active-low bits. Lower nibble = d-pad, upper nibble = buttons.
    let mut clear = |bit: u8| state &= !(1 << bit);
    // D-pad (lower nibble, returned when SEL=1)
    if pressed.contains(&Keycode::Up) { clear(0); }
    if pressed.contains(&Keycode::Right) { clear(1); }
    if pressed.contains(&Keycode::Down) { clear(2); }
    if pressed.contains(&Keycode::Left) { clear(3); }
    // Buttons (upper nibble, returned when SEL=0)
    if pressed.contains(&Keycode::Z) { clear(4); }       // I
    if pressed.contains(&Keycode::X) { clear(5); }       // II
    if pressed.contains(&Keycode::LShift) || pressed.contains(&Keycode::RShift) { clear(6); } // Select
    if pressed.contains(&Keycode::Return) { clear(7); }  // Run
    state
}
