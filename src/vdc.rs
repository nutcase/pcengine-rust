// VDC (HuC6270) — Video Display Controller
//
// Extracted from bus.rs.  The Bus still owns rendering and I/O dispatch;
// this module holds the VDC state machine, register file, and timing.

pub(crate) const VDC_REGISTER_COUNT: usize = 32;
pub(crate) const LINES_PER_FRAME: u16 = 263;
pub(crate) const FRAME_WIDTH: usize = 512; // internal stride (max 10 MHz width)
pub(crate) const FRAME_HEIGHT: usize = 240;
pub(crate) const VDC_BUSY_ACCESS_CYCLES: u32 = 64;
pub(crate) const VDC_DMA_WORD_CYCLES: u32 = 8;
pub(crate) const VDC_VBLANK_INTERVAL: u32 = 119_318; // ~7.16 MHz / 60 Hz

pub(crate) const TILE_WIDTH: usize = 8;
pub(crate) const TILE_HEIGHT: usize = 8;
pub(crate) const SPRITE_PATTERN_WIDTH: usize = 16;
pub(crate) const SPRITE_PATTERN_HEIGHT: usize = 16;
pub(crate) const SPRITE_PATTERN_WORDS: usize = 64;
pub(crate) const SPRITE_COUNT: usize = 64;
pub(crate) const VDC_CTRL_ENABLE_SPRITES_LEGACY: u16 = 0x0040;
pub(crate) const VDC_CTRL_ENABLE_BACKGROUND_LEGACY: u16 = 0x0080;
pub(crate) const VDC_CTRL_ENABLE_BACKGROUND: u16 = 0x0100;
pub(crate) const VDC_CTRL_ENABLE_SPRITES: u16 = 0x0200;

pub const VDC_STATUS_CR: u8 = 0x01;
pub const VDC_STATUS_OR: u8 = 0x02;
pub const VDC_STATUS_RCR: u8 = 0x04;
pub const VDC_STATUS_DS: u8 = 0x08;
pub const VDC_STATUS_DV: u8 = 0x10;
pub const VDC_STATUS_VBL: u8 = 0x20;
pub const VDC_STATUS_BUSY: u8 = 0x40;
const VDC_ACTIVE_COUNTER_BASE: usize = 0x40;
pub(crate) const DMA_CTRL_IRQ_SATB: u16 = 0x0001;
pub(crate) const DMA_CTRL_IRQ_VRAM: u16 = 0x0002;
pub(crate) const DMA_CTRL_SRC_DEC: u16 = 0x0004;
pub(crate) const DMA_CTRL_DST_DEC: u16 = 0x0008;
pub(crate) const DMA_CTRL_SATB_AUTO: u16 = 0x0010;
pub(crate) const VDC_VISIBLE_LINES: u16 = 240;
const VDC_MAX_VBLANK_START_LINE: usize = (LINES_PER_FRAME as usize) - 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug, bincode::Encode, bincode::Decode)]
pub(crate) enum VdcWritePhase {
    Low,
    High,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, bincode::Encode, bincode::Decode)]
pub(crate) enum VdcReadPhase {
    Low,
    High,
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub(crate) struct VerticalWindow {
    pub(crate) timing_programmed: bool,
    pub(crate) active_start_line: usize,
    pub(crate) active_line_count: usize,
    pub(crate) post_active_overscan_lines: usize,
    pub(crate) vblank_start_line: usize,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub(crate) struct Vdc {
    pub(crate) registers: [u16; VDC_REGISTER_COUNT],
    /// Per-register data latch (MAME's m_vdc_data).  ST1 writes the low byte
    /// of `vdc_data[AR]`; ST2 writes the high byte and commits the full word.
    /// Unlike `registers[]`, this is NOT overwritten by VRAM read/write side
    /// effects, so interleaved writes to different registers work correctly.
    pub(crate) vdc_data: [u16; VDC_REGISTER_COUNT],
    pub(crate) vram: Vec<u16>,
    pub(crate) satb: [u16; 0x100],
    pub(crate) selected: u8,
    pub(crate) latch_low: u8,
    pub(crate) write_phase: VdcWritePhase,
    pub(crate) read_phase: VdcReadPhase,
    pub(crate) read_buffer: u16,
    pub(crate) mawr: u16,
    pub(crate) marr: u16,
    pub(crate) status: u8,
    pub(crate) phi_scaled: u64,
    pub(crate) busy_cycles: u32,
    pub(crate) scanline: u16,
    pub(crate) dma_control: u16,
    pub(crate) dma_source: u16,
    pub(crate) dma_destination: u16,
    pub(crate) satb_source: u16,
    pub(crate) satb_pending: bool,
    pub(crate) satb_written: bool,
    pub(crate) in_vblank: bool,
    pub(crate) frame_trigger: bool,
    pub(crate) scroll_x: u16,
    pub(crate) scroll_y: u16,
    pub(crate) scroll_x_pending: u16,
    pub(crate) scroll_y_pending: u16,
    pub(crate) scroll_x_dirty: bool,
    pub(crate) scroll_y_dirty: bool,
    /// Number of active display lines since BYR was last loaded/written.
    /// Reset to 0 at the first active scanline of each frame and on BYR writes.
    pub(crate) bg_y_offset: u16,
    pub(crate) bg_y_offset_loaded: bool,
    pub(crate) zoom_x: u16,
    pub(crate) zoom_y: u16,
    pub(crate) zoom_x_pending: u16,
    pub(crate) zoom_y_pending: u16,
    pub(crate) zoom_x_dirty: bool,
    pub(crate) zoom_y_dirty: bool,
    pub(crate) scroll_line_x: [u16; LINES_PER_FRAME as usize],
    pub(crate) scroll_line_y: [u16; LINES_PER_FRAME as usize],
    /// Per-line BG Y offset (active lines since BYR was last set).
    pub(crate) scroll_line_y_offset: [u16; LINES_PER_FRAME as usize],
    pub(crate) zoom_line_x: [u16; LINES_PER_FRAME as usize],
    pub(crate) zoom_line_y: [u16; LINES_PER_FRAME as usize],
    pub(crate) control_line: [u16; LINES_PER_FRAME as usize],
    pub(crate) scroll_line_valid: [bool; LINES_PER_FRAME as usize],
    pub(crate) vram_dma_request: bool,
    pub(crate) cram_pending: bool,
    pub(crate) cram_dma_count: u64,
    pub(crate) control_write_count: u64,
    pub(crate) last_control_value: u16,
    pub(crate) render_control_latch: u16,
    pub(crate) last_cram_source: u16,
    pub(crate) last_cram_length: u16,
    pub(crate) vram_dma_count: u64,
    pub(crate) last_vram_dma_source: u16,
    pub(crate) last_vram_dma_destination: u16,
    pub(crate) last_vram_dma_length: u16,
    pub(crate) dcr_write_count: u64,
    pub(crate) last_dcr_value: u8,
    pub(crate) register_write_counts: [u64; VDC_REGISTER_COUNT],
    pub(crate) register_select_counts: [u64; VDC_REGISTER_COUNT],
    pub(crate) r05_low_writes: u64,
    pub(crate) r05_high_writes: u64,
    pub(crate) last_r05_low: u8,
    pub(crate) r05_low_value_counts: [u64; 0x100],
    pub(crate) r05_high_value_counts: [u64; 0x100],
    pub(crate) vram_data_low_writes: u64,
    pub(crate) vram_data_high_writes: u64,
    pub(crate) vram_data_high_without_low: u64,
    /// Track writes to a specific VRAM address range (for debugging font loading).
    pub(crate) vram_write_range_count: u64,
    pub(crate) vram_write_range_start: u16,
    pub(crate) vram_write_range_end: u16,
    /// Debug: log MAWR register set operations in a specified range.
    pub(crate) mawr_log: Vec<u16>,
    pub(crate) mawr_log_start: u16,
    pub(crate) mawr_log_end: u16,
    /// Debug: log the first N VRAM writes (address, value).
    pub(crate) vram_write_log: Vec<(u16, u16)>,
    pub(crate) vram_write_log_limit: usize,
    /// Debug: log BYR/RCR/CR writes with scanline. (scanline, reg_index, value)
    pub(crate) reg_trace: Vec<(u16, u8, u16)>,
    pub(crate) reg_trace_enabled: bool,
    /// BIOS font tile patterns: 96 tiles (ASCII 0x20-0x7F), 16 VRAM words each.
    /// Stored separately so they can be restored when game graphics overwrites them.
    pub(crate) bios_font_tiles: Vec<[u16; 16]>,
    /// Set when VRAM writes hit the font tile area, signalling that font tiles
    /// may need restoration before the next render.
    pub(crate) bios_font_dirty: bool,
    pub(crate) ignore_next_high_byte: bool,
    // Remember which register a low byte targeted so the paired high byte
    // commits to the same register even if ST0 is touched in between.
    pub(crate) pending_write_register: Option<u8>,
    #[cfg(feature = "trace_hw_writes")]
    pub(crate) pending_traced_register: Option<u8>,
    #[cfg(feature = "trace_hw_writes")]
    pub(crate) last_io_addr: u16,
    #[cfg(feature = "trace_hw_writes")]
    pub(crate) st0_hold_counter: u8,
    #[cfg(feature = "trace_hw_writes")]
    pub(crate) st0_hold_addr_hist: [u32; 0x100],
    pub(crate) st0_locked_until_commit: bool,
    /// Scanline that needs post-ISR scroll adjustment.  Set when an RCR
    /// interrupt fires; consumed on re-entry to apply the h-sync bg_y_offset
    /// increment so the next scanline renders at BYR+1.
    pub(crate) rcr_post_isr_line: Option<u16>,
}

/// Cached env-var flag: returns `true` when the env var is set (`.is_ok()`).
macro_rules! env_bool {
    ($name:ident, $var:expr) => {
        #[inline]
        fn $name() -> bool {
            use std::sync::OnceLock;
            static V: OnceLock<bool> = OnceLock::new();
            *V.get_or_init(|| std::env::var($var).is_ok())
        }
    };
}

/// Cached env-var parsed as `u32` with a non-zero filter and default.
macro_rules! env_u32 {
    ($name:ident, $var:expr, $default:expr) => {
        #[inline]
        fn $name() -> u32 {
            use std::sync::OnceLock;
            static V: OnceLock<u32> = OnceLock::new();
            *V.get_or_init(|| {
                std::env::var($var)
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|&n| n > 0)
                    .unwrap_or($default)
            })
        }
    };
}

impl Vdc {
    env_bool!(env_hold_dsdv, "PCE_HOLD_DSDV");
    env_u32!(env_vdc_busy_divisor, "PCE_VDC_BUSY_DIV", 1);

    pub(crate) fn new() -> Self {
        let mut vdc = Self {
            registers: [0; VDC_REGISTER_COUNT],
            vdc_data: [0; VDC_REGISTER_COUNT],
            vram: vec![0; 0x8000],
            satb: [0; 0x100],
            selected: 0,
            latch_low: 0,
            write_phase: VdcWritePhase::Low,
            read_phase: VdcReadPhase::Low,
            read_buffer: 0,
            mawr: 0,
            marr: 0,
            status: VDC_STATUS_VBL | VDC_STATUS_DS, // start inside VBlank with SATB DMA idle
            phi_scaled: 0,
            busy_cycles: 0,
            scanline: LINES_PER_FRAME - 1,
            dma_control: 0,
            dma_source: 0,
            dma_destination: 0,
            satb_source: 0,
            satb_pending: false,
            satb_written: false,
            in_vblank: true,
            frame_trigger: false,
            scroll_x: 0,
            scroll_y: 0,
            scroll_x_pending: 0,
            scroll_y_pending: 0,
            scroll_x_dirty: false,
            scroll_y_dirty: false,
            bg_y_offset: 0,
            bg_y_offset_loaded: false,
            zoom_x: 0x0010,
            zoom_y: 0x0010,
            zoom_x_pending: 0x0010,
            zoom_y_pending: 0x0010,
            zoom_x_dirty: false,
            zoom_y_dirty: false,
            scroll_line_x: [0; LINES_PER_FRAME as usize],
            scroll_line_y: [0; LINES_PER_FRAME as usize],
            scroll_line_y_offset: [0; LINES_PER_FRAME as usize],
            zoom_line_x: [0; LINES_PER_FRAME as usize],
            zoom_line_y: [0; LINES_PER_FRAME as usize],
            control_line: [0; LINES_PER_FRAME as usize],
            scroll_line_valid: [false; LINES_PER_FRAME as usize],
            vram_dma_request: false,
            cram_pending: false,
            cram_dma_count: 0,
            control_write_count: 0,
            last_control_value: 0,
            render_control_latch: 0,
            last_cram_source: 0,
            last_cram_length: 0,
            vram_dma_count: 0,
            last_vram_dma_source: 0,
            last_vram_dma_destination: 0,
            last_vram_dma_length: 0,
            dcr_write_count: 0,
            last_dcr_value: 0,
            register_write_counts: [0; VDC_REGISTER_COUNT],
            register_select_counts: [0; VDC_REGISTER_COUNT],
            r05_low_writes: 0,
            r05_high_writes: 0,
            last_r05_low: 0,
            r05_low_value_counts: [0; 0x100],
            r05_high_value_counts: [0; 0x100],
            vram_data_low_writes: 0,
            vram_data_high_writes: 0,
            vram_data_high_without_low: 0,
            vram_write_range_count: 0,
            vram_write_range_start: 0,
            vram_write_range_end: 0,
            mawr_log: Vec::new(),
            mawr_log_start: 0,
            mawr_log_end: 0,
            vram_write_log: Vec::new(),
            vram_write_log_limit: 0,
            reg_trace: Vec::new(),
            reg_trace_enabled: false,
            bios_font_tiles: Vec::new(),
            bios_font_dirty: false,
            ignore_next_high_byte: false,
            pending_write_register: None,
            #[cfg(feature = "trace_hw_writes")]
            pending_traced_register: None,
            #[cfg(feature = "trace_hw_writes")]
            last_io_addr: 0,
            #[cfg(feature = "trace_hw_writes")]
            st0_hold_counter: 0,
            #[cfg(feature = "trace_hw_writes")]
            st0_hold_addr_hist: [0; 0x100],
            st0_locked_until_commit: false,
            rcr_post_isr_line: None,
        };
        vdc.registers[0x04] = VDC_CTRL_ENABLE_BACKGROUND_LEGACY | VDC_CTRL_ENABLE_SPRITES_LEGACY;
        vdc.registers[0x05] = vdc.registers[0x04];
        vdc.last_control_value = vdc.registers[0x04];
        vdc.render_control_latch = vdc.registers[0x04];
        vdc.registers[0x09] = 0x0010; // default to 64x32 virtual map
        vdc.registers[0x0A] = 0x0010;
        vdc.registers[0x0B] = 0x0010;
        vdc.refresh_activity_flags();
        // Debug: optionally force status bits at power-on to unblock BIOS waits.
        if let Some(mask) = std::env::var("PCE_FORCE_VDC_STATUS")
            .ok()
            .and_then(|s| u8::from_str_radix(&s, 16).ok())
        {
            vdc.status |= mask;
        }
        // 初期化直後は BUSY を確実に落としておく（リセット直後の BIOS 待ちループ対策）
        vdc.status &= !VDC_STATUS_BUSY;
        vdc
    }

    pub(crate) fn reset(&mut self) {
        self.registers.fill(0);
        self.vdc_data.fill(0);
        self.vram.fill(0);
        self.satb.fill(0);
        self.selected = 0;
        self.latch_low = 0;
        self.write_phase = VdcWritePhase::Low;
        self.read_phase = VdcReadPhase::Low;
        self.read_buffer = 0;
        self.mawr = 0;
        self.marr = 0;
        self.status = VDC_STATUS_VBL | VDC_STATUS_DS;
        self.phi_scaled = 0;
        self.busy_cycles = 0;
        self.scanline = LINES_PER_FRAME - 1;
        self.dma_control = 0;
        self.dma_source = 0;
        self.dma_destination = 0;
        self.satb_source = 0;
        self.satb_pending = false;
        self.satb_written = false;
        self.in_vblank = true;
        self.frame_trigger = false;
        self.registers[0x09] = 0x0010;
        self.refresh_activity_flags();
        self.status &= !VDC_STATUS_BUSY;
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.scroll_x_pending = 0;
        self.scroll_y_pending = 0;
        self.scroll_x_dirty = false;
        self.scroll_y_dirty = false;
        self.bg_y_offset = 0;
        self.bg_y_offset_loaded = false;
        self.zoom_x = 0x0010;
        self.zoom_y = 0x0010;
        self.zoom_x_pending = 0x0010;
        self.zoom_y_pending = 0x0010;
        self.zoom_x_dirty = false;
        self.zoom_y_dirty = false;
        self.scroll_line_x = [0; LINES_PER_FRAME as usize];
        self.scroll_line_y = [0; LINES_PER_FRAME as usize];
        self.zoom_line_x = [0; LINES_PER_FRAME as usize];
        self.zoom_line_y = [0; LINES_PER_FRAME as usize];
        self.control_line = [0; LINES_PER_FRAME as usize];
        self.scroll_line_valid = [false; LINES_PER_FRAME as usize];
        self.vram_dma_request = false;
        self.cram_pending = false;
        self.cram_dma_count = 0;
        self.control_write_count = 0;
        self.registers[0x04] = VDC_CTRL_ENABLE_BACKGROUND_LEGACY | VDC_CTRL_ENABLE_SPRITES_LEGACY;
        self.registers[0x05] = self.registers[0x04];
        self.last_control_value = self.registers[0x04];
        self.render_control_latch = self.registers[0x04];
        self.last_cram_source = 0;
        self.last_cram_length = 0;
        self.vram_dma_count = 0;
        self.last_vram_dma_source = 0;
        self.last_vram_dma_destination = 0;
        self.last_vram_dma_length = 0;
        self.dcr_write_count = 0;
        self.last_dcr_value = 0;
        self.register_write_counts = [0; VDC_REGISTER_COUNT];
        self.register_select_counts = [0; VDC_REGISTER_COUNT];
        self.r05_low_writes = 0;
        self.r05_high_writes = 0;
        self.last_r05_low = 0;
        self.r05_low_value_counts = [0; 0x100];
        self.r05_high_value_counts = [0; 0x100];
        self.vram_data_low_writes = 0;
        self.vram_data_high_writes = 0;
        self.vram_data_high_without_low = 0;
        self.vram_write_range_count = 0;
        self.mawr_log.clear();
        self.vram_write_log.clear();
        self.vram_write_log_limit = 0;
        self.pending_write_register = None;
        self.registers[0x0A] = 0x0010;
        self.registers[0x0B] = 0x0010;
        self.ignore_next_high_byte = false;
    }

    pub(crate) fn read_status(&mut self) -> u8 {
        self.refresh_activity_flags();
        let value = self.status;
        let preserved = self.status & VDC_STATUS_BUSY;
        self.status = preserved;
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VDC status -> {:02X} (VBL={} DS={} DV={} BUSY={} busy_cycles={})",
            value,
            (value & VDC_STATUS_VBL) != 0,
            (value & VDC_STATUS_DS) != 0,
            (value & VDC_STATUS_DV) != 0,
            (value & VDC_STATUS_BUSY) != 0,
            self.busy_cycles
        );
        value
    }

    #[allow(dead_code)]
    pub(crate) fn raise_status(&mut self, mask: u8) {
        self.status |= mask;
    }

    pub(crate) fn status_bits(&self) -> u8 {
        self.status
    }

    pub(crate) fn control_write_count(&self) -> u64 {
        self.control_write_count
    }

    pub(crate) fn last_control_value(&self) -> u16 {
        self.last_control_value
    }

    pub(crate) fn r05_low_writes(&self) -> u64 {
        self.r05_low_writes
    }

    pub(crate) fn r05_high_writes(&self) -> u64 {
        self.r05_high_writes
    }

    pub(crate) fn last_r05_low(&self) -> u8 {
        self.last_r05_low
    }

    pub(crate) fn r05_low_value_counts(&self) -> &[u64; 0x100] {
        &self.r05_low_value_counts
    }

    pub(crate) fn r05_high_value_counts(&self) -> &[u64; 0x100] {
        &self.r05_high_value_counts
    }

    pub(crate) fn vram_data_low_writes(&self) -> u64 {
        self.vram_data_low_writes
    }

    pub(crate) fn vram_data_high_writes(&self) -> u64 {
        self.vram_data_high_writes
    }

    pub(crate) fn vram_data_high_without_low(&self) -> u64 {
        self.vram_data_high_without_low
    }

    pub(crate) fn satb_pending(&self) -> bool {
        self.satb_written || self.satb_pending
    }

    pub(crate) fn satb_source(&self) -> u16 {
        self.satb_source
    }

    pub(crate) fn clear_sprite_overflow(&mut self) {
        self.status &= !VDC_STATUS_OR;
    }

    pub(crate) fn irq_active(&self) -> bool {
        let mask = self.enabled_status_mask() | self.enabled_dma_status_mask();
        (self.status & mask) != 0
    }

    pub(crate) fn enabled_status_mask(&self) -> u8 {
        let ctrl = self.control();
        let vbl_ctrl = if ctrl == 0 && (self.render_control_latch & 0x3000) != 0 {
            self.render_control_latch
        } else {
            ctrl
        };
        let mut mask = 0;
        if ctrl & 0x0001 != 0 {
            mask |= VDC_STATUS_CR;
        }
        if ctrl & 0x0002 != 0 {
            mask |= VDC_STATUS_OR;
        }
        if ctrl & 0x0004 != 0 {
            mask |= VDC_STATUS_RCR;
        }
        if vbl_ctrl & 0x0008 != 0 || vbl_ctrl & 0x1000 != 0 || vbl_ctrl & 0x2000 != 0 {
            mask |= VDC_STATUS_VBL;
        }
        mask
    }

    pub(crate) fn enabled_dma_status_mask(&self) -> u8 {
        let mut mask = 0;
        if self.dma_control & DMA_CTRL_IRQ_SATB != 0 {
            mask |= VDC_STATUS_DS;
        }
        if self.dma_control & DMA_CTRL_IRQ_VRAM != 0 {
            mask |= VDC_STATUS_DV;
        }
        mask
    }

    pub(crate) fn control(&self) -> u16 {
        self.registers[0x04]
    }

    pub(crate) fn control_for_render(&self) -> u16 {
        let current = self.control();
        if current == 0 {
            self.render_control_latch
        } else {
            current
        }
    }

    pub(crate) fn vertical_window(&self) -> VerticalWindow {
        let timing_programmed = self.registers[0x0D] != 0
            || self.registers[0x0E] != 0
            || (self.registers[0x0C] & 0xFF00) != 0;

        if !timing_programmed {
            return VerticalWindow {
                timing_programmed: false,
                active_start_line: 0,
                active_line_count: VDC_VISIBLE_LINES as usize,
                post_active_overscan_lines: 0,
                vblank_start_line: VDC_VISIBLE_LINES as usize,
            };
        }

        let lines_per_frame = LINES_PER_FRAME as usize;
        let vpr = self.registers[0x0C];
        let vsw = (vpr & 0x001F) as usize;
        let vds = ((vpr >> 8) & 0x00FF) as usize;
        let vdw = self.registers[0x0D];
        let vcr = self.registers[0x0E];
        // HuC6270 datasheet: VSW register = desired_value - 1,
        // VDS register = desired_value - 2.  MAME huc6270.cpp uses
        // (vsw + 1) + (vds + 2) for the first active scanline.
        let active_start_line = (vsw + 1 + vds + 2) % lines_per_frame;
        let active_line_count = ((vdw & 0x01FF) as usize)
            .saturating_add(1)
            .max(1)
            .min(lines_per_frame);
        let post_active_overscan_lines = 3usize.saturating_add((vcr & 0x00FF) as usize);
        let vblank_start_line = active_start_line
            .saturating_add(active_line_count)
            .min(VDC_MAX_VBLANK_START_LINE)
            .min(lines_per_frame.saturating_sub(1));

        VerticalWindow {
            timing_programmed: true,
            active_start_line,
            active_line_count,
            post_active_overscan_lines,
            vblank_start_line,
        }
    }

    #[inline]
    pub(crate) fn frame_line_for_output_row(&self, window: &VerticalWindow, row: usize) -> usize {
        let lines_per_frame = LINES_PER_FRAME as usize;
        if window.timing_programmed {
            (window.active_start_line + row) % lines_per_frame
        } else {
            row % lines_per_frame
        }
    }

    pub(crate) fn active_row_for_output_row(&self, row: usize) -> Option<usize> {
        let window = self.vertical_window();
        if !window.timing_programmed {
            return (row < FRAME_HEIGHT).then_some(row);
        }

        let cycle_len = window
            .active_start_line
            .saturating_add(window.active_line_count)
            .saturating_add(window.post_active_overscan_lines)
            .max(1);
        let cycle_pos = self.frame_line_for_output_row(&window, row) % cycle_len;
        if cycle_pos < window.active_start_line {
            return None;
        }
        let active_end = window.active_start_line + window.active_line_count;
        if cycle_pos < active_end {
            return Some(cycle_pos - window.active_start_line);
        }
        None
    }

    pub(crate) fn output_row_in_active_window(&self, row: usize) -> bool {
        self.active_row_for_output_row(row).is_some()
    }

    pub(crate) fn vblank_start_scanline(&self) -> u16 {
        self.vertical_window().vblank_start_line as u16
    }

    pub(crate) fn rcr_scanline_for_target(&self, target: u16) -> Option<u16> {
        if target >= VDC_ACTIVE_COUNTER_BASE as u16 {
            // The raster counter starts at 0x40 at the first active display
            // line (active_start_line = vsw+1+vds+2).  It increments each
            // scanline.  RCR match fires when counter == target, i.e. at
            // scanline = active_start + (target - 0x40).
            let window = self.vertical_window();
            let relative = (target - VDC_ACTIVE_COUNTER_BASE as u16) as usize;
            let line = (window.active_start_line + relative) % (LINES_PER_FRAME as usize);
            if line < LINES_PER_FRAME as usize {
                Some(line as u16)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Re-latch scroll/zoom/control for a scanline whose RCR ISR has now
    /// completed.  Called once per RCR event, just before the scanline
    /// advances.  Updates bg_y_offset to account for the ISR's BYR write.
    pub(crate) fn consume_post_isr_scroll(&mut self, line: usize) {
        // After the CPU services an RCR ISR, consume any pending scroll/zoom
        // writes so bg_y_offset is updated.  On real hardware the VDC has
        // already started rendering the RCR scanline with the *pre-ISR*
        // latch values, so we do NOT overwrite the scroll arrays for the
        // current scanline.  Instead we only account for the h-sync
        // BYR-latch increment that occurs at the end of this scanline:
        // after apply_pending_scroll() resets bg_y_offset to 0, the
        // active-line increment below brings it to 1, so the *next*
        // scanline renders at BYR+1 (matching MAME's m_byr_latched
        // semantics).
        self.apply_pending_scroll();
        self.apply_pending_zoom();

        let window = self.vertical_window();
        let is_active = !self.in_vblank
            && line >= window.active_start_line
            && line < window.vblank_start_line;
        if is_active && self.bg_y_offset_loaded {
            self.bg_y_offset = self.bg_y_offset.wrapping_add(1);
        }
    }

    pub(crate) fn tick(&mut self, phi_cycles: u32) -> bool {
        if phi_cycles == 0 {
            return false;
        }

        self.consume_busy(phi_cycles);

        let frame_cycles = VDC_VBLANK_INTERVAL as u64;
        self.phi_scaled = self
            .phi_scaled
            .saturating_add(phi_cycles as u64 * LINES_PER_FRAME as u64);

        let mut irq_recalc = false;
        while self.phi_scaled >= frame_cycles {
            // Preserve per-frame line latches until the renderer consumes this frame.
            // Some HuC6280 instructions run long enough to cover many scanlines; if
            // we march past VBlank start in one call, line-state snapshots can be
            // overwritten before render_frame_from_vram() runs.
            if self.frame_trigger {
                break;
            }
            self.phi_scaled -= frame_cycles;

            // If an RCR ISR was pending, the CPU has now executed it
            // (a full scanline's worth of cycles elapsed for this loop
            // iteration).  Consume the ISR's scroll/zoom writes and
            // account for the h-sync BYR-latch increment, but keep
            // the pre-ISR scroll state for the RCR scanline (the VDC
            // has already rendered it with the old values).
            if let Some(line) = self.rcr_post_isr_line.take() {
                self.consume_post_isr_scroll(line as usize);
            }

            let wrapped = self.advance_scanline();
            if wrapped {
                irq_recalc = true;
            }

            // Latch line state with pre-ISR values.  If an RCR interrupt
            // fires below, we record the scanline: once the CPU finishes
            // the ISR (next tick()), consume_post_isr_scroll() accounts
            // for the h-sync BYR-latch increment without overwriting the
            // already-latched scroll for this scanline.
            self.latch_line_state(self.scanline as usize);

            let rcr_target = self.registers[0x06] & 0x03FF;
            if let Some(rcr_scanline) = self.rcr_scanline_for_target(rcr_target) {
                if self.scanline == rcr_scanline {
                    // Per HuC6270 hardware (confirmed by MAME): the RR status
                    // flag is only raised when CR bit 2 (RCR interrupt enable)
                    // is set.  Games like Kato-chan & Ken-chan rely on this —
                    // the ISR checks the RR bit to decide whether to apply a
                    // scroll offset, so raising it unconditionally would cause
                    // an incorrect BYR value on the title screen.
                    if self.control() & 0x0004 != 0 {
                        self.raise_status(VDC_STATUS_RCR);
                        self.rcr_post_isr_line = Some(self.scanline);
                        irq_recalc = true;
                        break;
                    }
                }
            }

            // Trigger frame render at the LAST active scanline (one before
            // VBlank).  Games like Power League III write sprite pattern data
            // to VRAM during active display; rendering at VBlank start would
            // miss those writes.  By deferring the trigger to the end of
            // active display, the batch renderer captures all mid-frame VRAM
            // updates while per-line scroll/control state is still intact.
            let vbl = self.vblank_start_scanline();
            if !self.in_vblank && !self.frame_trigger && vbl > 0
                && self.scanline == vbl - 1
            {
                self.frame_trigger = true;
                break;
            }

            if self.scanline == vbl {
                self.in_vblank = true;
                self.raise_status(VDC_STATUS_VBL);
                self.refresh_activity_flags();
                irq_recalc = true;
                if self.handle_vblank_start() {
                    irq_recalc = true;
                }
                break;
            }
        }

        irq_recalc
    }

    pub(crate) fn frame_ready(&self) -> bool {
        self.frame_trigger
    }

    pub(crate) fn clear_frame_trigger(&mut self) {
        self.frame_trigger = false;
    }

    pub(crate) fn set_busy(&mut self, cycles: u32) {
        let divisor = Self::env_vdc_busy_divisor().max(1);
        let scaled = if divisor == 1 {
            cycles
        } else {
            cycles / divisor
        };
        self.busy_cycles = self.busy_cycles.max(scaled);
        self.refresh_activity_flags();
    }

    pub(crate) fn consume_busy(&mut self, phi_cycles: u32) {
        if self.busy_cycles > 0 {
            if phi_cycles >= self.busy_cycles {
                self.busy_cycles = 0;
            } else {
                self.busy_cycles -= phi_cycles;
            }
        }
        self.refresh_activity_flags();
    }

    pub(crate) fn refresh_activity_flags(&mut self) {
        if self.busy_cycles > 0 {
            self.status |= VDC_STATUS_BUSY;
        } else {
            self.status &= !VDC_STATUS_BUSY;
        }
    }

    pub(crate) fn write_port(&mut self, port: usize, value: u8) {
        match port {
            0 => self.write_select(value),
            1 => self.write_data_low(value),
            2 => self.write_data_high_direct(value),
            _ => {}
        }
    }

    pub(crate) fn read_port(&mut self, port: usize) -> u8 {
        match port {
            0 => self.read_status(),
            1 => self.read_data_low(),
            2 => self.read_data_high(),
            _ => 0,
        }
    }

    pub(crate) fn selected_register(&self) -> u8 {
        self.map_register_index(self.selected & 0x1F)
    }

    pub(crate) fn map_register_index(&self, raw: u8) -> u8 {
        match raw {
            0x03 => 0x04, // CR (control) -> internal alias at 0x04/0x05
            _ => raw,
        }
    }

    pub(crate) fn register(&self, index: usize) -> Option<u16> {
        self.registers.get(index).copied()
    }

    pub(crate) fn register_write_count(&self, index: usize) -> u64 {
        self.register_write_counts.get(index).copied().unwrap_or(0)
    }

    pub(crate) fn register_select_count(&self, index: usize) -> u64 {
        self.register_select_counts.get(index).copied().unwrap_or(0)
    }

    pub(crate) fn write_select(&mut self, value: u8) {
        let new_sel = value & 0x1F;
        // Keep the low-byte target latched across ST0 writes until the paired
        // high-byte commit completes.
        if !self.st0_locked_until_commit {
            self.pending_write_register = None;
        }
        #[cfg(feature = "trace_hw_writes")]
        if new_sel == 0x05 {
            eprintln!("  TRACE select R05 (pc={:04X})", 0);
        }
        self.selected = new_sel;
        self.write_phase = VdcWritePhase::Low;
        self.ignore_next_high_byte = false;
        let index = self.map_register_index(self.selected) as usize;
        if let Some(count) = self.register_select_counts.get_mut(index) {
            *count = count.saturating_add(1);
        }
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VDC select {:02X} pending={:?} phase={:?}",
            self.selected, self.pending_write_register, self.write_phase
        );
    }

    pub(crate) fn write_data_low(&mut self, value: u8) {
        let index = self.selected_register() as usize;
        self.latch_low = value;
        self.pending_write_register = Some(self.selected_register());
        self.st0_locked_until_commit = true;

        // Per-register data latch (MAME m_vdc_data): ST1 writes the low byte
        // of the CURRENTLY SELECTED register.  This is per-register, so
        // interleaved ST1 writes to different registers don't clobber each
        // other's low bytes.
        if index < self.vdc_data.len() {
            self.vdc_data[index] = (self.vdc_data[index] & 0xFF00) | value as u16;
        }

        if self.pending_write_register == Some(0x02) {
            self.vram_data_low_writes = self.vram_data_low_writes.saturating_add(1);
        }
        if matches!(self.selected_register(), 0x04 | 0x05) {
            self.r05_low_writes = self.r05_low_writes.saturating_add(1);
            self.last_r05_low = value;
            if let Some(slot) = self.r05_low_value_counts.get_mut(value as usize) {
                *slot = slot.saturating_add(1);
            }
        }
        #[cfg(feature = "trace_hw_writes")]
        {
            let reg = self.selected_register();
            if matches!(reg, 0x04 | 0x05) {
                eprintln!("  TRACE R05 low {:02X}", value);
            } else if matches!(reg, 0x10 | 0x11 | 0x12) {
                eprintln!("  TRACE DMA reg {:02X} low {:02X}", reg, value);
            }
        }
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VDC low byte {:02X} latched for R{:02X} pending={:?} phase={:?}",
            value,
            self.selected_register(),
            self.pending_write_register,
            self.write_phase
        );
        if matches!(index, 0x02 | 0x0A | 0x0B | 0x0C | 0x12 | 0x14) {
            // HuC6270: these registers only commit on the high-byte write
            // (ST2).  $02 (VWR) must wait for both bytes before VRAM write;
            // $12 (LENR) must wait before starting DMA.  Timing registers
            // ($0A/$0B/$0C) also defer for safety.
            self.write_phase = VdcWritePhase::High;
            self.ignore_next_high_byte = false;
        } else {
            // MAME's COMBINE_DATA merges each byte into the register
            // immediately.  Registers like DCR ($0F) and DVSSR ($13)
            // have side effects (auto-SATB enable, DMA scheduling) that
            // must fire on low-byte-only writes — games such as
            // Bikkuriman World write DCR via ST1 without a following ST2.
            let existing = self.registers.get(index).copied().unwrap_or(0);
            let combined = (existing & 0xFF00) | value as u16;
            self.commit_register_write(index, combined);
            self.write_phase = VdcWritePhase::High;
            self.ignore_next_high_byte = false;
        }
    }

    pub(crate) fn write_data_high(&mut self, value: u8) {
        // HuC6270: the address register (AR) at the time of the ST2 write
        // determines which VDC register receives the 16-bit value.  The
        // pending_write_register is only used to detect whether a low-byte
        // latch is available (i.e. ST1 preceded this ST2).
        let use_latch = matches!(self.write_phase, VdcWritePhase::High)
            && self.pending_write_register.is_some();
        if use_latch && self.ignore_next_high_byte {
            self.write_phase = VdcWritePhase::Low;
            self.ignore_next_high_byte = false;
            self.pending_write_register = None;
            return;
        }
        // Per-register data latch (MAME m_vdc_data): ST2 writes the high
        // byte of the CURRENTLY SELECTED register, then commits the full
        // 16-bit word.  The low byte was stored by the previous ST1 for
        // this register and is NOT affected by intervening ST1 writes to
        // other registers.
        let index = self.selected_register() as usize;
        if index < self.vdc_data.len() {
            self.vdc_data[index] = (self.vdc_data[index] & 0x00FF) | ((value as u16) << 8);
        }
        let combined = self.vdc_data.get(index).copied().unwrap_or(
            u16::from_le_bytes([self.latch_low, value])
        );
        // Always use the CURRENT AR for the commit target — this matches
        // real HuC6270 behaviour where AR at ST2 time selects the register.
        let index = self.selected_register() as usize;
        self.pending_write_register = None;
        if index == 0x02 {
            self.vram_data_high_writes = self.vram_data_high_writes.saturating_add(1);
            if !use_latch {
                self.vram_data_high_without_low = self.vram_data_high_without_low.saturating_add(1);
            }
        }
        self.st0_locked_until_commit = false;
        #[cfg(feature = "trace_hw_writes")]
        {
            self.st0_hold_counter = 0;
        }
        if index == 0x04 || index == 0x05 {
            self.r05_high_writes = self.r05_high_writes.saturating_add(1);
            if let Some(slot) = self.r05_high_value_counts.get_mut(value as usize) {
                *slot = slot.saturating_add(1);
            }
        }
        #[cfg(feature = "trace_hw_writes")]
        {
            if index == 0x04 || index == 0x05 {
                eprintln!("  TRACE R05 high {:02X} commit {:04X}", value, combined);
            } else if matches!(index, 0x10 | 0x11 | 0x12) {
                eprintln!(
                    "  TRACE DMA reg {:02X} high {:02X} commit {:04X}",
                    index, value, combined
                );
            }
            self.debug_log_select_and_value(index as u8, combined);
        }
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VDC high byte {:02X} -> commit R{:02X} = {:04X} (selected={:02X} pending={:?} phase={:?})",
            value,
            index,
            combined,
            self.selected_register(),
            self.pending_write_register,
            self.write_phase
        );
        self.commit_register_write(index, combined);
        self.write_phase = VdcWritePhase::Low;
        if Self::env_hold_dsdv() {
            self.status |= VDC_STATUS_DS | VDC_STATUS_DV;
        }
    }

    pub(crate) fn write_data_high_direct(&mut self, value: u8) {
        if !matches!(self.write_phase, VdcWritePhase::High) {
            self.write_phase = VdcWritePhase::High;
            self.ignore_next_high_byte = false;
        }
        self.write_data_high(value);
    }

    #[cfg(feature = "trace_hw_writes")]
    pub(crate) fn debug_log_select_and_value(&self, reg: u8, value: u16) {
        if matches!(reg, 0x04 | 0x05 | 0x10 | 0x11 | 0x12) {
            eprintln!("  TRACE commit R{:02X} = {:04X}", reg, value);
        }
    }

    pub(crate) fn take_vram_dma_request(&mut self) -> bool {
        let pending = self.vram_dma_request;
        self.vram_dma_request = false;
        pending
    }

    pub(crate) fn commit_register_write(&mut self, index: usize, combined: u16) {
        #[cfg(feature = "trace_hw_writes")]
        {
            eprintln!(
                "  VDC write R{:02X} = {:04X} (sel={:02X})",
                index,
                combined,
                self.selected_register()
            );
            if index == 0x05 {
                eprintln!("  TRACE R05 commit {:04X}", combined);
            }
        }
        if self.reg_trace_enabled && matches!(index, 0x05 | 0x06 | 0x07 | 0x08) {
            self.reg_trace.push((self.scanline, index as u8, combined));
        }
        if index < self.registers.len() {
            let stored = if matches!(index, 0x00 | 0x01) {
                combined & 0x7FFF
            } else {
                combined
            };
            self.registers[index] = stored;
            if let Some(count) = self.register_write_counts.get_mut(index) {
                *count = count.saturating_add(1);
            }
        }
        match index {
            0x00 => {
                self.mawr = combined & 0x7FFF;
                self.registers[0x00] = self.mawr;
                // MAWR log
                if self.mawr >= self.mawr_log_start && self.mawr < self.mawr_log_end {
                    self.mawr_log.push(self.mawr);
                }
            }
            0x01 => {
                self.marr = combined & 0x7FFF;
                self.registers[0x01] = self.marr;
                self.prefetch_read();
                self.read_phase = VdcReadPhase::Low;
            }
            0x02 => self.write_vram(combined),
            0x04 | 0x05 => {
                // Mirror control into both slots so legacy/tests remain stable.
                self.registers[0x04] = combined;
                self.registers[0x05] = combined;
                self.control_write_count = self.control_write_count.saturating_add(1);
                self.last_control_value = combined;
                if combined != 0 {
                    self.render_control_latch = combined;
                }
                #[cfg(feature = "trace_hw_writes")]
                eprintln!("  VDC control <= {:04X}", combined);
            }
            0x07 => {
                let masked = combined & 0x03FF;
                self.registers[0x07] = masked;
                self.scroll_x_pending = masked;
                self.scroll_x_dirty = true;
            }
            0x08 => {
                let masked = combined & 0x01FF;
                self.registers[0x08] = masked;
                self.scroll_y_pending = masked;
                self.scroll_y_dirty = true;
                #[cfg(feature = "trace_hw_writes")]
                eprintln!("  VDC BYR <= {:04X} (raw {:04X}) @ scanline {}", masked, combined, self.scanline);
            }
            0x0A => {
                // HSR (Horizontal Sync Register) – timing only, not zoom.
                self.registers[0x0A] = combined;
            }
            0x0B => {
                // HDR (Horizontal Display Register) – timing only, not zoom.
                self.registers[0x0B] = combined;
            }
            0x0C => {
                // VPR (Vertical Position Register): VSW (low) + VDS (high).
                // Timing-only; stored for vertical window calculation.
                self.registers[0x0C] = combined;
            }
            0x0F => self.write_dma_control(combined),
            0x10 => self.write_dma_source(combined),
            0x11 => self.write_dma_destination(combined),
            0x12 => self.write_dma_length(combined),
            0x13 => self.write_satb_source(index, combined),
            // $14+ are beyond the HuC6270 register set — writes are stored
            // in the register array but have no side effects.
            _ => {}
        }
    }

    #[allow(dead_code)] // Internal utility; not triggered by standard HuC6270 registers.
    pub(crate) fn schedule_cram_dma(&mut self) {
        self.cram_pending = true;
        self.cram_dma_count += 1;
        self.last_cram_source = self.marr & 0x7FFF;
        self.last_cram_length = self.registers[0x12];
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VDC CRAM DMA scheduled (pending len {:04X}) source {:04X} (MAWR {:04X})",
            self.registers[0x12],
            self.last_cram_source,
            self.marr & 0x7FFF
        );
    }

    pub(crate) fn write_dma_control(&mut self, value: u16) {
        let masked = value & 0x001F;
        self.dma_control = masked;
        self.registers[0x0F] = masked;
        // Per MAME huc6270.cpp: writing DCR only stores the value.
        // Status flags (DS/DV) are NOT cleared here — they are only
        // cleared when the status register is read.
        if masked & DMA_CTRL_SATB_AUTO == 0 {
            self.satb_pending = false;
        }
    }

    pub(crate) fn write_dma_source(&mut self, value: u16) {
        self.dma_source = value;
        self.registers[0x10] = value;
    }

    pub(crate) fn write_dma_destination(&mut self, value: u16) {
        let masked = value & 0x7FFF;
        self.dma_destination = masked;
        self.registers[0x11] = masked;
    }

    pub(crate) fn write_dma_length(&mut self, value: u16) {
        self.registers[0x12] = value;
        self.vram_dma_request = true;
    }

    pub(crate) fn write_satb_source(&mut self, index: usize, value: u16) {
        let masked = value & 0x7FFF;
        self.satb_source = masked;
        if let Some(slot) = self.registers.get_mut(index) {
            *slot = masked;
        }
        // Match real hardware (MAME huc6270.cpp): writing DVSSR sets
        // a one-shot flag (satb_written) that schedules a transfer for
        // the next VBlank.  This flag is independent of the auto-
        // transfer bit in DCR, so writing DCR cannot cancel it.
        self.satb_written = true;
    }

    pub(crate) fn perform_satb_dma(&mut self) {
        let base = (self.satb_source & 0x7FFF) as usize;
        for i in 0..self.satb.len() {
            let idx = (base + i) & 0x7FFF;
            self.satb[i] = *self.vram.get(idx).unwrap_or(&0);
        }
        let busy_cycles = (self.satb.len() as u32).saturating_mul(VDC_DMA_WORD_CYCLES);
        self.set_busy(busy_cycles);
        self.raise_status(VDC_STATUS_DS);
        // Clear the one-shot flag; auto stays based on DCR bit 4.
        self.satb_written = false;
        self.satb_pending = (self.dma_control & DMA_CTRL_SATB_AUTO) != 0;
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VDC SATB DMA complete (source {:04X}) -> status {:02X}",
            self.satb_source, self.status
        );
    }

    pub(crate) fn handle_vblank_start(&mut self) -> bool {
        // Match MAME huc6270.cpp handle_vblank(): DMA runs when DVSSR was
        // written (one-shot) OR when auto-transfer (DCR bit 4) is enabled.
        let auto = (self.dma_control & DMA_CTRL_SATB_AUTO) != 0;
        if !self.satb_written && !auto {
            return false;
        }
        self.perform_satb_dma();
        true
    }

    pub(crate) fn advance_vram_addr(addr: u16, decrement: bool) -> u16 {
        let next = if decrement {
            addr.wrapping_sub(1)
        } else {
            addr.wrapping_add(1)
        };
        next & 0x7FFF
    }

    pub(crate) fn write_vram(&mut self, value: u16) {
        let addr = self.mawr & 0x7FFF;

        // Debug: range tracking and write log (always record, even if latched)
        if addr >= self.vram_write_range_start && addr < self.vram_write_range_end {
            self.vram_write_range_count = self.vram_write_range_count.saturating_add(1);
        }
        if self.vram_write_log.len() < self.vram_write_log_limit {
            self.vram_write_log.push((addr, value));
        }
        #[cfg(feature = "trace_hw_writes")]
        eprintln!("    VRAM[{:04X}] = {:04X}", addr, value);

        let idx = addr as usize;
        if let Some(slot) = self.vram.get_mut(idx) {
            *slot = value;
        }
        // Mark BIOS font as dirty if the write hits the font tile area
        if !self.bios_font_dirty && !self.bios_font_tiles.is_empty() {
            if addr >= 0x1200 && addr < 0x1800 {
                self.bios_font_dirty = true;
            }
        }
        self.set_busy(VDC_BUSY_ACCESS_CYCLES);

        // MAWR always advances immediately regardless of whether the
        // write was committed or latched (real HW behaviour).
        self.mawr = (self.mawr.wrapping_add(self.increment_step())) & 0x7FFF;
        self.registers[0x00] = self.mawr;
        self.registers[0x02] = value;
    }

    pub(crate) fn write_vram_dma_word(&mut self, addr: u16, value: u16) {
        let idx = (addr as usize) & 0x7FFF;
        if let Some(slot) = self.vram.get_mut(idx) {
            *slot = value;
        }
        #[cfg(feature = "trace_hw_writes")]
        eprintln!("    VRAM DMA[{:04X}] = {:04X}", addr & 0x7FFF, value);
    }

    pub(crate) fn read_data_low(&mut self) -> u8 {
        let reg = self.selected_register() as usize;
        if reg != 0x02 {
            self.read_phase = VdcReadPhase::High;
            return (self.registers.get(reg).copied().unwrap_or(0) & 0x00FF) as u8;
        }
        if self.read_phase == VdcReadPhase::Low {
            self.prefetch_read();
        }
        self.read_phase = VdcReadPhase::High;
        (self.read_buffer & 0x00FF) as u8
    }

    pub(crate) fn read_data_high(&mut self) -> u8 {
        let reg = self.selected_register() as usize;
        if reg != 0x02 {
            self.read_phase = VdcReadPhase::Low;
            return (self.registers.get(reg).copied().unwrap_or(0) >> 8) as u8;
        }
        if self.read_phase == VdcReadPhase::Low {
            self.prefetch_read();
        }
        let value = (self.read_buffer >> 8) as u8;
        self.advance_read_address();
        self.read_phase = VdcReadPhase::Low;
        value
    }

    pub(crate) fn prefetch_read(&mut self) {
        let idx = (self.marr as usize) & 0x7FFF;
        self.read_buffer = *self.vram.get(idx).unwrap_or(&0);
        self.set_busy(VDC_BUSY_ACCESS_CYCLES);
        self.registers[0x02] = self.read_buffer;
    }

    pub(crate) fn advance_read_address(&mut self) {
        self.marr = (self.marr.wrapping_add(self.increment_step())) & 0x7FFF;
        self.registers[0x01] = self.marr;
    }

    pub(crate) fn increment_step(&self) -> u16 {
        match (self.control() >> 11) & 0x03 {
            0 => 1,
            1 => 32,
            2 => 64,
            _ => 128,
        }
    }

    pub(crate) fn map_dimensions(&self) -> (usize, usize) {
        let mwr = self.registers[0x09];
        let width_code = ((mwr >> 4) & 0x03) as usize;
        let width_tiles = match width_code {
            0 => 32,
            1 => 64,
            2 => 128,
            _ => 128,
        };
        let height_tiles = if (mwr >> 6) & 0x01 == 0 { 32 } else { 64 };
        (width_tiles, height_tiles)
    }

    pub(crate) fn map_base_address(&self) -> usize {
        0
    }

    pub(crate) fn map_entry_address(&self, tile_row: usize, tile_col: usize) -> usize {
        let (map_width, map_height) = self.map_dimensions();
        let width = map_width.max(1);
        let height = map_height.max(1);
        let row = tile_row % height;
        let col = tile_col % width;
        // HuC6270 BAT uses flat row-major addressing (matching MAME/Mednafen):
        //   address = bat_y * map_width + bat_x
        // The MWR register determines the map dimensions.
        (self.map_base_address() + row * width + col) & 0x7FFF
    }

    #[cfg(test)]
    pub(crate) fn map_entry_address_for_test(&self, tile_row: usize, tile_col: usize) -> usize {
        self.map_entry_address(tile_row, tile_col)
    }

    #[cfg(test)]
    pub(crate) fn set_zoom_for_test(&mut self, zoom_x: u16, zoom_y: u16) {
        self.zoom_x = zoom_x & 0x001F;
        self.zoom_y = zoom_y & 0x001F;
        self.scroll_line_valid.fill(false);
    }

    pub(crate) fn apply_pending_scroll(&mut self) {
        if self.scroll_x_dirty {
            self.scroll_x = self.scroll_x_pending & 0x03FF;
            self.scroll_x_dirty = false;
        }
        if self.scroll_y_dirty {
            self.scroll_y = self.scroll_y_pending & 0x01FF;
            self.scroll_y_dirty = false;
            // Writing BYR resets the line offset counter.
            self.bg_y_offset = 0;
            self.bg_y_offset_loaded = true;
        }
    }

    pub(crate) fn apply_pending_zoom(&mut self) {
        if self.zoom_x_dirty {
            self.zoom_x = self.zoom_x_pending & 0x001F;
            self.zoom_x_dirty = false;
        }
        if self.zoom_y_dirty {
            self.zoom_y = self.zoom_y_pending & 0x001F;
            self.zoom_y_dirty = false;
        }
    }

    pub(crate) fn latch_line_state(&mut self, line: usize) {
        self.apply_pending_scroll();
        self.apply_pending_zoom();

        // HuC6270 BG Y offset tracking:
        //  - At the first active scanline of each frame, the offset resets to 0.
        //  - Each subsequent active scanline, the offset auto-increments.
        //  - Writing BYR mid-frame resets the offset to 0 (handled in
        //    apply_pending_scroll above).
        // The renderer computes: sample_y = BYR + offset * zoom_step.
        let window = self.vertical_window();
        let is_active =
            !self.in_vblank && line >= window.active_start_line && line < window.vblank_start_line;

        if is_active && !self.bg_y_offset_loaded {
            self.bg_y_offset = 0;
            self.bg_y_offset_loaded = true;
        }

        let idx = line % self.scroll_line_x.len();
        self.scroll_line_x[idx] = self.scroll_x;
        self.scroll_line_y[idx] = self.scroll_y;
        self.scroll_line_y_offset[idx] = self.bg_y_offset;
        self.zoom_line_x[idx] = self.zoom_x;
        self.zoom_line_y[idx] = self.zoom_y;
        self.control_line[idx] = self.control_for_render();
        self.scroll_line_valid[idx] = true;

        // After latching, increment the offset for the next active scanline.
        if is_active && self.bg_y_offset_loaded {
            self.bg_y_offset = self.bg_y_offset.wrapping_add(1);
        }
    }

    pub(crate) fn ensure_line_state(&mut self, line: usize) {
        if line >= self.scroll_line_x.len() {
            self.apply_pending_scroll();
            self.apply_pending_zoom();
            return;
        }
        if !self.scroll_line_valid[line] {
            self.apply_pending_scroll();
            self.apply_pending_zoom();
            self.scroll_line_x[line] = self.scroll_x;
            self.scroll_line_y[line] = self.scroll_y;
            // For lines not latched by the scanline loop (e.g. direct
            // render calls in tests), synthesise the Y offset.
            let window = self.vertical_window();
            let offset = if line >= window.active_start_line && line < window.vblank_start_line {
                (line - window.active_start_line) as u16
            } else {
                0
            };
            self.scroll_line_y_offset[line] = offset;
            self.zoom_line_x[line] = self.zoom_x;
            self.zoom_line_y[line] = self.zoom_y;
            self.control_line[line] = self.control_for_render();
            self.scroll_line_valid[line] = true;
        }
    }

    /// Returns (x_scroll, y_scroll, y_offset) for the given line.
    /// y_offset is the number of active lines since BYR was last loaded/written.
    pub(crate) fn scroll_values_for_line(&mut self, line: usize) -> (usize, usize, usize) {
        self.ensure_line_state(line);
        if line < self.scroll_line_x.len() {
            (
                self.scroll_line_x[line] as usize,
                self.scroll_line_y[line] as usize,
                self.scroll_line_y_offset[line] as usize,
            )
        } else {
            (self.scroll_x as usize, self.scroll_y as usize, 0)
        }
    }

    pub(crate) fn zoom_values_for_line(&mut self, line: usize) -> (u16, u16) {
        self.ensure_line_state(line);
        if line < self.zoom_line_x.len() {
            (self.zoom_line_x[line], self.zoom_line_y[line])
        } else {
            (self.zoom_x, self.zoom_y)
        }
    }

    pub(crate) fn control_values_for_line(&mut self, line: usize) -> u16 {
        self.ensure_line_state(line);
        if line < self.control_line.len() {
            self.control_line[line]
        } else {
            self.control_for_render()
        }
    }

    pub(crate) fn line_state_index_for_frame_row(&self, row: usize) -> usize {
        let window = self.vertical_window();
        self.frame_line_for_output_row(&window, row)
    }

    pub(crate) fn advance_scanline(&mut self) -> bool {
        self.scanline = self.scanline.wrapping_add(1);
        let mut wrapped = false;
        if self.scanline >= LINES_PER_FRAME {
            self.scanline = 0;
            self.in_vblank = false;
            self.scroll_line_valid.fill(false);
            self.refresh_activity_flags();
            self.bg_y_offset_loaded = false;
            wrapped = true;
        }
        // Don't latch here — the tick() loop latches after the RCR check
        // so that the CPU ISR can modify scroll registers before the
        // RCR scanline's state is committed.  The scroll_line_valid array
        // serves as the implicit "needs latch" flag: the new scanline's
        // entry is still false (cleared on frame wrap), so the tick loop
        // knows to latch it.
        wrapped
    }

    #[cfg(test)]
    pub(crate) fn advance_scanline_for_test(&mut self) {
        self.advance_scanline();
        // Tests expect line state to be latched immediately after advancing.
        self.latch_line_state(self.scanline as usize);
    }

    pub(crate) fn zoom_step_value(raw: u16) -> usize {
        let value = (raw & 0x001F) as usize;
        value.max(1).min(32)
    }

    #[cfg(test)]
    pub(crate) fn scroll_for_scanline(&mut self) -> (usize, usize) {
        self.apply_pending_scroll();
        (self.scroll_x as usize, self.scroll_y as usize)
    }

    pub(crate) fn scroll_line(&self, line: usize) -> (u16, u16) {
        if line < self.scroll_line_x.len() {
            (self.scroll_line_x[line], self.scroll_line_y[line])
        } else {
            (self.scroll_x, self.scroll_y)
        }
    }

    pub(crate) fn zoom_line(&self, line: usize) -> (u16, u16) {
        if line < self.zoom_line_x.len() {
            (self.zoom_line_x[line], self.zoom_line_y[line])
        } else {
            (self.zoom_x, self.zoom_y)
        }
    }

    pub(crate) fn control_line(&self, line: usize) -> u16 {
        if line < self.control_line.len() {
            self.control_line[line]
        } else {
            self.control_for_render()
        }
    }

    pub(crate) fn scroll_line_valid(&self, line: usize) -> bool {
        self.scroll_line_valid.get(line).copied().unwrap_or(false)
    }
}
