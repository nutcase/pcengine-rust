#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::{bus::Bus, emulator::Emulator};
use std::{collections::BTreeMap, env, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "trace_hw_writes")]
    eprintln!("trace_hw_writes feature active (dump_state)");
    let mut args = env::args().skip(1);
    let rom_path = args
        .next()
        .ok_or("usage: dump_state <rom.[bin|pce]> [frames] [cycles]")?;
    let frame_target: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(10);
    let cycle_budget: u64 = args
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5_000_000);

    let rom = fs::read(&rom_path)?;
    let is_pce = Path::new(&rom_path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);

    let mut emulator = Emulator::new();
    if is_pce {
        emulator.load_hucard(&rom)?;
    } else {
        emulator.load_program(0xC000, &rom);
    }
    let initial_mpr = emulator.bus.mpr_array();
    emulator.reset();

    let mut frames = 0usize;
    while frames < frame_target && emulator.cycles() < cycle_budget {
        let cycles = emulator.tick() as u64;
        if cycles == 0 && !emulator.cpu.is_waiting() {
            break;
        }
        if emulator.take_frame().is_some() {
            frames += 1;
        }
    }

    let (nonzero, unique) = vram_stats(&mut emulator.bus);
    let (vram_lo_nonzero, vram_hi_nonzero) = vram_byte_plane_stats(&mut emulator.bus);

    println!("frames collected: {frames}");
    println!("cycles executed: {}", emulator.cycles());
    println!(
        "vram words: {} non-zero: {} unique(non-zero): {}",
        VRAM_WORDS, nonzero, unique
    );
    println!(
        "vram byte planes: lo-nonzero {} hi-nonzero {}",
        vram_lo_nonzero, vram_hi_nonzero
    );
    let (map_w, map_h) = emulator.bus.vdc_map_dimensions();
    let mwr = emulator.bus.vdc_register(0x09).unwrap_or(0);
    let map_base = (((mwr >> 8) & 0x0F) as usize) << 10;
    println!("bat map: {}x{} base={:04X}", map_w, map_h, map_base);
    let mut map_bit11_count = 0usize;
    let mut map_nonzero_count = 0usize;
    let mut map_max_id_11 = 0u16;
    let mut map_max_id_12 = 0u16;
    for row in 0..map_h {
        for col in 0..map_w {
            let addr = (map_base + row * map_w + col) & 0x7FFF;
            let entry = emulator.bus.vdc_vram_word(addr as u16);
            if entry != 0 {
                map_nonzero_count += 1;
            }
            if (entry & 0x0800) != 0 {
                map_bit11_count += 1;
            }
            map_max_id_11 = map_max_id_11.max(entry & 0x07FF);
            map_max_id_12 = map_max_id_12.max(entry & 0x0FFF);
        }
    }
    println!(
        "bat stats: nonzero {} bit11-set {} max_id11 {:03X} max_id12 {:03X}",
        map_nonzero_count, map_bit11_count, map_max_id_11, map_max_id_12
    );
    print_bat_row_nonzero_with_stride(&emulator.bus, 32, 64, 64);
    print_bat_row_nonzero_with_stride(&emulator.bus, 64, 64, 64);
    print!("bat row nonzero:");
    for row in 0..map_h.min(64) {
        let mut row_nonzero = 0usize;
        for col in 0..map_w {
            let addr = (map_base + row * map_w + col) & 0x7FFF;
            if emulator.bus.vdc_vram_word(addr as u16) != 0 {
                row_nonzero += 1;
            }
        }
        print!(" {:02}:{:02}", row, row_nonzero);
    }
    println!();
    println!(
        "cpu pc: {:#06X} sp: {:#04X}",
        emulator.cpu.pc, emulator.cpu.sp
    );
    println!(
        "vdc control: {:04X}",
        emulator.bus.vdc_register(0x05).unwrap_or(0)
    );
    println!(
        "mpr (boot): {}",
        initial_mpr
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{}:{:02X}", i, v))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mpr = emulator.bus.mpr_array();
    println!(
        "mpr: {}",
        mpr.iter()
            .enumerate()
            .map(|(i, v)| format!("{}:{:02X}", i, v))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!(
        "vdc control: {:04X}",
        emulator.bus.vdc_register(0x05).unwrap_or(0)
    );
    let mut scroll_counts: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    let mut valid_scroll_lines = 0usize;
    let mut scroll_transitions: Vec<(usize, u16, u16, bool)> = Vec::new();
    let mut last_scroll: Option<(u16, u16, bool)> = None;
    for line in 0..240usize {
        let (sx, sy) = emulator.bus.vdc_scroll_line(line);
        let valid = emulator.bus.vdc_scroll_line_valid(line);
        if valid {
            valid_scroll_lines += 1;
        }
        *scroll_counts.entry((sx, sy)).or_insert(0) += 1;
        if last_scroll.map_or(true, |prev| prev != (sx, sy, valid)) {
            scroll_transitions.push((line, sx, sy, valid));
            last_scroll = Some((sx, sy, valid));
        }
    }
    let mut top_scrolls: Vec<((u16, u16), usize)> = scroll_counts.into_iter().collect();
    top_scrolls.sort_by(|a, b| b.1.cmp(&a.1));
    println!(
        "scroll lines valid: {valid_scroll_lines}/240 unique states: {}",
        top_scrolls.len()
    );
    print!("scroll top states:");
    for ((sx, sy), count) in top_scrolls.iter().take(8) {
        print!(" x={sx:03X} y={sy:03X}:{count}");
    }
    println!();
    print!("scroll transitions:");
    for (line, sx, sy, valid) in scroll_transitions.iter().take(12) {
        print!(
            " [{line:03}:{sx:03X}/{sy:03X} v={}]",
            if *valid { 1 } else { 0 }
        );
    }
    println!();
    let mut scroll_transitions_full: Vec<(usize, u16, u16, bool)> = Vec::new();
    let mut last_full: Option<(u16, u16, bool)> = None;
    for line in 0..263usize {
        let (sx, sy) = emulator.bus.vdc_scroll_line(line);
        let valid = emulator.bus.vdc_scroll_line_valid(line);
        if last_full.map_or(true, |prev| prev != (sx, sy, valid)) {
            scroll_transitions_full.push((line, sx, sy, valid));
            last_full = Some((sx, sy, valid));
        }
    }
    print!("scroll transitions full:");
    for (line, sx, sy, valid) in scroll_transitions_full.iter().take(32) {
        print!(
            " [{line:03}:{sx:03X}/{sy:03X} v={}]",
            if *valid { 1 } else { 0 }
        );
    }
    println!();
    print!("scroll lines 080-100:");
    for line in 80..=100usize {
        let (sx, sy) = emulator.bus.vdc_scroll_line(line);
        let valid = emulator.bus.vdc_scroll_line_valid(line);
        print!(
            " [{line:03}:{sx:03X}/{sy:03X} v={}]",
            if valid { 1 } else { 0 }
        );
    }
    println!();
    print!("scroll lines 156-176:");
    for line in 156..=176usize {
        let (sx, sy) = emulator.bus.vdc_scroll_line(line);
        let valid = emulator.bus.vdc_scroll_line_valid(line);
        print!(
            " [{line:03}:{sx:03X}/{sy:03X} v={}]",
            if valid { 1 } else { 0 }
        );
    }
    println!();
    let mut ctrl_counts: BTreeMap<u16, usize> = BTreeMap::new();
    let mut ctrl_transitions: Vec<(usize, u16)> = Vec::new();
    let mut last_ctrl: Option<u16> = None;
    for line in 0..240usize {
        let ctrl = emulator.bus.vdc_control_line(line);
        *ctrl_counts.entry(ctrl).or_insert(0) += 1;
        if last_ctrl.map_or(true, |prev| prev != ctrl) {
            ctrl_transitions.push((line, ctrl));
            last_ctrl = Some(ctrl);
        }
    }
    let mut top_ctrls: Vec<(u16, usize)> = ctrl_counts.into_iter().collect();
    top_ctrls.sort_by(|a, b| b.1.cmp(&a.1));
    print!("control top states:");
    for (ctrl, count) in top_ctrls.iter().take(8) {
        print!(" {ctrl:04X}:{count}");
    }
    println!();
    print!("control transitions:");
    for (line, ctrl) in ctrl_transitions.iter().take(16) {
        print!(" [{line:03}:{ctrl:04X}]");
    }
    println!();
    let mut zoom_counts: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    let mut zoom_transitions: Vec<(usize, u16, u16)> = Vec::new();
    let mut last_zoom: Option<(u16, u16)> = None;
    for line in 0..240usize {
        let (zx, zy) = emulator.bus.vdc_zoom_line(line);
        *zoom_counts.entry((zx, zy)).or_insert(0) += 1;
        if last_zoom.map_or(true, |prev| prev != (zx, zy)) {
            zoom_transitions.push((line, zx, zy));
            last_zoom = Some((zx, zy));
        }
    }
    let mut top_zooms: Vec<((u16, u16), usize)> = zoom_counts.into_iter().collect();
    top_zooms.sort_by(|a, b| b.1.cmp(&a.1));
    print!("zoom top states:");
    for ((zx, zy), count) in top_zooms.iter().take(8) {
        print!(" x={zx:03X} y={zy:03X}:{count}");
    }
    println!();
    print!("zoom transitions:");
    for (line, zx, zy) in zoom_transitions.iter().take(12) {
        print!(" [{line:03}:{zx:03X}/{zy:03X}]");
    }
    println!();
    for idx in 0x04..=0x0E {
        if let Some(value) = emulator.bus.vdc_register(idx) {
            println!("vdc R{idx:02X} = {:04X}", value);
        }
    }
    println!(
        "mawr {:04X} marr {:04X}",
        emulator.bus.vdc_register(0x00).unwrap_or(0),
        emulator.bus.vdc_register(0x01).unwrap_or(0)
    );
    println!(
        "satb pending {} source {:04X}",
        emulator.bus.vdc_satb_pending(),
        emulator.bus.vdc_satb_source()
    );
    println!(
        "satb non-zero words: {}",
        emulator.bus.vdc_satb_nonzero_words()
    );
    print!("satb[0..15]:");
    for i in 0..16 {
        print!(" {:04X}", emulator.bus.vdc_satb_word(i));
    }
    println!();
    println!("sat decoded (first 64):");
    for sprite in 0..64usize {
        let base = sprite * 4;
        let y_word = emulator.bus.vdc_satb_word(base);
        let x_word = emulator.bus.vdc_satb_word(base + 1);
        let pattern_word = emulator.bus.vdc_satb_word(base + 2);
        let attr_word = emulator.bus.vdc_satb_word(base + 3);
        if y_word == 0 && x_word == 0 && pattern_word == 0 && attr_word == 0 {
            continue;
        }
        let y = (y_word & 0x03FF) as i32 - 64;
        let x = (x_word & 0x03FF) as i32 - 32;
        let width = if (attr_word & 0x0100) != 0 { 32 } else { 16 };
        let height = match (attr_word >> 12) & 0x03 {
            0 => 16,
            1 => 32,
            _ => 64,
        };
        let pattern = (pattern_word >> 1) & 0x03FF;
        let pal = attr_word & 0x000F;
        let pri = if (attr_word & 0x0080) != 0 { 1 } else { 0 };
        println!(
            "  #{sprite:02} x={x:4} y={y:4} wh={width:2}x{height:2} pat={pattern:03X} cg={} pal={pal:X} pri={pri} hf={} vf={}",
            (pattern_word & 1) as u8,
            if (attr_word & 0x0800) != 0 { 1 } else { 0 },
            if (attr_word & 0x8000) != 0 { 1 } else { 0 },
        );
    }
    println!(
        "zp[00]={:02X} zp[01]={:02X} zp[20]={:02X} zp[21]={:02X} zp[22]={:02X} zp[23]={:02X}",
        emulator.bus.read_zero_page(0),
        emulator.bus.read_zero_page(1),
        emulator.bus.read_zero_page(0x20),
        emulator.bus.read_zero_page(0x21),
        emulator.bus.read_zero_page(0x22),
        emulator.bus.read_zero_page(0x23)
    );
    println!(
        "ram[2217]={:02X} ram[2218]={:02X} ram[220C]={:02X}",
        emulator.bus.read(0x2217),
        emulator.bus.read(0x2218),
        emulator.bus.read(0x220C)
    );
    println!(
        "irq mask={:02X} status={:02X} pending={:02X}",
        emulator.bus.read(0xFF12),
        emulator.bus.read(0xFF13),
        emulator.bus.pending_interrupts()
    );
    println!(
        "dma ctrl {:04X} src {:04X} dst {:04X} len {:04X}",
        emulator.bus.vdc_register(0x0F).unwrap_or(0),
        emulator.bus.vdc_register(0x10).unwrap_or(0),
        emulator.bus.vdc_register(0x11).unwrap_or(0),
        emulator.bus.vdc_register(0x12).unwrap_or(0)
    );

    let palette_vram_base = 0x0500u16;
    print!("VRAM[{palette_vram_base:04X}] ->");
    for offset in 0..8u16 {
        let addr = palette_vram_base.wrapping_add(offset);
        let word = read_vram_word(&mut emulator.bus, addr);
        print!(" {word:04X}");
    }
    println!();

    let palette_nonzero = palette_stats(&mut emulator.bus);
    println!("palette entries non-zero: {palette_nonzero}");
    let mut palette_bg_nonzero = 0usize;
    let mut palette_sprite_nonzero = 0usize;
    for idx in 0..0x100u16 {
        if read_palette_word(&mut emulator.bus, idx) != 0 {
            palette_bg_nonzero += 1;
        }
    }
    for idx in 0x100u16..0x200u16 {
        if read_palette_word(&mut emulator.bus, idx) != 0 {
            palette_sprite_nonzero += 1;
        }
    }
    println!(
        "palette non-zero split: bg {} sprite {}",
        palette_bg_nonzero, palette_sprite_nonzero
    );
    for idx in 0..16usize {
        let value = read_palette_word(&mut emulator.bus, idx as u16);
        println!("palette[{idx:02X}] = {value:04X}");
    }
    for idx in 0x100..0x110usize {
        let value = read_palette_word(&mut emulator.bus, idx as u16);
        println!("palette[{idx:03X}] = {value:04X}");
    }

    for base in [
        0x0000u16, 0x0100, 0x0200, 0x0300, 0x0400, 0x0600, 0x0700, 0x0780, 0x07C0, 0x0800, 0x0C00,
        0x0CC0, 0x0D00, 0x1000, 0x1400, 0x1800, 0x1C00, 0x2000, 0x2400, 0x2800, 0x2C00, 0x3000,
        0x3800, 0x4000, 0x4400, 0x4600, 0x4800, 0x4C00, 0x6000,
    ] {
        print!("VRAM[{base:04X}]:");
        for offset in 0..16u16 {
            let addr = base.wrapping_add(offset);
            let word = read_vram_word(&mut emulator.bus, addr);
            print!(" {word:04X}");
        }
        println!();
    }

    Ok(())
}

const VRAM_WORDS: usize = 0x8000;

fn read_vram_word(bus: &mut Bus, addr: u16) -> u16 {
    bus.write_st_port(0, 0x01);
    bus.write_st_port(1, (addr & 0xFF) as u8);
    bus.write_st_port(2, ((addr >> 8) & 0x7F) as u8);
    bus.write_st_port(0, 0x02);
    let lo = bus.read_st_port(1);
    let hi = bus.read_st_port(2);
    u16::from_le_bytes([lo, hi])
}

fn vram_stats(bus: &mut Bus) -> (usize, usize) {
    use std::collections::HashSet;
    let mut nonzero = 0usize;
    let mut set = HashSet::new();
    for addr in 0..VRAM_WORDS {
        let value = read_vram_word(bus, addr as u16);
        if value != 0 {
            nonzero += 1;
            set.insert(value);
        }
    }
    (nonzero, set.len())
}

fn vram_byte_plane_stats(bus: &mut Bus) -> (usize, usize) {
    let mut lo_nonzero = 0usize;
    let mut hi_nonzero = 0usize;
    for addr in 0..VRAM_WORDS {
        let value = read_vram_word(bus, addr as u16);
        if value & 0x00FF != 0 {
            lo_nonzero += 1;
        }
        if value & 0xFF00 != 0 {
            hi_nonzero += 1;
        }
    }
    (lo_nonzero, hi_nonzero)
}

fn read_palette_word(bus: &Bus, index: u16) -> u16 {
    bus.vce_palette_word(index as usize)
}

fn palette_stats(bus: &mut Bus) -> usize {
    let mut count = 0usize;
    for idx in 0..0x200u16 {
        if read_palette_word(bus, idx) != 0 {
            count += 1;
        }
    }
    count
}

fn print_bat_row_nonzero_with_stride(bus: &Bus, width: usize, height: usize, rows: usize) {
    print!("bat row nonzero ({}x{}):", width, height);
    for row in 0..rows.min(height) {
        let mut row_nonzero = 0usize;
        for col in 0..width {
            let addr = (row * width + col) & 0x7FFF;
            if bus.vdc_vram_word(addr as u16) != 0 {
                row_nonzero += 1;
            }
        }
        print!(" {:02}:{:02}", row, row_nonzero);
    }
    println!();
}
