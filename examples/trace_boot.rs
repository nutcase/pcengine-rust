#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    dead_code,
    unused_assignments,
    unused_comparisons
)]
use pce::bus::{
    Bus, VDC_STATUS_BUSY, VDC_STATUS_CR, VDC_STATUS_DS, VDC_STATUS_DV, VDC_STATUS_OR,
    VDC_STATUS_RCR, VDC_STATUS_VBL,
};
use pce::emulator::Emulator;
use std::{collections::HashMap, env, error::Error, fs, path::Path};

#[derive(Default)]
struct OpcTrace {
    count: usize,
    limit: usize,
    enable: bool,
}

impl OpcTrace {
    fn new(limit: usize, enable: bool) -> Self {
        Self {
            count: 0,
            limit,
            enable,
        }
    }

    fn log(&mut self, pc: u16, opcode: u8, mpr: &[u8; 8]) {
        if !self.enable || self.count >= self.limit {
            return;
        }
        self.count += 1;
        eprintln!(
            "OPC pc={:04X} op={:02X} mpr={:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            pc, opcode, mpr[0], mpr[1], mpr[2], mpr[3], mpr[4], mpr[5], mpr[6], mpr[7]
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut path = None;
    let mut max_steps: usize = 512;
    let mut break_mask: u8 = 0;
    let mut trace_vdc = false;
    #[allow(unused_mut)]
    let mut count_io_low: usize = 0;
    #[allow(unused_mut)]
    let mut count_io_high: usize = 0;
    let mut dump_addrs: Vec<u16> = Vec::new();
    let mut trace_zp: Vec<u8> = Vec::new();
    #[allow(unused_mut)]
    let mut io_hist: HashMap<u16, usize> = HashMap::new();
    let mut trace_opcodes = false;
    let mut trace_opc_limit: usize = 0;
    let mut pc_hist_limit: usize = 0;
    let mut pc_hist: HashMap<u16, usize> = HashMap::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--break-vblank" => break_mask |= VDC_STATUS_VBL,
            "--break-ds" => break_mask |= VDC_STATUS_DS,
            "--trace-vdc" => trace_vdc = true,
            "--dump-addr" => {
                if let Some(value) = args.next() {
                    let parsed = parse_u16(&value)?;
                    dump_addrs.push(parsed);
                } else {
                    return Err("--dump-addr requires a value".into());
                }
            }
            "--break-status" => {
                if let Some(value) = args.next() {
                    break_mask |= parse_status_list(&value)?;
                } else {
                    return Err("--break-status requires a value".into());
                }
            }
            "--steps" => {
                if let Some(value) = args.next() {
                    max_steps = value.parse().map_err(|_| "invalid --steps value")?;
                } else {
                    return Err("--steps requires a value".into());
                }
            }
            "--trace-zp" => {
                if let Some(value) = args.next() {
                    let addrs = parse_zp_list(&value)?;
                    trace_zp.extend(addrs);
                } else {
                    return Err("--trace-zp requires a value".into());
                }
            }
            "--pc-hist" => {
                if let Some(value) = args.next() {
                    pc_hist_limit = value.parse().map_err(|_| "invalid --pc-hist value")?;
                } else {
                    return Err("--pc-hist requires a value".into());
                }
            }
            arg if arg.starts_with("--dump-addr=") => {
                let value = &arg["--dump-addr=".len()..];
                let parsed = parse_u16(value)?;
                dump_addrs.push(parsed);
            }
            arg if arg.starts_with("--trace-zp=") => {
                let value = &arg["--trace-zp=".len()..];
                let addrs = parse_zp_list(value)?;
                trace_zp.extend(addrs);
            }
            arg if arg.starts_with("--steps=") => {
                let value = &arg["--steps=".len()..];
                max_steps = value.parse().map_err(|_| "invalid --steps value")?;
            }
            arg if arg.starts_with("--pc-hist=") => {
                let value = &arg["--pc-hist=".len()..];
                pc_hist_limit = value.parse().map_err(|_| "invalid --pc-hist value")?;
            }
            arg if arg.starts_with("--trace-opc=") => {
                let value = &arg["--trace-opc=".len()..];
                trace_opcodes = true;
                trace_opc_limit = value.parse().unwrap_or(256);
            }
            arg if arg.starts_with("--break-status=") => {
                let value = &arg["--break-status=".len()..];
                break_mask |= parse_status_list(value)?;
            }
            _ if path.is_none() => {
                path = Some(arg);
            }
            _ => {
                return Err(format!("unrecognised argument: {arg}").into());
            }
        }
    }

    let path = match path {
        Some(p) => p,
        None => {
            eprintln!(
                "Usage: trace_boot <program.[bin|pce]> [--steps N|--steps=N] [--break-vblank] [--break-ds] [--break-status flags] [--trace-vdc]"
            );
            return Ok(());
        }
    };

    let rom = fs::read(&path)?;
    let mut emulator = Emulator::new();
    let is_pce = Path::new(&path)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pce"))
        .unwrap_or(false);

    if is_pce {
        emulator.load_hucard(&rom)?;
    } else {
        emulator.load_program(0xC000, &rom);
    }
    emulator.reset();

    let reset_vector = emulator.bus.read_u16(0xFFFC);
    println!("Reset vector = 0x{reset_vector:04X}");

    for addr in dump_addrs.iter().copied() {
        dump_memory_window(&mut emulator.bus, addr);
    }

    print!("MPR:");
    for bank in 0..8 {
        print!(" {:02X}", emulator.bus.mpr(bank));
    }
    println!();

    let start = reset_vector.saturating_sub(0x20);
    let end = reset_vector.saturating_add(0x20);
    println!("Dumping memory around reset vector (0x{start:04X}..0x{end:04X}):");
    for addr in start..end {
        let byte = emulator.bus.read(addr);
        if addr % 16 == 0 {
            print!("\n{addr:04X}: ");
        }
        print!("{byte:02X} ");
    }
    println!("\n");

    println!("Stack snapshot:");
    for addr in 0x01F8u16..=0x01FF {
        let byte = emulator.bus.read(addr);
        if addr % 16 == 0 {
            print!("\n{addr:04X}: ");
        }
        print!("{byte:02X} ");
    }
    println!();
    for addr in 0x0100u16..=0x0106 {
        let byte = emulator.bus.read(addr);
        if (addr - 0x0100) % 16 == 0 {
            print!("\n{addr:04X}: ");
        }
        print!("{byte:02X} ");
    }
    println!("\n");
    println!(
        "seeded entry bytes: pcl={:02X} pch={:02X}",
        emulator.bus.read(0x01FF),
        emulator.bus.read(0x0100)
    );
    println!(
        "entry candidate bytes: [E2AA]={:02X} [E3A0]={:02X}",
        emulator.bus.read(0xE2AA),
        emulator.bus.read(0xE3A0)
    );
    println!(
        "zero[$0000]={:02X} zero[$0028]={:02X}",
        emulator.bus.read(0x0000),
        emulator.bus.read(0x0028)
    );

    println!("step,pc,opcode,a,x,y,sp,status,pending_irq,waiting,halted");

    let mut opc_trace = OpcTrace::new(trace_opc_limit, trace_opcodes);

    let mut prev_status = emulator.bus.vdc_status_bits();
    let mut prev_registers = if trace_vdc {
        (0..32)
            .map(|i| emulator.bus.vdc_register(i).unwrap_or(0))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut zp_watch: HashMap<u8, u8> = HashMap::new();

    for step in 0..max_steps {
        let pc = emulator.cpu.pc;
        let sp_before = emulator.cpu.sp;
        let cycles = emulator.tick();
        let opcode = emulator.cpu.last_opcode();

        // Count IO accesses in the low (0x0000-0x03FF) and high (0x2000-0x23FF) windows.
        // We approximate by sampling the last port that hardware logging captured.
        #[cfg(feature = "trace_hw_writes")]
        {
            // Using the global Bus logger would be noisy; instead peek at the raw address the CPU just executed for STA.
            // Only increment when opcode is a store (0x8D absolute, 0x9D abs,X, 0x99 abs,Y, 0x8F zp). Keep it simple: STA abs (0x8D) only for now.
            if opcode == 0x8D {
                let lo = emulator.bus.read(pc.wrapping_add(1));
                let hi = emulator.bus.read(pc.wrapping_add(2));
                let addr = u16::from_le_bytes([lo, hi]);
                if (0x0000..=0x03FF).contains(&addr) {
                    count_io_low = count_io_low.saturating_add(1);
                } else if (0x2000..=0x23FF).contains(&addr) {
                    count_io_high = count_io_high.saturating_add(1);
                }
                if (0x0000..=0x03FF).contains(&addr)
                    || (0x2000..=0x23FF).contains(&addr)
                    || (0xFF00..=0xFF7F).contains(&addr)
                {
                    *io_hist.entry(addr).or_insert(0) += 1;
                }
            }
        }
        println!(
            "{step},{pc:04X},{opcode:02X},{a:02X},{x:02X},{y:02X},{sp_before:02X}->{sp:02X},{status:02X},{pending:02X},{waiting},{halted}",
            step = step,
            pc = emulator.cpu.pc,
            opcode = opcode,
            a = emulator.cpu.a,
            x = emulator.cpu.x,
            y = emulator.cpu.y,
            sp_before = sp_before,
            sp = emulator.cpu.sp,
            status = emulator.cpu.status,
            pending = emulator.bus.pending_interrupts(),
            waiting = if emulator.cpu.is_waiting() { 1 } else { 0 },
            halted = if emulator.cpu.halted { 1 } else { 0 },
        );

        if trace_opcodes {
            opc_trace.log(emulator.cpu.pc, opcode, &emulator.bus.mpr_array());
        }

        if pc_hist_limit > 0 {
            *pc_hist.entry(emulator.cpu.pc).or_insert(0) += 1;
        }

        let status_now = emulator.bus.vdc_status_bits();
        if status_now != prev_status {
            println!(
                "    VDC status {:02X} (delta {:02X})",
                status_now,
                status_now ^ prev_status
            );
            prev_status = status_now;
        }

        if opcode == 0x53 || opcode == 0x43 {
            print!("    MPR:");
            for bank in 0..8 {
                print!(" {:02X}", emulator.bus.mpr(bank));
            }
            println!();
        }

        if opcode == 0x92 {
            println!(
                "    mpr0={:02X} zp[0]={:02X} zp[1]={:02X} zp[0x20]={:02X}",
                emulator.bus.mpr(0),
                emulator.bus.read(0x0000),
                emulator.bus.read(0x0001),
                emulator.bus.read(0x0020)
            );
        }

        if !trace_zp.is_empty() {
            for &addr in &trace_zp {
                let value = emulator.bus.read_zero_page(addr);
                if zp_watch.get(&addr).copied() != Some(value) {
                    println!("    ZP[{addr:02X}]={value:02X}");
                    zp_watch.insert(addr, value);
                }
            }
        }

        if opcode == 0x40 || emulator.cpu.halted {
            print!("    stack top:");
            for offset in 0..=5 {
                let addr = (0x0100u16).wrapping_sub(offset as u16);
                let byte = emulator.bus.read(addr);
                print!(" {:02X}", byte);
            }
            println!();
        }

        if trace_vdc {
            for reg in 0..32 {
                let value = emulator.bus.vdc_register(reg).unwrap_or(0);
                if value != prev_registers[reg] {
                    println!("    VDC R{:02X} = {:04X}", reg, value);
                    prev_registers[reg] = value;
                }
            }
        }

        if break_mask != 0 && (status_now & break_mask) != 0 {
            let reasons = collect_status_reasons(status_now & break_mask);
            println!("    Break due to {}", reasons.join(" & "));
            break;
        }

        if emulator.cpu.halted {
            println!(
                "    zero[$0000]={:02X} zero[$0001]={:02X}",
                emulator.bus.read(0x0000),
                emulator.bus.read(0x0001)
            );
        }

        println!("IO writes (STA abs) 0000-03FF: {}", count_io_low);
        println!("IO writes (STA abs) 2000-23FF: {}", count_io_high);
        if !io_hist.is_empty() {
            println!("Top IO STA addresses:");
            let mut entries: Vec<(u16, usize)> = io_hist.iter().map(|(k, v)| (*k, *v)).collect();
            entries.sort_by(|a, b| b.1.cmp(&a.1));
            for (addr, count) in entries.into_iter().take(16) {
                println!("  {:04X}: {}", addr, count);
            }
        }

        if trace_opcodes {
            opc_trace.log(emulator.cpu.pc, opcode, &emulator.bus.mpr_array());
        }

        if emulator.cpu.halted {
            println!("HALT after step {step} (cycles={cycles} pc={pc:04X})");
            break;
        }
        println!(
            "  -> post step pc={:04X} sp={:02X}",
            emulator.cpu.pc, emulator.cpu.sp
        );
    }

    if pc_hist_limit > 0 {
        let mut entries: Vec<(u16, usize)> = pc_hist.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        println!("Top PC histogram:");
        for (pc, count) in entries.into_iter().take(pc_hist_limit) {
            println!("  {:04X}: {}", pc, count);
        }
    }

    Ok(())
}

fn parse_zp_list(spec: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut addrs = Vec::new();
    for segment in spec.split(',') {
        if segment.is_empty() {
            continue;
        }
        let parsed = parse_u16(segment)?;
        if parsed > 0xFF {
            return Err("zero-page address out of range".into());
        }
        addrs.push(parsed as u8);
    }
    Ok(addrs)
}

fn parse_status_list(value: &str) -> Result<u8, Box<dyn Error>> {
    let mut mask = 0u8;
    for raw_token in value.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            continue;
        }
        let upper = token.to_ascii_uppercase();
        mask |= match upper.as_str() {
            "VBL" => VDC_STATUS_VBL,
            "DS" => VDC_STATUS_DS,
            "DV" => VDC_STATUS_DV,
            "RCR" => VDC_STATUS_RCR,
            "CR" => VDC_STATUS_CR,
            "OR" => VDC_STATUS_OR,
            "BUSY" => VDC_STATUS_BUSY,
            other => {
                return Err(format!("unknown VDC status flag '{other}'").into());
            }
        };
    }
    if mask == 0 {
        return Err("--break-status requires at least one recognised flag".into());
    }
    Ok(mask)
}

fn parse_u16(value: &str) -> Result<u16, Box<dyn Error>> {
    if let Some(stripped) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u16::from_str_radix(stripped, 16).map_err(|_| "invalid hex value".into())
    } else {
        value
            .parse::<u16>()
            .map_err(|_| "invalid 16-bit value".into())
    }
}

fn dump_memory_window(bus: &mut Bus, centre: u16) {
    const WINDOW: u16 = 0x20;
    let start = centre.saturating_sub(WINDOW);
    let end = centre.saturating_add(WINDOW);
    println!("Dumping memory around 0x{centre:04X} (0x{start:04X}..0x{end:04X}):");
    let mut addr = start;
    while addr <= end {
        if addr % 16 == 0 {
            print!("\n{addr:04X}: ");
        }
        let byte = bus.read(addr);
        print!("{byte:02X} ");
        addr = addr.wrapping_add(1);
        if addr == 0 {
            break;
        }
    }
    println!("\n");
}

fn collect_status_reasons(bits: u8) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if bits & VDC_STATUS_VBL != 0 {
        reasons.push("VBlank");
    }
    if bits & VDC_STATUS_DS != 0 {
        reasons.push("DS");
    }
    if bits & VDC_STATUS_DV != 0 {
        reasons.push("DV");
    }
    if bits & VDC_STATUS_RCR != 0 {
        reasons.push("RCR");
    }
    if bits & VDC_STATUS_CR != 0 {
        reasons.push("CR");
    }
    if bits & VDC_STATUS_OR != 0 {
        reasons.push("OR");
    }
    if bits & VDC_STATUS_BUSY != 0 {
        reasons.push("BUSY");
    }
    reasons
}
