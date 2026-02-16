use crate::psg::Psg;
use crate::vce::Vce;
use crate::vdc::{
    Vdc,
    FRAME_WIDTH, FRAME_HEIGHT,
    VDC_DMA_WORD_CYCLES,
    TILE_WIDTH, TILE_HEIGHT, SPRITE_PATTERN_WIDTH, SPRITE_PATTERN_HEIGHT,
    SPRITE_PATTERN_WORDS, SPRITE_COUNT,
    VDC_CTRL_ENABLE_SPRITES_LEGACY, VDC_CTRL_ENABLE_BACKGROUND_LEGACY,
    VDC_CTRL_ENABLE_BACKGROUND, VDC_CTRL_ENABLE_SPRITES,
    DMA_CTRL_SRC_DEC, DMA_CTRL_DST_DEC,
};
#[cfg(test)]
use crate::vdc::{
    LINES_PER_FRAME, VDC_BUSY_ACCESS_CYCLES, VDC_VBLANK_INTERVAL,
    DMA_CTRL_IRQ_SATB, DMA_CTRL_IRQ_VRAM, DMA_CTRL_SATB_AUTO,
    VDC_VISIBLE_LINES,
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
#[cfg(test)]
const PHI_CYCLES_PER_SAMPLE: u32 = MASTER_CLOCK_HZ / AUDIO_SAMPLE_RATE;
const FONT: [[u8; 10]; 96] = [
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b01100000, 0b10010000, 0b10100000, 0b01000000, 0b10100010, 0b10010100, 0b10001010,
        0b01110010, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b01111110, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00111000, 0b01000100, 0b10000010, 0b10000010, 0b11111110, 0b10000010, 0b10000010,
        0b10000010, 0b10000010, 0b00000000,
    ],
    [
        0b11111100, 0b10000010, 0b10000010, 0b11111100, 0b10000010, 0b10000010, 0b10000010,
        0b11111100, 0b00000000, 0b00000000,
    ],
    [
        0b00111100, 0b01000010, 0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b01000010,
        0b00111100, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b11111110, 0b10000000, 0b10000000, 0b11111100, 0b10000000, 0b10000000, 0b10000000,
        0b11111110, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b10000010, 0b10000010, 0b10000010, 0b11111110, 0b10000010, 0b10000010, 0b10000010,
        0b10000010, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b10000100, 0b10001000, 0b10010000, 0b11100000, 0b10010000, 0b10001000, 0b10000100,
        0b10000010, 0b00000000, 0b00000000,
    ],
    [
        0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b10000000,
        0b11111110, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b10000010, 0b11000010, 0b10100010, 0b10010010, 0b10001010, 0b10000110, 0b10000010,
        0b10000010, 0b00000000, 0b00000000,
    ],
    [
        0b00111100, 0b01000010, 0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b01000010,
        0b00111100, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b11111110, 0b00010000, 0b00010000, 0b00010000, 0b00010000, 0b00010000, 0b00010000,
        0b00010000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
    [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000, 0b00000000, 0b00000000,
    ],
];


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
    #[cfg(feature = "trace_hw_writes")]
    last_pc_for_trace: Option<u16>,
    #[cfg(debug_assertions)]
    debug_force_ds_after: u64,
    #[cfg(feature = "trace_hw_writes")]
    st0_lock_window: u8,
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

/// Cached env-var flag: returns `true` only when the env var is set to `"1"`.
macro_rules! env_bool_eq1 {
    ($name:ident, $var:expr) => {
        #[inline]
        fn $name() -> bool {
            use std::sync::OnceLock;
            static V: OnceLock<bool> = OnceLock::new();
            *V.get_or_init(|| matches!(std::env::var($var), Ok(v) if v == "1"))
        }
    };
}

/// Cached env-var parsed as `i32` (decimal) with a default.
macro_rules! env_i32 {
    ($name:ident, $var:expr, $default:expr) => {
        #[inline]
        fn $name() -> i32 {
            use std::sync::OnceLock;
            static V: OnceLock<i32> = OnceLock::new();
            *V.get_or_init(|| {
                std::env::var($var)
                    .ok()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or($default)
            })
        }
    };
}

/// Cached env-var parsed as `Option<usize>` (with optional `> 0` filter).
macro_rules! env_option_usize {
    ($name:ident, $var:expr) => {
        fn $name() -> Option<usize> {
            use std::sync::OnceLock;
            static V: OnceLock<Option<usize>> = OnceLock::new();
            *V.get_or_init(|| {
                std::env::var($var)
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
            })
        }
    };
    ($name:ident, $var:expr, nonzero) => {
        fn $name() -> Option<usize> {
            use std::sync::OnceLock;
            static V: OnceLock<Option<usize>> = OnceLock::new();
            *V.get_or_init(|| {
                std::env::var($var)
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|&v| v > 0)
            })
        }
    };
}

impl Bus {
    env_bool!(env_force_mpr1_hardware, "PCE_FORCE_MPR1_HW");
    env_bool!(env_force_display_on,    "PCE_FORCE_DISPLAY_ON");
    env_bool!(env_fold_io_02xx,        "PCE_FOLD_IO_02XX");
    env_bool!(env_force_test_palette,  "PCE_FORCE_TEST_PALETTE");
    env_bool!(env_vce_catchall,        "PCE_VCE_CATCHALL");
    env_bool!(env_extreme_mirror,      "PCE_VDC_EXTREME_MIRROR");
    env_bool!(env_vdc_ultra_mirror,    "PCE_VDC_ULTRA_MIRROR");
    env_bool!(env_vdc_catchall,        "PCE_VDC_CATCHALL");
    env_bool!(env_timer_default_start, "PCE_TIMER_DEFAULT_START");
    env_bool!(env_force_palette_every_frame, "PCE_FORCE_PALETTE");
    env_bool!(env_bg_bit_lsb,         "PCE_BG_BIT_LSB");
    env_bool!(env_bg_swap_words,       "PCE_BG_SWAP_WORDS");
    env_bool!(env_bg_swap_bytes,       "PCE_BG_SWAP_BYTES");
    env_bool!(env_bg_plane_major,      "PCE_BG_PLANE_MAJOR");
    env_bool!(env_bg_tile12,           "PCE_BG_TILE12");
    env_bool!(env_bg_force_chr0_only,  "PCE_BG_CHR0_ONLY");
    env_bool!(env_bg_force_chr1_only,  "PCE_BG_CHR1_ONLY");
    env_bool!(env_bg_row_words,        "PCE_BG_ROW_WORDS");
    env_bool!(env_bg_force_tile0_zero, "PCE_BG_TILE0_ZERO");
    env_bool!(env_bg_palette_zero_visible, "PCE_BG_PAL0_VISIBLE");
    env_bool!(env_sprite_reverse_priority, "PCE_SPR_REVERSE_PRIORITY");
    env_bool!(env_no_sprite_line_limit, "PCE_NO_SPR_LINE_LIMIT");
    env_bool!(env_sprite_pattern_raw_index, "PCE_SPR_PATTERN_RAW");
    env_bool!(env_sprite_row_interleaved,   "PCE_SPR_ROW_INTERLEAVED");
    env_bool!(env_force_timer,         "PCE_FORCE_TIMER");
    env_bool!(env_force_vdc_dsdv,      "PCE_FORCE_VDC_DSDV");
    env_bool!(env_force_irq1,          "PCE_FORCE_IRQ1");
    env_bool!(env_force_irq2,          "PCE_FORCE_IRQ2");
    env_bool!(env_debug_bg_only,       "PCE_DEBUG_BG_ONLY");
    env_bool!(env_debug_spr_only,      "PCE_DEBUG_SPR_ONLY");
    env_bool!(env_force_cram_from_vram, "PCE_FORCE_CRAM_FROM_VRAM");

    #[cfg(feature = "trace_hw_writes")]
    env_bool!(env_trace_mpr, "PCE_TRACE_MPR");

    env_bool_eq1!(env_relax_io_mirror,     "PCE_RELAX_IO_MIRROR");
    env_bool_eq1!(env_force_title_now,     "PCE_FORCE_TITLE");
    env_bool_eq1!(env_vdc_force_hot_ports, "PCE_VDC_FORCE_HOT");
    env_bool_eq1!(env_force_title_scene,   "PCE_FORCE_TITLE_SCENE");

    env_i32!(env_bg_y_bias, "PCE_BG_Y_BIAS", 0);

    env_option_usize!(env_bg_map_height_override, "PCE_BG_MAP_H_TILES", nonzero);
    env_option_usize!(env_bg_map_width_override,  "PCE_BG_MAP_W_TILES", nonzero);
    env_option_usize!(env_sprite_max_entries,      "PCE_SPR_MAX_ENTRIES");

    // Special env helpers with custom parsing (hex u8, hex-or-decimal i32)
    #[inline]
    fn env_pad_default() -> u8 {
        use std::sync::OnceLock;
        static V: OnceLock<u8> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_PAD_DEFAULT")
                .ok()
                .and_then(|s| u8::from_str_radix(&s, 16).ok())
                .unwrap_or(0xFF)
        })
    }

    #[inline]
    fn env_irq_status_default() -> Option<u8> {
        use std::sync::OnceLock;
        static V: OnceLock<Option<u8>> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_IRQ_STATUS_DEFAULT")
                .ok()
                .and_then(|s| u8::from_str_radix(&s, 16).ok())
        })
    }

    fn env_bg_map_base_bias() -> i32 {
        use std::sync::OnceLock;
        static V: OnceLock<i32> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_BG_MAP_BASE_BIAS")
                .ok()
                .and_then(|s| {
                    i32::from_str_radix(&s, 16)
                        .ok()
                        .or_else(|| s.parse::<i32>().ok())
                })
                .unwrap_or(0)
        })
    }

    fn env_bg_tile_base_bias() -> i32 {
        use std::sync::OnceLock;
        static V: OnceLock<i32> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_BG_TILE_BASE_BIAS")
                .ok()
                .and_then(|s| {
                    i32::from_str_radix(&s, 16)
                        .ok()
                        .or_else(|| s.parse::<i32>().ok())
                })
                .unwrap_or(0)
        })
    }

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

    fn render_frame_from_vram(&mut self) {
        let display_width = self.compute_display_width();
        self.current_display_width = display_width;
        let (display_height, y_offset) = self.compute_display_height();
        self.current_display_height = display_height;
        self.current_display_y_offset = y_offset;
        self.vdc.clear_frame_trigger();
        let force_bg_only = Self::env_debug_bg_only();
        let force_spr_only = Self::env_debug_spr_only();
        let mut background_line_enabled = [false; FRAME_HEIGHT];
        let mut sprite_line_enabled = [false; FRAME_HEIGHT];
        let mut active_window_line = [false; FRAME_HEIGHT];
        for y in 0..FRAME_HEIGHT {
            let in_active_window = self.vdc.output_row_in_active_window(y);
            active_window_line[y] = in_active_window;
            if !in_active_window {
                continue;
            }
            let line_idx = self.vdc.line_state_index_for_frame_row(y);
            let ctrl = self.vdc.control_values_for_line(line_idx);
            let force_display_on = Self::env_force_display_on();
            let mut sprites_enabled =
                (ctrl & VDC_CTRL_ENABLE_SPRITES_LEGACY) != 0 || force_display_on;
            let mut background_enabled =
                (ctrl & VDC_CTRL_ENABLE_BACKGROUND_LEGACY) != 0 || force_display_on;
            if force_bg_only {
                sprites_enabled = false;
                background_enabled = true;
            }
            if force_spr_only {
                sprites_enabled = true;
                background_enabled = false;
            }
            background_line_enabled[y] = background_enabled;
            sprite_line_enabled[y] = sprites_enabled;
        }
        if !background_line_enabled.iter().any(|&enabled| enabled)
            && !sprite_line_enabled.iter().any(|&enabled| enabled)
        {
            // Burst mode: both BG and SPR are disabled for the entire frame.
            // The VDC does not drive any pixel data; the display is blank
            // (black).  VCE[0] is NOT used here — the VDC simply outputs no
            // colour information in burst mode.
            for y in 0..FRAME_HEIGHT {
                let row_start = y * FRAME_WIDTH;
                self.framebuffer[row_start..row_start + display_width].fill(0x000000);
            }
            self.frame_ready = true;
            return;
        }

        if self.vdc.vram.is_empty() {
            let background_colour = self.vce.palette_rgb(0);
            let overscan_colour = self.vce.palette_rgb(0x100);
            for y in 0..FRAME_HEIGHT {
                let row_start = y * FRAME_WIDTH;
                let row_end = row_start + display_width;
                let colour = if active_window_line[y] {
                    background_colour
                } else {
                    overscan_colour
                };
                self.framebuffer[row_start..row_end].fill(colour);
            }
            self.frame_ready = true;
            return;
        }

        #[derive(Clone, Copy, Default)]
        struct TileSample {
            chr0: u16,
            chr1: u16,
            tile_base: usize,
            palette_base: usize,
            priority: bool,
        }

        self.bg_opaque.fill(false);
        self.bg_priority.fill(false);
        for count in self.sprite_line_counts.iter_mut() {
            *count = 0;
        }
        self.vdc.clear_sprite_overflow();

        let background_colour = self.vce.palette_rgb(0);
        if Self::env_force_test_palette() {
            // デバッグ: パレットを簡易グラデーションに初期化
            for i in 0..self.vce.palette.len() {
                let v = i as u16;
                if let Some(slot) = self.vce.palette.get_mut(i) {
                    *slot = ((v & 0x0F) << 8) | ((v >> 4) & 0x0F) << 4 | (v & 0x0F);
                }
            }
        }
        if Self::env_force_palette_every_frame() {
            for i in 0..self.vce.palette.len() {
                let v = (i as u16) & 0x3FF;
                if let Some(slot) = self.vce.palette.get_mut(i) {
                    *slot = ((v & 0x0F) << 8) | (((v >> 4) & 0x0F) << 4) | (v & 0x0F);
                }
            }
        }
        if background_line_enabled.iter().any(|&enabled| enabled) {
            let mut tile_cache: Vec<TileSample> =
                Vec::with_capacity((display_width / TILE_WIDTH) + 2);
            let (map_width_tiles, map_height_tiles) = self.vdc.map_dimensions();
            let map_width = Self::env_bg_map_width_override()
                .unwrap_or(map_width_tiles)
                .max(1);
            let map_height = Self::env_bg_map_height_override()
                .unwrap_or(map_height_tiles)
                .max(1);
            let mwr = self.vdc.registers[0x09] as usize;
            let cg_mode_bit = (mwr >> 7) & 0x01;
            let pixel_width_mode = mwr & 0x03;
            let restrict_planes = pixel_width_mode == 0x03;
            let vram_mask = self.vdc.vram.len().saturating_sub(1);
            let vram_byte_mask = self.vdc.vram.len().saturating_mul(2).saturating_sub(1);
            let plane_major = Self::env_bg_plane_major();

            for y in 0..FRAME_HEIGHT {
                let line_state_index = self.vdc.line_state_index_for_frame_row(y);
                if !background_line_enabled[y] {
                    // BG disabled on this line: fill black.
                    // Covers overscan, burst mode (BG+SPR off), and
                    // sprite-only lines (BG off, SPR on) during transitions.
                    let row_start = y * FRAME_WIDTH;
                    let row_end = row_start + display_width;
                    self.framebuffer[row_start..row_end].fill(0x000000);
                    continue;
                }
                let _active_row = self.vdc.active_row_for_output_row(y).unwrap_or(0);
                if Self::env_force_test_palette() {
                    // パレットを毎行クリアして強制表示色を維持
                    for i in 0..self.vce.palette.len() {
                        let v = i as u16;
                        if let Some(slot) = self.vce.palette.get_mut(i) {
                            *slot = ((v & 0x0F) << 8) | (((v >> 4) & 0x0F) << 4) | (v & 0x0F);
                        }
                    }
                }
                let (x_scroll, y_scroll, y_offset) =
                    self.vdc.scroll_values_for_line(line_state_index);
                let (zoom_x_raw, zoom_y_raw) = self.vdc.zoom_values_for_line(line_state_index);
                let step_x = Vdc::zoom_step_value(zoom_x_raw);
                let step_y = Vdc::zoom_step_value(zoom_y_raw);
                // BG Y scroll: y_scroll is the latched BYR value, y_offset
                // is the number of active lines since BYR was last set.
                // The offset (not active_row) is used so that mid-frame
                // BYR writes (split-screen) produce the correct Y position.
                let y_origin_bias = 0i32;
                let effective_y_scroll = y_scroll as i32;
                let vram = &self.vdc.vram;
                let read_vram_byte = |byte_addr: usize| -> u8 {
                    let word = vram[(byte_addr >> 1) & vram_mask];
                    if (byte_addr & 1) == 0 {
                        (word & 0x00FF) as u8
                    } else {
                        (word >> 8) as u8
                    }
                };
                let swap_words = Self::env_bg_swap_words();
                let swap_bytes = Self::env_bg_swap_bytes();
                let bit_lsb = Self::env_bg_bit_lsb();
                let start_x_fp = (x_scroll as usize) << 4;
                let sample_y_fp =
                    ((effective_y_scroll + y_origin_bias) << 4) + (step_y as i32 * y_offset as i32);
                let sample_y = {
                    let raw = (sample_y_fp >> 4) + Self::env_bg_y_bias();
                    raw.rem_euclid((map_height * TILE_HEIGHT) as i32) as usize
                };
                let tile_row = (sample_y / TILE_HEIGHT) % map_height;
                let line_in_tile = (sample_y % TILE_HEIGHT) as usize;
                let start_sample_x = start_x_fp >> 4;
                let start_tile_int = start_sample_x / TILE_WIDTH;
                let end_sample_x_fp = start_x_fp + step_x * (display_width - 1);
                let end_sample_x = (end_sample_x_fp >> 4) + 1;
                let end_tile_int = (end_sample_x + TILE_WIDTH - 1) / TILE_WIDTH;
                let mut tiles_needed = end_tile_int.saturating_sub(start_tile_int) + 2;
                tiles_needed = tiles_needed.max(1);

                tile_cache.clear();
                tile_cache.reserve(tiles_needed);

                for tile_offset in 0..tiles_needed {
                    let tile_col = (start_tile_int + tile_offset) % map_width;
                    let map_addr = {
                        let raw = self.vdc.map_entry_address(tile_row, tile_col) as i32
                            + Self::env_bg_map_base_bias();
                        raw.rem_euclid(self.vdc.vram.len() as i32) as usize
                    };
                    let tile_entry = vram.get(map_addr & vram_mask).copied().unwrap_or(0);
                    let tile_mask = if Self::env_bg_tile12() {
                        0x0FFF
                    } else {
                        0x07FF
                    };
                    let tile_id = (tile_entry & tile_mask) as usize;
                    let palette_bank = ((tile_entry >> 12) & 0x0F) as usize;
                    let tile_base = ((tile_id as i32 * 16 + Self::env_bg_tile_base_bias())
                        .rem_euclid(self.vdc.vram.len() as i32))
                        as usize;
                    let row_index = line_in_tile;
                    let (row_addr_a, row_addr_b) = if Self::env_bg_row_words() {
                        let a = (tile_base + row_index * 2) & vram_mask;
                        (a, (a + 1) & vram_mask)
                    } else {
                        let a = (tile_base + row_index) & vram_mask;
                        (a, (a + 8) & vram_mask)
                    };
                    let mut chr_a = vram.get(row_addr_a).copied().unwrap_or(0);
                    let mut chr_b = vram.get(row_addr_b).copied().unwrap_or(0);
                    if swap_words {
                        std::mem::swap(&mut chr_a, &mut chr_b);
                    }
                    if Self::env_bg_force_chr0_only() {
                        chr_b = 0;
                    }
                    if Self::env_bg_force_chr1_only() {
                        chr_a = 0;
                    }
                    if Self::env_bg_force_tile0_zero() && tile_id == 0 {
                        chr_a = 0;
                        chr_b = 0;
                    }
                    if restrict_planes {
                        if cg_mode_bit == 0 {
                            chr_b = 0;
                        } else {
                            chr_a = 0;
                        }
                    }
                    tile_cache.push(TileSample {
                        chr0: chr_a,
                        chr1: chr_b,
                        tile_base,
                        palette_base: (palette_bank << 4) & 0x1F0,
                        priority: !Self::env_bg_tile12() && (tile_entry & 0x0800) != 0,
                    });
                }

                let mut sample_x_fp = start_x_fp;
                let start_tile_int = start_tile_int;
                for x in 0..display_width {
                    let screen_index = y * FRAME_WIDTH + x;
                    let sample_x = (sample_x_fp >> 4) as usize;
                    let tile_idx_int = sample_x / TILE_WIDTH;
                    let tile_offset = tile_idx_int.saturating_sub(start_tile_int);
                    let sample = tile_cache.get(tile_offset).copied().unwrap_or_default();
                    let intra_tile_x = sample_x % TILE_WIDTH;
                    let bit_index = intra_tile_x;
                    let shift = if bit_lsb { bit_index } else { 7 - bit_index };
                    let (plane0, plane1, plane2, plane3) = if plane_major {
                        let base_byte = (sample.tile_base << 1) & vram_byte_mask;
                        let row = line_in_tile;
                        let mut planes = [
                            read_vram_byte((base_byte + row) & vram_byte_mask),
                            read_vram_byte((base_byte + 8 + row) & vram_byte_mask),
                            read_vram_byte((base_byte + 16 + row) & vram_byte_mask),
                            read_vram_byte((base_byte + 24 + row) & vram_byte_mask),
                        ];
                        if swap_words {
                            planes.swap(0, 2);
                            planes.swap(1, 3);
                        }
                        if swap_bytes {
                            planes.swap(0, 1);
                            planes.swap(2, 3);
                        }
                        if restrict_planes {
                            if cg_mode_bit == 0 {
                                planes[2] = 0;
                                planes[3] = 0;
                            } else {
                                planes[0] = 0;
                                planes[1] = 0;
                            }
                        }
                        (
                            ((planes[0] >> shift) & 0x01) as u8,
                            ((planes[1] >> shift) & 0x01) as u8,
                            ((planes[2] >> shift) & 0x01) as u8,
                            ((planes[3] >> shift) & 0x01) as u8,
                        )
                    } else if swap_bytes {
                        (
                            ((sample.chr0 >> (shift + 8)) & 0x01) as u8,
                            ((sample.chr0 >> shift) & 0x01) as u8,
                            ((sample.chr1 >> (shift + 8)) & 0x01) as u8,
                            ((sample.chr1 >> shift) & 0x01) as u8,
                        )
                    } else {
                        (
                            ((sample.chr0 >> shift) & 0x01) as u8,
                            ((sample.chr0 >> (shift + 8)) & 0x01) as u8,
                            ((sample.chr1 >> shift) & 0x01) as u8,
                            ((sample.chr1 >> (shift + 8)) & 0x01) as u8,
                        )
                    };
                    let pixel = plane0 | (plane1 << 1) | (plane2 << 2) | (plane3 << 3);
                    if pixel == 0 {
                        if Self::env_bg_palette_zero_visible() {
                            let colour_idx = sample.palette_base & 0x1FF;
                            self.framebuffer[screen_index] = self.vce.palette_rgb(colour_idx);
                        } else {
                            self.framebuffer[screen_index] = background_colour;
                        }
                    } else {
                        self.bg_opaque[screen_index] = true;
                        self.bg_priority[screen_index] = sample.priority;
                        let colour_idx = (sample.palette_base | pixel as usize) & 0x1FF;
                        self.framebuffer[screen_index] = self.vce.palette_rgb(colour_idx);
                    }
                    sample_x_fp += step_x;
                }
            }
        } else {
            // No BG-enabled lines — all black (burst mode / sprite-only transitions).
            for y in 0..FRAME_HEIGHT {
                let row_start = y * FRAME_WIDTH;
                self.framebuffer[row_start..row_start + display_width].fill(0x000000);
            }
        }
        // Only render sprites when BG is active on at least one line.
        // When BG is entirely off (scene transitions: burst → sprite-only),
        // the game is still setting up VRAM/palette; rendering sprites here
        // would flash partially-loaded content.  Once BG is re-enabled the
        // scene is ready and sprites render normally.
        let any_bg = background_line_enabled.iter().any(|&e| e);
        if any_bg && sprite_line_enabled.iter().any(|&enabled| enabled) {
            self.render_sprites(&sprite_line_enabled);
        }

        self.frame_ready = true;
    }

    fn render_sprites(&mut self, line_enabled: &[bool; FRAME_HEIGHT]) {
        if self.vdc.vram.is_empty() {
            return;
        }
        #[derive(Clone, Copy)]
        struct LineSprite {
            x: i32,
            visible_width: usize,
            full_width: usize,
            src_tile_y: usize,
            row_in_tile: usize,
            pattern_base_index: usize,
            palette_base: usize,
            high_priority: bool,
            h_flip: bool,
            use_upper_cg_pair: bool,
        }

        let vram = &self.vdc.vram;
        let vram_mask = vram.len().saturating_sub(1);
        let mut overflow_detected = false;
        let mwr = self.vdc.registers[0x09];
        let sprite_dot_period = (mwr >> 2) & 0x03;
        let cg_mode_enabled = sprite_dot_period == 0x01;
        let reverse_priority = Self::env_sprite_reverse_priority();
        let no_sprite_line_limit = Self::env_no_sprite_line_limit();
        let pattern_raw_index = Self::env_sprite_pattern_raw_index();
        let row_interleaved = Self::env_sprite_row_interleaved();
        let sprite_max_entries = Self::env_sprite_max_entries().unwrap_or(SPRITE_COUNT);

        for dest_row in 0..FRAME_HEIGHT {
            if !line_enabled[dest_row] {
                continue;
            }
            let Some(active_row) = self.vdc.active_row_for_output_row(dest_row) else {
                continue;
            };
            let mut line_sprites = Vec::with_capacity(16);
            let mut slots_used = 0u8;
            let scanline_y = active_row as i32;

            for sprite_idx in 0..SPRITE_COUNT.min(sprite_max_entries) {
                let sprite = if reverse_priority {
                    SPRITE_COUNT - 1 - sprite_idx
                } else {
                    sprite_idx
                };
                let base = sprite * 4;
                let y_word = self.vdc.satb.get(base).copied().unwrap_or(0);
                let x_word = self.vdc.satb.get(base + 1).copied().unwrap_or(0);
                let pattern_word = self.vdc.satb.get(base + 2).copied().unwrap_or(0);
                let attr_word = self.vdc.satb.get(base + 3).copied().unwrap_or(0);

                // MAME uses (raster_count - 1 - sat_y) for sprite line calc,
                // effectively shifting sprites 1 line down relative to BG.
                // The +1 here matches that hardware pipeline delay.
                let y = (y_word & 0x03FF) as i32 - 64 + 1;
                let x = (x_word & 0x03FF) as i32 - 32;
                let width_cells = if (attr_word & 0x0100) != 0 {
                    2usize
                } else {
                    1usize
                };
                let height_code = ((attr_word >> 12) & 0x03) as usize;
                let height_cells = match height_code {
                    0 => 1,
                    1 => 2,
                    _ => 4,
                };
                let full_width = width_cells * SPRITE_PATTERN_WIDTH;
                let full_height = height_cells * SPRITE_PATTERN_HEIGHT;
                if scanline_y < y || scanline_y >= y + full_height as i32 {
                    continue;
                }

                if !no_sprite_line_limit && slots_used >= 16 {
                    overflow_detected = true;
                    continue;
                }
                // MAME: accepted sprites always render full width even when
                // pushing the slot count past 16 (a 32px sprite at slot 15
                // uses slots 15+16 and renders both cells fully).
                slots_used = slots_used.saturating_add(width_cells as u8);

                let mut pattern_base_index = if pattern_raw_index {
                    (pattern_word & 0x03FF) as usize
                } else {
                    ((pattern_word >> 1) & 0x03FF) as usize
                };
                if width_cells == 2 {
                    pattern_base_index &= !0x0001;
                }
                // MAME: each height-code bit independently masks a pattern bit.
                //   cgy bit 0 → mask pattern bit 1
                //   cgy bit 1 → mask pattern bit 2
                if height_code & 1 != 0 {
                    pattern_base_index &= !0x0002;
                }
                if height_code & 2 != 0 {
                    pattern_base_index &= !0x0004;
                }

                let v_flip = (attr_word & 0x8000) != 0;
                let local_y = (scanline_y - y) as usize;
                let src_y = if v_flip {
                    full_height - 1 - local_y
                } else {
                    local_y
                };
                let src_tile_y = src_y / SPRITE_PATTERN_HEIGHT;
                let row_in_tile = src_y % SPRITE_PATTERN_HEIGHT;

                line_sprites.push(LineSprite {
                    x,
                    visible_width: full_width,
                    full_width,
                    src_tile_y,
                    row_in_tile,
                    pattern_base_index,
                    palette_base: 0x100usize | (((attr_word & 0x000F) as usize) << 4),
                    high_priority: (attr_word & 0x0080) != 0,
                    h_flip: (attr_word & 0x0800) != 0,
                    use_upper_cg_pair: (pattern_word & 0x0001) != 0,
                });
            }

            self.sprite_line_counts[dest_row] = slots_used;

            for screen_x in 0..self.current_display_width {
                let offset = dest_row * FRAME_WIDTH + screen_x;
                for sprite in line_sprites.iter() {
                    if (screen_x as i32) < sprite.x
                        || (screen_x as i32) >= sprite.x + sprite.visible_width as i32
                    {
                        continue;
                    }

                    let local_x = (screen_x as i32 - sprite.x) as usize;
                    let src_x = if sprite.h_flip {
                        sprite.full_width - 1 - local_x
                    } else {
                        local_x
                    };
                    let src_tile_x = src_x / SPRITE_PATTERN_WIDTH;
                    let col_in_tile = src_x % SPRITE_PATTERN_WIDTH;
                    let pattern_index = sprite.pattern_base_index
                        + sprite.src_tile_y * 2
                        + src_tile_x;
                    let pattern_base = (pattern_index * SPRITE_PATTERN_WORDS) & vram_mask;

                    let (plane0_word, plane1_word, plane2_word, plane3_word) = if row_interleaved {
                        let row_base = (pattern_base + sprite.row_in_tile * 4) & vram_mask;
                        (
                            vram[row_base],
                            vram[(row_base + 1) & vram_mask],
                            vram[(row_base + 2) & vram_mask],
                            vram[(row_base + 3) & vram_mask],
                        )
                    } else {
                        (
                            vram[(pattern_base + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 16 + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 32 + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 48 + sprite.row_in_tile) & vram_mask],
                        )
                    };
                    let shift = 15usize.saturating_sub(col_in_tile);
                    let mut plane0 = ((plane0_word >> shift) & 0x01) as u8;
                    let mut plane1 = ((plane1_word >> shift) & 0x01) as u8;
                    let mut plane2 = ((plane2_word >> shift) & 0x01) as u8;
                    let mut plane3 = ((plane3_word >> shift) & 0x01) as u8;

                    if cg_mode_enabled {
                        if sprite.use_upper_cg_pair {
                            plane0 = plane2;
                            plane1 = plane3;
                            plane2 = 0;
                            plane3 = 0;
                        } else {
                            plane2 = 0;
                            plane3 = 0;
                        }
                    }

                    let pixel = plane0 | (plane1 << 1) | (plane2 << 2) | (plane3 << 3);
                    if pixel == 0 {
                        continue;
                    }

                    let bg_opaque = self.bg_opaque[offset];
                    let bg_forces_front = self.bg_priority[offset];
                    if !bg_opaque || (sprite.high_priority && !bg_forces_front) {
                        let colour_index = (sprite.palette_base | pixel as usize) & 0x1FF;
                        self.framebuffer[offset] = self.vce.palette_rgb(colour_index);
                    }
                    // The first opaque sprite pixel wins, regardless of BG blend result.
                    break;
                }
            }
        }

        if overflow_detected {
            self.vdc.raise_status(VDC_STATUS_OR);
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

    fn env_route_02xx_hw() -> bool {
        use std::sync::OnceLock;
        static FLAG: OnceLock<bool> = OnceLock::new();
        *FLAG.get_or_init(|| match std::env::var("PCE_ROUTE_02XX_HW") {
            Ok(v) if v == "0" => false,
            _ => true, // default: route 0x0200–0x021F to hardware
        })
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
#[cfg(test)]
use crate::psg::PSG_WAVE_SIZE;
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
mod tests {
    use super::*;
    use crate::psg::*;

    const VCE_ADDRESS_ADDR: u16 = 0x0402;
    const VCE_ADDRESS_HIGH_ADDR: u16 = 0x0403;
    const VCE_DATA_ADDR: u16 = 0x0404;
    const VCE_DATA_HIGH_ADDR: u16 = 0x0405;
    const PSG_ADDR_REG: u16 = 0x1C60;
    const PSG_WRITE_REG: u16 = 0x1C61;
    const PSG_READ_REG: u16 = 0x1C62;
    const PSG_STATUS_REG: u16 = 0x1C63;
    const TIMER_STD_BASE: u16 = 0x0C00;
    const JOYPAD_BASE_ADDR: u16 = 0x1000;
    const IRQ_TIMER_BASE: u16 = 0x1400;
    const CPU_IRQ_MASK: u16 = 0x1402;
    const CPU_IRQ_STATUS: u16 = 0x1403;
    const VDC_CTRL_DISPLAY_FULL: u16 = VDC_CTRL_ENABLE_BACKGROUND
        | VDC_CTRL_ENABLE_BACKGROUND_LEGACY
        | VDC_CTRL_ENABLE_SPRITES
        | VDC_CTRL_ENABLE_SPRITES_LEGACY;

    fn set_vdc_control(bus: &mut Bus, value: u16) {
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, (value & 0x00FF) as u8);
        bus.write_st_port(2, (value >> 8) as u8);
    }

    fn prepare_bus_for_zoom() -> Bus {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        const MAP_WIDTH: usize = 32;
        for col in 0..MAP_WIDTH {
            let tile_id = 0x100 + col;
            let palette_bank = (col & 0x0F) as usize;
            bus.vdc.vram[col] = ((tile_id as u16) & 0x07FF) | ((palette_bank as u16) << 12);
            let base = (tile_id * 16) & 0x7FFF;
            for row in 0..8 {
                bus.vdc.vram[(base + row) & 0x7FFF] = 0x00FF;
                bus.vdc.vram[(base + row + 8) & 0x7FFF] = 0x0000;
            }
        }

        for bank in 0..16 {
            let colour = (bank as u16) * 0x041;
            bus.vce.palette[(bank << 4) | 1] = colour;
        }

        bus
    }

    fn render_zoom_pair(zoom_x: u16) -> ([u32; FRAME_WIDTH], [u32; FRAME_WIDTH]) {
        let mut baseline = prepare_bus_for_zoom();
        baseline.render_frame_from_vram();
        let mut zoomed = prepare_bus_for_zoom();
        zoomed.vdc.set_zoom_for_test(zoom_x, 0x0010);
        zoomed.render_frame_from_vram();

        let mut base_line = [0u32; FRAME_WIDTH];
        let mut zoom_line = [0u32; FRAME_WIDTH];
        base_line.copy_from_slice(&baseline.framebuffer[0..FRAME_WIDTH]);
        zoom_line.copy_from_slice(&zoomed.framebuffer[0..FRAME_WIDTH]);
        (base_line, zoom_line)
    }

    fn prepare_bus_for_vertical_zoom() -> Bus {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        const MAP_WIDTH: usize = 32;
        for row in 0..32 {
            let tile_id = 0x200 + row * MAP_WIDTH;
            let palette_bank = (row & 0x0F) as usize;
            for col in 0..MAP_WIDTH {
                let idx = row * MAP_WIDTH + col;
                bus.vdc.vram[idx] = ((tile_id as u16) & 0x07FF) | ((palette_bank as u16) << 12);
            }
            let base = (tile_id * 16) & 0x7FFF;
            for line in 0..8 {
                bus.vdc.vram[(base + line) & 0x7FFF] = 0x00FF;
                bus.vdc.vram[(base + line + 8) & 0x7FFF] = 0x0000;
            }
        }

        for bank in 0..16 {
            let colour = 0x0100 | ((bank as u16) * 0x021);
            bus.vce.palette[(bank << 4) | 1] = colour;
        }

        bus
    }

    fn render_vertical_zoom_pair(zoom_y: u16) -> (Vec<u32>, Vec<u32>) {
        let mut baseline = prepare_bus_for_vertical_zoom();
        baseline.render_frame_from_vram();
        let mut zoomed = prepare_bus_for_vertical_zoom();
        zoomed.vdc.set_zoom_for_test(0x0010, zoom_y);
        zoomed.render_frame_from_vram();
        (baseline.framebuffer.clone(), zoomed.framebuffer.clone())
    }

    #[test]
    fn load_and_bank_switch_rom() {
        let mut bus = Bus::new();
        bus.load(0x0000, &[0xAA, 0xBB]);
        assert_eq!(bus.read(0x0000), 0xAA);

        bus.load_rom_image(vec![0x10; PAGE_SIZE * 2]);
        bus.map_bank_to_rom(4, 1);
        assert_eq!(bus.read(0x8000), 0x10);

        bus.write(0x8000, 0x77); // ignored because ROM
        assert_eq!(bus.read(0x8000), 0x10);

        bus.map_bank_to_ram(4, 0);
        bus.write(0x8000, 0x12);
        assert_eq!(bus.read(0x8000), 0x12);
    }

    #[test]
    fn mpr_mirrors_apply_across_high_page() {
        let mut bus = Bus::new();
        bus.load_rom_image(vec![0x55; PAGE_SIZE * 2]);

        // MPR registers at $FF80-$FFBF are only accessible when MPR7
        // maps to the hardware page ($FF).
        bus.set_mpr(7, 0xFF);

        // 0xFF95 mirrors MPR5
        bus.write(0xFF95, (bus.total_ram_pages() + 1) as u8);
        assert_eq!(bus.mpr(5), (bus.total_ram_pages() + 1) as u8);

        // ROM page 1 is filled with 0x55
        assert_eq!(bus.read(0xA000), 0x55);

        // Reading from a mirror location returns the same register value.
        assert_eq!(bus.read(0xFFAD), bus.mpr(5));
    }

    #[test]
    fn io_port_reads_selected_joypad_nibble() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        bus.set_joypad_input(0x5A);

        // SEL=1 -> d-pad (lower nibble of input)
        bus.write(JOYPAD_BASE_ADDR, 0x01);
        assert_eq!(bus.read(JOYPAD_BASE_ADDR) & 0x0F, 0x0A);

        // SEL=0 -> buttons (upper nibble of input)
        bus.write(JOYPAD_BASE_ADDR, 0x00);
        assert_eq!(bus.read(JOYPAD_BASE_ADDR) & 0x0F, 0x05);
    }

    #[test]
    fn st_ports_store_values() {
        let mut bus = Bus::new();
        bus.write_st_port(0, 0x12);
        bus.write_st_port(1, 0x34);
        bus.write_st_port(2, 0x56);
        assert_eq!(bus.st_port(0), 0x12);
        assert_eq!(bus.st_port(1), 0x34);
        assert_eq!(bus.st_port(2), 0x56);
    }

    #[test]
    fn io_registers_round_trip_and_reset() {
        let mut bus = Bus::new();
        // Test I/O register round-trip via direct read_io/write_io
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x20), 0);
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x7F), 0);

        bus.write_io(HW_CPU_CTRL_BASE + 0x20, 0xAA);
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x20), 0xAA);
        bus.write_io(HW_CPU_CTRL_BASE + 0x7F, 0x55);
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x7F), 0x55);

        bus.write_io(HW_CPU_CTRL_BASE + 0x30, 0x42);
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x30), 0x42);

        bus.clear();
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x20), 0x00);
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x30), 0x00);
        assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x7F), 0x00);
    }

    #[test]
    fn timer_borrow_sets_request_bit() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        bus.write(0x0C00, 0x02); // reload value
        bus.write(0x0C01, TIMER_CONTROL_START);

        let fired = bus.tick(1024u32 * 3, true);
        assert!(fired);
        assert_eq!(bus.read(0x1403) & IRQ_REQUEST_TIMER, IRQ_REQUEST_TIMER);

        bus.write(0x1403, IRQ_REQUEST_TIMER);
        assert_eq!(bus.read(0x1403) & IRQ_REQUEST_TIMER, 0);
    }

    #[test]
    fn timer_accessible_via_standard_io_offset() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(TIMER_STD_BASE, 0x02);
        bus.write(TIMER_STD_BASE + 1, TIMER_CONTROL_START);

        let fired = bus.tick(1024u32 * 3, true);
        assert!(fired);
        assert_eq!(
            bus.read(CPU_IRQ_STATUS) & IRQ_REQUEST_TIMER,
            IRQ_REQUEST_TIMER
        );

        bus.write(CPU_IRQ_STATUS, IRQ_REQUEST_TIMER);
        assert_eq!(bus.read(CPU_IRQ_STATUS) & IRQ_REQUEST_TIMER, 0);
    }

    #[test]
    fn irq_registers_not_aliased_to_timer() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(TIMER_STD_BASE, 0x05);
        bus.write(TIMER_STD_BASE + 1, TIMER_CONTROL_START);
        assert_eq!(bus.read(TIMER_STD_BASE + 1) & TIMER_CONTROL_START, 1);

        bus.write(IRQ_TIMER_BASE, 0xAA);
        bus.write(IRQ_TIMER_BASE + 1, 0x55);

        assert_eq!(bus.read(IRQ_TIMER_BASE), 0xAA);
        assert_eq!(bus.read(IRQ_TIMER_BASE + 1), 0x55);
        assert_eq!(bus.read(TIMER_STD_BASE + 1) & TIMER_CONTROL_START, 1);
    }

    #[test]
    fn hardware_page_irq_registers_alias_cpu_space() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(IRQ_TIMER_BASE + 0x02, 0xFF);
        assert_eq!(
            bus.read(CPU_IRQ_MASK),
            IRQ_DISABLE_IRQ2 | IRQ_DISABLE_IRQ1 | IRQ_DISABLE_TIMER
        );

        bus.write(CPU_IRQ_MASK, 0x00);
        bus.write(IRQ_TIMER_BASE + 0x03, IRQ_REQUEST_TIMER);
        assert_eq!(bus.read(CPU_IRQ_STATUS) & IRQ_REQUEST_TIMER, 0);
    }

    #[test]
    fn cart_ram_banks_map_into_memory_space() {
        let mut bus = Bus::new();
        bus.configure_cart_ram(PAGE_SIZE * 2);

        let cart_base = 0x80u8;
        bus.set_mpr(2, cart_base);
        bus.write(0x4000, 0x5A);
        assert_eq!(bus.cart_ram[0], 0x5A);
        assert_eq!(bus.read(0x4000), 0x5A);

        bus.set_mpr(2, cart_base + 1);
        bus.write(0x4000, 0xCC);
        assert_eq!(bus.cart_ram[PAGE_SIZE], 0xCC);
        assert_eq!(bus.read(0x4000), 0xCC);

        bus.set_mpr(2, cart_base);
        assert_eq!(bus.read(0x4000), 0x5A);
    }

    #[test]
    fn cart_ram_load_and_snapshot_round_trip() {
        let mut bus = Bus::new();
        bus.configure_cart_ram(PAGE_SIZE);
        let pattern = vec![0xAB; PAGE_SIZE];
        assert!(bus.load_cart_ram(&pattern).is_ok());
        assert_eq!(bus.cart_ram().unwrap()[0], 0xAB);
        let cart_base = 0x80u8;
        bus.set_mpr(2, cart_base);
        let cart_addr = 0x4000u16;
        assert_eq!(bus.read(cart_addr), 0xAB);

        if let Some(data) = bus.cart_ram_mut() {
            data.fill(0x11);
        }
        assert_eq!(bus.read(cart_addr), 0x11);
    }

    #[test]
    fn sprite_priority_respects_background_mask() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);
        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        const BG_TILE_ID: usize = 200;
        const SPRITE_PATTERN_ID: usize = 201;
        const BG_PALETTE_BANK: usize = 1;
        const SPRITE_PALETTE_BANK: usize = 2;

        for entry in bus.vdc.vram.iter_mut().take(32 * 32) {
            *entry = ((BG_TILE_ID as u16) & 0x07FF) | ((BG_PALETTE_BANK as u16) << 12);
        }

        let bg_base = BG_TILE_ID * 16;
        for row in 0..8 {
            bus.vdc.vram[bg_base + row] = 0xFFFF;
            bus.vdc.vram[bg_base + 8 + row] = 0xFFFF;
        }

        write_constant_sprite_tile(&mut bus, SPRITE_PATTERN_ID, 0x01);

        bus.vce.palette[0x1F] = 0x001F;
        bus.vce.palette[0x121] = 0x03E0;

        bus.render_frame_from_vram();
        let bg_colour = bus.framebuffer[0];
        assert_ne!(bg_colour, 0);
        assert!(bus.bg_opaque[0]);

        let satb_index = 0;
        // SAT Y=63 puts sprite at screen row 0 (with the +1 pipeline offset)
        let y_word = ((0 + 63) & 0x03FF) as u16;
        let x_word = ((0 + 32) & 0x03FF) as u16;
        bus.vdc.satb[satb_index] = y_word;
        bus.vdc.satb[satb_index + 1] = x_word;
        bus.vdc.satb[satb_index + 2] = (SPRITE_PATTERN_ID as u16) << 1;
        bus.vdc.satb[satb_index + 3] = SPRITE_PALETTE_BANK as u16;

        bus.render_frame_from_vram();
        assert_eq!(bus.framebuffer[0], bg_colour);

        bus.vdc.satb[satb_index + 3] |= 0x0080;
        bus.render_frame_from_vram();
        let sprite_colour = bus.vce.palette_rgb(0x121);
        assert_eq!(bus.framebuffer[0], sprite_colour);
    }

    fn write_constant_sprite_tile(bus: &mut Bus, pattern_index: usize, value: u8) {
        let base = (pattern_index * SPRITE_PATTERN_WORDS) & 0x7FFF;
        let plane0 = if value & 0x01 != 0 { 0xFFFF } else { 0x0000 };
        let plane1 = if value & 0x02 != 0 { 0xFFFF } else { 0x0000 };
        let plane2 = if value & 0x04 != 0 { 0xFFFF } else { 0x0000 };
        let plane3 = if value & 0x08 != 0 { 0xFFFF } else { 0x0000 };
        for row in 0..SPRITE_PATTERN_HEIGHT {
            bus.vdc.vram[(base + row) & 0x7FFF] = plane0;
            bus.vdc.vram[(base + 16 + row) & 0x7FFF] = plane1;
            bus.vdc.vram[(base + 32 + row) & 0x7FFF] = plane2;
            bus.vdc.vram[(base + 48 + row) & 0x7FFF] = plane3;
        }
    }

    #[test]
    fn sprites_render_when_background_disabled() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.vce.palette[0x00] = 0x0000;
        bus.vce.palette[0x100] = 0x0000;
        bus.vce.palette[0x101] = 0x7C00;

        write_constant_sprite_tile(&mut bus, 0, 0x01);

        let sprite_y = 32;
        let sprite_x = 24;
        // SAT Y = screen_y + 63 to account for +1 pipeline offset
        bus.vdc.satb[0] = ((sprite_y + 63) & 0x03FF) as u16;
        bus.vdc.satb[1] = ((sprite_x + 32) & 0x03FF) as u16;
        bus.vdc.satb[2] = 0x0000;
        bus.vdc.satb[3] = 0x0000;

        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x40);
        bus.write_st_port(2, 0x00);

        bus.render_frame_from_vram();

        let background_colour = bus.vce.palette_rgb(0x00);
        assert_eq!(bus.framebuffer[0], background_colour);

        let sprite_index = sprite_y as usize * FRAME_WIDTH + sprite_x as usize;
        let sprite_colour = bus.vce.palette_rgb(0x101);
        assert_eq!(bus.framebuffer[sprite_index], sprite_colour);
        assert!(
            bus.framebuffer.iter().any(|&pixel| pixel == sprite_colour),
            "expected sprite colour to appear in framebuffer"
        );
    }

    #[test]
    fn sprite_double_width_draws_all_columns() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        const BASE_PATTERN: usize = 0x200;
        bus.vce.palette[0x101] = 0x0111;
        bus.vce.palette[0x102] = 0x0222;
        write_constant_sprite_tile(&mut bus, BASE_PATTERN, 0x01);
        write_constant_sprite_tile(&mut bus, BASE_PATTERN + 1, 0x02);

        let sprite_base = 0;
        let sprite_y = 32;
        let sprite_x = 24;
        // SAT Y = screen_y + 63 to account for +1 pipeline offset
        bus.vdc.satb[sprite_base] = ((sprite_y + 63) & 0x03FF) as u16;
        bus.vdc.satb[sprite_base + 1] = ((sprite_x + 32) & 0x03FF) as u16;
        bus.vdc.satb[sprite_base + 2] = (BASE_PATTERN as u16) << 1;
        bus.vdc.satb[sprite_base + 3] = 0x0100 | 0x0080;

        bus.render_frame_from_vram();

        let row_start = sprite_y * FRAME_WIDTH + sprite_x;
        let left = bus.framebuffer[row_start];
        let right = bus.framebuffer[row_start + 16];
        assert_eq!(left, bus.vce.palette_rgb(0x101));
        assert_eq!(right, bus.vce.palette_rgb(0x102));
    }

    #[test]
    fn sprite_scanline_overflow_sets_status() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        const TILE_ID: usize = 0x400;
        write_constant_sprite_tile(&mut bus, TILE_ID, 0x01);

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        let y_pos = 48;
        for sprite in 0..17 {
            let base = sprite * 4;
            let x_pos = sprite as i32 * 8;
            bus.vdc.satb[base] = ((y_pos + 63) & 0x03FF) as u16;
            bus.vdc.satb[base + 1] = ((x_pos + 32) & 0x03FF) as u16;
            bus.vdc.satb[base + 2] = (TILE_ID as u16) << 1;
            bus.vdc.satb[base + 3] = 0x0000;
        }

        bus.render_frame_from_vram();
        let max_count = bus
            .sprite_line_counts_for_test()
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        assert_eq!(max_count, 16);
        assert_ne!(bus.vdc.status_bits() & VDC_STATUS_OR, 0);

        let overflow_sprite = 16 * 4;
        bus.vdc.satb[overflow_sprite] = 0;
        bus.vdc.satb[overflow_sprite + 1] = 0;
        bus.vdc.satb[overflow_sprite + 2] = 0;
        bus.vdc.satb[overflow_sprite + 3] = 0;

        bus.render_frame_from_vram();
        assert_eq!(bus.vdc.status_bits() & VDC_STATUS_OR, 0);
    }

    #[test]
    fn sprite_size_scaling_plots_full_extent() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        const BASE_TILE: usize = 0x300;
        const WIDTH_UNITS: usize = 2;
        const HEIGHT_UNITS: usize = 2;
        const WIDTH_TILES: usize = WIDTH_UNITS;
        const HEIGHT_TILES: usize = HEIGHT_UNITS;

        for tile in 0..(WIDTH_TILES * HEIGHT_TILES) {
            write_constant_sprite_tile(&mut bus, BASE_TILE + tile, 0x0F);
        }

        let sprite_colour = 0x7C00;
        bus.vce.palette[0x12F] = sprite_colour;

        let x_pos = 40;
        let y_pos = 32;
        let satb_index = 0;
        // SAT Y = screen_y + 63 to account for +1 pipeline offset
        bus.vdc.satb[satb_index] = ((y_pos + 63) & 0x03FF) as u16;
        bus.vdc.satb[satb_index + 1] = ((x_pos + 32) & 0x03FF) as u16;
        bus.vdc.satb[satb_index + 2] = (BASE_TILE as u16) << 1;
        bus.vdc.satb[satb_index + 3] = 0x1000 | 0x0100 | 0x0002;

        bus.render_frame_from_vram();

        let colour = bus.vce.palette_rgb(0x12F);
        let idx = (y_pos + HEIGHT_UNITS * 16 - 1) * FRAME_WIDTH + (x_pos + WIDTH_UNITS * 16 - 1);
        assert_eq!(bus.framebuffer[idx], colour);
    }

    #[test]
    fn sprite_quad_height_plots_bottom_row() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        const BASE_TILE: usize = 0x320;
        const TILES_WIDE: usize = 1;
        const TILES_HIGH: usize = 4;

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        for cell_y in 0..TILES_HIGH {
            for cell_x in 0..TILES_WIDE {
                write_constant_sprite_tile(&mut bus, BASE_TILE + cell_y * 2 + cell_x, 0x0F);
            }
        }

        let sprite_colour = 0x03FF;
        bus.vce.palette[0x10F] = sprite_colour;

        let x_pos = 24;
        let y_pos = 40;
        let satb_index = 0;
        // SAT Y = screen_y + 63 to account for +1 pipeline offset
        bus.vdc.satb[satb_index] = ((y_pos + 63) & 0x03FF) as u16;
        bus.vdc.satb[satb_index + 1] = ((x_pos + 32) & 0x03FF) as u16;
        bus.vdc.satb[satb_index + 2] = (BASE_TILE as u16) << 1;
        bus.vdc.satb[satb_index + 3] = 0x2000;

        bus.render_frame_from_vram();

        let expected = bus.vce.palette_rgb(0x10F);
        let drawn_pixels = bus
            .framebuffer
            .iter()
            .filter(|&&pixel| pixel == expected)
            .count();
        assert!(drawn_pixels > 0);
        let top_row = &bus.framebuffer[y_pos * FRAME_WIDTH..(y_pos + 1) * FRAME_WIDTH];
        assert!(top_row.iter().any(|&pixel| pixel == expected));
        let bottom_row = &bus.framebuffer[(y_pos + 63) * FRAME_WIDTH..(y_pos + 64) * FRAME_WIDTH];
        assert!(bottom_row.iter().any(|&pixel| pixel == expected));
    }

    #[test]
    fn scroll_registers_latch_on_scanline_boundary() {
        let mut vdc = Vdc::new();
        let (x0, y0) = vdc.scroll_for_scanline();
        assert_eq!(x0, 0);
        assert_eq!(y0, 0);

        vdc.write_select(0x07);
        vdc.write_data_low(0x34);
        vdc.write_data_high(0x12);
        let (x1, y1) = vdc.scroll_for_scanline();
        assert_eq!(x1, 0x1234 & 0x03FF);
        assert_eq!(y1, 0);

        vdc.write_select(0x08);
        vdc.write_data_low(0x78);
        vdc.write_data_high(0x05);
        let (x2, y2) = vdc.scroll_for_scanline();
        assert_eq!(x2, x1);
        assert_eq!(y2, 0x0578 & 0x01FF);

        let (x3, y3) = vdc.scroll_for_scanline();
        assert_eq!(x3, x2);
        assert_eq!(y3, y2);
    }

    #[test]
    fn scroll_writes_apply_on_next_visible_scanline() {
        let mut vdc = Vdc::new();
        vdc.advance_scanline_for_test();
        let (x0, _, _) = vdc.scroll_values_for_line(0);
        assert_eq!(x0, 0);

        vdc.write_select(0x07);
        vdc.write_data_low(0x34);
        vdc.write_data_high(0x12);

        let (x_still, _, _) = vdc.scroll_values_for_line(0);
        assert_eq!(x_still, 0);

        vdc.advance_scanline_for_test();
        let (x1, _, _) = vdc.scroll_values_for_line(1);
        assert_eq!(x1, 0x1234 & 0x03FF);

        let (x_now, _) = vdc.scroll_for_scanline();
        assert_eq!(x_now, 0x1234 & 0x03FF);
    }

    #[test]
    fn register_select_between_low_and_high_uses_current_ar() {
        let mut vdc = Vdc::new();

        vdc.write_select(0x07); // AR = BXR
        vdc.write_data_low(0x34); // BXR low byte committed immediately → BXR = 0x0034
        // On real HuC6270, changing AR between ST1 and ST2 means the
        // high-byte commit targets the NEW register, not the old one.
        // Per MAME, each register has its own data latch (m_vdc_data[]),
        // so the low byte 0x34 stays in BXR's latch, NOT BYR's.
        vdc.write_select(0x08); // AR = BYR
        vdc.write_data_high_direct(0x12); // commits vdc_data[8]=(0x12,0x00) to BYR

        // BXR only received the low-byte commit (0x0034).
        assert_eq!(vdc.registers[0x07], 0x0034);
        // BYR received (high=0x12, low=0x00 from its own latch), masked to 9 bits.
        // The 0x34 from BXR's ST1 does NOT leak to BYR's data latch.
        assert_eq!(vdc.registers[0x08], 0x1200 & 0x01FF);
    }

    #[test]
    fn vdc_vertical_window_uses_vpr_vdw_vcr() {
        let mut vdc = Vdc::new();
        vdc.registers[0x0C] = 0x0F02;
        vdc.registers[0x0D] = 0x00EF;
        vdc.registers[0x0E] = 0x0003;

        let window = vdc.vertical_window();
        assert_eq!(window.active_start_line, 0x14);
        assert_eq!(window.active_line_count, 0x0F0);
        assert_eq!(window.post_active_overscan_lines, 6);
        assert_eq!(window.vblank_start_line, 260);
        assert_eq!(vdc.vblank_start_scanline(), 260);
    }

    #[test]
    fn vdc_output_row_active_window_honours_vdw_vcr_gap() {
        let mut vdc = Vdc::new();
        vdc.registers[0x0C] = 0x0100;
        vdc.registers[0x0D] = 0x0003; // 4 active lines
        vdc.registers[0x0E] = 0x0002; // 5 overscan lines (VCR + 3)

        for row in 0..4 {
            assert!(
                vdc.output_row_in_active_window(row),
                "row {row} should be active in first display pass"
            );
        }
        for row in 4..13 {
            assert!(
                !vdc.output_row_in_active_window(row),
                "row {row} should be overscan"
            );
        }
        for row in 13..17 {
            assert!(
                vdc.output_row_in_active_window(row),
                "row {row} should be active after display-counter reset"
            );
        }
    }

    #[test]
    fn line_state_index_tracks_vpr_active_start() {
        let mut vdc = Vdc::new();
        vdc.registers[0x0C] = 0x0302; // VSW=2, VDS=3 => start line (2+1)+(3+2)=8
        vdc.registers[0x0D] = 0x0001;

        assert_eq!(vdc.line_state_index_for_frame_row(0), 8);
        assert_eq!(
            vdc.line_state_index_for_frame_row(239),
            (8 + 239) % (LINES_PER_FRAME as usize)
        );
    }

    #[test]
    fn rcr_scanline_uses_active_start_offset() {
        let mut vdc = Vdc::new();
        // VPR: VSW=2 VDS=15 → active_start = vsw+1+vds+2 = 20
        vdc.registers[0x0C] = 0x0F02;
        vdc.registers[0x0D] = 0x00EF;
        vdc.registers[0x0E] = 0x0003;

        // target 0x40: counter = 0x40 at active_start → line = 20
        assert_eq!(vdc.rcr_scanline_for_target(0x0040), Some(20));
        // target 0x63: line = 20 + 35 = 55 (Kato-chan HUD split)
        assert_eq!(vdc.rcr_scanline_for_target(0x0063), Some(55));
        // target below 0x40: not reachable
        assert_eq!(vdc.rcr_scanline_for_target(0x0002), None);
    }

    #[test]
    fn map_dimensions_follow_mwr_width_height_bits() {
        let mut vdc = Vdc::new();

        vdc.registers[0x09] = 0x0000;
        assert_eq!(vdc.map_dimensions(), (32, 32));

        vdc.registers[0x09] = 0x0010;
        assert_eq!(vdc.map_dimensions(), (64, 32));

        vdc.registers[0x09] = 0x0020;
        assert_eq!(vdc.map_dimensions(), (128, 32));

        vdc.registers[0x09] = 0x0030;
        assert_eq!(vdc.map_dimensions(), (128, 32));

        vdc.registers[0x09] = 0x0050;
        assert_eq!(vdc.map_dimensions(), (64, 64));
    }

    #[test]
    fn map_entry_address_64x64_flat() {
        let mut vdc = Vdc::new();
        vdc.registers[0x09] = 0x0050; // 64x64

        // HuC6270 BAT uses flat row-major addressing (MAME/Mednafen):
        //   address = row * map_width + col
        assert_eq!(vdc.map_entry_address_for_test(0, 0), 0x0000);
        // Row 1, col 0: 1*64 = 64
        assert_eq!(vdc.map_entry_address_for_test(1, 0), 64);
        // Row 0, col 31: 31
        assert_eq!(vdc.map_entry_address_for_test(0, 31), 31);
        // Row 0, col 32: 32
        assert_eq!(vdc.map_entry_address_for_test(0, 32), 32);
        // Row 31, col 0: 31*64 = 1984
        assert_eq!(vdc.map_entry_address_for_test(31, 0), 31 * 64);
        // Row 32, col 0: 32*64 = 2048
        assert_eq!(vdc.map_entry_address_for_test(32, 0), 32 * 64);
        // Row 32, col 32: 32*64+32 = 2080
        assert_eq!(vdc.map_entry_address_for_test(32, 32), 32 * 64 + 32);
    }

    #[test]
    fn bat_always_starts_at_vram_zero() {
        let mut vdc = Vdc::new();
        for mwr in [0x0000u16, 0x0010, 0x0050, 0x1150, 0xFF50] {
            vdc.registers[0x09] = mwr;
            assert_eq!(vdc.map_base_address(), 0, "mwr={mwr:04X}");
        }
    }

    #[test]
    fn bg_disabled_when_cr_bit7_clear() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Top-left BAT cell points to a visible tile.
        bus.vdc.vram[0x0000] = 0x0001;
        for row in 0..8usize {
            bus.vdc.vram[0x0010 + row] = if row == 0 { 0x0080 } else { 0x0000 };
            bus.vdc.vram[0x0018 + row] = 0x0000;
        }
        bus.vce.palette[0x001] = 0x01FF;

        // BG bit (CR bit7) is clear, while increment bits 11-12 are set.
        set_vdc_control(&mut bus, VDC_CTRL_ENABLE_SPRITES_LEGACY | (0b11 << 11));

        bus.render_frame_from_vram();

        assert_eq!(bus.framebuffer[0], bus.vce.palette_rgb(0));
        assert!(!bus.bg_opaque[0]);
    }

    #[test]
    fn tile_entry_zero_uses_tile_zero_pattern_data() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // BAT (row0,col1) is entry value 0 => tile 0.
        bus.vdc.vram[0x0001] = 0x0000;
        // Tile 0 row 0 plane data (overlaps BAT area by hardware design).
        bus.vdc.vram[0x0000] = 0x0080;
        bus.vdc.vram[0x0008] = 0x0000;
        bus.vce.palette[0x001] = 0x01FF;

        set_vdc_control(&mut bus, VDC_CTRL_ENABLE_BACKGROUND_LEGACY);
        bus.render_frame_from_vram();

        assert_eq!(bus.framebuffer[8], bus.vce.palette_rgb(0x001));
        assert!(bus.bg_opaque[8]);
    }

    #[test]
    fn renderer_honours_vertical_window_overscan_rows() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Configure visible/overscan colours.
        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x01);
        bus.write(VCE_DATA_ADDR, 0x3F);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x11);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x3F);

        // Single opaque tile in the top-left BAT cell (palette 1, tile 1).
        // Tile pixels = colour index 1 → VCE[0x11] = 0x003F (non-black).
        write_vram_word(&mut bus, 0x0000, 0x1001);
        for row in 0..8u16 {
            // Plane 0 lo=0xFF, plane 1 hi=0x00 → index bit 0 set, bit 1 clear = index 1
            write_vram_word(&mut bus, 0x0010 + row, 0x00FF);
            write_vram_word(&mut bus, 0x0018 + row, 0x0000);
        }

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);
        bus.write_st_port(0, 0x0C);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x01);
        bus.write_st_port(0, 0x0D);
        bus.write_st_port(1, 0x03); // 4 active lines
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x0E);
        bus.write_st_port(1, 0x02); // 5 overscan lines
        bus.write_st_port(2, 0x00);

        bus.render_frame_from_vram();
        // VDW=3 → 4 active output rows (0..3), rows 4+ are overscan → black.
        let active_pixel = bus.framebuffer[0]; // first active line
        let overscan_pixel = bus.framebuffer[6 * FRAME_WIDTH]; // post-active overscan
        assert_ne!(active_pixel, 0x000000, "active line should render tile data");
        assert_eq!(overscan_pixel, 0x000000, "overscan lines should be black");
    }

    #[test]
    fn background_horizontal_zoom_scales_source() {
        let mut baseline = prepare_bus_for_zoom();
        baseline.render_frame_from_vram();
        let base0 = baseline.framebuffer[0];
        let base8 = baseline.framebuffer[8];
        let base16 = baseline.framebuffer[16];
        assert_ne!(base0, base8);
        assert_ne!(base8, base16);

        let mut zoomed = prepare_bus_for_zoom();
        zoomed.vdc.set_zoom_for_test(0x08, 0x0010);
        zoomed.render_frame_from_vram();
        let zoom0 = zoomed.framebuffer[0];
        let zoom16 = zoomed.framebuffer[16];
        let zoom32 = zoomed.framebuffer[32];
        assert_eq!(zoom0, base0);
        assert_eq!(zoom16, base8);
        assert_eq!(zoom32, base16);
    }

    #[test]
    fn background_horizontal_zoom_shrinks_source() {
        let (baseline, zoomed) = render_zoom_pair(0x18);
        assert_eq!(zoomed[0], baseline[0]);
        assert_eq!(zoomed[16], baseline[24]);
    }

    #[test]
    fn background_horizontal_zoom_extreme_zoom_in() {
        let (baseline, zoomed) = render_zoom_pair(0x01);
        let colour = baseline[0];
        for x in 0..16 {
            assert_eq!(zoomed[x], colour);
        }
    }

    #[test]
    fn background_horizontal_zoom_extreme_shrink() {
        let (baseline, zoomed) = render_zoom_pair(0x1F);
        assert_eq!(zoomed[0], baseline[0]);
        assert_eq!(zoomed[8], baseline[15]);
        assert_eq!(zoomed[16], baseline[31]);
    }

    #[test]
    fn background_priority_bit_sets_bg_priority_mask() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        const TILE_ID: usize = 0x180;
        let tile_entry = (TILE_ID as u16) & 0x07FF;
        bus.vdc.vram[0] = tile_entry;
        bus.vdc.vram[1] = tile_entry | 0x0800;

        let tile_base = (TILE_ID * 16) & 0x7FFF;
        bus.vdc.vram[(tile_base) & 0x7FFF] = 0x0080;
        for row in 1..8 {
            bus.vdc.vram[(tile_base + row) & 0x7FFF] = 0;
        }
        for row in 0..8 {
            bus.vdc.vram[(tile_base + row + 8) & 0x7FFF] = 0;
        }

        bus.vce.palette[0x01] = 0x7C00;

        bus.render_frame_from_vram();
        let colour = bus.vce.palette_rgb(0x01);
        let bg = bus.vce.palette_rgb(0x00);

        assert_eq!(bus.framebuffer[0], colour);
        assert_eq!(bus.framebuffer[8], colour);
        assert_eq!(bus.framebuffer[1], bg);
        assert!(!bus.bg_priority[0]);
        assert!(bus.bg_priority[8]);
    }

    #[test]
    fn background_priority_overrides_sprite_pixels() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        bus.write(VCE_ADDRESS_ADDR, 0x10);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x3F);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        bus.write(VCE_ADDRESS_ADDR, 0x20);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x3F);

        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x10);
        bus.write_st_port(2, 0x00);

        let tile_index = 0x0100u16;
        let priority_entry = tile_index | 0x1000 | 0x0800;
        let addr_priority = bus.vdc.map_entry_address_for_test(0, 0) as u16;
        write_vram_word(&mut bus, addr_priority, priority_entry);

        let addr_plain = bus.vdc.map_entry_address_for_test(0, 1) as u16;
        write_vram_word(&mut bus, addr_plain, tile_index | 0x1000);

        let tile_addr = tile_index * 16;
        write_vram_word(&mut bus, tile_addr, 0x0080);
        for offset in 1..16 {
            write_vram_word(&mut bus, tile_addr + offset as u16, 0x0000);
        }

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        bus.write_st_port(0, 0x07);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x08);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        bus.render_frame_from_vram();
        assert!(bus.bg_priority[0]);
        assert!(!bus.bg_priority[8]);
    }

    #[test]
    fn background_vertical_zoom_scales_source() {
        let (baseline, zoomed) = render_vertical_zoom_pair(0x08);
        assert_ne!(baseline[0], baseline[8 * FRAME_WIDTH]);
        assert_ne!(baseline[8 * FRAME_WIDTH], baseline[16 * FRAME_WIDTH]);
        assert_eq!(zoomed[0], baseline[0]);
        assert_eq!(zoomed[16 * FRAME_WIDTH], baseline[8 * FRAME_WIDTH]);
        assert_eq!(zoomed[32 * FRAME_WIDTH], baseline[16 * FRAME_WIDTH]);
    }

    #[test]
    fn background_vertical_zoom_extreme_zoom_in() {
        let (baseline, zoomed) = render_vertical_zoom_pair(0x01);
        let colour = baseline[0];
        for y in 0..16 {
            assert_eq!(zoomed[y * FRAME_WIDTH], colour);
        }
    }

    #[test]
    fn background_vertical_zoom_extreme_shrink() {
        let (baseline, zoomed) = render_vertical_zoom_pair(0x1F);
        assert_eq!(zoomed[0], baseline[0]);
        assert_eq!(zoomed[8 * FRAME_WIDTH], baseline[15 * FRAME_WIDTH]);
        assert_eq!(zoomed[16 * FRAME_WIDTH], baseline[31 * FRAME_WIDTH]);
    }

    #[test]
    fn timer_disable_masks_irq_line() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        bus.write(0x0C00, 0x01);
        bus.write(0x1402, IRQ_DISABLE_TIMER);
        bus.write(0x0C01, TIMER_CONTROL_START);

        let fired = bus.tick(1024u32 * 2, true);
        assert!(!fired);
        assert_eq!(bus.read(0x1403) & IRQ_REQUEST_TIMER, IRQ_REQUEST_TIMER);

        bus.write(0x1402, 0x00);
        assert!(bus.tick(0, true));
        bus.write(0x1403, IRQ_REQUEST_TIMER);
        assert!(!bus.tick(0, true));
    }

    #[test]
    fn timer_uses_slow_clock_divider() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        bus.write(0x0C00, 0x00);
        bus.write(0x0C01, TIMER_CONTROL_START);

        let fired = bus.tick(256u32, false);
        assert!(fired);
    }

    #[test]
    fn hardware_page_routes_vdc_registers() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write_st_port(0, 0x05); // select control register
        bus.write_st_port(1, 0x08);
        bus.write_st_port(2, 0x00);

        assert_eq!(bus.vdc_register(5), Some(0x0008));
    }

    #[test]
    fn io_space_mirror_routes_vdc_and_vce() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        bus.set_mpr(1, 0xFF);

        // VCE palette write via 0x2000-mirrored address space.
        bus.write(0x2402, 0x00); // address low
        bus.write(0x2403, 0x00); // address high
        bus.write(0x2404, 0x56); // data low
        bus.write(0x2405, 0x34); // data high
        assert_eq!(bus.vce_palette_word(0x0000), 0x3456);

        // VDC register select/data via mirrored offsets inside 0x0000-0x03FF.
        bus.write(0x2201, 0x05); // select control register (odd address mirror)
        assert_eq!(bus.st_port(0), 0x05);

        // Use a higher-offset mirror (0x2202/0x2203) to exercise the 0x100-spaced mirrors.
        bus.write(0x2202, 0xAA); // low byte (ST1 mirror)
        bus.write(0x2203, 0x00); // high byte via ST2 mirror
        assert_eq!(bus.vdc_register(5), Some(0x00AA));
    }

    #[test]
    fn hardware_page_status_read_clears_irq() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Enable VBlank interrupt and raise the status flag.
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x08);
        bus.write_st_port(2, 0x00);
        bus.vdc_set_status_for_test(VDC_STATUS_VBL);
        assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);

        let status = bus.read_io(0x00);
        assert!(status & VDC_STATUS_VBL != 0);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
        assert_eq!(bus.read_io(0x00) & VDC_STATUS_VBL, 0);
    }

    #[test]
    fn vce_palette_write_and_read_round_trip() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Select palette index 0x0010.
        bus.write(VCE_ADDRESS_ADDR, 0x10);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);

        bus.write(VCE_DATA_ADDR, 0x34);
        bus.write(VCE_DATA_HIGH_ADDR, 0x12);

        assert_eq!(bus.vce_palette_word(0x0010), 0x1234);

        // Reading back should return the stored value and advance the index.
        bus.write(VCE_ADDRESS_ADDR, 0x10);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        let lo = bus.read(VCE_DATA_ADDR);
        let hi = bus.read(VCE_DATA_HIGH_ADDR);
        assert_eq!(lo, 0x34);
        assert_eq!(hi, 0x12);
        assert_eq!(bus.vce_palette_word(0x0011), 0);
    }

    #[test]
    fn vce_sequential_writes_auto_increment_index() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);

        for i in 0..4u16 {
            let value = 0x1000 | i;
            bus.write(VCE_DATA_ADDR, (value & 0x00FF) as u8);
            bus.write(VCE_DATA_HIGH_ADDR, (value >> 8) as u8);
        }

        assert_eq!(bus.vce_palette_word(0), 0x1000);
        assert_eq!(bus.vce_palette_word(1), 0x1001);
        assert_eq!(bus.vce_palette_word(2), 0x1002);
        assert_eq!(bus.vce_palette_word(3), 0x1003);
    }

    #[test]
    fn hardware_page_psg_accesses_data_ports() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        // Use the standard PSG address range ($0800-$0BFF) which is within
        // the real I/O decode range.  The legacy $1C60 mirror is only
        // reachable via write_io/read_io_internal (offsets $1800+ now fall
        // through to ROM in the read path).
        bus.write(0x0800, 0x02); // channel select = 2
        bus.write(0x0805, 0x7F); // channel balance = 0x7F
        bus.write(0x0800, 0x02); // re-select channel 2
        assert_eq!(bus.read(0x0805), 0x7F);
        assert_eq!(bus.psg.channels[2].balance, 0x7F);
    }

    #[test]
    fn hardware_page_psg_direct_register_map_is_available() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(0x0800, 0x02); // channel select
        bus.write(0x0805, 0x7F); // channel balance for selected channel
        assert_eq!(bus.psg.channels[2].balance, 0x7F);
        assert_eq!(bus.read(0x0805), 0x7F);
    }

    #[test]
    fn vce_palette_rgb_applies_brightness_and_channels() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Select palette index zero.
        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);

        // Write palette word with G=3, R=5, B=7 (raw layout: GGGRRRBBB).
        let raw = (0x3 << 6) | (0x5 << 3) | 0x7;
        bus.write(VCE_DATA_ADDR, (raw & 0xFF) as u8);
        bus.write(VCE_DATA_HIGH_ADDR, (raw >> 8) as u8);

        let rgb = bus.vce_palette_rgb(0);
        let r = ((rgb >> 16) & 0xFF) as u8;
        let g = ((rgb >> 8) & 0xFF) as u8;
        let b = (rgb & 0xFF) as u8;

        assert_eq!(r, (0x5 * 255 / 0x07) as u8);
        assert_eq!(g, (0x3 * 255 / 0x07) as u8);
        assert_eq!(b, 255);
    }

    #[cfg(test)]
    fn write_vram_word(bus: &mut Bus, addr: u16, value: u16) {
        bus.write_st_port(0, 0x00);
        bus.write_st_port(1, (addr & 0x00FF) as u8);
        bus.write_st_port(2, ((addr >> 8) & 0x7F) as u8);
        bus.write_st_port(0, 0x02);
        bus.write_st_port(1, (value & 0x00FF) as u8);
        bus.write_st_port(2, (value >> 8) as u8);
    }

    #[cfg(test)]
    fn fetch_frame(bus: &mut Bus, steps: u32) -> Vec<u32> {
        for _ in 0..(steps.saturating_mul(2)) {
            bus.tick(1, true);
            if let Some(frame) = bus.take_frame() {
                return frame;
            }
        }
        panic!("expected frame output");
    }

    #[test]
    fn render_blank_frame_uses_palette_zero() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Write a vivid palette entry at index 0.
        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        let raw_colour = 0x01FF; // full intensity (R=7,G=7,B=7)
        bus.write(VCE_DATA_ADDR, (raw_colour & 0x00FF) as u8);
        bus.write(VCE_DATA_HIGH_ADDR, (raw_colour >> 8) as u8);

        // Enable VBlank IRQ so tick processing advances display timing.
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x08);
        bus.write_st_port(2, 0x00);

        // Run long enough to hit VBlank.
        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
        for _ in 0..steps {
            bus.tick(1, true);
        }

        let frame = bus.take_frame().expect("expected frame after VBlank");
        assert_eq!(frame.len(), bus.display_width() * bus.display_height());
        // With both BG and SPR disabled the VDC is in burst mode — the screen
        // should be black (0x000000) regardless of VCE[0].
        assert!(frame.iter().all(|&pixel| pixel == 0x000000));
    }

    #[test]
    fn render_frame_uses_vram_palette_indices() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Palette index 0 -> background colour.
        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        // Palette index 0x10 -> black, 0x11 -> bright red.
        bus.write(VCE_ADDRESS_ADDR, 0x10);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x11);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x38); // red max
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        // Write tile map entry at VRAM 0 pointing to tile index 0x0100 with palette bank 1.
        let tile_index: u16 = 0x0100;
        let map_entry = tile_index | 0x1000;
        let map_addr = bus.vdc.map_entry_address_for_test(0, 0) as u16;
        write_vram_word(&mut bus, map_addr, map_entry);

        // Write a simple tile at tile index 0x0100: first pixel uses colour 1, others 0.
        let tile_addr = tile_index * 16;
        write_vram_word(&mut bus, tile_addr, 0x0080);
        for offset in 1..16 {
            let addr = tile_addr.wrapping_add(offset as u16);
            write_vram_word(&mut bus, addr, 0x0000);
        }

        // Enable background and configure scroll.
        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x10);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x07); // X scroll
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x08); // Y scroll
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x88);
        bus.write_st_port(2, 0x80);
        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
        for _ in 0..steps {
            bus.tick(1, true);
        }

        let frame = bus.take_frame().expect("expected frame");
        assert_eq!(frame.len(), bus.display_width() * bus.display_height());
        let colour1 = bus.vce_palette_rgb(0x11);
        let colour0 = bus.vce_palette_rgb(0x00);
        assert_eq!(frame[0], colour1);
        assert_eq!(frame[1], colour0);
    }

    #[test]
    fn render_frame_respects_map_size_and_scroll() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Configure palette entries.
        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x10);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x11);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x38);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        // Configure virtual map to 64x32 and scroll so tile column 40 appears at x=0.
        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x10);
        bus.write_st_port(2, 0x00);
        let scroll_x = 40 * TILE_WIDTH as u16;
        bus.write_st_port(0, 0x07);
        bus.write_st_port(1, (scroll_x & 0xFF) as u8);
        bus.write_st_port(2, ((scroll_x >> 8) & 0x03) as u8);
        bus.write_st_port(0, 0x08);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        // Write map entry for column 40 with palette bank 1.
        let tile_index: u16 = 0x0100;
        let map_entry = tile_index | 0x1000;
        let map_addr = bus.vdc.map_entry_address_for_test(0, 40) as u16;
        write_vram_word(&mut bus, map_addr, map_entry);

        // Tile pattern data for tile 0x0100.
        let tile_addr = tile_index * 16;
        write_vram_word(&mut bus, tile_addr, 0x0080);
        for offset in 1..16 {
            let addr = tile_addr.wrapping_add(offset as u16);
            write_vram_word(&mut bus, addr, 0x0000);
        }

        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x88);
        bus.write_st_port(2, 0x80);

        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
        for _ in 0..steps {
            bus.tick(1, true);
        }

        let frame = bus.take_frame().expect("expected frame");
        assert_eq!(frame.len(), bus.display_width() * bus.display_height());
        let colour1 = bus.vce_palette_rgb(0x11);
        let colour0 = bus.vce_palette_rgb(0x00);
        assert_eq!(frame[0], colour1);
        assert_eq!(frame[1], colour0);
    }

    #[test]
    fn render_frame_honours_map_base_offset() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x11);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x38);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x50);
        bus.write_st_port(2, 0x0A);

        bus.write_st_port(0, 0x07);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x08);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        let tile_index: u16 = 0x0100;
        let map_entry = tile_index | 0x1000;
        let map_addr = bus.vdc.map_entry_address_for_test(0, 0) as u16;
        write_vram_word(&mut bus, map_addr, map_entry);

        let tile_addr = tile_index * 16;
        write_vram_word(&mut bus, tile_addr, 0x0080);
        for offset in 1..16 {
            let addr = tile_addr.wrapping_add(offset as u16);
            write_vram_word(&mut bus, addr, 0x0000);
        }

        set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
        let frame = fetch_frame(&mut bus, steps);
        let colour = bus.vce_palette_rgb(0x11);
        assert_eq!(frame[0], colour);
    }

    #[test]
    fn render_frame_respects_cg_mode_restricted_planes() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        // Configure palettes: index 0 = background, 0x14 = visible colour.
        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x14);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x38); // bright red
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        // Map tile 0x0100 at origin using palette bank 1.
        let tile_index: u16 = 0x0100;
        let map_entry = tile_index | 0x1000;
        let map_addr = bus.vdc.map_entry_address_for_test(0, 0) as u16;
        write_vram_word(&mut bus, map_addr, map_entry);

        // Tile data: only plane2 bit set so colour index = 4.
        let tile_addr = tile_index * 16;
        write_vram_word(&mut bus, tile_addr, 0x0000);
        write_vram_word(&mut bus, tile_addr + 8, 0x0080);
        for offset in 1..16 {
            if offset == 8 {
                continue;
            }
            let addr = tile_addr.wrapping_add(offset as u16);
            write_vram_word(&mut bus, addr, 0x0000);
        }

        // Scroll to origin and enable background.
        bus.write_st_port(0, 0x07);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x08);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        // Use restricted CG mode with CM=0 (only CG0 valid).
        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x03);
        bus.write_st_port(2, 0x00);

        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x88);
        bus.write_st_port(2, 0x80);

        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
        let frame = fetch_frame(&mut bus, steps);
        let bg_colour = bus.vce_palette_rgb(0x00);
        assert_eq!(
            frame[0], bg_colour,
            "plane2 data should be ignored when CM=0"
        );

        // Switch to CM=1 and rerun a frame; plane2 data should now be visible.
        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x83);
        bus.write_st_port(2, 0x00);

        let frame_cm1 = fetch_frame(&mut bus, steps);
        let colour_plane2 = bus.vce_palette_rgb(0x14);
        assert_eq!(
            frame_cm1[0], colour_plane2,
            "plane2 data should produce colour when CM=1"
        );
    }

    #[test]
    fn render_frame_wraps_horizontally_on_64x64_map() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x11);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x38);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x50);
        bus.write_st_port(2, 0x00);

        let tile_index: u16 = 0x0100;
        let map_entry = tile_index | 0x1000;
        let map_addr = bus.vdc.map_entry_address_for_test(0, 63) as u16;
        write_vram_word(&mut bus, map_addr, map_entry);

        let tile_addr = tile_index * 16;
        write_vram_word(&mut bus, tile_addr, 0x0080);
        for offset in 1..16 {
            let addr = tile_addr.wrapping_add(offset as u16);
            write_vram_word(&mut bus, addr, 0x0000);
        }

        let scroll_x = 63 * TILE_WIDTH as u16;
        bus.write_st_port(0, 0x07);
        bus.write_st_port(1, (scroll_x & 0xFF) as u8);
        bus.write_st_port(2, ((scroll_x >> 8) & 0x03) as u8);
        bus.write_st_port(0, 0x08);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x88);
        bus.write_st_port(2, 0x80);

        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
        let frame = fetch_frame(&mut bus, steps);
        let expected = bus.vce_palette_rgb(0x11);
        assert_eq!(
            frame[0], expected,
            "scrolled column 63 should appear at x=0"
        );
        assert_eq!(
            frame[TILE_WIDTH],
            bus.vce_palette_rgb(0x00),
            "next column should wrap to column 0 background"
        );
    }

    #[test]
    fn render_frame_wraps_vertically_on_64x64_map() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);

        bus.write(VCE_ADDRESS_ADDR, 0x00);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x00);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);
        bus.write(VCE_ADDRESS_ADDR, 0x11);
        bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
        bus.write(VCE_DATA_ADDR, 0x38);
        bus.write(VCE_DATA_HIGH_ADDR, 0x00);

        bus.write_st_port(0, 0x09);
        bus.write_st_port(1, 0x50);
        bus.write_st_port(2, 0x00);

        let tile_index: u16 = 0x0100;
        let map_entry = tile_index | 0x1000;
        let map_addr = bus.vdc.map_entry_address_for_test(63, 0) as u16;
        write_vram_word(&mut bus, map_addr, map_entry);

        let tile_addr = tile_index * 16;
        write_vram_word(&mut bus, tile_addr, 0x0080);
        for offset in 1..16 {
            let addr = tile_addr.wrapping_add(offset as u16);
            write_vram_word(&mut bus, addr, 0x0000);
        }

        bus.write_st_port(0, 0x07);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        let scroll_y = 63 * TILE_HEIGHT as u16;
        bus.write_st_port(0, 0x08);
        bus.write_st_port(1, (scroll_y & 0xFF) as u8);
        bus.write_st_port(2, ((scroll_y >> 8) & 0x01) as u8);

        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x88);
        bus.write_st_port(2, 0x80);

        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
        let frame = fetch_frame(&mut bus, steps);
        assert_eq!(
            frame[0],
            bus.vce_palette_rgb(0x11),
            "scrolled row 63 should appear at y=0"
        );
        assert_eq!(
            frame[bus.display_width() * TILE_HEIGHT],
            bus.vce_palette_rgb(0x00),
            "next row should wrap to row 0 background"
        );
    }

    #[test]
    fn vdc_vblank_flag_clears_during_display() {
        let mut bus = Bus::new();
        bus.read_io(0x00); // clear initial flags.

        let mut seen_high = false;
        let mut saw_cleared_after = false;
        for _ in 0..(LINES_PER_FRAME as usize * 4) {
            bus.tick(500, true);
            let status = bus.read_io(0x00);
            if status & VDC_STATUS_VBL != 0 {
                seen_high = true;
            } else if seen_high {
                saw_cleared_after = true;
                break;
            }
        }
        assert!(seen_high, "VBlank status bit never asserted");
        assert!(
            saw_cleared_after,
            "VBlank status bit never cleared after asserting"
        );
    }

    #[test]
    fn vdc_vblank_flag_returns_after_display() {
        let mut bus = Bus::new();
        bus.read_io(0x00); // clear initial flags.

        let mut phase = 0;
        let mut seen_second_high = false;
        for _ in 0..(LINES_PER_FRAME as usize * 4) {
            bus.tick(500, true);
            let status = bus.read_io(0x00);
            match phase {
                0 => {
                    if status & VDC_STATUS_VBL != 0 {
                        phase = 1;
                    }
                }
                1 => {
                    if status & VDC_STATUS_VBL == 0 {
                        phase = 2;
                    }
                }
                _ => {
                    if status & VDC_STATUS_VBL != 0 {
                        seen_second_high = true;
                        break;
                    }
                }
            }
        }
        assert!(
            seen_second_high,
            "VBlank status bit never asserted again after clearing"
        );
    }

    #[test]
    fn vdc_tick_holds_on_first_frame_trigger_for_large_cycle_chunk() {
        let mut vdc = Vdc::new();
        vdc.scanline = 0;
        vdc.in_vblank = false;
        vdc.frame_trigger = false;
        vdc.scroll_line_valid.fill(false);
        let frame_cycles = VDC_VBLANK_INTERVAL;

        // One large chunk can cover more than a full frame worth of scanline steps.
        // We should stop at the first VBlank/frame trigger and preserve latched line state.
        let _ = vdc.tick(frame_cycles);
        assert!(vdc.frame_ready(), "expected frame trigger after large tick");
        // Frame trigger now fires at the last active scanline (one before VBlank),
        // so that mid-frame VRAM writes are captured by the batch renderer.
        assert_eq!(
            vdc.scanline,
            VDC_VISIBLE_LINES - 1,
            "scanline should stop one line before VBlank"
        );
        assert!(vdc.scroll_line_valid[1], "line 1 should be latched");
        assert!(
            vdc.scroll_line_valid[(VDC_VISIBLE_LINES - 1) as usize],
            "last active line should be latched"
        );
        assert!(
            !vdc.scroll_line_valid[VDC_VISIBLE_LINES as usize],
            "VBlank line should remain unlatched until frame is consumed"
        );
    }

    #[test]
    fn vdc_register_write_sequence() {
        let mut bus = Bus::new();
        bus.write_st_port(0, 0x00); // MAWR
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x04);
        assert_eq!(bus.vdc_register(0), Some(0x0400));

        bus.write_st_port(0, 0x02); // VRAM data
        bus.write_st_port(0, 0x02); // select VRAM data port
        bus.write_st_port(1, 0x34);
        bus.write_st_port(2, 0x12);
        assert_eq!(bus.vdc_vram_word(0x0400), 0x1234);
        assert_eq!(bus.vdc_register(0), Some(0x0401));

        // Subsequent data write should auto-increment MAWR
        bus.write_st_port(1, 0x78);
        bus.write_st_port(2, 0x56);
        assert_eq!(bus.vdc_vram_word(0x0401), 0x5678);
        assert_eq!(bus.vdc_register(0), Some(0x0402));
    }

    #[test]
    fn vdc_status_initial_vblank_and_clear() {
        let mut bus = Bus::new();
        let status = bus.read_io(0x00);
        assert!(status & VDC_STATUS_VBL != 0);
        let status_after = bus.read_io(0x00);
        assert_eq!(status_after & VDC_STATUS_VBL, 0);
    }

    #[test]
    fn vdc_vblank_irq_raises_when_enabled() {
        let mut bus = Bus::new();
        bus.set_mpr(1, 0xFF);
        // Clear the initial VBlank state.
        bus.read_io(0x00);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);

        // Enable VBlank IRQ (bit 3 of control register).
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x08);
        bus.write_st_port(2, 0x00);

        for _ in 0..400 {
            bus.tick(200, false);
        }

        assert!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1 != 0);
        let status = bus.read_io(0x00);
        assert!(status & VDC_STATUS_VBL != 0);
        bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_status_interrupt_respects_control() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        bus.set_mpr(1, 0xFF);

        // Enable VBlank IRQ (bit 3 of control register).
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x08);
        bus.write_st_port(2, 0x00);

        bus.vdc_set_status_for_test(VDC_STATUS_VBL);
        assert_eq!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, IRQ_REQUEST_IRQ1);

        let status = bus.read(0x2000);
        assert_eq!(status & VDC_STATUS_VBL, VDC_STATUS_VBL);
        bus.write(0x1403, IRQ_REQUEST_IRQ1);
        assert_eq!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);

        // Disable VBlank interrupt and ensure no IRQ is raised.
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        bus.vdc_set_status_for_test(VDC_STATUS_VBL);
        bus.write(0x1403, IRQ_REQUEST_IRQ1);
        bus.vdc_set_status_for_test(VDC_STATUS_VBL);
        assert_eq!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_vram_increment_uses_control_bits() {
        let mut bus = Bus::new();

        bus.write_st_port(0, 0x00); // MAWR = 0
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        // Set increment mode to 32 (INC field = 01b at bits 12..11).
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x08);

        bus.write_st_port(0, 0x02); // VRAM data write
        bus.write_st_port(1, 0xAA);
        bus.write_st_port(2, 0x55);
        assert_eq!(bus.vdc_vram_word(0x0000), 0x55AA);
        assert_eq!(bus.vdc_register(0), Some(0x0020));

        bus.write_st_port(1, 0xBB);
        bus.write_st_port(2, 0x66);
        assert_eq!(bus.vdc_vram_word(0x0020), 0x66BB);
        assert_eq!(bus.vdc_register(0), Some(0x0040));
    }

    #[test]
    fn vdc_vram_reads_prefetch_and_increment() {
        let mut bus = Bus::new();
        bus.set_mpr(1, 0xFF);

        // Populate VRAM with two words.
        bus.write_st_port(0, 0x00); // MAWR = 0
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x02);
        bus.write_st_port(1, 0x34);
        bus.write_st_port(2, 0x12);
        bus.write_st_port(1, 0x78);
        bus.write_st_port(2, 0x56);

        assert_eq!(bus.vdc_vram_word(0x0000), 0x1234);
        assert_eq!(bus.vdc_vram_word(0x0001), 0x5678);
        assert_eq!(bus.vdc_register(0), Some(0x0002));

        // Point VRR to zero.
        bus.write_st_port(0, 0x01); // MARR
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x02); // select VRAM data register for reads

        let lo = bus.read(0x2002);
        assert_eq!(lo, 0x34);
        assert_eq!(bus.vdc_register(1), Some(0x0000));

        let hi = bus.read(0x2003);
        assert_eq!(hi, 0x12);
        assert_eq!(bus.vdc_register(1), Some(0x0001));

        let next_lo = bus.read(0x2002);
        assert_eq!(next_lo, 0x78);
        let next_hi = bus.read(0x2003);
        assert_eq!(next_hi, 0x56);
        assert_eq!(bus.vdc_register(1), Some(0x0002));
    }

    #[test]
    fn vdc_data_low_port_always_returns_low_byte() {
        let mut bus = Bus::new();
        bus.set_mpr(1, 0xFF);

        write_vram_word(&mut bus, 0x0000, 0x1234);
        write_vram_word(&mut bus, 0x0001, 0x5678);

        bus.write_st_port(0, 0x01); // MARR
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x02); // data register

        let lo1 = bus.read(0x2002);
        let lo2 = bus.read(0x2002);
        assert_eq!(lo1, 0x34);
        assert_eq!(lo2, 0x34);
        assert_eq!(bus.vdc_register(1), Some(0x0000));

        let hi = bus.read(0x2003);
        assert_eq!(hi, 0x12);
        assert_eq!(bus.vdc_register(1), Some(0x0001));
    }

    #[test]
    fn vdc_data_port_reads_selected_register_for_non_vram_index() {
        let mut bus = Bus::new();
        bus.set_mpr(1, 0xFF);

        // Write control register (R05) through normal data ports.
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x5A);
        bus.write_st_port(2, 0x08);

        // Read back from data ports and ensure we get R05 contents, not VRAM.
        bus.write_st_port(0, 0x05);
        let lo = bus.read(0x2002);
        let hi = bus.read(0x2003);
        assert_eq!(lo, 0x5A);
        assert_eq!(hi, 0x08);
        assert_eq!(bus.vdc_register(0x05), Some(0x085A));

        // MARR should remain untouched by non-VRAM register reads.
        assert_eq!(bus.vdc_register(0x01), Some(0x0000));
    }

    #[test]
    fn vdc_satb_dma_copies_sprite_table_and_sets_interrupt() {
        let mut bus = Bus::new();
        // Clear initial VBlank flag.
        bus.read_io(0x00);

        // Seed VRAM at $0200 with sprite attribute data.
        bus.write_st_port(0, 0x00); // MAWR
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x02);
        bus.write_st_port(0, 0x02); // VRAM data write
        for &word in &[0x1234u16, 0x5678, 0x9ABC, 0xDEF0] {
            bus.write_st_port(1, (word & 0x00FF) as u8);
            bus.write_st_port(2, (word >> 8) as u8);
        }

        // Enable SATB DMA IRQ and schedule a transfer from $0200.
        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, DMA_CTRL_IRQ_SATB as u8);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x13);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x02);

        // Run enough cycles to hit the next VBlank and service the DMA.
        for _ in 0..4 {
            bus.tick(200_000, true);
        }

        assert_eq!(bus.vdc_satb_word(0), 0x1234);
        assert_eq!(bus.vdc_satb_word(1), 0x5678);
        assert_eq!(bus.vdc_satb_word(2), 0x9ABC);
        assert_eq!(bus.vdc_satb_word(3), 0xDEF0);

        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
            IRQ_REQUEST_IRQ1
        );
        let status = bus.read_io(0x00);
        assert!(status & VDC_STATUS_DS != 0);
        bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_satb_dma_sets_ds_when_source_written() {
        let mut bus = Bus::new();
        bus.read_io(0x00); // clear initial DS/VBlank bits

        const SATB_SOURCE: u16 = 0x0200;
        let sample = [0xAAAAu16, 0xBBBB, 0xCCCC, 0xDDDD];

        // Populate VRAM at $0200 with sample sprite attributes.
        bus.write_st_port(0, 0x00); // MAWR
        bus.write_st_port(1, (SATB_SOURCE & 0x00FF) as u8);
        bus.write_st_port(2, (SATB_SOURCE >> 8) as u8);
        bus.write_st_port(0, 0x02); // VRAM data write
        for &word in &sample {
            bus.write_st_port(1, (word & 0x00FF) as u8);
            bus.write_st_port(2, (word >> 8) as u8);
        }

        // Writing the SATB source schedules a DMA for the next VBlank.
        bus.write_st_port(0, 0x13);
        bus.write_st_port(1, (SATB_SOURCE & 0x00FF) as u8);
        bus.write_st_port(2, (SATB_SOURCE >> 8) as u8);

        // DMA is deferred — SATB should NOT be updated yet.
        assert_eq!(bus.vdc_satb_word(0), 0, "SATB should not update before VBlank");

        // Advance past VBlank so the deferred DMA executes.
        for _ in 0..4 {
            bus.tick(200_000, true);
        }

        for (idx, &expected) in sample.iter().enumerate() {
            assert_eq!(
                bus.vdc_satb_word(idx),
                expected,
                "SATB entry {idx} did not match VRAM word"
            );
        }
        assert_ne!(bus.vdc_status_bits() & VDC_STATUS_DS, 0);
    }

    #[test]
    fn vdc_cram_dma_transfers_palette_from_vram() {
        let mut bus = Bus::new();
        bus.read_io(0x00); // clear initial status bits

        const VRAM_BASE: u16 = 0x0500;
        let words = [0x0011u16, 0x2233, 0x4455, 0x6677];

        // Seed VRAM at $0500 with palette words.
        bus.write_st_port(0, 0x00); // MAWR
        bus.write_st_port(1, (VRAM_BASE & 0x00FF) as u8);
        bus.write_st_port(2, (VRAM_BASE >> 8) as u8);
        bus.write_st_port(0, 0x02);
        for &word in &words {
            bus.write_st_port(1, (word & 0x00FF) as u8);
            bus.write_st_port(2, (word >> 8) as u8);
        }

        // Point the VRAM read address at the same base for CRAM DMA.
        bus.write_st_port(0, 0x01); // MARR
        bus.write_st_port(1, (VRAM_BASE & 0x00FF) as u8);
        bus.write_st_port(2, (VRAM_BASE >> 8) as u8);

        // Request four words for the upcoming CRAM DMA.
        bus.vdc.registers[0x12] = 0x0004;
        // Schedule CRAM DMA directly (not a standard HuC6270 feature —
        // our emulator provides this as an internal utility).
        bus.vdc.schedule_cram_dma();

        // Tick through VBlank so the pending CRAM DMA executes.
        for _ in 0..4 {
            bus.tick(200_000, true);
        }

        for (idx, &expected) in words.iter().enumerate() {
            assert_eq!(
                bus.vce_palette_word(idx),
                expected,
                "palette entry {idx} did not match VRAM word"
            );
        }
        assert_eq!(bus.vdc_register(0x00), Some(VRAM_BASE + words.len() as u16));
        assert_eq!(
            bus.read_io(VCE_ADDRESS_ADDR as usize) & 0xFF,
            words.len() as u8
        );
        assert_ne!(bus.vdc_status_bits() & VDC_STATUS_DV, 0);
    }

    #[test]
    fn vdc_vram_dma_copies_words_and_raises_status() {
        let mut bus = Bus::new();
        bus.read_io(0x00); // clear initial VBlank

        const SOURCE: u16 = 0x0200;
        let words = [0x0AA0u16, 0x0BB1, 0x0CC2];
        for (index, &word) in words.iter().enumerate() {
            bus.vdc.vram[(SOURCE as usize + index) & 0x7FFF] = word;
        }

        // Configure VRAM DMA: enable IRQ, set source/destination, and trigger by writing LENR MSB.
        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, DMA_CTRL_IRQ_VRAM as u8);
        bus.write_st_port(2, 0x00);

        bus.write_st_port(0, 0x10);
        bus.write_st_port(1, (SOURCE & 0x00FF) as u8);
        bus.write_st_port(2, (SOURCE >> 8) as u8);

        bus.write_st_port(0, 0x11);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x05);

        bus.write_st_port(0, 0x12);
        bus.write_st_port(1, (words.len() as u16 - 1) as u8);
        bus.write_st_port(2, 0x00);

        assert_eq!(bus.vdc_vram_word(0x0500), 0x0AA0);
        assert_eq!(bus.vdc_vram_word(0x0501), 0x0BB1);
        assert_eq!(bus.vdc_vram_word(0x0502), 0x0CC2);
        assert_eq!(
            bus.vdc_register(0x10),
            Some(SOURCE.wrapping_add(words.len() as u16))
        );
        assert_eq!(bus.vdc_register(0x11), Some(0x0503));
        assert_eq!(bus.vdc_register(0x12), Some(0xFFFF));

        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
            IRQ_REQUEST_IRQ1
        );
        let status = bus.read_io(0x00);
        assert!(status & VDC_STATUS_DV != 0);
        bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_dma_status_clears_after_status_read() {
        let mut bus = Bus::new();
        bus.read_io(0x00); // clear initial VBlank

        // Configure VRAM DMA with IRQ enabled and execute a single-word copy.
        const SOURCE: u16 = 0x0100;
        bus.vdc.vram[SOURCE as usize & 0x7FFF] = 0xDEAD;

        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, DMA_CTRL_IRQ_VRAM as u8);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x10);
        bus.write_st_port(1, (SOURCE & 0x00FF) as u8);
        bus.write_st_port(2, (SOURCE >> 8) as u8);
        bus.write_st_port(0, 0x11);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x02);
        bus.write_st_port(0, 0x12);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
            IRQ_REQUEST_IRQ1
        );
        let status = bus.read_io(0x00);
        assert!(status & VDC_STATUS_DV != 0);
        assert_eq!(bus.read_io(0x00) & VDC_STATUS_DV, 0);
    }

    #[test]
    fn vdc_dma_status_clears_on_status_read() {
        let mut bus = Bus::new();
        bus.read_io(0x00);

        const SOURCE: u16 = 0x0400;
        bus.vdc.vram[SOURCE as usize & 0x7FFF] = 0x1234;

        // Trigger VRAM DMA.
        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, DMA_CTRL_IRQ_VRAM as u8);
        bus.write_st_port(2, 0x00);
        bus.write_st_port(0, 0x10);
        bus.write_st_port(1, (SOURCE & 0x00FF) as u8);
        bus.write_st_port(2, (SOURCE >> 8) as u8);
        bus.write_st_port(0, 0x11);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x03);
        bus.write_st_port(0, 0x12);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);

        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
            IRQ_REQUEST_IRQ1
        );

        // Per MAME: writing DCR does NOT clear status flags.
        // DV survives the DCR write.
        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x00);
        assert_ne!(bus.read_io(0x00) & VDC_STATUS_DV, 0, "DV should survive DCR write");

        // Reading status clears DV and drops the IRQ.
        assert_eq!(bus.read_io(0x00) & VDC_STATUS_DV, 0, "DV cleared after status read");
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_satb_auto_transfer_stops_when_disabled() {
        let mut bus = Bus::new();
        bus.read_io(0x00);

        // Seed VRAM at $0300 with initial sprite words.
        bus.write_st_port(0, 0x00); // MAWR
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x03);
        bus.write_st_port(0, 0x02);
        let first_words = [0xAAAAu16, 0xBBBB];
        for &word in &first_words {
            bus.write_st_port(1, (word & 0x00FF) as u8);
            bus.write_st_port(2, (word >> 8) as u8);
        }

        // Enable SATB DMA with auto-transfer and IRQs.
        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, (DMA_CTRL_IRQ_SATB | DMA_CTRL_SATB_AUTO) as u8);
        bus.write_st_port(2, 0x00);

        // Point SATB DMA at $0300.
        bus.write_st_port(0, 0x13);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x03);

        // Run until VBlank triggers the auto SATB DMA.
        for _ in 0..4 {
            bus.tick(200_000, true);
        }

        assert_eq!(bus.vdc_satb_word(0), 0xAAAA);
        assert_eq!(bus.vdc_satb_word(1), 0xBBBB);

        // Acknowledge the interrupt by reading status (per real HW).
        bus.read_io(0x00);

        // Change VRAM words to a new pattern.
        bus.write_st_port(0, 0x00);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x03);
        bus.write_st_port(0, 0x02);
        let second_words = [0xCCCCu16, 0xDDDD];
        for &word in &second_words {
            bus.write_st_port(1, (word & 0x00FF) as u8);
            bus.write_st_port(2, (word >> 8) as u8);
        }

        // Disable auto-transfer.
        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, DMA_CTRL_IRQ_SATB as u8);
        bus.write_st_port(2, 0x00);

        // Acknowledge any pending DS from the first DMA.
        bus.read_io(0x00);

        // Next frame should not pull new SATB data.
        for _ in 0..4 {
            bus.tick(200_000, true);
        }

        assert_eq!(bus.vdc_satb_word(0), 0xAAAA);
        assert_eq!(bus.vdc_satb_word(1), 0xBBBB);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_satb_auto_transfer_repeats_when_enabled() {
        let mut bus = Bus::new();
        bus.read_io(0x00);

        // Seed VRAM at $0300 with an initial pattern.
        bus.write_st_port(0, 0x00); // MAWR
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x03);
        bus.write_st_port(0, 0x02);
        let initial_words = [0x1111u16, 0x2222];
        for &word in &initial_words {
            bus.write_st_port(1, (word & 0x00FF) as u8);
            bus.write_st_port(2, (word >> 8) as u8);
        }

        // Enable SATB auto-transfer with IRQs.
        bus.write_st_port(0, 0x0F);
        bus.write_st_port(1, (DMA_CTRL_IRQ_SATB | DMA_CTRL_SATB_AUTO) as u8);
        bus.write_st_port(2, 0x00);

        // Point SATB DMA at $0300. DMA is deferred to next VBlank.
        bus.write_st_port(0, 0x13);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x03);

        // Advance past VBlank so the deferred DMA executes.
        for _ in 0..4 {
            bus.tick(200_000, true);
        }

        assert_eq!(bus.vdc_satb_word(0), 0x1111);
        assert_eq!(bus.vdc_satb_word(1), 0x2222);
        assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);

        // Acknowledge the initial interrupt and clear DS.
        bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
        bus.read_io(0x00);

        // Overwrite VRAM with a new pattern; auto-transfer should pick it up on next VBlank.
        bus.write_st_port(0, 0x00);
        bus.write_st_port(1, 0x00);
        bus.write_st_port(2, 0x03);
        bus.write_st_port(0, 0x02);
        let updated_words = [0x3333u16, 0x4444];
        for &word in &updated_words {
            bus.write_st_port(1, (word & 0x00FF) as u8);
            bus.write_st_port(2, (word >> 8) as u8);
        }

        // Advance enough cycles to cover another frame; auto-transfer should fire.
        for _ in 0..4 {
            bus.tick(200_000, true);
        }

        assert_eq!(bus.vdc_satb_word(0), 0x3333);
        assert_eq!(bus.vdc_satb_word(1), 0x4444);
        assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_rcr_irq_sets_irq1() {
        let mut bus = Bus::new();
        // Enable RCR interrupt (CR bit 2).
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x04);
        bus.write_st_port(2, 0x00);
        // Set RCR target to 0x42 (valid: counter base 0x40 + 2 = scanline 3).
        bus.write_st_port(0, 0x06);
        bus.write_st_port(1, 0x42);
        bus.write_st_port(2, 0x00);

        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
        for _ in 0..100_000 {
            if bus.pending_interrupts() & IRQ_REQUEST_IRQ1 != 0 {
                break;
            }
            bus.tick(1, true);
        }
        assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
        bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_vblank_irq_fires_via_tick() {
        let mut bus = Bus::new();
        bus.set_mpr(0, 0xFF);
        bus.set_mpr(1, 0xFF);

        // Enable VBlank IRQ.
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x08);
        bus.write_st_port(2, 0x00);

        // Clear any pending VBlank from power-on state.
        bus.read_io(0x00);
        bus.write(0x1403, IRQ_REQUEST_IRQ1);
        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        let visible_lines = VDC_VISIBLE_LINES as u32;
        let min_expected = line_cycles * visible_lines.saturating_sub(1);
        let max_expected = line_cycles * visible_lines.saturating_add(1);

        let mut trigger_iter = None;
        for iter in 0..(VDC_VBLANK_INTERVAL * 2) {
            if bus.tick(1, true) {
                trigger_iter = Some(iter);
                break;
            }
        }
        let trigger_iter =
            trigger_iter.expect("VBlank IRQ did not trigger within two frame intervals");
        assert!(
            trigger_iter >= min_expected && trigger_iter <= max_expected,
            "VBlank IRQ fired outside expected window: iter={trigger_iter}, min={min_expected}, max={max_expected}"
        );
        assert_ne!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);
        let status = bus.read(0x2000);
        assert!(status & VDC_STATUS_VBL != 0);
        bus.write(0x1403, IRQ_REQUEST_IRQ1);

        // Low-speed mode should need 4x cycles (fresh bus to reset accumulator).
        let mut slow_bus = Bus::new();
        slow_bus.set_mpr(0, 0xFF);
        slow_bus.set_mpr(1, 0xFF);
        slow_bus.write_st_port(0, 0x05);
        slow_bus.write_st_port(1, 0x08);
        slow_bus.write_st_port(2, 0x00);
        slow_bus.read_io(0x00);
        slow_bus.write(0x1403, IRQ_REQUEST_IRQ1);
        let mut trigger_iter_slow = None;
        for iter in 0..(max_expected * 2) {
            if slow_bus.tick(1, false) {
                trigger_iter_slow = Some(iter);
                break;
            }
        }
        let trigger_iter_slow =
            trigger_iter_slow.expect("VBlank IRQ (slow clock) did not trigger within window");
        let slow_phi = trigger_iter_slow * 4;
        assert!(
            slow_phi >= min_expected && slow_phi <= max_expected,
            "Slow-clock VBlank IRQ fired outside expected window: cycles={} min={} max={}",
            slow_phi,
            min_expected,
            max_expected
        );
        assert_ne!(slow_bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);
    }

    #[test]
    fn vdc_rcr_flag_clears_on_status_read() {
        let mut bus = Bus::new();
        bus.set_mpr(1, 0xFF);
        // Enable RCR interrupt (CR bit 2) — required for the RR status flag
        // to be raised on raster counter match (per HuC6270 hardware).
        bus.write_st_port(0, 0x05);
        bus.write_st_port(1, 0x04);
        bus.write_st_port(2, 0x00);
        // Set RCR target to 0x42 (counter base 0x40 + 2 → scanline 3).
        bus.write_st_port(0, 0x06);
        bus.write_st_port(1, 0x42);
        bus.write_st_port(2, 0x00);

        let line_cycles =
            (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
        // Advance past scanline 3 to trigger the RCR.
        for _ in 0..=4 {
            bus.tick(line_cycles, true);
        }

        let status = bus.read(0x2000);
        assert!(status & VDC_STATUS_RCR != 0);
        let status_after = bus.read(0x2000);
        assert_eq!(status_after & VDC_STATUS_RCR, 0);
    }

    #[test]
    fn vdc_busy_flag_counts_down() {
        let mut bus = Bus::new();
        bus.set_mpr(1, 0xFF);
        bus.write_st_port(0, 0x02);
        bus.write_st_port(1, 0xAA);
        bus.write_st_port(2, 0x55);

        let status = bus.read(0x2000);
        assert!(status & VDC_STATUS_BUSY != 0);

        bus.tick(VDC_BUSY_ACCESS_CYCLES * 2, true);
        let cleared = bus.read(0x2000);
        assert_eq!(cleared & VDC_STATUS_BUSY, 0);
    }

    #[test]
    fn psg_irq2_triggers_when_enabled() {
        let mut bus = Bus::new();
        bus.write_io(0x1C60, PSG_REG_TIMER_LO as u8);
        bus.write_io(0x1C61, 0x20);
        bus.write_io(0x1C60, PSG_REG_TIMER_HI as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_TIMER_CTRL as u8);
        bus.write_io(0x1C61, PSG_CTRL_ENABLE | PSG_CTRL_IRQ_ENABLE);

        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
        for _ in 0..0x20 {
            bus.tick(1, true);
        }
        assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
        bus.acknowledge_irq(IRQ_REQUEST_IRQ2);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
    }

    #[test]
    fn psg_sample_uses_waveform_ram() {
        let mut bus = Bus::new();

        bus.write_io(0x1C60, 0x00);
        bus.write_io(0x1C61, 0x10);
        bus.write_io(0x1C61, 0x01);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C61, 0x1F);

        bus.write_io(0x1C60, PSG_REG_COUNT as u8);
        bus.write_io(0x1C61, 0x1F);

        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);

        bus.write_io(0x1C60, PSG_REG_TIMER_LO as u8);
        bus.write_io(0x1C61, 0x20);
        bus.write_io(0x1C60, PSG_REG_TIMER_HI as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_TIMER_CTRL as u8);
        bus.write_io(0x1C61, PSG_CTRL_ENABLE);

        for _ in 0..(PHI_CYCLES_PER_SAMPLE * 4) {
            bus.tick(1, true);
        }
        let samples = bus.take_audio_samples();
        assert!(samples.iter().any(|s| *s > 0));
    }

    #[test]
    fn psg_dda_mode_outputs_direct_level() {
        let mut bus = Bus::new();

        bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);
        bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | PSG_CH_CTRL_DDA | 0x1F);

        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x1F);
        let hi = bus.psg_sample();

        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x00);
        let lo = bus.psg_sample();

        assert!(hi > 0, "DDA high level should produce positive sample");
        assert!(lo < 0, "DDA low level should produce negative sample");
    }

    #[test]
    fn psg_noise_channel_changes_sample_values() {
        let mut bus = Bus::new();

        bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
        bus.write_io(0x1C61, 0x04); // channel 4 supports noise
        bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);
        bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);
        bus.write_io(0x1C60, PSG_REG_NOISE_CTRL as u8);
        bus.write_io(0x1C61, PSG_NOISE_ENABLE | 0x1F);

        let mut distinct = std::collections::BTreeSet::new();
        for _ in 0..2048 {
            distinct.insert(bus.psg_sample());
        }
        assert!(
            distinct.len() > 1,
            "noise channel should not output a constant level"
        );
    }

    #[test]
    fn psg_balance_registers_affect_output_amplitude() {
        let mut bus = Bus::new();

        bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | PSG_CH_CTRL_DDA | 0x1F);
        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x1F);
        bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);

        bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);
        let full = bus.psg_sample().abs();

        bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
        bus.write_io(0x1C61, 0x11);
        let reduced = bus.psg_sample().abs();

        assert!(full > 0);
        assert!(reduced < full);
    }

    #[test]
    fn psg_wave_writes_ignored_while_channel_enabled() {
        let mut bus = Bus::new();

        bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);
        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x1F);

        assert_eq!(bus.psg.waveform_ram[0], 0);
        assert_eq!(bus.psg.channels[0].wave_write_pos, 0);
    }

    #[test]
    fn psg_clearing_dda_resets_wave_write_index() {
        let mut bus = Bus::new();

        bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x04);
        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x05);
        assert_eq!(bus.psg.channels[0].wave_write_pos, 2);

        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_DDA);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, 0x00);
        assert_eq!(bus.psg.channels[0].wave_write_pos, 0);

        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x1E);
        assert_eq!(bus.psg.waveform_ram[0], 0x1E);
    }

    #[test]
    fn psg_wave_writes_allowed_with_dda_when_key_off() {
        let mut bus = Bus::new();

        bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_DDA);
        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x1A);
        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        bus.write_io(0x1C61, 0x05);

        assert_eq!(bus.psg.waveform_ram[0], 0x1A);
        assert_eq!(bus.psg.waveform_ram[1], 0x05);
        assert_eq!(bus.psg.channels[0].wave_write_pos, 2);
    }

    #[test]
    fn psg_frequency_divider_uses_inverse_pitch_relation() {
        fn transition_count_for_divider(divider: u16) -> usize {
            let mut bus = Bus::new();
            bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
            bus.write_io(0x1C61, 0x00);
            bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
            bus.write_io(0x1C61, 0xFF);
            bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
            bus.write_io(0x1C61, 0xFF);
            bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
            bus.write_io(0x1C61, 0x00);
            bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
            for i in 0..PSG_WAVE_SIZE {
                bus.write_io(0x1C61, if i & 0x01 == 0 { 0x00 } else { 0x1F });
            }
            bus.write_io(0x1C60, PSG_REG_FREQ_LO as u8);
            bus.write_io(0x1C61, divider as u8);
            bus.write_io(0x1C60, PSG_REG_FREQ_HI as u8);
            bus.write_io(0x1C61, ((divider >> 8) as u8) & 0x0F);
            bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
            bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);

            let mut transitions = 0usize;
            let mut prev = bus.psg_sample();
            for _ in 0..2048 {
                let sample = bus.psg_sample();
                if (sample >= 0) != (prev >= 0) {
                    transitions += 1;
                }
                prev = sample;
            }
            transitions
        }

        let fast = transition_count_for_divider(0x0001);
        let slow = transition_count_for_divider(0x0FFF);
        assert!(
            fast > slow.saturating_mul(8),
            "expected divider 0x001 to run much faster than 0xFFF (fast={fast}, slow={slow})"
        );
    }
}
