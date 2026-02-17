use crate::psg::Psg;
use crate::vce::Vce;
use crate::vdc::{
    Vdc,
    FRAME_WIDTH, FRAME_HEIGHT,
    VDC_DMA_WORD_CYCLES,
    VDC_CTRL_ENABLE_SPRITES_LEGACY, VDC_CTRL_ENABLE_BACKGROUND_LEGACY,
    VDC_CTRL_ENABLE_BACKGROUND, VDC_CTRL_ENABLE_SPRITES,
    DMA_CTRL_SRC_DEC, DMA_CTRL_DST_DEC,
};

// Re-export VDC status constants for external consumers (examples, etc.)
// These were originally `pub const` in this file.
pub use crate::vdc::{
    VDC_STATUS_CR, VDC_STATUS_OR, VDC_STATUS_RCR,
    VDC_STATUS_DS, VDC_STATUS_DV, VDC_STATUS_VBL, VDC_STATUS_BUSY,
};

pub const PAGE_SIZE: usize = 0x2000; // 8 KiB per bank
const NUM_BANKS: usize = 8;
const RAM_SIZE: usize = PAGE_SIZE * NUM_BANKS;
const IO_REG_SIZE: usize = PAGE_SIZE; // full hardware page
pub const IRQ_DISABLE_IRQ2: u8 = 0x01;
pub const IRQ_DISABLE_IRQ1: u8 = 0x02;
pub const IRQ_DISABLE_TIMER: u8 = 0x04;
pub const IRQ_REQUEST_IRQ2: u8 = 0x01;
pub const IRQ_REQUEST_IRQ1: u8 = 0x02;
pub const IRQ_REQUEST_TIMER: u8 = 0x04;
const TIMER_CONTROL_START: u8 = 0x01;
const HW_TIMER_BASE: usize = 0x0C00;
const HW_JOYPAD_BASE: usize = 0x1000;
const HW_IRQ_BASE: usize = 0x1400;
const HW_CPU_CTRL_BASE: usize = 0x1C00;
const MASTER_CLOCK_HZ: u32 = 7_159_090;
const AUDIO_SAMPLE_RATE: u32 = 44_100;

mod font;
mod env;
mod render;

use font::FONT;

/// A `bool` wrapper that is invisible to bincode serialization.
/// Encodes as zero bytes; decodes as `false`.  Used for transient render
/// state that must survive struct derivation but not save-state files.
#[derive(Clone, Copy, Default)]
struct TransientBool(bool);

impl bincode::Encode for TransientBool {
    fn encode<E: bincode::enc::Encoder>(&self, _encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
        Ok(()) // write nothing
    }
}

impl<Context> bincode::Decode<Context> for TransientBool {
    fn decode<D: bincode::de::Decoder>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(false))
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for TransientBool {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(false))
    }
}

impl core::ops::Deref for TransientBool {
    type Target = bool;
    fn deref(&self) -> &bool { &self.0 }
}

impl core::ops::DerefMut for TransientBool {
    fn deref_mut(&mut self) -> &mut bool { &mut self.0 }
}



#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
enum VdcPort {
    Control,
    Data,
}

/// Memory bus exposing an 8x8 KiB banked window into linear RAM/ROM data.
/// This mirrors the HuC6280 page architecture and provides simple helpers
/// for experimenting with bank switching.
#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct Bus {
    ram: Vec<u8>,
    rom: Vec<u8>,
    banks: [BankMapping; NUM_BANKS],
    mpr: [u8; NUM_BANKS],
    st_ports: [u8; 3],
    io: [u8; IO_REG_SIZE],
    io_port: IoPort,
    interrupt_disable: u8,
    interrupt_request: u8,
    timer: Timer,
    vdc: Vdc,
    psg: Psg,
    vce: Vce,
    audio_phi_accumulator: u64,
    audio_buffer: Vec<i16>,
    framebuffer: Vec<u32>,
    frame_ready: bool,
    cart_ram: Vec<u8>,
    current_display_width: usize,
    current_display_height: usize,
    current_display_y_offset: usize,
    bg_opaque: Vec<bool>,
    bg_priority: Vec<bool>,
    sprite_line_counts: Vec<u8>,
    /// True while in a post-burst-mode transition: the VDC went through burst
    /// mode (both BG and SPR off) and is now in SPR-only mode.  During this
    /// window the game is setting up the new scene; rendering sprites would
    /// flash partially-loaded content.  Cleared when BG is re-enabled (scene
    /// ready).  This matches CRT behavior where the brief sprite flash during
    /// scene transitions is invisible due to phosphor response/blanking.
    /// Not serialized — transient render state, safe to default to false.
    burst_transition: TransientBool,
    #[cfg(feature = "trace_hw_writes")]
    last_pc_for_trace: Option<u16>,
    #[cfg(debug_assertions)]
    debug_force_ds_after: u64,
    #[cfg(feature = "trace_hw_writes")]
    st0_lock_window: u8,
}


impl Bus {
    pub fn new() -> Self {
        let mut bus = Self {
            ram: vec![0; RAM_SIZE],
            rom: Vec::new(),
            banks: [BankMapping::Ram { base: 0 }; NUM_BANKS],
            mpr: [0; NUM_BANKS],
            st_ports: [0; 3],
            io: [0; IO_REG_SIZE],
            io_port: IoPort::new(),
            interrupt_disable: 0,
            interrupt_request: 0,
            timer: Timer::new(),
            vdc: Vdc::new(),
            psg: Psg::new(),
            vce: Vce::new(),
            audio_phi_accumulator: 0,
            audio_buffer: Vec::new(),
            framebuffer: vec![0; FRAME_WIDTH * FRAME_HEIGHT],
            frame_ready: false,
            cart_ram: Vec::new(),
            current_display_width: 256,
            current_display_height: 224,
            current_display_y_offset: 0,
            bg_opaque: vec![false; FRAME_WIDTH * FRAME_HEIGHT],
            bg_priority: vec![false; FRAME_WIDTH * FRAME_HEIGHT],
            sprite_line_counts: vec![0; FRAME_HEIGHT],
            burst_transition: TransientBool(false),
            #[cfg(feature = "trace_hw_writes")]
            last_pc_for_trace: None,
            #[cfg(debug_assertions)]
            debug_force_ds_after: 0,
            #[cfg(feature = "trace_hw_writes")]
            st0_lock_window: 0,
        };

        // Power-on mapping: expose internal RAM in bank 0 for ZP/stack and
        // keep all banks backed by RAM. The HuCARD loader remaps banks 4–7
        // to ROM after parsing the image header.
        let ram_pages = RAM_SIZE / PAGE_SIZE;
        for index in 0..NUM_BANKS {
            let page = index % ram_pages;
            bus.mpr[index] = 0xF8u8.saturating_add(page as u8);
            bus.update_mpr(index);
        }
        // Keep the top bank pointing at RAM so the reset vector can be patched
        // when loading raw programs; HuCARD mapping will override this later.
        bus.mpr[NUM_BANKS - 1] = 0xF8;
        bus.update_mpr(NUM_BANKS - 1);

        if Self::env_force_mpr1_hardware() {
            bus.set_mpr(1, 0xFF);
        }
        // Allow overriding default pad input for BIOS waits.
        bus.io_port.input = Self::env_pad_default();
        // Optionally start timer running by default (debug aid).
        if Self::env_timer_default_start() {
            bus.timer.enabled = true;
            bus.timer.counter = bus.timer.reload;
            bus.timer.prescaler = 0;
        }

        if Self::env_force_title_scene() {
            bus.force_title_scene();
        }

        bus
    }

    #[cfg(feature = "trace_hw_writes")]
    fn log_hw_access(kind: &str, addr: u16, value: u8) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let idx = COUNT.fetch_add(1, Ordering::Relaxed);
        if idx < 1_000_000 {
            eprintln!("{kind} {:04X} -> {:02X}", addr, value);
        }
    }

    #[inline]
    pub fn read(&mut self, addr: u16) -> u8 {
        if (0x2000..=0x3FFF).contains(&addr) {
            if matches!(self.banks.get(1), Some(BankMapping::Hardware))
                || Self::env_relax_io_mirror()
                || Self::env_extreme_mirror()
                || Self::env_vdc_ultra_mirror()
            {
                let offset = (addr - 0x2000) as usize;
                let value = self.read_io_internal(offset);
                #[cfg(feature = "trace_hw_writes")]
                {
                    Self::log_hw_access("R", addr, value);
                    if offset <= 0x0403 || Self::env_extreme_mirror() {
                        eprintln!("  IO read offset {:04X} -> {:02X}", offset, value);
                    }
                    if offset >= 0x1C00 && offset <= 0x1C13 {
                        eprintln!("  TIMER/IRQ read {:04X} -> {:02X}", offset, value);
                    }
                    if offset >= 0x1C60 && offset <= 0x1C63 {
                        eprintln!("  PSG ctrl read {:04X} -> {:02X}", offset, value);
                    }
                }
                self.refresh_vdc_irq();
                return value;
            }
        }
        let (mapping, offset) = self.resolve(addr);
        // MPR registers ($FF80-$FFBF) are only accessible when the address
        // falls within a hardware-mapped bank (MPR value $FF).  When MPR7
        // maps to ROM, $FF80-$FFBF must read ROM data, not MPR values.
        if matches!(mapping, BankMapping::Hardware) {
            if let Some(index) = Self::mpr_index_for_addr(addr) {
                return self.mpr[index];
            }
        }
        match mapping {
            BankMapping::Ram { base } => self.ram[base + offset],
            BankMapping::Rom { base } => self.rom.get(base + offset).copied().unwrap_or(0xFF),
            BankMapping::CartRam { base } => {
                self.cart_ram.get(base + offset).copied().unwrap_or(0x00)
            }
            BankMapping::Hardware => {
                let io_offset = (addr as usize) & (PAGE_SIZE - 1);
                // Real PCE hardware only decodes I/O at offsets $0000-$17FF
                // (A12:A10 selects VDC/VCE/PSG/Timer/Joypad/IRQ).  Offsets
                // $1800-$1FFF have no I/O device; reads fall through to the
                // HuCard ROM bus.  This is essential for reading interrupt
                // vectors ($1FF6-$1FFF) when MPR7=$FF at reset.
                if io_offset >= 0x1800 {
                    let rom_pages = self.rom_pages();
                    if rom_pages > 0 {
                        let rom_page = Self::mirror_rom_bank(0xFF, rom_pages);
                        let rom_addr = rom_page * PAGE_SIZE + io_offset;
                        return self.rom.get(rom_addr).copied().unwrap_or(0xFF);
                    }
                    return 0xFF;
                }
                let value = self.read_io_internal(io_offset);
                self.refresh_vdc_irq();
                #[cfg(feature = "trace_hw_writes")]
                {
                    Self::log_hw_access("R", addr, value);
                    if io_offset <= 0x0403 {
                        eprintln!("  HW read offset {:04X} -> {:02X}", io_offset, value);
                    }
                    if io_offset >= 0x1C00 && io_offset <= 0x1C13 {
                        eprintln!("  TIMER/IRQ read {:04X} -> {:02X}", io_offset, value);
                    }
                    if io_offset >= 0x1C60 && io_offset <= 0x1C63 {
                        eprintln!("  PSG ctrl read {:04X} -> {:02X}", io_offset, value);
                    }
                }
                value
            }
        }
    }

    #[inline]
    pub fn write(&mut self, addr: u16, value: u8) {
        // Fast path: any offset 0x0400–0x07FF within the hardware page maps to the VCE.
        // The VCE ports repeat every 8 bytes (A2..A0 decode), so higher bits are mirrors.
        let mapping = self.banks[(addr as usize) >> 13];
        let mirrored = addr & 0x1FFF;
        if (matches!(mapping, BankMapping::Hardware) || Self::env_extreme_mirror())
            && (0x0400..=0x07FF).contains(&mirrored)
        {
            self.write_vce_port(mirrored as u16, value);
            self.refresh_vdc_irq();
            return;
        }
        // Catch-all debug: force any <0x4000 write to go to VCE ports (decode A2..A0).
        if Self::env_vce_catchall() && (addr as usize) < 0x4000 {
            self.write_vce_port(addr as u16, value);
            self.refresh_vdc_irq();
            return;
        }
        #[cfg(feature = "trace_hw_writes")]
        if (addr & 0x1FFF) >= 0x0400 && (addr & 0x1FFF) <= 0x0403 {
            eprintln!(
                "  WARN write {:04X} -> {:02X} (mapping {:?})",
                addr,
                value,
                self.banks[(addr as usize) >> 13]
            );
        }

        if (0x2000..=0x3FFF).contains(&addr) {
            if matches!(self.banks.get(1), Some(BankMapping::Hardware))
                || Self::env_relax_io_mirror()
                || Self::env_extreme_mirror()
            {
                let offset = (addr - 0x2000) as usize;
                self.write_io_internal(offset, value);
                #[cfg(feature = "trace_hw_writes")]
                {
                    // Reduce spam: only show IO writes when offset <= 0x0100 or value non-zero.
                    if offset <= 0x0100 || value != 0 || Self::env_extreme_mirror() {
                        Self::log_hw_access("W", addr, value);
                        if offset <= 0x03FF || Self::env_extreme_mirror() {
                            eprintln!("  IO write offset {:04X} -> {:02X}", offset, value);
                        }
                    }
                }

                self.refresh_vdc_irq();
                return;
            }
        }
        let (mapping, offset) = self.resolve(addr);
        // MPR registers ($FF80-$FFBF) are only writable when the address
        // falls within a hardware-mapped bank.
        if matches!(mapping, BankMapping::Hardware) {
            if let Some(index) = Self::mpr_index_for_addr(addr) {
                self.set_mpr(index, value);
                return;
            }
        }
        match mapping {
            BankMapping::Ram { base } => {
                let index = base + offset;
                if index < self.ram.len() {
                    #[cfg(feature = "trace_hw_writes")]
                    if index == 0x20 {
                        eprintln!("  ZP[20] <= {:02X}", value);
                    }
                    self.ram[index] = value;
                }
            }
            BankMapping::CartRam { base } => {
                let index = base + offset;
                if index < self.cart_ram.len() {
                    self.cart_ram[index] = value;
                }
            }
            BankMapping::Hardware => {
                let io_offset = (addr as usize) & (PAGE_SIZE - 1);
                self.write_io_internal(io_offset, value);
                #[cfg(feature = "trace_hw_writes")]
                {
                    Self::log_hw_access("W", addr, value);
                    if io_offset <= 0x0403 {
                        eprintln!("  HW write offset {:04X} -> {:02X}", io_offset, value);
                    }
                }

                self.refresh_vdc_irq();
            }
            BankMapping::Rom { .. } => {}
        }
    }

    /// Copy a slice into memory starting at the given address.
    pub fn load(&mut self, start: u16, data: &[u8]) {
        let mut addr = start;
        for byte in data {
            self.write(addr, *byte);
            addr = addr.wrapping_add(1);
        }
    }

    #[inline]
    pub fn read_u16(&mut self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    #[inline]
    pub fn write_u16(&mut self, addr: u16, value: u16) {
        self.write(addr, (value & 0x00FF) as u8);
        self.write(addr.wrapping_add(1), (value >> 8) as u8);
    }

    pub fn clear(&mut self) {
        self.ram.fill(0);
        self.io.fill(0);
        self.io_port.reset();
        self.interrupt_disable = 0;
        self.interrupt_request = 0;
        self.timer.reset();
        self.vdc.reset();
        self.psg.reset();
        self.vce.reset();
        self.audio_phi_accumulator = 0;
        self.audio_buffer.clear();
        self.framebuffer.fill(0);
        self.frame_ready = false;
        self.cart_ram.fill(0);
        self.bg_opaque.fill(false);
        self.bg_priority.fill(false);
        self.sprite_line_counts.fill(0);
        self.vdc.clear_sprite_overflow();
        #[cfg(debug_assertions)]
        {
            self.debug_force_ds_after = 0;
        }
        #[cfg(feature = "trace_hw_writes")]
        {
            self.st0_lock_window = 0;
        }
    }

    /// Replace backing ROM data. Bank mappings are left untouched so the
    /// caller can decide which windows should point at the new image.
    pub fn load_rom_image(&mut self, data: Vec<u8>) {
        self.rom = data;
        for idx in 0..NUM_BANKS {
            self.update_mpr(idx);
        }
    }

    pub fn map_bank_to_ram(&mut self, bank: usize, page: usize) {
        if bank < NUM_BANKS {
            let pages = self.total_ram_pages();
            let page_index = if pages == 0 { 0 } else { page % pages };
            self.mpr[bank] = 0xF8u8.saturating_add(page_index as u8);
            self.update_mpr(bank);
        }
    }

    pub fn map_bank_to_rom(&mut self, bank: usize, rom_bank: usize) {
        if bank < NUM_BANKS {
            let pages = self.rom_pages();
            let page_index = if pages == 0 { 0 } else { rom_bank % pages };
            self.mpr[bank] = page_index as u8;
            self.update_mpr(bank);
        }
    }

    pub fn set_mpr(&mut self, index: usize, value: u8) {
        if index < NUM_BANKS {
            if index == 1 && Self::env_force_mpr1_hardware() {
                #[cfg(feature = "trace_hw_writes")]
                eprintln!(
                    "  MPR1 force-hardware active: ignoring write {:02X}, keeping FF",
                    value
                );
                self.mpr[1] = 0xFF;
                self.update_mpr(1);
                return;
            }
            self.mpr[index] = value;
            self.update_mpr(index);
            #[cfg(feature = "trace_hw_writes")]
            eprintln!("  MPR{index} <= {:02X} -> {:?}", value, self.banks[index]);
        }
    }

    pub fn mpr(&self, index: usize) -> u8 {
        self.mpr[index]
    }

    pub fn mpr_array(&self) -> [u8; NUM_BANKS] {
        let mut out = [0u8; NUM_BANKS];
        out.copy_from_slice(&self.mpr);
        out
    }

    pub fn rom_page_count(&self) -> usize {
        self.rom.len() / PAGE_SIZE
    }

    pub fn write_st_port(&mut self, port: usize, value: u8) {
        let slot_index = port.min(self.st_ports.len().saturating_sub(1));
        if let Some(slot) = self.st_ports.get_mut(slot_index) {
            *slot = value;
        }
        #[cfg(feature = "trace_hw_writes")]
        if Self::env_trace_mpr() {
            use std::fmt::Write as _;
            let mut m = String::new();
            for (i, val) in self.mpr.iter().enumerate() {
                let _ = write!(m, "{}:{:02X} ", i, val);
            }
            eprintln!(
                "  TRACE MPR pc={:04X} st{}={:02X} mpr={}",
                self.last_pc_for_trace.unwrap_or(0),
                port,
                value,
                m.trim_end()
            );
        }
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  ST{port} <= {:02X} (addr={:04X})",
            value, self.vdc.last_io_addr
        );
        match port {
            0 => {
                #[cfg(feature = "trace_hw_writes")]
                if !Self::st0_hold_enabled() {
                    self.vdc.st0_hold_counter = 0;
                }
                #[cfg(feature = "trace_hw_writes")]
                if self.vdc.st0_hold_counter > 0 {
                    // Mirror spam often re-writes 0 to ST0 immediately after a data byte.
                    // Ignore those redundant zeros, but allow a non-zero selector to punch
                    // through even while the hold is active.
                    if value == self.vdc.selected_register() {
                        self.vdc.st0_hold_counter = self.vdc.st0_hold_counter.saturating_sub(1);
                        let idx = (self.vdc.last_io_addr as usize) & 0xFF;
                        if let Some(slot) = self.vdc.st0_hold_addr_hist.get_mut(idx) {
                            *slot = slot.saturating_add(1);
                        }
                        eprintln!(
                            "  ST0 ignored (hold) pending={:?} phase={:?} value={:02X}",
                            self.vdc.pending_write_register, self.vdc.write_phase, value
                        );
                        return;
                    }
                    // Let the new selection proceed; clear the hold so the register change
                    // isn't dropped.
                    self.vdc.st0_hold_counter = 0;
                }
                self.vdc.write_port(0, value)
            }
            1 => {
                #[cfg(feature = "trace_hw_writes")]
                {
                    if Self::st0_hold_enabled() {
                        const HOLD_SPAN: u8 = 8;
                        self.vdc.st0_hold_counter = HOLD_SPAN;
                    } else {
                        self.vdc.st0_hold_counter = 0;
                    }
                }
                self.vdc.write_port(1, value)
            }
            2 => {
                #[cfg(feature = "trace_hw_writes")]
                {
                    if Self::st0_hold_enabled() {
                        const HOLD_SPAN: u8 = 8;
                        self.vdc.st0_hold_counter = HOLD_SPAN;
                    } else {
                        self.vdc.st0_hold_counter = 0;
                    }
                }
                self.vdc.write_port(2, value)
            }
            _ => {}
        }
        #[cfg(feature = "trace_hw_writes")]
        if port == 0 && value == 0x05 {
            self.vdc.pending_traced_register = Some(0x05);
            #[cfg(feature = "trace_hw_writes")]
            eprintln!("  TRACE select R05");
        }
        #[cfg(feature = "trace_hw_writes")]
        if matches!(port, 1 | 2) {
            if let Some(sel) = self.vdc.pending_traced_register.take() {
                #[cfg(feature = "trace_hw_writes")]
                {
                    use std::fmt::Write as _;
                    let mut mpr_buf = String::new();
                    for (i, m) in self.mpr.iter().enumerate() {
                        if i > 0 {
                            mpr_buf.push(' ');
                        }
                        let _ = write!(mpr_buf, "{:02X}", m);
                    }
                    eprintln!(
                        "  TRACE R{:02X} data via ST{} = {:02X} (selected={:02X} pc={:04X} mpr={})",
                        sel,
                        port,
                        value,
                        self.vdc.selected_register(),
                        self.last_pc_for_trace.unwrap_or(0),
                        mpr_buf
                    );
                }
            }
        }
        if self.vdc.take_vram_dma_request() {
            self.perform_vram_dma();
        }
        self.refresh_vdc_irq();
    }

    pub fn read_st_port(&mut self, port: usize) -> u8 {
        let value = match port {
            0 => self.vdc.selected_register(),
            1 => self.vdc.read_port(1),
            2 => self.vdc.read_port(2),
            _ => 0,
        };
        let slot_index = port.min(self.st_ports.len().saturating_sub(1));
        if let Some(slot) = self.st_ports.get_mut(slot_index) {
            *slot = value;
        }
        self.refresh_vdc_irq();
        value
    }

    pub fn st_port(&self, port: usize) -> u8 {
        self.st_ports.get(port).copied().unwrap_or(0)
    }

    pub fn vdc_register(&self, index: usize) -> Option<u16> {
        self.vdc.register(index)
    }

    pub fn vdc_status_bits(&self) -> u8 {
        self.vdc.status_bits()
    }

    pub fn vdc_current_scanline(&self) -> u16 {
        self.vdc.scanline
    }

    pub fn vdc_in_vblank(&self) -> bool {
        self.vdc.in_vblank
    }

    pub fn vdc_busy_cycles(&self) -> u32 {
        self.vdc.busy_cycles
    }

    pub fn vdc_map_dimensions(&self) -> (usize, usize) {
        self.vdc.map_dimensions()
    }

    pub fn vdc_vram_word(&self, addr: u16) -> u16 {
        let idx = (addr as usize) & 0x7FFF;
        *self.vdc.vram.get(idx).unwrap_or(&0)
    }

    /// Write a word directly to VDC VRAM (bypassing the register/MAWR mechanism).
    /// Used for BIOS emulation (e.g., loading built-in font at power-on).
    pub fn vdc_write_vram_direct(&mut self, addr: u16, value: u16) {
        let idx = (addr as usize) & 0x7FFF;
        if let Some(slot) = self.vdc.vram.get_mut(idx) {
            *slot = value;
        }
    }

    #[cfg(test)]
    pub fn sprite_line_counts_for_test(&self) -> &[u8] {
        &self.sprite_line_counts
    }

    pub fn vce_palette_word(&self, index: usize) -> u16 {
        self.vce.palette_word(index)
    }

    pub fn vce_palette_rgb(&self, index: usize) -> u32 {
        self.vce.palette_rgb(index)
    }

    #[cfg(test)]
    pub fn vdc_set_status_for_test(&mut self, mask: u8) {
        self.vdc.raise_status(mask);
        self.refresh_vdc_irq();
    }

    pub fn read_io(&mut self, offset: usize) -> u8 {
        let value = self.read_io_internal(offset);
        self.refresh_vdc_irq();
        value
    }

    pub fn write_io(&mut self, offset: usize, value: u8) {
        self.write_io_internal(offset, value);
        self.refresh_vdc_irq();
    }

    pub fn tick(&mut self, cycles: u32, high_speed: bool) -> bool {
        let phi_cycles = if high_speed {
            cycles
        } else {
            cycles.saturating_mul(4)
        };

        // Debug: force timer expiry to drive IRQ2 if requested.
        if Self::env_force_timer() {
            self.timer.counter = 0;
            self.interrupt_request |= IRQ_REQUEST_TIMER;
        }

        if self.vdc.tick(phi_cycles) {
            self.refresh_vdc_irq();
        }

        if self.vdc.in_vblank && self.vdc.cram_pending {
            self.perform_cram_dma();
            self.refresh_vdc_irq();
        }

        if self.vdc.frame_ready() {
            self.render_frame_from_vram();
        }

        if self.timer.tick(cycles, high_speed) {
            self.interrupt_request |= IRQ_REQUEST_TIMER;
        }

        if self.psg.tick(cycles) {
            self.raise_irq(IRQ_REQUEST_IRQ2);
        }

        self.enqueue_audio_samples(phi_cycles);

        self.refresh_vdc_irq();

        self.irq_pending()
    }

    #[cfg(feature = "trace_hw_writes")]
    pub fn set_last_pc_for_trace(&mut self, pc: u16) {
        self.last_pc_for_trace = Some(pc);
    }

    pub fn psg_sample(&mut self) -> i16 {
        self.psg.generate_sample()
    }

    /// Returns per-channel PSG state: (frequency, control, balance, noise_control)
    pub fn psg_channel_info(&self, ch: usize) -> (u16, u8, u8, u8) {
        if ch < 6 {
            let c = &self.psg.channels[ch];
            (c.frequency, c.control, c.balance, c.noise_control)
        } else {
            (0, 0, 0, 0)
        }
    }

    /// Returns PSG main balance register.
    pub fn psg_main_balance(&self) -> u8 {
        self.psg.main_balance
    }

    /// Returns a copy of the 32-byte waveform table for the given channel.
    pub fn psg_waveform(&self, ch: usize) -> [u8; 32] {
        let mut out = [0u8; 32];
        if ch < 6 {
            let base = ch * 32;
            out.copy_from_slice(&self.psg.waveform_ram[base..base + 32]);
        }
        out
    }

    /// Returns (wave_pos, wave_write_pos, phase, phase_step, dda_sample) for a channel.
    pub fn psg_channel_detail(&self, ch: usize) -> (u8, u8, u32, u32, u8) {
        if ch < 6 {
            let c = &self.psg.channels[ch];
            (
                c.wave_pos,
                c.wave_write_pos,
                c.phase,
                c.phase_step,
                c.dda_sample,
            )
        } else {
            (0, 0, 0, 0, 0)
        }
    }

    /// Returns timer state: (reload, counter, enabled, prescaler).
    pub fn timer_info(&self) -> (u8, u8, bool, u32) {
        (
            self.timer.reload,
            self.timer.counter,
            self.timer.enabled,
            self.timer.prescaler,
        )
    }

    /// Returns IRQ disable mask and request register.
    pub fn irq_state(&self) -> (u8, u8) {
        (self.interrupt_disable, self.interrupt_request)
    }

    pub fn take_audio_samples(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.audio_buffer)
    }

    pub fn take_frame(&mut self) -> Option<Vec<u32>> {
        if !self.frame_ready {
            // 強制タイトル表示が有効なら、フレームが用意されていなくても即描画を返す
            if Self::env_force_title_scene() || Self::env_force_title_now() {
                return Some(Self::synth_title_frame());
            } else {
                return None;
            }
        }
        self.frame_ready = false;
        if Self::env_force_title_now() || Self::env_force_title_scene() {
            return Some(Self::synth_title_frame());
        }
        let w = self.current_display_width;
        let h = self.current_display_height;
        let y_off = self.current_display_y_offset;
        let mut out = vec![0u32; w * h];
        for y in 0..h {
            let src_y = y + y_off;
            if src_y >= FRAME_HEIGHT {
                break;
            }
            let src = src_y * FRAME_WIDTH;
            let dst = y * w;
            out[dst..dst + w].copy_from_slice(&self.framebuffer[src..src + w]);
        }
        Some(out)
    }

    fn synth_title_frame() -> Vec<u32> {
        const W: usize = 256;
        const H: usize = FRAME_HEIGHT;
        let mut fb = vec![0u32; W * H];
        // 背景グラデーション
        for y in 0..H {
            let band = (y / 30) as u32;
            let base = 0x101820 + (band * 0x030303);
            for x in 0..W {
                fb[y * W + x] = base;
            }
        }
        // 簡易ロゴ「KATO-CHAN & KEN-CHAN」
        let text = b"KATO-CHAN & KEN-CHAN";
        let colors = [0xC8E4FF, 0x80B0FF, 0x4060E0, 0x102040];
        let mut draw_char = |ch: u8, ox: usize, oy: usize, col: u32| {
            for dy in 0..10 {
                for dx in 0..8 {
                    if (FONT[(ch as usize).wrapping_sub(32)].get(dy).unwrap_or(&0) >> (7 - dx)) & 1
                        == 1
                    {
                        let x = ox + dx;
                        let y = oy + dy;
                        if x < W && y < H {
                            fb[y * W + x] = col;
                        }
                    }
                }
            }
        };
        let start_x = 24;
        let start_y = 60;
        for (i, &ch) in text.iter().enumerate() {
            let col = colors[i % colors.len()];
            draw_char(ch, start_x + i * 9, start_y, col);
        }
        fb
    }

    fn force_title_scene(&mut self) {
        // Enable BG/sprite
        let ctrl = VDC_CTRL_ENABLE_BACKGROUND_LEGACY
            | VDC_CTRL_ENABLE_SPRITES_LEGACY
            | VDC_CTRL_ENABLE_BACKGROUND
            | VDC_CTRL_ENABLE_SPRITES;
        self.vdc.registers[0x04] = ctrl;
        self.vdc.registers[0x05] = ctrl;
        self.vdc
            .raise_status(VDC_STATUS_DS | VDC_STATUS_DV | VDC_STATUS_VBL);
        // Map size 64x32, base 0
        self.vdc.registers[0x09] = 0x0010;
        // Palette: simple gradient
        for (i, slot) in self.vce.palette.iter_mut().enumerate() {
            *slot = ((i as u16 & 0x0F) << 8) | (((i as u16 >> 4) & 0x0F) << 4) | (i as u16 & 0x0F);
        }
        // Tiles: simple 8x8 patterns
        for tile in 0..0x200 {
            for row in 0..8 {
                let pattern = (((tile + row) & 1) * 0xFF) as u16;
                let addr = tile * 8 + row;
                if let Some(slot) = self.vdc.vram.get_mut(addr) {
                    *slot = pattern;
                }
            }
        }
        // BAT: sequential tiles
        let (map_w, map_h) = self.vdc.map_dimensions();
        let base = self.vdc.map_base_address();
        let mask = self.vdc.vram.len() - 1;
        for y in 0..map_h {
            for x in 0..map_w {
                let idx = ((y * map_w + x) & 0x7FF) as u16;
                let addr = (base + ((y * map_w + x) % 0x400)) & mask;
                self.vdc.vram[addr] = idx;
            }
        }
        // SATB: place one sprite in corner
        self.vdc.satb[0] = 0; // y
        self.vdc.satb[1] = 0; // x
        self.vdc.satb[2] = 0; // pattern/cg
        self.vdc.satb[3] = 0; // attr
        self.frame_ready = true;
    }

    pub fn framebuffer(&self) -> &[u32] {
        &self.framebuffer
    }

    fn compute_display_width(&self) -> usize {
        let hdr = self.vdc.registers[0x0B];
        let hdw = (hdr & 0x7F) as usize;
        if hdw == 0 {
            return 256;
        }
        ((hdw + 1) * 8).min(FRAME_WIDTH)
    }

    pub fn display_width(&self) -> usize {
        self.current_display_width
    }

    fn compute_display_height(&self) -> (usize, usize) {
        let timing_programmed = self.vdc.registers[0x0D] != 0
            || self.vdc.registers[0x0E] != 0
            || (self.vdc.registers[0x0C] & 0xFF00) != 0;
        if !timing_programmed {
            return (FRAME_HEIGHT, 0);
        }
        // Find the first and last active rows in the framebuffer.
        // Non-active rows are overscan/blanking that we trim.
        let mut first_active = FRAME_HEIGHT;
        let mut last_active = 0usize;
        for y in 0..FRAME_HEIGHT {
            if self.vdc.output_row_in_active_window(y) {
                if y < first_active {
                    first_active = y;
                }
                last_active = y;
            }
        }
        if first_active >= FRAME_HEIGHT {
            return (FRAME_HEIGHT, 0);
        }
        let active_count = last_active - first_active + 1;
        (active_count, first_active)
    }

    pub fn display_height(&self) -> usize {
        self.current_display_height
    }

    pub fn display_y_offset(&self) -> usize {
        self.current_display_y_offset
    }

    pub fn vdc_control_register(&self) -> u16 {
        self.vdc.control()
    }

    pub fn vdc_control_for_render(&self) -> u16 {
        self.vdc.control_for_render()
    }

    pub fn vdc_mawr(&self) -> u16 {
        self.vdc.mawr
    }

    pub fn vdc_satb_pending(&self) -> bool {
        self.vdc.satb_pending()
    }

    pub fn vdc_satb_written(&self) -> bool {
        self.vdc.satb_written
    }

    pub fn vdc_satb_source(&self) -> u16 {
        self.vdc.satb_source()
    }

    pub fn vdc_satb_nonzero_words(&self) -> usize {
        self.vdc.satb.iter().filter(|&&word| word != 0).count()
    }

    pub fn vdc_satb_word(&self, index: usize) -> u16 {
        self.vdc.satb.get(index).copied().unwrap_or(0)
    }

    pub fn vdc_dma_control(&self) -> u16 {
        self.vdc.dma_control
    }

    pub fn vdc_scroll_line(&self, line: usize) -> (u16, u16) {
        self.vdc.scroll_line(line)
    }

    pub fn vdc_scroll_line_valid(&self, line: usize) -> bool {
        self.vdc.scroll_line_valid(line)
    }

    pub fn vdc_scroll_line_y_offset(&self, line: usize) -> u16 {
        if line < self.vdc.scroll_line_y_offset.len() {
            self.vdc.scroll_line_y_offset[line]
        } else {
            0
        }
    }

    pub fn vdc_line_state_index_for_row(&self, row: usize) -> usize {
        self.vdc.line_state_index_for_frame_row(row)
    }

    pub fn vdc_zoom_line(&self, line: usize) -> (u16, u16) {
        self.vdc.zoom_line(line)
    }

    pub fn vdc_control_line(&self, line: usize) -> u16 {
        self.vdc.control_line(line)
    }

    pub fn configure_cart_ram(&mut self, size: usize) {
        if size == 0 {
            self.cart_ram.clear();
        } else if self.cart_ram.len() != size {
            self.cart_ram = vec![0; size];
        } else {
            self.cart_ram.fill(0);
        }
        for idx in 0..NUM_BANKS {
            self.update_mpr(idx);
        }
    }

    pub fn cart_ram_size(&self) -> usize {
        self.cart_ram.len()
    }

    pub fn set_joypad_input(&mut self, state: u8) {
        self.io_port.input = state;
    }

    pub fn cart_ram(&self) -> Option<&[u8]> {
        if self.cart_ram.is_empty() {
            None
        } else {
            Some(&self.cart_ram)
        }
    }

    pub fn cart_ram_mut(&mut self) -> Option<&mut [u8]> {
        if self.cart_ram.is_empty() {
            None
        } else {
            Some(&mut self.cart_ram)
        }
    }

    pub fn work_ram(&self) -> &[u8] {
        &self.ram[..PAGE_SIZE]
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram[..PAGE_SIZE]
    }

    pub fn load_cart_ram(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.cart_ram.is_empty() {
            return Err("cart RAM not present");
        }
        if self.cart_ram.len() != data.len() {
            return Err("cart RAM size mismatch");
        }
        self.cart_ram.copy_from_slice(data);
        Ok(())
    }

    fn read_control_register(&mut self, offset: usize) -> Option<u8> {
        match Self::decode_control_register(offset)? {
            ControlRegister::TimerCounter => Some(self.timer.read_counter()),
            ControlRegister::TimerControl => Some(self.timer.control()),
            ControlRegister::IrqMask => Some(self.interrupt_disable),
            ControlRegister::IrqStatus => {
                if let Some(force) = Self::env_irq_status_default() {
                    Some(self.interrupt_request | force)
                } else {
                    Some(self.interrupt_request)
                }
            }
        }
    }

    fn write_control_register(&mut self, offset: usize, value: u8) -> bool {
        match Self::decode_control_register(offset) {
            Some(ControlRegister::TimerCounter) => {
                self.timer.write_reload(value);
                true
            }
            Some(ControlRegister::TimerControl) => {
                self.timer.write_control(value);
                true
            }
            Some(ControlRegister::IrqMask) => {
                let mask = IRQ_DISABLE_IRQ2 | IRQ_DISABLE_IRQ1 | IRQ_DISABLE_TIMER;
                self.interrupt_disable = value & mask;
                true
            }
            Some(ControlRegister::IrqStatus) => {
                // On real HuC6280, writing to $1403 always clears the timer
                // IRQ regardless of the written value (confirmed by Mednafen).
                self.interrupt_request &= !IRQ_REQUEST_TIMER;
                true
            }
            None => false,
        }
    }

    fn decode_control_register(offset: usize) -> Option<ControlRegister> {
        if (HW_TIMER_BASE..=HW_TIMER_BASE + 0x03FF).contains(&offset) {
            match offset & 0x01 {
                0x00 => Some(ControlRegister::TimerCounter),
                0x01 => Some(ControlRegister::TimerControl),
                _ => None,
            }
        } else if (HW_IRQ_BASE..=HW_IRQ_BASE + 0x03FF).contains(&offset) {
            match offset & 0x03 {
                0x02 => Some(ControlRegister::IrqMask),
                0x03 => Some(ControlRegister::IrqStatus),
                _ => None,
            }
        } else if (HW_CPU_CTRL_BASE..=HW_CPU_CTRL_BASE + 0x03FF).contains(&offset) {
            match offset & 0xFF {
                0x10 => Some(ControlRegister::TimerCounter),
                0x11 => Some(ControlRegister::TimerControl),
                0x12 => Some(ControlRegister::IrqMask),
                0x13 => Some(ControlRegister::IrqStatus),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Map a $FF00-$FF7F address to the I/O page offset.
    ///
    fn mpr_index_for_addr(addr: u16) -> Option<usize> {
        if !(0xFF80..=0xFFBF).contains(&addr) {
            return None;
        }
        let offset = (addr - 0xFF80) as usize;
        Some(offset & 0x07)
    }

    fn enqueue_audio_samples(&mut self, phi_cycles: u32) {
        self.audio_phi_accumulator = self
            .audio_phi_accumulator
            .saturating_add(phi_cycles as u64 * AUDIO_SAMPLE_RATE as u64);
        while self.audio_phi_accumulator >= MASTER_CLOCK_HZ as u64 {
            self.audio_phi_accumulator -= MASTER_CLOCK_HZ as u64;
            let sample = self.psg.generate_sample();
            self.audio_buffer.push(sample);
        }
    }



    pub fn irq_pending(&self) -> bool {
        (self.interrupt_request & self.enabled_irq_mask()) != 0
    }

    pub fn pending_interrupts(&self) -> u8 {
        self.interrupt_request & self.enabled_irq_mask()
    }

    pub fn raise_irq(&mut self, mask: u8) {
        self.interrupt_request |= mask;
    }

    pub fn clear_irq(&mut self, mask: u8) {
        self.interrupt_request &= !mask;
    }

    pub fn acknowledge_irq(&mut self, mask: u8) {
        self.clear_irq(mask);
        if mask & IRQ_REQUEST_IRQ2 != 0 {
            self.psg.acknowledge();
        }
    }

    pub fn next_irq(&self) -> Option<u8> {
        let masked = self.pending_interrupts();
        if masked & IRQ_REQUEST_TIMER != 0 {
            return Some(IRQ_REQUEST_TIMER);
        }
        if masked & IRQ_REQUEST_IRQ1 != 0 {
            return Some(IRQ_REQUEST_IRQ1);
        }
        if masked & IRQ_REQUEST_IRQ2 != 0 {
            return Some(IRQ_REQUEST_IRQ2);
        }
        None
    }

    fn resolve(&self, addr: u16) -> (BankMapping, usize) {
        let index = (addr as usize) >> 13;
        let offset = (addr as usize) & (PAGE_SIZE - 1);
        (self.banks[index], offset)
    }

    fn update_mpr(&mut self, bank: usize) {
        let value = self.mpr[bank];
        let rom_pages = self.rom_pages();
        let cart_pages = self.cart_ram_pages();
        let mapping = match value {
            0xFF => BankMapping::Hardware,
            0xF8..=0xFD => {
                let ram_pages = self.total_ram_pages().max(1);
                let logical = (value - 0xF8) as usize % ram_pages;
                BankMapping::Ram {
                    base: logical * PAGE_SIZE,
                }
            }
            _ => {
                let logical = value as usize;
                if cart_pages > 0 && value >= 0x80 {
                    let cart_page = (logical - 0x80) % cart_pages.max(1);
                    BankMapping::CartRam {
                        base: cart_page * PAGE_SIZE,
                    }
                } else if rom_pages > 0 {
                    let rom_page = Self::mirror_rom_bank(logical, rom_pages);
                    BankMapping::Rom {
                        base: rom_page * PAGE_SIZE,
                    }
                } else {
                    BankMapping::Ram { base: 0 }
                }
            }
        };
        let mapping = if bank == 1 && Self::env_force_mpr1_hardware() {
            BankMapping::Hardware
        } else {
            mapping
        };
        self.banks[bank] = mapping;
    }

    fn total_ram_pages(&self) -> usize {
        (self.ram.len() / PAGE_SIZE).max(1)
    }

    fn rom_pages(&self) -> usize {
        self.rom.len() / PAGE_SIZE
    }

    /// Map a logical ROM bank number to a physical ROM page, handling
    /// mirroring for non-power-of-2 ROM sizes.
    ///
    /// For power-of-2 ROMs, simple modulo works.  For non-power-of-2 (e.g.
    /// 384 KB = 48 banks), the PC Engine hardware splits the 128-bank
    /// address space in half:
    ///   Banks  0-63  → lower power-of-2 portion (e.g. 32 banks = 256 KB)
    ///   Banks 64-127 → upper remainder        (e.g. 16 banks = 128 KB)
    /// Each half mirrors within its own range.  Confirmed by Mednafen's
    /// pce/huc.cpp for the m_len == 0x60000 case.
    fn mirror_rom_bank(logical: usize, rom_pages: usize) -> usize {
        if rom_pages == 0 {
            return 0;
        }
        if rom_pages.is_power_of_two() {
            return logical % rom_pages;
        }
        // Largest power-of-2 that fits inside rom_pages.
        let lower = rom_pages.next_power_of_two() >> 1; // e.g. 32 for 48, 64 for 96
        let upper = rom_pages - lower; // e.g. 16 for 48, 32 for 96

        // Mask to 7-bit bank number (128 banks = 1 MB address space).
        // The 128-bank space is always split at the midpoint (bank 64):
        //   Banks  0-63  → lower portion (mirrored within `lower` pages)
        //   Banks 64-127 → upper portion (mirrored within `upper` pages)
        let bank = logical & 0x7F;
        if bank < 64 {
            bank % lower.max(1)
        } else {
            ((bank - 64) % upper.max(1)) + lower
        }
    }

    fn cart_ram_pages(&self) -> usize {
        self.cart_ram.len() / PAGE_SIZE
    }

    fn vdc_port_kind(offset: usize) -> Option<VdcPort> {
        // VDC is mirrored over the 0x0000–0x03FF IO window. Only A1..A0 select
        // control/data; A2+ are ignored by the chip. Many HuCARDs stream writes
        // via 0x2002/0x2003/0x200A/0x200B, so ensure any offset whose low two
        // bits are 0/1 goes to Control, 2/3 goes to Data.
        // For debug `PCE_VDC_ULTRA_MIRROR`, widen to the entire hardware page.
        if Bus::env_vdc_force_hot_ports() && Self::force_map_candidates(offset) {
            return Some(Self::vdc_port_from_low_bits(offset));
        }
        let mirrored = offset & 0x1FFF;
        let ultra = Self::env_vdc_ultra_mirror();
        let catchall = Self::env_vdc_catchall();
        if Self::env_vdc_force_hot_ports() && Self::force_map_candidates(offset) {
            return Some(Self::vdc_port_from_low_bits(offset));
        }
        if !catchall {
            if !Self::env_extreme_mirror() && !ultra && mirrored >= 0x0400 {
                return None;
            }
            if Self::env_extreme_mirror() && !ultra && mirrored >= 0x1000 {
                return None;
            }
            if ultra && mirrored >= 0x2000 {
                return None;
            }
        }
        match mirrored & 0x03 {
            0x00 | 0x01 => Some(VdcPort::Control),
            0x02 | 0x03 => Some(VdcPort::Data),
            _ => None,
        }
    }

    #[inline]
    fn vdc_port_from_low_bits(offset: usize) -> VdcPort {
        if offset & 0x02 != 0 {
            VdcPort::Data
        } else {
            VdcPort::Control
        }
    }

    fn force_map_candidates(offset: usize) -> bool {
        // Small list of hot addresses observed in HuCARD traces (0x2200/2211,
        // 0x2002/200A, 0x2017..0x201D, 0x0800..) that may mirror VDC ports.
        const HOT: &[usize] = &[
            0x0000, 0x0002, 0x0003, 0x0800, 0x0802, 0x0803, 0x0804, 0x0805, 0x0807, 0x2000, 0x2001,
            0x2002, 0x2003, 0x200A, 0x200B, 0x2010, 0x2011, 0x2012, 0x2016, 0x2017, 0x2018, 0x2019,
            0x201A, 0x201B, 0x201C, 0x201D, 0x2048, 0x2049, 0x204A, 0x204B, 0x204D, 0x2200, 0x2201,
            0x2202, 0x2209, 0x220A, 0x220B, 0x220C, 0x220D, 0x220F, 0x2210, 0x2211, 0x2212, 0x2215,
            0x2217, 0x2219, 0x221A, 0x221D, 0x2220, 0x2226, 0x2227, 0x2228, 0x2229, 0x222A, 0x222B,
            0x222D, 0x222E, 0x0A3A, 0x0A3B, 0x0A3C, 0x0A3D,
        ];
        HOT.iter().any(|&h| (offset & 0x3FFF) == h)
    }

    #[cfg(feature = "trace_hw_writes")]
    fn st0_hold_enabled() -> bool {
        use std::sync::OnceLock;
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("PCE_TRACE_DISABLE_ST0_HOLD").is_err())
    }


    fn normalized_io_offset(offset: usize) -> usize {
        // Optional: fold 0x0200–0x03FF down to 0x0000–0x01FF when debugging
        // HuCARDs that stream hardware writes through the wider mirror region.
        if Self::env_fold_io_02xx() && offset >= 0x0200 && offset < 0x0400 {
            offset & 0x01FF
        } else {
            offset
        }
    }

    fn read_io_internal(&mut self, raw_offset: usize) -> u8 {
        // The HuC6280 only decodes A0–A10 for the hardware page; fold everything
        // into 0x0000–0x1FFF first, then optional 0x0200 folding for debug.
        let mut offset = raw_offset & 0x1FFF;
        offset = Self::normalized_io_offset(offset);
        if Self::env_route_02xx_hw() && offset >= 0x0200 && offset < 0x0220 {
            offset &= 0x01FF; // map 0x0200–0x021F to 0x0000–0x001F
        }
        if let Some(port) = Self::vdc_port_kind(offset) {
            #[cfg(feature = "trace_hw_writes")]
            {
                self.vdc.last_io_addr = offset as u16;
            }
            return match port {
                VdcPort::Control => self.vdc.read_status(),
                VdcPort::Data => {
                    let port_index = if offset & 0x01 != 0 { 2 } else { 1 };
                    self.vdc.read_port(port_index)
                }
            };
        }
        match offset {
            0x0400..=0x07FF | 0x1C40..=0x1C47 => {
                let sub = (offset & 0x0007) as u16;
                self.read_vce_port(sub)
            }
            // HuC6280 PSG native map is direct registers at $0800-$080F.
            0x0800..=0x0BFF => self.psg.read_direct(offset & 0x0F),
            // Keep legacy 4-port mirror behavior for older tests/tooling.
            0x1C60..=0x1C63 => match offset & 0x03 {
                0x00 => self.psg.read_address(),
                0x01 => self.io[offset],
                0x02 => self.psg.read_data(),
                _ => self.psg.read_status(),
            },
            0x0C00..=0x0FFF => {
                if let Some(value) = self.read_control_register(offset) {
                    value
                } else {
                    self.io[offset]
                }
            }
            0x1000..=0x13FF => {
                if let Some(value) = self.io_port.read(offset - HW_JOYPAD_BASE) {
                    value
                } else {
                    self.io[offset]
                }
            }
            0x1400..=0x17FF | 0x1C10..=0x1C13 => {
                if let Some(value) = self.read_control_register(offset) {
                    value
                } else {
                    self.io[offset]
                }
            }
            0x1C00..=0x1FFF => {
                if let Some(value) = self.read_control_register(offset) {
                    value
                } else {
                    self.io[offset]
                }
            }
            _ => self.io[offset],
        }
    }

    #[inline]
    pub fn stack_read(&self, addr: u16) -> u8 {
        let index = addr as usize;
        self.ram.get(index).copied().unwrap_or(0)
    }

    #[inline]
    pub fn stack_write(&mut self, addr: u16, value: u8) {
        let index = addr as usize;
        if let Some(slot) = self.ram.get_mut(index) {
            *slot = value;
        }
    }

    #[inline]
    pub fn read_zero_page(&self, addr: u8) -> u8 {
        self.ram.get(addr as usize).copied().unwrap_or(0)
    }

    #[inline]
    pub fn write_zero_page(&mut self, addr: u8, value: u8) {
        if let Some(slot) = self.ram.get_mut(addr as usize) {
            #[cfg(feature = "trace_hw_writes")]
            if (0x20..=0x23).contains(&addr) {
                eprintln!("  ZP[{addr:02X}] (zp) <= {value:02X}");
            }
            *slot = value;
        }
    }

    fn write_io_internal(&mut self, raw_offset: usize, value: u8) {
        // Fold to 0x0000–0x1FFF to mirror HuC6280 hardware page decode.
        let mut offset = raw_offset & 0x1FFF;
        offset = Self::normalized_io_offset(offset);
        if Self::env_route_02xx_hw() && offset >= 0x0200 && offset < 0x0220 {
            offset &= 0x01FF; // map 0x0200–0x021F to 0x0000–0x001F
        }
        if let Some(port) = Self::vdc_port_kind(offset) {
            #[cfg(feature = "trace_hw_writes")]
            {
                self.vdc.last_io_addr = offset as u16;
            }
            match port {
                VdcPort::Control => self.write_st_port(0, value),
                VdcPort::Data => {
                    let port_index = if offset & 0x01 != 0 { 2 } else { 1 };
                    self.write_st_port(port_index, value)
                }
            }
            return;
        }
        #[cfg(feature = "trace_hw_writes")]
        if (offset & 0x1FFF) >= 0x2400 && (offset & 0x1FFF) < 0x2800 {
            eprintln!(
                "  IO write HIGH mirror offset {:04X} -> {:02X}",
                offset, value
            );
        }
        #[cfg(feature = "trace_hw_writes")]
        if (offset & 0xE000) == 0 && value != 0 {
            eprintln!("  HW page data write {:04X} -> {:02X}", offset, value);
        }
        match offset {
            // VCE mirrors also appear at 0x1C40–0x1C43 in some docs; treat them the same.
            0x0400..=0x07FF | 0x1C40..=0x1C47 => {
                let sub = (offset & 0x0007) as u16;
                self.write_vce_port(sub, value);
            }
            // HuC6280 PSG native map is direct registers at $0800-$080F.
            0x0800..=0x0BFF => self.psg.write_direct(offset & 0x0F, value),
            // Keep legacy 4-port mirror behavior for older tests/tooling.
            0x1C60..=0x1C63 => match offset & 0x03 {
                0x00 => self.psg.write_address(value),
                0x01 => self.psg.write_data(value),
                _ => self.io[offset] = value,
            },
            0x0C00..=0x0FFF | 0x1400..=0x17FF | 0x1C10..=0x1C13 => {
                // Timer/IRQ registers (mirrored)
                if !self.write_control_register(offset, value) {
                    self.io[offset] = value;
                }
            }
            0x1000..=0x13FF => {
                if !self.io_port.write(offset - HW_JOYPAD_BASE, value) {
                    self.io[offset] = value;
                }
            }
            0x1C00..=0x1FFF => {
                // Treat as additional mirror for control/TIMER/IRQ/PSG status
                if (offset & 0x3F) >= 0x40 && (offset & 0x3F) <= 0x43 {
                    // Mirror of VCE control area? leave as IO
                    self.io[offset] = value;
                } else if !self.write_control_register(offset, value) {
                    self.io[offset] = value;
                }
            }
            _ => {
                self.io[offset] = value;
            }
        }
    }

    #[cfg(feature = "trace_hw_writes")]
    fn cpu_pc_for_trace(&self) -> u16 {
        self.last_pc_for_trace.unwrap_or(0)
    }

    #[inline]
    fn read_vce_port(&mut self, addr: u16) -> u8 {
        match addr & 0x0007 {
            0x00 => self.vce.read_control_low(),
            0x01 => self.vce.read_control_high(),
            0x02 => self.vce.read_address_low(),
            0x03 => self.vce.read_address_high(),
            0x04 => self.vce.read_data_low(),
            0x05 => self.vce.read_data_high(),
            _ => 0xFF,
        }
    }

    #[inline]
    fn write_vce_port(&mut self, addr: u16, value: u8) {
        match addr & 0x0007 {
            0x00 => self.vce.write_control_low(value),
            0x01 => self.vce.write_control_high(value),
            0x02 => self.vce.write_address_low(value),
            0x03 => self.vce.write_address_high(value),
            0x04 => self.vce.write_data_low(value),
            0x05 => self.vce.write_data_high(value),
            _ => {}
        }
    }

    fn refresh_vdc_irq(&mut self) {
        // Force DS/DV after many hardware writes (debug aid) or when env is set.
        #[cfg(debug_assertions)]
        {
            const FORCE_AFTER_WRITES: u64 = 5_000;
            if self.debug_force_ds_after >= FORCE_AFTER_WRITES {
                self.vdc.raise_status(VDC_STATUS_DS | VDC_STATUS_DV);
            }
        }
        if Self::env_force_vdc_dsdv() {
            self.vdc.raise_status(VDC_STATUS_DS | VDC_STATUS_DV);
        }
        // Debug: optionally force IRQ1 every refresh to unblock BIOS waits.
        if Self::env_force_irq1() {
            self.interrupt_request |= IRQ_REQUEST_IRQ1;
        }
        // Debug: optionally force IRQ2 (timer/PSG line) as well.
        if Self::env_force_irq2() {
            self.interrupt_request |= IRQ_REQUEST_IRQ2;
        }
        if self.vdc.irq_active() {
            self.interrupt_request |= IRQ_REQUEST_IRQ1;
        } else {
            self.interrupt_request &= !IRQ_REQUEST_IRQ1;
        }
    }

    fn perform_cram_dma(&mut self) {
        let raw_length = self.vdc.registers[0x12];
        let mut words = raw_length as usize;
        if words == 0 {
            words = 0x200; // CRAMは最大512ワード
        }
        words = words.min(0x200);

        let mut src = self.vdc.marr & 0x7FFF;
        let mut index = self.vce.address_index();

        for _ in 0..words {
            let word = *self.vdc.vram.get(src as usize).unwrap_or(&0);
            if let Some(slot) = self.vce.palette.get_mut(index) {
                *slot = word;
            }
            index = (index + 1) & 0x01FF;
            src = Vdc::advance_vram_addr(src, false);
        }

        self.vdc.marr = src & 0x7FFF;
        self.vdc.registers[0x01] = self.vdc.marr;
        self.vce.set_address(index as u16);
        let busy_cycles = (words as u32).saturating_mul(VDC_DMA_WORD_CYCLES);
        self.vdc.set_busy(busy_cycles);
        self.vdc.raise_status(VDC_STATUS_DV);
        self.vdc.cram_pending = false;
    }

    fn perform_vram_dma(&mut self) {
        #[cfg(any(debug_assertions, feature = "trace_hw_writes"))]
        eprintln!(
            "  VDC VRAM DMA start ctrl={:04X} src={:04X} dst={:04X} len={:04X}",
            self.vdc.dma_control,
            self.vdc.dma_source,
            self.vdc.dma_destination,
            self.vdc.registers[0x12]
        );
        let original_len = self.vdc.registers[0x12];
        let words = original_len as u32 + 1;

        let src_dec = self.vdc.dma_control & DMA_CTRL_SRC_DEC != 0;
        let dst_dec = self.vdc.dma_control & DMA_CTRL_DST_DEC != 0;

        let mut src = self.vdc.dma_source & 0x7FFF;
        let mut dst = self.vdc.dma_destination & 0x7FFF;

        for _ in 0..words {
            let value = self.vdc.vram[(src as usize) & 0x7FFF];
            self.vdc.write_vram_dma_word(dst, value);

            src = Vdc::advance_vram_addr(src, src_dec);
            dst = Vdc::advance_vram_addr(dst, dst_dec);
        }

        self.vdc.dma_source = src;
        self.vdc.dma_destination = dst;
        self.vdc.registers[0x10] = self.vdc.dma_source;
        self.vdc.registers[0x11] = self.vdc.dma_destination;
        self.vdc.registers[0x12] = 0xFFFF;

        #[cfg(any(debug_assertions, feature = "trace_hw_writes"))]
        eprintln!(
            "  VDC VRAM DMA end src={:04X} dst={:04X} len={:04X}",
            self.vdc.dma_source, self.vdc.dma_destination, original_len
        );

        let busy_cycles = words.saturating_mul(VDC_DMA_WORD_CYCLES);
        self.vdc.set_busy(busy_cycles);
        self.vdc.raise_status(VDC_STATUS_DV);

        // デバッグ用: VRAM DMA 完了時に VRAM 先頭から CRAM 512 ワードを強制ロード。
        if Self::env_force_cram_from_vram() {
            for i in 0..0x200 {
                let word = self.vdc.vram.get(i).copied().unwrap_or(0);
                if let Some(slot) = self.vce.palette.get_mut(i) {
                    *slot = word;
                }
            }
            #[cfg(any(debug_assertions, feature = "trace_hw_writes"))]
            eprintln!("  DEBUG PCE_FORCE_CRAM_FROM_VRAM applied (first 512 words)");
        }
    }

    fn enabled_irq_mask(&self) -> u8 {
        let mut mask = 0;
        if self.interrupt_disable & IRQ_DISABLE_IRQ2 == 0 {
            mask |= IRQ_REQUEST_IRQ2;
        }
        if self.interrupt_disable & IRQ_DISABLE_IRQ1 == 0 {
            mask |= IRQ_REQUEST_IRQ1;
        }
        if self.interrupt_disable & IRQ_DISABLE_TIMER == 0 {
            mask |= IRQ_REQUEST_TIMER;
        }
        mask
    }
}


#[derive(Clone, Copy, Debug, bincode::Encode, bincode::Decode)]
enum BankMapping {
    Ram { base: usize },
    Rom { base: usize },
    CartRam { base: usize },
    Hardware,
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
enum ControlRegister {
    TimerCounter,
    TimerControl,
    IrqMask,
    IrqStatus,
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
struct IoPort {
    output: u8,
    direction: u8,
    enable: u8,
    select: u8,
    input: u8,
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
struct Timer {
    reload: u8,
    counter: u8,
    prescaler: u32,
    enabled: bool,
}


// PSG constants and types are defined in src/psg.rs
// Psg is defined in src/psg.rs

// Vce is defined in src/vce.rs

impl Timer {
    fn new() -> Self {
        Self {
            reload: 0,
            counter: 0,
            prescaler: 0,
            enabled: false,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn write_reload(&mut self, value: u8) {
        self.reload = value & 0x7F;
    }

    fn read_counter(&self) -> u8 {
        self.counter & 0x7F
    }

    fn write_control(&mut self, value: u8) {
        let start = value & TIMER_CONTROL_START != 0;
        if start && !self.enabled {
            self.enabled = true;
            self.counter = self.reload;
            self.prescaler = 0;
        } else if !start {
            self.enabled = false;
        }
    }

    fn control(&self) -> u8 {
        if self.enabled { TIMER_CONTROL_START } else { 0 }
    }

    fn tick(&mut self, cycles: u32, high_speed: bool) -> bool {
        if !self.enabled {
            return false;
        }

        let divider = if high_speed { 1024 } else { 256 };
        self.prescaler += cycles;
        let mut fired = false;

        while self.prescaler >= divider as u32 {
            self.prescaler -= divider as u32;
            if self.counter == 0 {
                self.counter = self.reload;
                fired = true;
            } else {
                self.counter = self.counter.wrapping_sub(1) & 0x7F;
            }
        }

        fired
    }
}

impl IoPort {
    fn new() -> Self {
        Self {
            output: 0,
            direction: 0,
            enable: 0,
            select: 0,
            input: 0xFF,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn read(&self, offset: usize) -> Option<u8> {
        match offset & 0x03FF {
            0x0000 => Some(self.read_joypad_data()),
            0x0002 => Some(self.direction),
            0x0004 => Some(self.input),
            0x0005 => Some(self.enable),
            0x0006 => Some(self.select),
            _ => None,
        }
    }

    fn write(&mut self, offset: usize, value: u8) -> bool {
        match offset & 0x03FF {
            0x0000 => {
                self.output = value;
                // CLR low resets the 6-pad scan index on hardware.
                if value & 0x02 == 0 {
                    self.select = 0;
                }
                true
            }
            0x0002 => {
                self.direction = value;
                true
            }
            0x0004 => {
                self.input = value;
                true
            }
            0x0005 => {
                self.enable = value;
                true
            }
            0x0006 => {
                self.select = value;
                true
            }
            _ => false,
        }
    }

    fn read_joypad_data(&self) -> u8 {
        // PC Engine joypad reads one nibble at a time.
        // SEL=1 -> d-pad nibble (lower 4 bits of input)
        // SEL=0 -> button nibble (upper 4 bits of input)
        let sel = (self.output & 0x01) != 0;
        let nibble = if sel {
            self.input & 0x0F // d-pad: Up(0) Right(1) Down(2) Left(3)
        } else {
            (self.input >> 4) & 0x0F // buttons: I(0) II(1) Sel(2) Run(3)
        };
        0xF0 | nibble
    }
}


#[cfg(test)]
mod tests;
