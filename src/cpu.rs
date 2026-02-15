use crate::bus::{Bus, IRQ_REQUEST_IRQ1, IRQ_REQUEST_IRQ2, IRQ_REQUEST_TIMER};

pub const FLAG_CARRY: u8 = 0b0000_0001;
pub const FLAG_ZERO: u8 = 0b0000_0010;
pub const FLAG_INTERRUPT_DISABLE: u8 = 0b0000_0100;
pub const FLAG_DECIMAL: u8 = 0b0000_1000;
pub const FLAG_BREAK: u8 = 0b0001_0000;
pub const FLAG_T: u8 = 0b0010_0000;
pub const FLAG_OVERFLOW: u8 = 0b0100_0000;
pub const FLAG_NEGATIVE: u8 = 0b1000_0000;
const VECTOR_IRQ2_BRK: u16 = 0xFFF6;
const VECTOR_IRQ1: u16 = 0xFFF8;
const VECTOR_TIMER: u16 = 0xFFFA;
const VECTOR_NMI: u16 = 0xFFFC;
const VECTOR_RESET: u16 = 0xFFFE;
const VECTOR_LEGACY_SHARED_IRQ: u16 = 0xFFFE;
const VECTOR_LEGACY_RESET: u16 = 0xFFFC;
const VECTOR_LEGACY_NMI: u16 = 0xFFFA;

/// HuC6280 CPU core.
/// Implements a growing subset of the instruction matrix shared with the 65C02,
/// covering common loads/stores, arithmetic, branches, and subroutine control.
#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: u8,
    pub halted: bool,
    pub clock_high_speed: bool,
    waiting: bool,
    irq_pending: bool,
    nmi_pending: bool,
    last_opcode: u8,
    block_transfer_cycles: Option<u32>,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: FLAG_INTERRUPT_DISABLE,
            halted: false,
            clock_high_speed: true,
            waiting: false,
            irq_pending: false,
            nmi_pending: false,
            last_opcode: 0,
            block_transfer_cycles: None,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.sp = 0xFD;
        let reset_vector = Self::vector_slot_with_fallback(bus, VECTOR_RESET, VECTOR_LEGACY_RESET);
        self.pc = bus.read_u16(reset_vector);
        self.status = FLAG_INTERRUPT_DISABLE;
        self.halted = false;
        self.clock_high_speed = true;
        self.waiting = false;
        self.irq_pending = false;
        self.nmi_pending = false;
        self.last_opcode = 0;
        self.block_transfer_cycles = None;
    }

    pub fn request_irq(&mut self) {
        self.irq_pending = true;
    }

    pub fn request_nmi(&mut self) {
        self.nmi_pending = true;
    }

    /// HuC6280-accurate cycle table.
    ///
    /// Key differences from 65C02:
    ///   - ZP read/write: 4 (was 3)
    ///   - Absolute read/write: 5 (was 4)
    ///   - (ind),Y / (zp): 7 (was 5+page-cross)
    ///   - (ind,X): 7 (was 6)
    ///   - JSR/RTS/RTI: 7 (was 6)
    ///   - BRK: 8 (was 7)
    ///   - JMP abs: 4 (was 3)
    ///   - JMP (ind): 7 (was 5)
    ///   - No page-crossing penalties
    ///   - Branch taken: +2 (not +1)
    #[inline]
    #[allow(unreachable_patterns)]
    fn opcode_base_cycles(opcode: u8) -> u8 {
        match opcode {
            // 2-cycle: implied, accumulator, immediate, branches (not-taken base)
            0x09 | 0x0A | 0x0B | 0x10 | 0x18 | 0x1A | 0x29 | 0x2A | 0x2B | 0x30 | 0x38 | 0x3A
            | 0x49 | 0x4A | 0x50 | 0x58 | 0x62 | 0x69 | 0x6A | 0x70 | 0x78 | 0x80 | 0x82 | 0x88
            | 0x89 | 0x8A | 0x90 | 0x98 | 0x9A | 0xA0 | 0xA2 | 0xA8 | 0xA9 | 0xAA | 0xB0 | 0xB8
            | 0xBA | 0xC0 | 0xC2 | 0xC8 | 0xC9 | 0xCA | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xE9 | 0xEA
            | 0xEB | 0xF0 | 0xF4 | 0xF8 | 0x1B | 0x33 | 0x3B | 0x4B | 0x5B | 0x5C | 0x63 | 0x6B
            | 0x8B | 0x9B | 0xAB | 0xBB | 0xDC | 0xE2 | 0xFB | 0xFC => 2,

            // 3-cycle: push (PHP/PHA/PHY/PHX), SXY/SAX/SAY, CSL/CSH, WAI
            0x02 | 0x08 | 0x22 | 0x42 | 0x48 | 0x54 | 0x5A | 0xCB | 0xD4 | 0xDA | 0xDB => 3,

            // 4-cycle: ZP read/write, ZP-indexed read/write, PLP/PLA/PLY/PLX,
            //          JMP abs, TMA
            0x05 | 0x15 | 0x24 | 0x25 | 0x28 | 0x34 | 0x35 | 0x43 | 0x45 | 0x4C | 0x55 | 0x64
            | 0x65 | 0x68 | 0x74 | 0x75 | 0x7A | 0x84 | 0x85 | 0x86 | 0x94 | 0x95 | 0x96 | 0xA4
            | 0xA5 | 0xA6 | 0xB4 | 0xB5 | 0xB6 | 0xC4 | 0xC5 | 0xD5 | 0xE4 | 0xE5 | 0xF5 | 0xFA => {
                4
            }

            // 5-cycle: absolute read/write, absolute indexed read/write,
            //          ST0/ST1/ST2, TAM
            0x03 | 0x0D | 0x13 | 0x19 | 0x1D | 0x23 | 0x2C | 0x2D | 0x39 | 0x3C | 0x3D | 0x4D
            | 0x53 | 0x59 | 0x5D | 0x6D | 0x79 | 0x7D | 0x8C | 0x8D | 0x8E | 0x99 | 0x9C | 0x9D
            | 0x9E | 0xAC | 0xAD | 0xAE | 0xB9 | 0xBC | 0xBD | 0xBE | 0xCC | 0xCD | 0xD9 | 0xDD
            | 0xEC | 0xED | 0xF9 | 0xFD => 5,

            // 6-cycle: ZP RMW, absolute RMW, ZP-indexed RMW,
            //          BBR/BBS (not-taken base)
            0x04 | 0x06 | 0x0C | 0x0E | 0x0F | 0x14 | 0x16 | 0x1C | 0x1F | 0x26 | 0x2E | 0x2F
            | 0x36 | 0x3F | 0x46 | 0x4E | 0x4F | 0x56 | 0x5F | 0x66 | 0x6E | 0x6F | 0x76 | 0x7F
            | 0x8F | 0x9F | 0xAF | 0xBF | 0xC6 | 0xCE | 0xCF | 0xD6 | 0xDF | 0xE6 | 0xEE | 0xEF
            | 0xF6 | 0xFF => 6,

            // 7-cycle: (ind,X), (ind),Y, (zp indirect), JMP (ind), JMP (abs,X),
            //          JSR, RTS, RTI, RMB/SMB, absolute-indexed RMW
            0x01 | 0x07 | 0x11 | 0x12 | 0x17 | 0x20 | 0x21 | 0x27 | 0x31 | 0x32 | 0x37 | 0x40
            | 0x41 | 0x47 | 0x51 | 0x52 | 0x57 | 0x60 | 0x61 | 0x67 | 0x6C | 0x71 | 0x72 | 0x77
            | 0x7C | 0x81 | 0x87 | 0x91 | 0x92 | 0x97 | 0xA1 | 0xA7 | 0xB1 | 0xB2 | 0xB7 | 0xC1
            | 0xC7 | 0xD1 | 0xD2 | 0xD7 | 0xE1 | 0xE7 | 0xF1 | 0xF2 | 0xF7 | 0x1E | 0x3E | 0x5E
            | 0x7E | 0xDE | 0xFE => 7,

            // 8-cycle: BRK, BSR, block-transfer setup (BBR/BBS taken handled separately)
            0x00 | 0x44 | 0x7B | 0x83 | 0xA3 | 0x93 | 0xB3 => 8,

            // Block transfer setup cycles; per-byte transfer cost is accounted elsewhere.
            0x73 | 0xC3 | 0xD3 | 0xE3 | 0xF3 => 17,

            _ => 0,
        }
    }

    #[allow(unreachable_patterns)]
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if self.halted {
            return 0;
        }

        if !self.nmi_pending && bus.irq_pending() {
            self.irq_pending = true;
        }

        if self.nmi_pending {
            self.nmi_pending = false;
            let vector_slot = Self::vector_slot_with_fallback(bus, VECTOR_NMI, VECTOR_LEGACY_NMI);
            return self.handle_interrupt(bus, vector_slot, false) as u32;
        }

        if (self.irq_pending || bus.irq_pending())
            && (!self.get_flag(FLAG_INTERRUPT_DISABLE) || self.waiting)
        {
            self.irq_pending = false;
            let source = if let Some(mask) = bus.next_irq() {
                bus.acknowledge_irq(mask);
                mask
            } else {
                IRQ_REQUEST_IRQ2
            };
            let vector_slot = Self::vector_slot_for_irq_source(bus, source);
            return self.handle_interrupt(bus, vector_slot, false) as u32;
        }

        if self.waiting {
            return 0;
        }

        let opcode = self.fetch_byte(bus);
        self.last_opcode = opcode;
        // HuC6280 T-mode is consumed by the next fetched instruction.
        let t_mode_active = self.get_flag(FLAG_T);
        self.set_flag(FLAG_T, false);
        let base_cycles = Self::opcode_base_cycles(opcode);
        let cycles = match opcode {
            // Load A
            0xA9 => {
                let value = self.fetch_byte(bus);
                self.lda(value, base_cycles)
            }
            0xA5 => {
                let addr = self.addr_zeropage(bus);
                self.lda(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0xA1 => {
                let addr = self.addr_indexed_indirect_x(bus);
                self.lda(bus.read(addr), base_cycles)
            }
            0xB5 => {
                let addr = self.addr_zeropage_x(bus);
                self.lda(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0xAD => {
                let addr = self.addr_absolute(bus);
                self.lda(bus.read(addr), base_cycles)
            }
            0xBD => {
                let (addr, _) = self.addr_absolute_x(bus);
                let cycles = self.lda(bus.read(addr), base_cycles);
                cycles
            }
            0xB9 => {
                let (addr, _) = self.addr_absolute_y(bus);
                let cycles = self.lda(bus.read(addr), base_cycles);
                cycles
            }
            0xB1 => {
                let (addr, _) = self.addr_indirect_y(bus);
                let cycles = self.lda(bus.read(addr), base_cycles);
                cycles
            }
            0xB2 => {
                let addr = self.addr_indirect(bus);
                self.lda(bus.read(addr), base_cycles)
            }

            // Load X
            0xA2 => {
                let value = self.fetch_byte(bus);
                self.ldx(value, base_cycles)
            }
            0xA6 => {
                let addr = self.addr_zeropage(bus);
                self.ldx(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0xB6 => {
                let addr = self.addr_zeropage_y(bus);
                self.ldx(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0xAE => {
                let addr = self.addr_absolute(bus);
                self.ldx(bus.read(addr), base_cycles)
            }
            0xBE => {
                let (addr, _) = self.addr_absolute_y(bus);
                let cycles = self.ldx(bus.read(addr), base_cycles);
                cycles
            }

            // Load Y
            0xA0 => {
                let value = self.fetch_byte(bus);
                self.ldy(value, base_cycles)
            }
            0xA4 => {
                let addr = self.addr_zeropage(bus);
                self.ldy(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0xB4 => {
                let addr = self.addr_zeropage_x(bus);
                self.ldy(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0xAC => {
                let addr = self.addr_absolute(bus);
                self.ldy(bus.read(addr), base_cycles)
            }
            0xBC => {
                let (addr, _) = self.addr_absolute_x(bus);
                let cycles = self.ldy(bus.read(addr), base_cycles);
                cycles
            }

            // Store A
            0x85 => {
                let addr = self.addr_zeropage(bus);
                Cpu::write_operand(bus, addr, self.a, true);
                base_cycles
            }
            0x81 => {
                let addr = self.addr_indexed_indirect_x(bus);
                bus.write(addr, self.a);
                base_cycles
            }
            0x95 => {
                let addr = self.addr_zeropage_x(bus);
                Cpu::write_operand(bus, addr, self.a, true);
                base_cycles
            }
            0x8D => {
                let addr = self.addr_absolute(bus);
                bus.write(addr, self.a);
                base_cycles
            }
            0x9D => {
                let (addr, _) = self.addr_absolute_x(bus);
                bus.write(addr, self.a);
                base_cycles
            }
            0x99 => {
                let (addr, _) = self.addr_absolute_y(bus);
                bus.write(addr, self.a);
                base_cycles
            }
            0x92 => {
                let addr = self.addr_indirect(bus);
                bus.write(addr, self.a);
                base_cycles
            }
            0x91 => {
                let (addr, _) = self.addr_indirect_y(bus);
                bus.write(addr, self.a);
                base_cycles
            }

            // Store X
            0x86 => {
                let addr = self.addr_zeropage(bus);
                Cpu::write_operand(bus, addr, self.x, true);
                base_cycles
            }
            0x96 => {
                let addr = self.addr_zeropage_y(bus);
                Cpu::write_operand(bus, addr, self.x, true);
                base_cycles
            }
            0x8E => {
                let addr = self.addr_absolute(bus);
                bus.write(addr, self.x);
                base_cycles
            }

            // Store Y
            0x84 => {
                let addr = self.addr_zeropage(bus);
                Cpu::write_operand(bus, addr, self.y, true);
                base_cycles
            }
            0x94 => {
                let addr = self.addr_zeropage_x(bus);
                Cpu::write_operand(bus, addr, self.y, true);
                base_cycles
            }
            0x8C => {
                let addr = self.addr_absolute(bus);
                bus.write(addr, self.y);
                base_cycles
            }

            // Arithmetic
            0x69 => {
                let value = self.fetch_byte(bus);
                self.adc_with_t(bus, value, base_cycles, true, t_mode_active)
            }
            0x65 => {
                let addr = self.addr_zeropage(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.adc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x61 => {
                let addr = self.addr_indexed_indirect_x(bus);
                let value = bus.read(addr);
                self.adc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x75 => {
                let addr = self.addr_zeropage_x(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.adc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x6D => {
                let addr = self.addr_absolute(bus);
                let value = bus.read(addr);
                self.adc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x7D => {
                let (addr, _) = self.addr_absolute_x(bus);
                let value = bus.read(addr);
                let cycles = self.adc_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x79 => {
                let (addr, _) = self.addr_absolute_y(bus);
                let value = bus.read(addr);
                let cycles = self.adc_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x71 => {
                let (addr, _) = self.addr_indirect_y(bus);
                let value = bus.read(addr);
                let cycles = self.adc_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x72 => {
                let addr = self.addr_indirect(bus);
                let value = bus.read(addr);
                self.adc_with_t(bus, value, base_cycles, false, t_mode_active)
            }

            0xE9 | 0xEB => {
                let value = self.fetch_byte(bus);
                self.sbc_with_t(bus, value, base_cycles, true, t_mode_active)
            }
            0xE5 => {
                let addr = self.addr_zeropage(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.sbc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0xE1 => {
                let addr = self.addr_indexed_indirect_x(bus);
                let value = bus.read(addr);
                self.sbc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0xF5 => {
                let addr = self.addr_zeropage_x(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.sbc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0xED => {
                let addr = self.addr_absolute(bus);
                let value = bus.read(addr);
                self.sbc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0xF1 => {
                let (addr, _) = self.addr_indirect_y(bus);
                let value = bus.read(addr);
                let cycles = self.sbc_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0xF2 => {
                let addr = self.addr_indirect(bus);
                let value = bus.read(addr);
                self.sbc_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0xFD => {
                let (addr, _) = self.addr_absolute_x(bus);
                let value = bus.read(addr);
                let cycles = self.sbc_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0xF9 => {
                let (addr, _) = self.addr_absolute_y(bus);
                let value = bus.read(addr);
                let cycles = self.sbc_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }

            // Logical
            0x29 => {
                let value = self.fetch_byte(bus);
                self.and_with_t(bus, value, base_cycles, true, t_mode_active)
            }
            0x0B | 0x2B => {
                let value = self.fetch_byte(bus);
                self.anc(value, base_cycles)
            }
            0x25 => {
                let addr = self.addr_zeropage(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.and_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x21 => {
                let addr = self.addr_indexed_indirect_x(bus);
                let value = bus.read(addr);
                self.and_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x35 => {
                let addr = self.addr_zeropage_x(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.and_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x2D => {
                let addr = self.addr_absolute(bus);
                let value = bus.read(addr);
                self.and_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x3D => {
                let (addr, _) = self.addr_absolute_x(bus);
                let value = bus.read(addr);
                let cycles = self.and_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x39 => {
                let (addr, _) = self.addr_absolute_y(bus);
                let value = bus.read(addr);
                let cycles = self.and_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x31 => {
                let (addr, _) = self.addr_indirect_y(bus);
                let value = bus.read(addr);
                let cycles = self.and_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x32 => {
                let addr = self.addr_indirect(bus);
                let value = bus.read(addr);
                self.and_with_t(bus, value, base_cycles, false, t_mode_active)
            }

            0x09 => {
                let value = self.fetch_byte(bus);
                self.ora_with_t(bus, value, base_cycles, true, t_mode_active)
            }
            0x05 => {
                let addr = self.addr_zeropage(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.ora_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x01 => {
                let addr = self.addr_indexed_indirect_x(bus);
                let value = bus.read(addr);
                self.ora_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x15 => {
                let addr = self.addr_zeropage_x(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.ora_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x0D => {
                let addr = self.addr_absolute(bus);
                let value = bus.read(addr);
                self.ora_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x1D => {
                let (addr, _) = self.addr_absolute_x(bus);
                let value = bus.read(addr);
                let cycles = self.ora_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x19 => {
                let (addr, _) = self.addr_absolute_y(bus);
                let value = bus.read(addr);
                let cycles = self.ora_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x11 => {
                let (addr, _) = self.addr_indirect_y(bus);
                let value = bus.read(addr);
                let cycles = self.ora_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x12 => {
                let addr = self.addr_indirect(bus);
                let value = bus.read(addr);
                self.ora_with_t(bus, value, base_cycles, false, t_mode_active)
            }

            0x49 => {
                let value = self.fetch_byte(bus);
                self.eor_with_t(bus, value, base_cycles, true, t_mode_active)
            }
            0x45 => {
                let addr = self.addr_zeropage(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.eor_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x41 => {
                let addr = self.addr_indexed_indirect_x(bus);
                let value = bus.read(addr);
                self.eor_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x55 => {
                let addr = self.addr_zeropage_x(bus);
                let value = Cpu::read_operand(bus, addr, true);
                self.eor_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x4D => {
                let addr = self.addr_absolute(bus);
                let value = bus.read(addr);
                self.eor_with_t(bus, value, base_cycles, false, t_mode_active)
            }
            0x5D => {
                let (addr, _) = self.addr_absolute_x(bus);
                let value = bus.read(addr);
                let cycles = self.eor_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x59 => {
                let (addr, _) = self.addr_absolute_y(bus);
                let value = bus.read(addr);
                let cycles = self.eor_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x51 => {
                let (addr, _) = self.addr_indirect_y(bus);
                let value = bus.read(addr);
                let cycles = self.eor_with_t(bus, value, base_cycles, false, t_mode_active);
                cycles
            }
            0x52 => {
                let addr = self.addr_indirect(bus);
                let value = bus.read(addr);
                self.eor_with_t(bus, value, base_cycles, false, t_mode_active)
            }

            // BIT tests accumulator against memory without modifying A
            0x24 => {
                let addr = self.addr_zeropage(bus);
                self.bit(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0x34 => {
                let addr = self.addr_zeropage_x(bus);
                self.bit(Cpu::read_operand(bus, addr, true), base_cycles)
            }
            0x2C => {
                let addr = self.addr_absolute(bus);
                self.bit(bus.read(addr), base_cycles)
            }
            0x3C => {
                let (addr, _) = self.addr_absolute_x(bus);
                let cycles = self.bit(bus.read(addr), base_cycles);
                cycles
            }
            0x89 => {
                let value = self.fetch_byte(bus);
                self.bit(value, base_cycles)
            }

            // Store zero / test and set/reset bits
            0x64 => {
                let addr = self.addr_zeropage(bus);
                self.stz(bus, addr, base_cycles)
            }
            0x74 => {
                let addr = self.addr_zeropage_x(bus);
                self.stz(bus, addr, base_cycles)
            }
            0x9C => {
                let addr = self.addr_absolute(bus);
                bus.write(addr, 0);
                base_cycles
            }
            0x9E => {
                let (addr, _) = self.addr_absolute_x(bus);
                bus.write(addr, 0);
                base_cycles
            }

            0x04 => {
                let addr = self.addr_zeropage(bus);
                self.tsb(bus, addr, base_cycles)
            }
            0x0C => {
                let addr = self.addr_absolute(bus);
                self.tsb(bus, addr, base_cycles)
            }

            0x14 => {
                let addr = self.addr_zeropage(bus);
                self.trb(bus, addr, base_cycles)
            }
            0x1C => {
                let addr = self.addr_absolute(bus);
                self.trb(bus, addr, base_cycles)
            }

            // Shift / rotate
            0x0A => self.asl_acc(base_cycles),
            0x06 => {
                let addr = self.addr_zeropage(bus);
                self.asl_mem(bus, addr, base_cycles)
            }
            0x16 => {
                let addr = self.addr_zeropage_x(bus);
                self.asl_mem(bus, addr, base_cycles)
            }
            0x0E => {
                let addr = self.addr_absolute(bus);
                self.asl_mem(bus, addr, base_cycles)
            }
            0x1E => {
                let (addr, _) = self.addr_absolute_x(bus);
                self.asl_mem(bus, addr, base_cycles)
            }

            0x4A => self.lsr_acc(base_cycles),
            0x46 => {
                let addr = self.addr_zeropage(bus);
                self.lsr_mem(bus, addr, base_cycles)
            }
            0x56 => {
                let addr = self.addr_zeropage_x(bus);
                self.lsr_mem(bus, addr, base_cycles)
            }
            0x4E => {
                let addr = self.addr_absolute(bus);
                self.lsr_mem(bus, addr, base_cycles)
            }
            0x5E => {
                let (addr, _) = self.addr_absolute_x(bus);
                self.lsr_mem(bus, addr, base_cycles)
            }

            // Increment memory
            0xE6 => {
                let addr = self.addr_zeropage(bus);
                self.inc_mem(bus, addr, base_cycles)
            }
            0xF6 => {
                let addr = self.addr_zeropage_x(bus);
                self.inc_mem(bus, addr, base_cycles)
            }
            0xEE => {
                let addr = self.addr_absolute(bus);
                self.inc_mem(bus, addr, base_cycles)
            }
            0xFE => {
                let (addr, _) = self.addr_absolute_x(bus);
                self.inc_mem(bus, addr, base_cycles)
            }
            0xC6 => {
                let addr = self.addr_zeropage(bus);
                self.dec_mem(bus, addr, base_cycles)
            }
            0xD6 => {
                let addr = self.addr_zeropage_x(bus);
                self.dec_mem(bus, addr, base_cycles)
            }
            0xCE => {
                let addr = self.addr_absolute(bus);
                self.dec_mem(bus, addr, base_cycles)
            }
            0xDE => {
                let (addr, _) = self.addr_absolute_x(bus);
                self.dec_mem(bus, addr, base_cycles)
            }

            0x2A => self.rol_acc(base_cycles),
            0x26 => {
                let addr = self.addr_zeropage(bus);
                self.rol_mem(bus, addr, base_cycles)
            }
            0x36 => {
                let addr = self.addr_zeropage_x(bus);
                self.rol_mem(bus, addr, base_cycles)
            }
            0x2E => {
                let addr = self.addr_absolute(bus);
                self.rol_mem(bus, addr, base_cycles)
            }
            0x3E => {
                let (addr, _) = self.addr_absolute_x(bus);
                self.rol_mem(bus, addr, base_cycles)
            }

            0x6A => self.ror_acc(base_cycles),
            0x66 => {
                let addr = self.addr_zeropage(bus);
                self.ror_mem(bus, addr, base_cycles)
            }
            0x76 => {
                let addr = self.addr_zeropage_x(bus);
                self.ror_mem(bus, addr, base_cycles)
            }
            0x6E => {
                let addr = self.addr_absolute(bus);
                self.ror_mem(bus, addr, base_cycles)
            }
            0x7E => {
                let (addr, _) = self.addr_absolute_x(bus);
                self.ror_mem(bus, addr, base_cycles)
            }
            0x7B => {
                let (addr, _) = self.addr_absolute_y(bus);
                self.rra_mem(bus, addr, base_cycles)
            }

            // Stack pushes/pulls
            0x48 => self.pha(bus, base_cycles),
            0x5A => self.phy(bus, base_cycles),
            0xDA => self.phx(bus, base_cycles),
            0x08 => self.php(bus, base_cycles),
            0x68 => self.pla(bus, base_cycles),
            0x7A => self.ply(bus, base_cycles),
            0xFA => self.plx(bus, base_cycles),
            0x28 => self.plp(bus, base_cycles),
            0x40 => self.rti(bus, base_cycles),
            0xCB => self.wai(base_cycles),
            0x53 => self.tam(bus, base_cycles),
            0x43 => self.tma(bus, base_cycles),
            0x07 => self.rmb(bus, 0, base_cycles),
            0x17 => self.rmb(bus, 1, base_cycles),
            0x27 => self.rmb(bus, 2, base_cycles),
            0x37 => self.rmb(bus, 3, base_cycles),
            0x47 => self.rmb(bus, 4, base_cycles),
            0x57 => self.rmb(bus, 5, base_cycles),
            0x67 => self.rmb(bus, 6, base_cycles),
            0x77 => self.rmb(bus, 7, base_cycles),
            0x87 => self.smb(bus, 0, base_cycles),
            0x97 => self.smb(bus, 1, base_cycles),
            0xA7 => self.smb(bus, 2, base_cycles),
            0xB7 => self.smb(bus, 3, base_cycles),
            0xC7 => self.smb(bus, 4, base_cycles),
            0xD7 => self.smb(bus, 5, base_cycles),
            0xE7 => self.smb(bus, 6, base_cycles),
            0xF7 => self.smb(bus, 7, base_cycles),
            0x0F => self.bbr(bus, 0, base_cycles),
            0x1F => self.bbr(bus, 1, base_cycles),
            0x2F => self.bbr(bus, 2, base_cycles),
            0x3F => self.bbr(bus, 3, base_cycles),
            0x4F => self.bbr(bus, 4, base_cycles),
            0x5F => self.bbr(bus, 5, base_cycles),
            0x6F => self.bbr(bus, 6, base_cycles),
            0x7F => self.bbr(bus, 7, base_cycles),
            0x8F => self.bbs(bus, 0, base_cycles),
            0x9F => self.bbs(bus, 1, base_cycles),
            0xAF => self.bbs(bus, 2, base_cycles),
            0xBF => self.bbs(bus, 3, base_cycles),
            0xCF => self.bbs(bus, 4, base_cycles),
            0xDF => self.bbs(bus, 5, base_cycles),
            0xEF => self.bbs(bus, 6, base_cycles),
            0xFF => self.bbs(bus, 7, base_cycles),
            0x83 => self.tst_zero_page(bus, base_cycles),
            0xA3 => self.tst_zero_page_x(bus, base_cycles),
            0x93 => self.tst_absolute(bus, base_cycles),
            0xB3 => self.tst_absolute_x(bus, base_cycles),
            0x03 => self.st_port(bus, 0, base_cycles),
            0x13 => self.st_port(bus, 1, base_cycles),
            0x23 => self.st_port(bus, 2, base_cycles),
            0xDB => self.stp(base_cycles),
            0x73 => self.exec_block_transfer(bus, BlockMode::Tii, base_cycles),
            0xC3 => self.exec_block_transfer(bus, BlockMode::Tdd, base_cycles),
            0xD3 => self.exec_block_transfer(bus, BlockMode::Tin, base_cycles),
            0xE3 => self.exec_block_transfer(bus, BlockMode::Tia, base_cycles),
            0xF3 => self.exec_block_transfer(bus, BlockMode::Tai, base_cycles),

            // Increment / Decrement
            0xE8 => self.inx(base_cycles),
            0xC8 => self.iny(base_cycles),
            0x1A => self.ina(base_cycles),
            0xCA => self.dex(base_cycles),
            0x88 => self.dey(base_cycles),
            0x3A => self.dea(base_cycles),

            // Comparisons
            0xC9 => {
                let value = self.fetch_byte(bus);
                self.cmp(value, self.a, base_cycles)
            }
            0xC5 => {
                let addr = self.addr_zeropage(bus);
                self.cmp(Cpu::read_operand(bus, addr, true), self.a, base_cycles)
            }
            0xC1 => {
                let addr = self.addr_indexed_indirect_x(bus);
                self.cmp(bus.read(addr), self.a, base_cycles)
            }
            0xD5 => {
                let addr = self.addr_zeropage_x(bus);
                self.cmp(bus.read(addr), self.a, base_cycles)
            }
            0xCD => {
                let addr = self.addr_absolute(bus);
                self.cmp(bus.read(addr), self.a, base_cycles)
            }
            0xDD => {
                let (addr, _) = self.addr_absolute_x(bus);
                let cycles = self.cmp(bus.read(addr), self.a, base_cycles);
                cycles
            }
            0xD9 => {
                let (addr, _) = self.addr_absolute_y(bus);
                let cycles = self.cmp(bus.read(addr), self.a, base_cycles);
                cycles
            }
            0xD1 => {
                let (addr, _) = self.addr_indirect_y(bus);
                let cycles = self.cmp(bus.read(addr), self.a, base_cycles);
                cycles
            }
            0xD2 => {
                let addr = self.addr_indirect(bus);
                self.cmp(bus.read(addr), self.a, base_cycles)
            }

            0xE0 => {
                let value = self.fetch_byte(bus);
                self.cmp(value, self.x, base_cycles)
            }
            0xE4 => {
                let addr = self.addr_zeropage(bus);
                self.cmp(Cpu::read_operand(bus, addr, true), self.x, base_cycles)
            }
            0xEC => {
                let addr = self.addr_absolute(bus);
                self.cmp(bus.read(addr), self.x, base_cycles)
            }

            0xC0 => {
                let value = self.fetch_byte(bus);
                self.cmp(value, self.y, base_cycles)
            }
            0xC4 => {
                let addr = self.addr_zeropage(bus);
                self.cmp(Cpu::read_operand(bus, addr, true), self.y, base_cycles)
            }
            0xCC => {
                let addr = self.addr_absolute(bus);
                self.cmp(bus.read(addr), self.y, base_cycles)
            }

            // Branches
            0x90 => self.branch(bus, !self.get_flag(FLAG_CARRY), base_cycles),
            0xB0 => self.branch(bus, self.get_flag(FLAG_CARRY), base_cycles),
            0xF0 => self.branch(bus, self.get_flag(FLAG_ZERO), base_cycles),
            0x30 => self.branch(bus, self.get_flag(FLAG_NEGATIVE), base_cycles),
            0xD0 => self.branch(bus, !self.get_flag(FLAG_ZERO), base_cycles),
            0x10 => self.branch(bus, !self.get_flag(FLAG_NEGATIVE), base_cycles),
            0x50 => self.branch(bus, !self.get_flag(FLAG_OVERFLOW), base_cycles),
            0x70 => self.branch(bus, self.get_flag(FLAG_OVERFLOW), base_cycles),
            0x80 => self.branch(bus, true, base_cycles),

            // Status
            0x18 => {
                self.set_flag(FLAG_CARRY, false);
                base_cycles
            }
            0x38 => {
                self.set_flag(FLAG_CARRY, true);
                base_cycles
            }
            0x58 => {
                self.set_flag(FLAG_INTERRUPT_DISABLE, false);
                base_cycles
            }
            0x78 => {
                self.set_flag(FLAG_INTERRUPT_DISABLE, true);
                base_cycles
            }
            0xB8 => {
                self.set_flag(FLAG_OVERFLOW, false);
                base_cycles
            }
            0xD8 => {
                self.set_flag(FLAG_DECIMAL, false);
                base_cycles
            }
            0xF8 => {
                self.set_flag(FLAG_DECIMAL, true);
                base_cycles
            }
            0xF4 => self.set_t_flag(base_cycles),
            0xD4 => self.csh(base_cycles),
            0x54 => self.csl(base_cycles),

            // Transfers
            0x62 => self.cla(base_cycles),
            0x82 => self.clx(base_cycles),
            0xC2 => self.cly(base_cycles),
            0xAA => self.tax(base_cycles),
            0xA8 => self.tay(base_cycles),
            0x8A => self.txa(base_cycles),
            0x98 => self.tya(base_cycles),
            0xBA => self.tsx(base_cycles),
            0x9A => self.txs(base_cycles),
            0x22 => self.sax(base_cycles),
            0x42 => self.say(base_cycles),
            0x02 => self.sxy(base_cycles),

            // Stack / control
            0x44 => self.bsr(bus, base_cycles),
            0x20 => self.jsr(bus, base_cycles),
            0x4C => self.jmp_absolute(bus, base_cycles),
            0x6C => self.jmp_indirect(bus, base_cycles),
            0x7C => self.jmp_indirect_indexed(bus, base_cycles),
            0x60 => self.rts(bus, base_cycles),
            0x00 => self.brk(bus, base_cycles),
            0xEA | 0x1B | 0x33 | 0x3B | 0x4B | 0x5B | 0x5C | 0x63 | 0x6B | 0x8B | 0x9B | 0xAB
            | 0xBB | 0xDC | 0xE2 | 0xFB | 0xFC => base_cycles, // NOP

            _ => unreachable!("opcode dispatch table out of sync: {opcode:#04X}"),
        };
        self.block_transfer_cycles.take().unwrap_or(cycles as u32)
    }

    fn lda(&mut self, value: u8, cycles: u8) -> u8 {
        self.a = value;
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn ldx(&mut self, value: u8, cycles: u8) -> u8 {
        self.x = value;
        self.update_zero_and_negative(self.x);
        cycles
    }

    fn ldy(&mut self, value: u8, cycles: u8) -> u8 {
        self.y = value;
        self.update_zero_and_negative(self.y);
        cycles
    }

    #[inline]
    fn transfer_address(&self) -> u16 {
        0x2000 | self.x as u16
    }

    fn adc_with_t(
        &mut self,
        bus: &mut Bus,
        value: u8,
        cycles: u8,
        immediate: bool,
        t_mode_active: bool,
    ) -> u8 {
        if t_mode_active && !immediate {
            let addr = self.transfer_address();
            let mem = bus.read(addr);
            let carry = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
            let sum = value as u16 + mem as u16 + carry as u16;
            let result = sum as u8;
            self.set_flag(FLAG_CARRY, sum > 0xFF);
            self.set_flag(
                FLAG_OVERFLOW,
                (!(value ^ mem) & (value ^ result) & 0x80) != 0,
            );
            bus.write(addr, result);
            self.update_zero_and_negative(result);
            return cycles;
        }
        self.adc(value, cycles)
    }

    fn sbc_with_t(
        &mut self,
        bus: &mut Bus,
        value: u8,
        cycles: u8,
        immediate: bool,
        t_mode_active: bool,
    ) -> u8 {
        if t_mode_active && !immediate {
            let addr = self.transfer_address();
            let mem = bus.read(addr);
            let carry = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
            let subtrahend = value as u16 + (1 - carry) as u16;
            let minuend = mem as u16;
            let result = minuend.wrapping_sub(subtrahend);
            let result_byte = result as u8;
            self.set_flag(FLAG_CARRY, minuend >= subtrahend);
            self.set_flag(
                FLAG_OVERFLOW,
                ((mem ^ result_byte) & (mem ^ value) & 0x80) != 0,
            );
            bus.write(addr, result_byte);
            self.update_zero_and_negative(result_byte);
            return cycles;
        }
        self.sbc(value, cycles)
    }

    fn and_with_t(
        &mut self,
        bus: &mut Bus,
        value: u8,
        cycles: u8,
        immediate: bool,
        t_mode_active: bool,
    ) -> u8 {
        if t_mode_active && !immediate {
            let addr = self.transfer_address();
            let result = value & bus.read(addr);
            bus.write(addr, result);
            self.update_zero_and_negative(result);
            return cycles;
        }
        self.and(value, cycles)
    }

    fn ora_with_t(
        &mut self,
        bus: &mut Bus,
        value: u8,
        cycles: u8,
        immediate: bool,
        t_mode_active: bool,
    ) -> u8 {
        if t_mode_active && !immediate {
            let addr = self.transfer_address();
            let result = value | bus.read(addr);
            bus.write(addr, result);
            self.update_zero_and_negative(result);
            return cycles;
        }
        self.ora(value, cycles)
    }

    fn eor_with_t(
        &mut self,
        bus: &mut Bus,
        value: u8,
        cycles: u8,
        immediate: bool,
        t_mode_active: bool,
    ) -> u8 {
        if t_mode_active && !immediate {
            let addr = self.transfer_address();
            let result = value ^ bus.read(addr);
            bus.write(addr, result);
            self.update_zero_and_negative(result);
            return cycles;
        }
        self.eor(value, cycles)
    }

    fn adc(&mut self, value: u8, cycles: u8) -> u8 {
        let carry = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let binary_sum = self.a as u16 + value as u16 + carry as u16;
        let binary_result = binary_sum as u8;

        self.set_flag(
            FLAG_OVERFLOW,
            (!(self.a ^ value) & (self.a ^ binary_result) & 0x80) != 0,
        );
        if self.get_flag(FLAG_DECIMAL) {
            let mut bcd_sum = binary_sum;
            if (self.a & 0x0F) as u16 + (value & 0x0F) as u16 + carry as u16 > 9 {
                bcd_sum = bcd_sum.wrapping_add(0x06);
            }
            self.set_flag(FLAG_CARRY, bcd_sum > 0x99);
            if bcd_sum > 0x99 {
                bcd_sum = bcd_sum.wrapping_add(0x60);
            }
            self.a = bcd_sum as u8;
        } else {
            self.set_flag(FLAG_CARRY, binary_sum > 0xFF);
            self.a = binary_result;
        }
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn sbc(&mut self, value: u8, cycles: u8) -> u8 {
        let carry = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let subtrahend = value as u16 + (1 - carry) as u16;
        let minuend = self.a as u16;
        let result = minuend.wrapping_sub(subtrahend);
        let binary_result = result as u8;

        self.set_flag(
            FLAG_OVERFLOW,
            ((self.a ^ binary_result) & (self.a ^ value) & 0x80) != 0,
        );
        self.set_flag(FLAG_CARRY, minuend >= subtrahend);
        if self.get_flag(FLAG_DECIMAL) {
            let mut low = (self.a & 0x0F) as i16 - (value & 0x0F) as i16 - (1 - carry) as i16;
            let mut high = (self.a >> 4) as i16 - (value >> 4) as i16;
            if low < 0 {
                low -= 6;
                high -= 1;
            }
            if high < 0 {
                high -= 6;
            }
            self.a = (((high << 4) & 0xF0) | (low & 0x0F)) as u8;
        } else {
            self.a = binary_result;
        }
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn and(&mut self, value: u8, cycles: u8) -> u8 {
        self.a &= value;
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn anc(&mut self, value: u8, cycles: u8) -> u8 {
        self.a &= value;
        self.update_zero_and_negative(self.a);
        self.set_flag(FLAG_CARRY, (self.a & 0x80) != 0);
        cycles
    }

    fn ora(&mut self, value: u8, cycles: u8) -> u8 {
        self.a |= value;
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn eor(&mut self, value: u8, cycles: u8) -> u8 {
        self.a ^= value;
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn asl_acc(&mut self, cycles: u8) -> u8 {
        let carry = (self.a & 0x80) != 0;
        self.a = self.a.wrapping_shl(1);
        self.set_flag(FLAG_CARRY, carry);
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn asl_mem(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page);
        let carry = (value & 0x80) != 0;
        let result = value.wrapping_shl(1);
        Cpu::write_operand(bus, addr, result, zero_page);
        self.set_flag(FLAG_CARRY, carry);
        self.update_zero_and_negative(result);
        cycles
    }

    fn lsr_acc(&mut self, cycles: u8) -> u8 {
        let carry = (self.a & 0x01) != 0;
        self.a >>= 1;
        self.set_flag(FLAG_CARRY, carry);
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn lsr_mem(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page);
        let carry = (value & 0x01) != 0;
        let result = value >> 1;
        Cpu::write_operand(bus, addr, result, zero_page);
        self.set_flag(FLAG_CARRY, carry);
        self.update_zero_and_negative(result);
        cycles
    }

    fn rol_acc(&mut self, cycles: u8) -> u8 {
        let carry_in = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let carry_out = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | carry_in;
        self.set_flag(FLAG_CARRY, carry_out);
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn rol_mem(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page);
        let carry_in = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let carry_out = (value & 0x80) != 0;
        let result = (value << 1) | carry_in;
        Cpu::write_operand(bus, addr, result, zero_page);
        self.set_flag(FLAG_CARRY, carry_out);
        self.update_zero_and_negative(result);
        cycles
    }

    fn ror_acc(&mut self, cycles: u8) -> u8 {
        let carry_in = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let carry_out = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | (carry_in << 7);
        self.set_flag(FLAG_CARRY, carry_out);
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn ror_mem(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page);
        let carry_in = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let carry_out = (value & 0x01) != 0;
        let result = (value >> 1) | (carry_in << 7);
        Cpu::write_operand(bus, addr, result, zero_page);
        self.set_flag(FLAG_CARRY, carry_out);
        self.update_zero_and_negative(result);
        cycles
    }

    fn rra_mem(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page);
        let carry_in = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let carry_out = (value & 0x01) != 0;
        let rotated = (value >> 1) | (carry_in << 7);
        Cpu::write_operand(bus, addr, rotated, zero_page);
        self.set_flag(FLAG_CARRY, carry_out);
        let _ = self.adc(rotated, 0);
        cycles
    }

    fn inc_mem(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page).wrapping_add(1);
        Cpu::write_operand(bus, addr, value, zero_page);
        self.update_zero_and_negative(value);
        cycles
    }

    fn dec_mem(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page).wrapping_sub(1);
        Cpu::write_operand(bus, addr, value, zero_page);
        self.update_zero_and_negative(value);
        cycles
    }

    fn pha(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        self.push_byte(bus, self.a);
        cycles
    }

    fn phy(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        self.push_byte(bus, self.y);
        cycles
    }

    fn phx(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        self.push_byte(bus, self.x);
        cycles
    }

    fn php(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let value = self.status | FLAG_BREAK;
        self.push_byte(bus, value);
        cycles
    }

    fn pla(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let value = self.pop_byte(bus);
        self.a = value;
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn ply(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        self.y = self.pop_byte(bus);
        self.update_zero_and_negative(self.y);
        cycles
    }

    fn plx(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        self.x = self.pop_byte(bus);
        self.update_zero_and_negative(self.x);
        cycles
    }

    fn plp(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let value = self.pop_byte(bus);
        self.status = (self.status & 0x30) | (value & 0xCF);
        self.halted = false;
        self.waiting = false;
        cycles
    }

    fn rti(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let status = self.pop_byte(bus);
        self.status = (self.status & 0x30) | (status & 0xCF);
        let lo = self.pop_byte(bus) as u16;
        let hi = self.pop_byte(bus) as u16;
        self.pc = (hi << 8) | lo;
        self.halted = false;
        self.waiting = false;
        cycles
    }

    fn wai(&mut self, cycles: u8) -> u8 {
        self.waiting = true;
        cycles
    }

    fn tam(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let mask = self.fetch_byte(bus);
        for i in 0..8 {
            if mask & (1 << i) != 0 {
                bus.set_mpr(i, self.a);
            }
        }
        cycles
    }

    fn tma(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let mask = self.fetch_byte(bus);
        let mut value = self.a;
        for i in 0..8 {
            if mask & (1 << i) != 0 {
                value = bus.mpr(i);
                break;
            }
        }
        self.a = value;
        cycles
    }

    fn rmb(&mut self, bus: &mut Bus, bit: u8, cycles: u8) -> u8 {
        let addr = self.fetch_byte(bus);
        let mut value = bus.read_zero_page(addr);
        value &= !(1 << bit);
        bus.write_zero_page(addr, value);
        cycles
    }

    fn smb(&mut self, bus: &mut Bus, bit: u8, cycles: u8) -> u8 {
        let addr = self.fetch_byte(bus);
        let mut value = bus.read_zero_page(addr);
        value |= 1 << bit;
        bus.write_zero_page(addr, value);
        cycles
    }

    fn bbr(&mut self, bus: &mut Bus, bit: u8, cycles: u8) -> u8 {
        self.branch_on_bit(bus, bit, false, cycles)
    }

    fn bbs(&mut self, bus: &mut Bus, bit: u8, cycles: u8) -> u8 {
        self.branch_on_bit(bus, bit, true, cycles)
    }

    fn tst_zero_page(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let mask = self.fetch_byte(bus);
        let addr = self.fetch_byte(bus);
        let value = bus.read_zero_page(addr);
        self.tst(mask, value);
        cycles
    }

    fn tst_zero_page_x(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let mask = self.fetch_byte(bus);
        let addr = self.fetch_byte(bus).wrapping_add(self.x);
        let value = bus.read_zero_page(addr);
        self.tst(mask, value);
        cycles
    }

    fn tst_absolute(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let mask = self.fetch_byte(bus);
        let addr = self.fetch_word(bus);
        let value = bus.read(addr);
        self.tst(mask, value);
        cycles
    }

    fn tst_absolute_x(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let mask = self.fetch_byte(bus);
        let base = self.fetch_word(bus);
        let addr = base.wrapping_add(self.x as u16);
        let value = bus.read(addr);
        self.tst(mask, value);
        cycles
    }

    fn st_port(&mut self, bus: &mut Bus, port: usize, cycles: u8) -> u8 {
        let value = self.fetch_byte(bus);
        bus.write_st_port(port, value);
        cycles
    }

    fn stp(&mut self, cycles: u8) -> u8 {
        self.halted = true;
        cycles
    }

    fn inx(&mut self, cycles: u8) -> u8 {
        self.x = self.x.wrapping_add(1);
        self.update_zero_and_negative(self.x);
        cycles
    }

    fn iny(&mut self, cycles: u8) -> u8 {
        self.y = self.y.wrapping_add(1);
        self.update_zero_and_negative(self.y);
        cycles
    }

    fn ina(&mut self, cycles: u8) -> u8 {
        self.a = self.a.wrapping_add(1);
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn dex(&mut self, cycles: u8) -> u8 {
        self.x = self.x.wrapping_sub(1);
        self.update_zero_and_negative(self.x);
        cycles
    }

    fn dey(&mut self, cycles: u8) -> u8 {
        self.y = self.y.wrapping_sub(1);
        self.update_zero_and_negative(self.y);
        cycles
    }

    fn dea(&mut self, cycles: u8) -> u8 {
        self.a = self.a.wrapping_sub(1);
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn cmp(&mut self, value: u8, register: u8, cycles: u8) -> u8 {
        let result = register.wrapping_sub(value);
        self.set_flag(FLAG_CARRY, register >= value);
        self.update_zero_and_negative(result);
        cycles
    }

    fn bit(&mut self, value: u8, cycles: u8) -> u8 {
        self.set_flag(FLAG_ZERO, (self.a & value) == 0);
        self.set_flag(FLAG_NEGATIVE, value & 0x80 != 0);
        self.set_flag(FLAG_OVERFLOW, value & 0x40 != 0);
        cycles
    }

    fn stz(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        Cpu::write_operand(bus, addr, 0, zero_page);
        cycles
    }

    fn tsb(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page);
        let test = self.a & value;
        self.set_flag(FLAG_ZERO, test == 0);
        let result = value | self.a;
        Cpu::write_operand(bus, addr, result, zero_page);
        cycles
    }

    fn trb(&mut self, bus: &mut Bus, addr: u16, cycles: u8) -> u8 {
        let zero_page = addr < 0x0100;
        let value = Cpu::read_operand(bus, addr, zero_page);
        let test = self.a & value;
        self.set_flag(FLAG_ZERO, test == 0);
        let result = value & !self.a;
        Cpu::write_operand(bus, addr, result, zero_page);
        cycles
    }

    fn handle_interrupt(&mut self, bus: &mut Bus, vector: u16, set_break: bool) -> u8 {
        let pc = self.pc;
        self.push_byte(bus, (pc >> 8) as u8);
        self.push_byte(bus, pc as u8);
        let mut status = self.status;
        if set_break {
            status |= FLAG_BREAK;
        } else {
            status &= !FLAG_BREAK;
        }
        self.push_byte(bus, status);
        self.set_flag(FLAG_INTERRUPT_DISABLE, true);
        self.set_flag(FLAG_DECIMAL, false);
        self.pc = bus.read_u16(vector);
        self.waiting = false;
        self.halted = false;
        8 // HuC6280: IRQ/NMI vectoring takes 8 cycles
    }

    fn exec_block_transfer(&mut self, bus: &mut Bus, mode: BlockMode, cycles: u8) -> u8 {
        let (source, dest, length) = self.fetch_block_params(bus);

        // Hardware pushes A, X, Y to the stack before the transfer.
        self.push_byte(bus, self.a);
        self.push_byte(bus, self.x);
        self.push_byte(bus, self.y);

        let transfer_cycles = self.block_transfer(bus, source, dest, length, mode);
        self.block_transfer_cycles = Some(transfer_cycles);

        self.y = self.pop_byte(bus);
        self.x = self.pop_byte(bus);
        self.a = self.pop_byte(bus);
        cycles
    }

    fn block_transfer(
        &mut self,
        bus: &mut Bus,
        source: u16,
        dest: u16,
        length: u32,
        mode: BlockMode,
    ) -> u32 {
        let mut remaining = length;
        let mut src_ptr = source;
        let mut dest_ptr = dest;
        let mut dest_alt: u16 = 0;
        let mut src_alt: u16 = 0;
        let mut cycles: u32 = 17;

        while remaining > 0 {
            match mode {
                BlockMode::Tii => {
                    let value = bus.read(src_ptr);
                    bus.write(dest_ptr, value);
                    src_ptr = src_ptr.wrapping_add(1);
                    dest_ptr = dest_ptr.wrapping_add(1);
                }
                BlockMode::Tdd => {
                    let value = bus.read(src_ptr);
                    bus.write(dest_ptr, value);
                    src_ptr = src_ptr.wrapping_sub(1);
                    dest_ptr = dest_ptr.wrapping_sub(1);
                }
                BlockMode::Tin => {
                    let value = bus.read(src_ptr);
                    bus.write(dest_ptr, value);
                    src_ptr = src_ptr.wrapping_add(1);
                }
                BlockMode::Tia => {
                    let value = bus.read(src_ptr);
                    let target = dest.wrapping_add(dest_alt);
                    bus.write(target, value);
                    src_ptr = src_ptr.wrapping_add(1);
                    dest_alt ^= 1;
                }
                BlockMode::Tai => {
                    let addr = source.wrapping_add(src_alt);
                    let value = bus.read(addr);
                    bus.write(dest_ptr, value);
                    dest_ptr = dest_ptr.wrapping_add(1);
                    src_alt ^= 1;
                }
            }

            remaining -= 1;
            cycles = cycles.saturating_add(6);
        }

        self.waiting = false;
        cycles
    }

    fn branch_on_bit(&mut self, bus: &mut Bus, bit: u8, branch_if_set: bool, cycles: u8) -> u8 {
        let zp_addr = self.fetch_byte(bus);
        let value = bus.read_zero_page(zp_addr);
        let offset = self.fetch_byte(bus) as i8;
        let bit_set = (value & (1 << bit)) != 0;
        let condition = if branch_if_set { bit_set } else { !bit_set };

        if condition {
            let target = ((self.pc as i32 + offset as i32) as u32) as u16;
            self.pc = target;
            // HuC6280: +2 for taken, no page-crossing penalty
            cycles + 2
        } else {
            cycles
        }
    }

    fn tst(&mut self, mask: u8, value: u8) {
        self.set_flag(FLAG_ZERO, (mask & value) == 0);
        self.set_flag(FLAG_NEGATIVE, value & 0x80 != 0);
        self.set_flag(FLAG_OVERFLOW, value & 0x40 != 0);
    }

    fn cla(&mut self, cycles: u8) -> u8 {
        self.a = 0;
        cycles
    }

    fn clx(&mut self, cycles: u8) -> u8 {
        self.x = 0;
        cycles
    }

    fn cly(&mut self, cycles: u8) -> u8 {
        self.y = 0;
        cycles
    }

    fn sax(&mut self, cycles: u8) -> u8 {
        std::mem::swap(&mut self.a, &mut self.x);
        cycles
    }

    fn say(&mut self, cycles: u8) -> u8 {
        std::mem::swap(&mut self.a, &mut self.y);
        cycles
    }

    fn sxy(&mut self, cycles: u8) -> u8 {
        std::mem::swap(&mut self.x, &mut self.y);
        cycles
    }

    fn set_t_flag(&mut self, cycles: u8) -> u8 {
        self.set_flag(FLAG_T, true);
        cycles
    }

    fn csh(&mut self, cycles: u8) -> u8 {
        self.clock_high_speed = true;
        self.set_flag(FLAG_T, false);
        cycles
    }

    fn csl(&mut self, cycles: u8) -> u8 {
        self.clock_high_speed = false;
        self.set_flag(FLAG_T, false);
        cycles
    }

    fn bsr(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let offset = self.fetch_byte(bus) as i8;
        let return_addr = self.pc.wrapping_sub(1);
        self.push_byte(bus, (return_addr >> 8) as u8);
        self.push_byte(bus, return_addr as u8);
        self.pc = ((self.pc as i32 + offset as i32) as u32) as u16;
        cycles
    }

    fn branch(&mut self, bus: &mut Bus, condition: bool, cycles: u8) -> u8 {
        let offset = self.fetch_byte(bus) as i8;
        if condition {
            self.pc = ((self.pc as i32 + offset as i32) as u32) as u16;
            // HuC6280: taken branches always +2 cycles, no page-crossing penalty
            cycles.saturating_add(2)
        } else {
            cycles
        }
    }

    fn tax(&mut self, cycles: u8) -> u8 {
        self.x = self.a;
        self.update_zero_and_negative(self.x);
        cycles
    }

    fn tay(&mut self, cycles: u8) -> u8 {
        self.y = self.a;
        self.update_zero_and_negative(self.y);
        cycles
    }

    fn txa(&mut self, cycles: u8) -> u8 {
        self.a = self.x;
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn tya(&mut self, cycles: u8) -> u8 {
        self.a = self.y;
        self.update_zero_and_negative(self.a);
        cycles
    }

    fn tsx(&mut self, cycles: u8) -> u8 {
        self.x = self.sp;
        self.update_zero_and_negative(self.x);
        cycles
    }

    fn txs(&mut self, cycles: u8) -> u8 {
        self.sp = self.x;
        cycles
    }

    fn jsr(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let addr = self.addr_absolute(bus);
        let return_addr = self.pc.wrapping_sub(1);
        self.push_byte(bus, (return_addr >> 8) as u8);
        self.push_byte(bus, return_addr as u8);
        self.pc = addr;
        cycles
    }

    fn rts(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let lo = self.pop_byte(bus) as u16;
        let hi = self.pop_byte(bus) as u16;
        self.pc = ((hi << 8) | lo).wrapping_add(1);
        cycles
    }

    fn brk(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        // BRK consumes an extra byte, so advance PC to skip the padding.
        self.pc = self.pc.wrapping_add(1);
        let vector = bus.read_u16(VECTOR_IRQ2_BRK);
        if !Self::vector_initialized(vector) {
            // No BRK vector installed: emulate development ROMs/tests by halting.
            self.halted = true;
            return cycles;
        }

        // Defer to the standard interrupt sequence so cartridge handlers observe
        // the pushed PC/status bytes just like on hardware.
        let _ = self.handle_interrupt(bus, VECTOR_IRQ2_BRK, true);
        cycles
    }

    fn jmp_absolute(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let target = self.fetch_word(bus);
        self.pc = target;
        cycles
    }

    fn jmp_indirect(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let ptr = self.fetch_word(bus);
        let lo = bus.read(ptr);
        let hi_addr = ptr.wrapping_add(1);
        let hi = bus.read(hi_addr);
        self.pc = ((hi as u16) << 8) | lo as u16;
        cycles
    }

    fn jmp_indirect_indexed(&mut self, bus: &mut Bus, cycles: u8) -> u8 {
        let base = self.fetch_word(bus);
        let ptr = base.wrapping_add(self.x as u16);
        let lo = bus.read(ptr);
        let hi = bus.read(ptr.wrapping_add(1));
        self.pc = ((hi as u16) << 8) | lo as u16;
        cycles
    }

    fn addr_zeropage(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_byte(bus) as u16
    }

    fn addr_zeropage_x(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_byte(bus).wrapping_add(self.x) as u16
    }

    fn addr_zeropage_y(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_byte(bus).wrapping_add(self.y) as u16
    }

    fn addr_absolute(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_word(bus)
    }

    fn addr_absolute_x(&mut self, bus: &mut Bus) -> (u16, bool) {
        let base = self.fetch_word(bus);
        let addr = base.wrapping_add(self.x as u16);
        (addr, Cpu::page_crossed(base, addr))
    }

    fn addr_absolute_y(&mut self, bus: &mut Bus) -> (u16, bool) {
        let base = self.fetch_word(bus);
        let addr = base.wrapping_add(self.y as u16);
        (addr, Cpu::page_crossed(base, addr))
    }

    fn addr_indirect(&mut self, bus: &mut Bus) -> u16 {
        let ptr = self.fetch_byte(bus);
        Cpu::read_zero_page_word(bus, ptr)
    }

    fn addr_indexed_indirect_x(&mut self, bus: &mut Bus) -> u16 {
        let base = self.fetch_byte(bus).wrapping_add(self.x);
        Cpu::read_zero_page_word(bus, base)
    }

    fn addr_indirect_y(&mut self, bus: &mut Bus) -> (u16, bool) {
        let base_ptr = self.fetch_byte(bus);
        let base = Cpu::read_zero_page_word(bus, base_ptr);
        let addr = base.wrapping_add(self.y as u16);
        (addr, Cpu::page_crossed(base, addr))
    }

    fn fetch_byte(&mut self, bus: &mut Bus) -> u8 {
        let value = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn fetch_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch_byte(bus) as u16;
        let hi = self.fetch_byte(bus) as u16;
        (hi << 8) | lo
    }

    fn fetch_block_params(&mut self, bus: &mut Bus) -> (u16, u16, u32) {
        let src_lo = self.fetch_byte(bus) as u16;
        let src_hi = self.fetch_byte(bus) as u16;
        let dst_lo = self.fetch_byte(bus) as u16;
        let dst_hi = self.fetch_byte(bus) as u16;
        let len_lo = self.fetch_byte(bus) as u16;
        let len_hi = self.fetch_byte(bus) as u16;
        let source = (src_hi << 8) | src_lo;
        let dest = (dst_hi << 8) | dst_lo;
        let length_raw = (len_hi << 8) | len_lo;
        let length = if length_raw == 0 {
            0x1_0000
        } else {
            length_raw as u32
        };
        (source, dest, length)
    }

    fn read_zero_page_word(bus: &mut Bus, addr: u8) -> u16 {
        let lo = bus.read_zero_page(addr) as u16;
        let hi = bus.read_zero_page(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    #[inline]
    fn read_operand(bus: &mut Bus, addr: u16, zero_page: bool) -> u8 {
        if zero_page {
            bus.read_zero_page(addr as u8)
        } else {
            bus.read(addr)
        }
    }

    #[inline]
    fn write_operand(bus: &mut Bus, addr: u16, value: u8, zero_page: bool) {
        if zero_page {
            bus.write_zero_page(addr as u8, value);
        } else {
            bus.write(addr, value);
        }
    }

    fn push_byte(&mut self, bus: &mut Bus, value: u8) {
        let addr = 0x0100 | self.sp as u16;
        bus.stack_write(addr, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop_byte(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let addr = 0x0100 | self.sp as u16;
        bus.stack_read(addr)
    }

    fn update_zero_and_negative(&mut self, value: u8) {
        self.set_flag(FLAG_ZERO, value == 0);
        self.set_flag(FLAG_NEGATIVE, value & 0x80 != 0);
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }

    fn get_flag(&self, flag: u8) -> bool {
        self.status & flag != 0
    }

    fn page_crossed(a: u16, b: u16) -> bool {
        (a & 0xFF00) != (b & 0xFF00)
    }

    #[inline]
    fn vector_initialized(vector: u16) -> bool {
        vector != 0x0000 && vector != 0xFFFF
    }

    fn vector_slot_with_fallback(bus: &mut Bus, primary: u16, fallback: u16) -> u16 {
        let primary_value = bus.read_u16(primary);
        if primary == fallback || Self::vector_initialized(primary_value) {
            return primary;
        }

        let fallback_value = bus.read_u16(fallback);
        if Self::vector_initialized(fallback_value) {
            fallback
        } else {
            primary
        }
    }

    fn vector_slot_for_irq_source(bus: &mut Bus, source: u8) -> u16 {
        if source & IRQ_REQUEST_TIMER != 0 {
            return Self::vector_slot_with_fallback(bus, VECTOR_TIMER, VECTOR_LEGACY_SHARED_IRQ);
        }
        if source & IRQ_REQUEST_IRQ1 != 0 {
            return Self::vector_slot_with_fallback(bus, VECTOR_IRQ1, VECTOR_LEGACY_SHARED_IRQ);
        }
        Self::vector_slot_with_fallback(bus, VECTOR_IRQ2_BRK, VECTOR_LEGACY_SHARED_IRQ)
    }

    #[allow(dead_code)]
    pub fn flag(&self, flag: u8) -> bool {
        self.get_flag(flag)
    }

    pub fn is_waiting(&self) -> bool {
        self.waiting
    }

    pub fn last_opcode(&self) -> u8 {
        self.last_opcode
    }
}

#[derive(Clone, Copy, Debug)]
enum BlockMode {
    Tii,
    Tin,
    Tdd,
    Tia,
    Tai,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{IRQ_REQUEST_IRQ1, IRQ_REQUEST_IRQ2, IRQ_REQUEST_TIMER, PAGE_SIZE};

    fn setup_cpu_with_program(program: &[u8]) -> (Cpu, Bus) {
        let mut bus = Bus::new();
        bus.load(0x8000, program);
        bus.write_u16(0xFFFC, 0x8000);

        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);
        (cpu, bus)
    }

    fn block_transfer_program(opcode: u8, source: u16, dest: u16, length: u16) -> [u8; 7] {
        [
            opcode,
            (source & 0x00FF) as u8,
            (source >> 8) as u8,
            (dest & 0x00FF) as u8,
            (dest >> 8) as u8,
            (length & 0x00FF) as u8,
            (length >> 8) as u8,
        ]
    }

    #[test]
    fn opcode_cycle_table_covers_implemented_dispatch_set() {
        let implemented = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1C,
            0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
            0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
            0x3A, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
            0x49, 0x4A, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57,
            0x58, 0x59, 0x5A, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x64, 0x65, 0x66, 0x67, 0x68,
            0x69, 0x6A, 0x6C, 0x6D, 0x6E, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77,
            0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85,
            0x86, 0x87, 0x88, 0x89, 0x8A, 0x8C, 0x8D, 0x8E, 0x8F, 0x90, 0x91, 0x92, 0x93, 0x94,
            0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2, 0xA3,
            0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAC, 0xAD, 0xAE, 0xAF, 0xB0, 0xB1, 0xB2,
            0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xC1,
            0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF,
            0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDD, 0xDE,
            0xDF, 0xE0, 0xE1, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED,
            0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFD,
            0xFE, 0xFF, 0x1B, 0x33, 0x3B, 0x4B, 0x5B, 0x5C, 0x63, 0x6B, 0x8B, 0x9B, 0xAB, 0xBB,
            0xDC, 0xE2, 0xFB, 0xFC,
        ];
        assert_eq!(implemented.len(), 256);

        let mut implemented_mask = [false; 256];
        for opcode in implemented {
            implemented_mask[opcode as usize] = true;
            assert_ne!(
                Cpu::opcode_base_cycles(opcode),
                0,
                "implemented opcode {:02X} has zero cycle entry",
                opcode
            );
        }

        for opcode in 0u8..=u8::MAX {
            let cycles = Cpu::opcode_base_cycles(opcode);
            if implemented_mask[opcode as usize] {
                assert_ne!(cycles, 0, "missing cycle entry for opcode {:02X}", opcode);
            } else {
                assert_eq!(
                    cycles, 0,
                    "unexpected cycle entry for opcode {:02X}",
                    opcode
                );
            }
        }

        for (opcode, expected) in [(0xA9, 2), (0xB1, 7), (0x7B, 8), (0x44, 8), (0x73, 17)] {
            assert_eq!(Cpu::opcode_base_cycles(opcode), expected);
        }
    }

    #[test]
    fn undefined_opcodes_behave_as_nops() {
        let program = [
            0x1B, 0x33, 0x3B, 0x4B, 0x5B, 0x5C, 0x63, 0x6B, 0x8B, 0x9B, 0xAB, 0xBB, 0xDC, 0xE2,
            0xFB, 0xFC, 0x00,
        ];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x12;
        cpu.x = 0x34;
        cpu.y = 0x56;
        cpu.sp = 0xEF;
        cpu.status = FLAG_CARRY | FLAG_OVERFLOW;
        let start_pc = cpu.pc;

        for i in 0..16u16 {
            let cycles = cpu.step(&mut bus);
            assert_eq!(cycles, 2, "opcode at index {i} should be 2-cycle NOP");
            assert_eq!(cpu.pc, start_pc + i + 1);
            assert_eq!(cpu.a, 0x12);
            assert_eq!(cpu.x, 0x34);
            assert_eq!(cpu.y, 0x56);
            assert_eq!(cpu.sp, 0xEF);
            assert_eq!(cpu.status, FLAG_CARRY | FLAG_OVERFLOW);
            assert!(!cpu.halted);
        }
    }

    #[test]
    fn adc_handles_carry_and_overflow() {
        let program = [0x69, 0x01, 0x69, 0x80, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x7F;

        cpu.step(&mut bus); // ADC #$01 => 0x80
        assert_eq!(cpu.a, 0x80);
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(cpu.flag(FLAG_OVERFLOW));

        cpu.step(&mut bus); // ADC #$80 => 0x00 with carry
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn adc_decimal_mode_adds_bcd_values() {
        let program = [0xF8, 0x69, 0x34, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x12;
        cpu.set_flag(FLAG_CARRY, false);

        cpu.step(&mut bus); // SED
        cpu.step(&mut bus); // ADC #$34
        assert_eq!(cpu.a, 0x46);
        assert!(!cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn adc_decimal_mode_handles_digit_carry() {
        let program = [0xF8, 0x69, 0x27, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x19;
        cpu.set_flag(FLAG_CARRY, false);

        cpu.step(&mut bus); // SED
        cpu.step(&mut bus); // ADC #$27
        assert_eq!(cpu.a, 0x46);
        assert!(!cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn adc_decimal_mode_uses_input_carry() {
        let program = [0xF8, 0x69, 0x00, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x99;
        cpu.set_flag(FLAG_CARRY, true);

        cpu.step(&mut bus); // SED
        cpu.step(&mut bus); // ADC #$00 with carry-in
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag(FLAG_CARRY));
        assert!(cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn sbc_decimal_mode_subtracts_bcd_values() {
        let program = [0xF8, 0xE9, 0x29, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x50;
        cpu.set_flag(FLAG_CARRY, true);

        cpu.step(&mut bus); // SED
        cpu.step(&mut bus); // SBC #$29
        assert_eq!(cpu.a, 0x21);
        assert!(cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn sbc_decimal_mode_handles_borrow() {
        let program = [0xF8, 0xE9, 0x01, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x00;
        cpu.set_flag(FLAG_CARRY, true);

        cpu.step(&mut bus); // SED
        cpu.step(&mut bus); // SBC #$01
        assert_eq!(cpu.a, 0x99);
        assert!(!cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn branch_taken_adds_cycles_and_adjusts_pc() {
        // BNE +2 to skip BRK, then immediate BRK to halt.
        let program = [0xD0, 0x02, 0x00, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.status &= !FLAG_ZERO;
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4); // HuC6280: branch taken = base 2 + 2 penalty
        assert_eq!(cpu.pc, 0x8004);
    }

    #[test]
    fn jsr_and_rts_round_trip() {
        // JSR $8004 ; LDA #$42 ; RTS ; BRK
        let program = [0x20, 0x04, 0x80, 0x00, 0xA9, 0x42, 0x60, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.step(&mut bus); // JSR
        assert_eq!(cpu.pc, 0x8004);
        assert_eq!(bus.read(0x01FC), 0x02);
        assert_eq!(bus.read(0x01FD), 0x80);
        cpu.step(&mut bus); // LDA
        assert_eq!(cpu.a, 0x42);
        cpu.step(&mut bus); // RTS
        assert_eq!(cpu.pc, 0x8003); // return to byte after JSR operand
    }

    #[test]
    fn lda_indexed_indirect_x_reads_correct_value() {
        let program = [0xA1, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.x = 0x05;
        bus.write(0x0015, 0x00);
        bus.write(0x0016, 0x90);
        bus.write(0x9000, 0xAB);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cpu.a, 0xAB);
        assert_eq!(cycles, 7);
    }

    #[test]
    fn lda_indirect_y_page_cross_adds_cycle() {
        // HuC6280: (ind),Y is always 7 cycles, no page-crossing penalty
        let program = [0xB1, 0x20, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0020, 0xFF);
        bus.write(0x0021, 0x80);
        bus.write(0x8100, 0x34);
        cpu.y = 0x01;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x34);
        assert_eq!(cycles, 7); // no extra cycle for page cross on HuC6280
    }

    #[test]
    fn sta_indirect_y_stores_value() {
        let program = [0x91, 0x30, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x77;
        cpu.y = 0x05;
        bus.write(0x0030, 0x00);
        bus.write(0x0031, 0x44);

        let cycles = cpu.step(&mut bus);
        assert_eq!(bus.read(0x4405), 0x77);
        assert_eq!(cycles, 7);
    }

    #[test]
    fn zero_page_indirect_logic_adc_cmp_opcodes_work() {
        let program = [
            0xA9, 0x10, // LDA #$10
            0x12, 0x20, // ORA ($20) -> 0x13
            0x32, 0x22, // AND ($22) -> 0x03
            0x52, 0x24, // EOR ($24) -> 0xFC
            0x18, // CLC
            0x72, 0x26, // ADC ($26) -> 0xFD
            0xD2, 0x28, // CMP ($28) -> equal
            0x00,
        ];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.set_flag(FLAG_T, false);
        bus.write(0x0020, 0x00);
        bus.write(0x0021, 0x90);
        bus.write(0x0022, 0x01);
        bus.write(0x0023, 0x90);
        bus.write(0x0024, 0x02);
        bus.write(0x0025, 0x90);
        bus.write(0x0026, 0x03);
        bus.write(0x0027, 0x90);
        bus.write(0x0028, 0x04);
        bus.write(0x0029, 0x90);
        bus.write(0x9000, 0x03);
        bus.write(0x9001, 0x0F);
        bus.write(0x9002, 0xFF);
        bus.write(0x9003, 0x01);
        bus.write(0x9004, 0xFD);

        cpu.step(&mut bus); // LDA
        let cycles = cpu.step(&mut bus); // ORA (zp)
        assert_eq!(cycles, 7);
        assert_eq!(cpu.a, 0x13);

        let cycles = cpu.step(&mut bus); // AND (zp)
        assert_eq!(cycles, 7);
        assert_eq!(cpu.a, 0x03);

        let cycles = cpu.step(&mut bus); // EOR (zp)
        assert_eq!(cycles, 7);
        assert_eq!(cpu.a, 0xFC);

        cpu.step(&mut bus); // CLC
        let cycles = cpu.step(&mut bus); // ADC (zp)
        assert_eq!(cycles, 7);
        assert_eq!(cpu.a, 0xFD);
        assert!(!cpu.flag(FLAG_CARRY));

        let cycles = cpu.step(&mut bus); // CMP (zp)
        assert_eq!(cycles, 7);
        assert!(cpu.flag(FLAG_ZERO));
        assert!(cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn bit_immediate_updates_flags_without_touching_accumulator() {
        let program = [0x89, 0xC0, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0xFF;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 2);
        assert_eq!(cpu.a, 0xFF);
        assert!(!cpu.flag(FLAG_ZERO));
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(cpu.flag(FLAG_OVERFLOW));
    }

    #[test]
    fn bit_zeropage_sets_zero_when_mask_clears_bits() {
        let program = [0x24, 0x40, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x10;
        bus.write(0x0040, 0x04);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4); // HuC6280: ZP read is 4 cycles
        assert!(cpu.flag(FLAG_ZERO));
        assert!(!cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_OVERFLOW));
    }

    #[test]
    fn anc_immediate_updates_carry_from_sign_bit() {
        let program = [0xA9, 0xFF, 0x2B, 0x80, 0x0B, 0x01, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.step(&mut bus); // LDA #$FF
        let cycles = cpu.step(&mut bus); // ANC #$80
        assert_eq!(cycles, 2);
        assert_eq!(cpu.a, 0x80);
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(cpu.flag(FLAG_CARRY));
        assert!(!cpu.flag(FLAG_ZERO));

        let cycles = cpu.step(&mut bus); // ANC #$01 via 0x0B alias
        assert_eq!(cycles, 2);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag(FLAG_ZERO));
        assert!(!cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn asl_accumulator_sets_carry() {
        let program = [0x0A, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x81;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 2);
        assert_eq!(cpu.a, 0x02);
        assert!(cpu.flag(FLAG_CARRY));
        assert!(!cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn ror_zeropage_rotates_through_carry() {
        let program = [0x66, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.status |= FLAG_CARRY;
        bus.write(0x0010, 0x02);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 6); // HuC6280: ZP RMW is 6 cycles
        assert_eq!(bus.read(0x0010), 0x81);
        assert!(!cpu.flag(FLAG_CARRY));
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn rra_absolute_y_rotates_and_adds() {
        let program = [0x7B, 0x00, 0x90, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x10;
        cpu.y = 0x05;
        cpu.status |= FLAG_CARRY;
        bus.write(0x9005, 0x04);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8);
        assert_eq!(bus.read(0x9005), 0x82);
        assert_eq!(cpu.a, 0x92);
        assert!(!cpu.flag(FLAG_CARRY));
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn pha_pla_round_trip() {
        let program = [0xA9, 0x12, 0x48, 0xA9, 0x00, 0x68, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.step(&mut bus); // LDA #$12
        assert_eq!(cpu.a, 0x12);

        cpu.step(&mut bus); // PHA
        assert_eq!(bus.read(0x01FD), 0x12);
        assert_eq!(cpu.sp, 0xFC);

        cpu.step(&mut bus); // LDA #$00
        assert_eq!(cpu.a, 0x00);

        cpu.step(&mut bus); // PLA
        assert_eq!(cpu.a, 0x12);
        assert_eq!(cpu.sp, 0xFD);
        assert!(!cpu.flag(FLAG_ZERO));
        assert!(!cpu.flag(FLAG_NEGATIVE));
    }

    #[test]
    fn php_pushes_status_with_break_bit() {
        let program = [0x08, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.status = FLAG_CARRY;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 3);
        assert_eq!(cpu.sp, 0xFC);
        let pushed = bus.read(0x01FD);
        assert_eq!(pushed, FLAG_CARRY | FLAG_BREAK);
    }

    #[test]
    fn plp_restores_flags_from_stack() {
        let program = [0x28, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.push_byte(&mut bus, FLAG_NEGATIVE | FLAG_CARRY);
        cpu.status = 0;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4);
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(cpu.flag(FLAG_CARRY));
        // T is cleared by step() before PLP executes, and PLP preserves
        // the current T (not the stack value), so T remains false.
        assert!(!cpu.flag(FLAG_T));
    }

    #[test]
    fn plp_preserves_break_and_t_bits() {
        let program = [0x28, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        // Stack value has B and T set, but current status has neither
        cpu.push_byte(&mut bus, FLAG_BREAK | FLAG_T | FLAG_ZERO);
        cpu.status = 0;

        cpu.step(&mut bus);
        assert!(cpu.flag(FLAG_ZERO), "Z should be restored from stack");
        assert!(
            !cpu.flag(FLAG_BREAK),
            "B (bit 4) should be preserved from current status, not stack"
        );
        assert!(
            !cpu.flag(FLAG_T),
            "T (bit 5) should be preserved from current status, not stack"
        );
    }

    #[test]
    fn stz_zeroes_memory_without_touching_a() {
        let program = [0xA9, 0xFF, 0x64, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.step(&mut bus); // LDA #$FF
        cpu.step(&mut bus); // STZ $10

        assert_eq!(cpu.a, 0xFF);
        assert_eq!(bus.read(0x0010), 0x00);
    }

    #[test]
    fn tsb_sets_bits_and_updates_zero_flag() {
        let program = [0xA9, 0x0F, 0x04, 0x20, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0020, 0xF3);

        cpu.step(&mut bus); // LDA #$0F
        cpu.step(&mut bus); // TSB $20

        assert_eq!(bus.read(0x0020), 0xFF);
        assert!(!cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn trb_sets_zero_flag_when_no_overlap() {
        let program = [0xA9, 0xF0, 0x14, 0x30, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0030, 0x0F);

        cpu.step(&mut bus); // LDA #$F0
        cpu.step(&mut bus); // TRB $30 (no overlap)
        assert_eq!(bus.read(0x0030), 0x0F);
        assert!(cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn trb_clears_bits_when_overlap_exists() {
        let program = [0xA9, 0xF0, 0x14, 0x30, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0030, 0xF3);

        cpu.step(&mut bus); // LDA #$F0
        cpu.step(&mut bus); // TRB $30 (overlap)

        assert_eq!(bus.read(0x0030), 0x03);
        assert!(!cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn tii_transfers_incrementing_addresses() {
        let program = [0x73, 0x00, 0x90, 0x00, 0x40, 0x03, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x9000, 0x11);
        bus.write(0x9001, 0x22);
        bus.write(0x9002, 0x33);

        cpu.step(&mut bus);

        assert_eq!(bus.read(0x4000), 0x11);
        assert_eq!(bus.read(0x4001), 0x22);
        assert_eq!(bus.read(0x4002), 0x33);
        assert_eq!(cpu.sp, 0xFD);
    }

    #[test]
    fn tin_leaves_destination_fixed() {
        let program = [0xD3, 0x00, 0x90, 0x00, 0x40, 0x02, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x9000, 0xAA);
        bus.write(0x9001, 0xBB);

        cpu.step(&mut bus);

        assert_eq!(bus.read(0x4000), 0xBB);
        assert_eq!(bus.read(0x4001), 0x00);
        assert_eq!(cpu.sp, 0xFD);
    }

    #[test]
    fn tia_alternates_destination_bytes() {
        let program = [0xE3, 0x00, 0x90, 0x00, 0x40, 0x02, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x9000, 0x5A);
        bus.write(0x9001, 0xC3);

        cpu.step(&mut bus);

        assert_eq!(bus.read(0x4000), 0x5A);
        assert_eq!(bus.read(0x4001), 0xC3);
        assert_eq!(cpu.sp, 0xFD);
    }

    #[test]
    fn tdd_transfers_decrementing_addresses() {
        let program = [0xC3, 0x02, 0x90, 0x02, 0x40, 0x03, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x9002, 0x11);
        bus.write(0x9001, 0x22);
        bus.write(0x9000, 0x33);

        cpu.step(&mut bus);

        assert_eq!(bus.read(0x4002), 0x11);
        assert_eq!(bus.read(0x4001), 0x22);
        assert_eq!(bus.read(0x4000), 0x33);
        assert_eq!(cpu.sp, 0xFD);
    }

    #[test]
    fn tai_reads_alternating_source_bytes() {
        let program = [0xF3, 0x00, 0x90, 0x00, 0x30, 0x04, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x9000, 0xAA);
        bus.write(0x9001, 0xBB);
        bus.write(0x9002, 0xCC);

        cpu.step(&mut bus);

        assert_eq!(bus.read(0x3000), 0xAA);
        assert_eq!(bus.read(0x3001), 0xBB);
        assert_eq!(bus.read(0x3002), 0xAA);
        assert_eq!(bus.read(0x3003), 0xBB);
        assert_eq!(cpu.sp, 0xFD);
    }

    #[test]
    fn block_moves_treat_zero_length_as_65536_iterations() {
        let program = block_transfer_program(0x73, 0x9000, 0x2000, 0x0000);
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x9000, 0x42);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 17 + 6 * 0x1_0000u32);
        assert_eq!(cpu.sp, 0xFD);
    }

    #[test]
    fn block_moves_report_full_cycle_counts() {
        let cases = [
            (0x73, 0x9000, 0x4000), // TII
            (0xC3, 0x9002, 0x4002), // TDD
            (0xD3, 0x9000, 0x4000), // TIN
            (0xE3, 0x9000, 0x4000), // TIA
            (0xF3, 0x9000, 0x4000), // TAI
        ];
        for (opcode, source, dest) in cases {
            let program = block_transfer_program(opcode, source, dest, 0x0003);
            let (mut cpu, mut bus) = setup_cpu_with_program(&program);
            for i in 0..4u16 {
                bus.write(source.wrapping_add(i), (i as u8).wrapping_add(0x10));
            }
            let cycles = cpu.step(&mut bus);
            assert_eq!(
                cycles,
                17 + 6 * 3u32,
                "opcode {:02X} returned unexpected cycle count",
                opcode
            );
        }
    }

    #[test]
    fn block_move_can_target_timer_io_registers() {
        let program = block_transfer_program(0x73, 0x9000, 0x0C00, 0x0002);
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.set_mpr(0, 0xFF);
        bus.write(0x9000, 0x02); // timer reload
        bus.write(0x9001, 0x01); // timer start

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 17 + 6 * 2u32);
        assert_eq!(bus.read(0x0C00), 0x02);
        assert_eq!(bus.read(0x0C01) & 0x01, 0x01);

        // Confirm the timer side effect is live after DMA-style register writes.
        bus.tick(1024u32 * 3, true);
        assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_TIMER, 0);
    }

    #[test]
    fn ina_dea_adjust_accumulator_and_flags() {
        let program = [0x1A, 0x1A, 0x3A, 0x3A, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x7F;
        cpu.step(&mut bus); // INA -> 0x80
        assert_eq!(cpu.a, 0x80);
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_ZERO));

        cpu.step(&mut bus); // INA -> 0x81
        assert_eq!(cpu.a, 0x81);

        cpu.step(&mut bus); // DEA -> 0x80
        assert_eq!(cpu.a, 0x80);
        assert!(cpu.flag(FLAG_NEGATIVE));

        cpu.step(&mut bus); // DEA -> 0x7F
        assert_eq!(cpu.a, 0x7F);
        assert!(!cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_ZERO));
    }

    #[test]
    fn phx_plx_and_phy_ply_round_trip_registers() {
        let program = [0xDA, 0xFA, 0x5A, 0x7A, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.x = 0x42;
        cpu.y = 0x80;
        cpu.step(&mut bus); // PHX (push at $01FD, sp -> 0xFC)
        assert_eq!(bus.read(0x01FD), 0x42);
        cpu.x = 0x00;
        cpu.step(&mut bus); // PLX
        assert_eq!(cpu.x, 0x42);
        assert!(!cpu.flag(FLAG_ZERO));

        cpu.step(&mut bus); // PHY (rewrites $01FD, sp -> 0xFC)
        assert_eq!(bus.read(0x01FD), 0x80);
        cpu.y = 0x00;
        cpu.step(&mut bus); // PLY
        assert_eq!(cpu.y, 0x80);
        assert!(cpu.flag(FLAG_NEGATIVE));
    }

    #[test]
    fn sta_zero_page_indirect_stores_value() {
        let program = [0xA9, 0x5A, 0x92, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x00);
        bus.write(0x0011, 0xC0);

        while !cpu.halted {
            cpu.step(&mut bus);
        }

        assert_eq!(bus.read(0xC000), 0x5A);
    }

    #[test]
    fn jmp_absolute_sets_pc() {
        let program = [0x4C, 0x05, 0x80, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x8005);
    }

    #[test]
    fn jmp_indirect_crosses_page_boundary() {
        // 65C02/HuC6280 fixed the 6502 page-wrap bug: high byte is read
        // from ptr+1 even when ptr is at a page boundary ($xxFF).
        let program = [0x6C, 0xFF, 0x82, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x82FF, 0x34);
        bus.write(0x8300, 0x12); // 65C02 reads from $8300, not $8200
        bus.write(0x8200, 0xFF); // decoy: 6502 would read this
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x1234);
    }

    #[test]
    fn jmp_indirect_indexed_uses_offset() {
        let program = [0xA2, 0x02, 0x7C, 0x00, 0x90, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.load(0x9002, &[0x78, 0x56]);
        cpu.step(&mut bus); // LDX #$02
        cpu.step(&mut bus); // JMP ($9000,X)
        assert_eq!(cpu.pc, 0x5678);
    }

    #[test]
    fn tam_updates_mprs_and_remaps_page() {
        let program = [
            0xA9, 0xF8, // LDA #$F8 (internal RAM window)
            0x53, 0x01, // TAM #$01 (MPR0)
            0xA9, 0x5A, // LDA #$5A
            0x8D, 0x00, 0x00, // STA $0000 -> maps to page selected by MPR0
            0x00,
        ];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        while !cpu.halted {
            cpu.step(&mut bus);
        }

        assert_eq!(bus.mpr(0), 0xF8);
        assert_eq!(bus.read(0x0000), 0x5A);
    }

    #[test]
    fn tma_reads_from_selected_mpr() {
        let program = [0xA9, 0x00, 0x43, 0x08, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.set_mpr(3, 0x44);

        while !cpu.halted {
            cpu.step(&mut bus);
        }

        assert_eq!(cpu.a, 0x44);
        // TMA does not affect flags
    }

    #[test]
    fn rmb_clears_bit_in_zero_page() {
        let program = [0xA9, 0xFF, 0x85, 0x10, 0x07, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.step(&mut bus); // LDA #$FF
        cpu.step(&mut bus); // STA $10
        cpu.step(&mut bus); // RMB0 $10

        assert_eq!(bus.read(0x0010), 0xFE);
    }

    #[test]
    fn smb_sets_bit_in_zero_page() {
        let program = [0xA9, 0x00, 0x85, 0x11, 0xC7, 0x11, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.step(&mut bus); // LDA #$00
        cpu.step(&mut bus); // STA $11
        cpu.step(&mut bus); // SMB4 $11

        assert_eq!(bus.read(0x0011), 0x10);
    }

    #[test]
    fn bbr_branches_when_bit_reset() {
        let program = [0x0F, 0x10, 0x01, 0xEA, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x00);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8); // HuC6280: BBR base 6 + 2 taken
        assert_eq!(cpu.pc, 0x8004);
        cpu.step(&mut bus);
        assert!(cpu.halted);
    }

    #[test]
    fn bbs_skips_when_bit_clear() {
        let program = [0x8F, 0x10, 0x01, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x00);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 6); // HuC6280: BBS not-taken base is 6
        assert_eq!(cpu.pc, 0x8003);
        cpu.step(&mut bus);
        assert!(cpu.halted);
    }

    #[test]
    fn bbs_branches_when_bit_set() {
        let program = [0x8F, 0x10, 0x01, 0xEA, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x01);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8); // HuC6280: BBS base 6 + 2 taken
        assert_eq!(cpu.pc, 0x8004);
        cpu.step(&mut bus);
        assert!(cpu.halted);
    }

    #[test]
    fn bbr_taken_cross_page_costs_extra_cycle() {
        let mut program = vec![0xEA; 0xFC];
        program.extend([0x0F, 0x10, 0x02, 0xEA, 0x00, 0x00]);
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x00);
        cpu.pc = 0x80FC;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x8101);
        assert_eq!(bus.read(0x0010), 0x00);
        cpu.step(&mut bus);
        assert!(cpu.halted);
    }

    #[test]
    fn bbs_taken_cross_page_costs_extra_cycle() {
        let mut program = vec![0xEA; 0xFC];
        program.extend([0x8F, 0x10, 0x02, 0xEA, 0x00, 0x00]);
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x01);
        cpu.pc = 0x80FC;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x8101);
        assert_eq!(bus.read(0x0010), 0x01);
        cpu.step(&mut bus);
        assert!(cpu.halted);
    }

    #[test]
    fn tst_zp_sets_flags_based_on_mask_and_value() {
        let program = [0x83, 0xF0, 0x20, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0020, 0xF0);
        cpu.a = 0x00; // TST does not use A but ensure non-zero

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8); // HuC6280: TST zp is 8 cycles
        assert!(!cpu.flag(FLAG_ZERO));
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(cpu.flag(FLAG_OVERFLOW));
    }

    #[test]
    fn tst_abs_sets_zero_when_masked_out() {
        // mask=0x0F, memory=0xF0: AND result = 0x00 => Z=1
        // N and V come from memory value (0xF0): N=1, V=1
        let program = [0x93, 0x0F, 0x00, 0x90, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x9000, 0xF0);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8);
        assert!(cpu.flag(FLAG_ZERO));
        assert!(cpu.flag(FLAG_NEGATIVE), "N comes from memory bit 7");
        assert!(cpu.flag(FLAG_OVERFLOW), "V comes from memory bit 6");
    }

    #[test]
    fn cla_clx_cly_clear_registers() {
        let program = [0x62, 0x82, 0xC2, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0xFF;
        cpu.x = 0x80;
        cpu.y = 0x01;
        cpu.set_flag(FLAG_NEGATIVE, true);
        cpu.set_flag(FLAG_ZERO, false);

        cpu.step(&mut bus); // CLA
        assert_eq!(cpu.a, 0);
        assert!(cpu.flag(FLAG_NEGATIVE), "CLA should not affect flags");
        assert!(!cpu.flag(FLAG_ZERO), "CLA should not affect flags");

        cpu.step(&mut bus); // CLX
        assert_eq!(cpu.x, 0);
        assert!(cpu.flag(FLAG_NEGATIVE), "CLX should not affect flags");

        cpu.step(&mut bus); // CLY
        assert_eq!(cpu.y, 0);
        assert!(cpu.flag(FLAG_NEGATIVE), "CLY should not affect flags");
    }

    #[test]
    fn sax_say_sxy_swap_registers() {
        let program = [0x22, 0x42, 0x02, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x12;
        cpu.x = 0x34;
        cpu.y = 0x56;

        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x34);
        assert_eq!(cpu.x, 0x12);

        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x56);
        assert_eq!(cpu.y, 0x34);

        cpu.step(&mut bus);
        assert_eq!(cpu.x, 0x34);
        assert_eq!(cpu.y, 0x12);
    }

    #[test]
    fn set_and_clock_switch_instructions() {
        let program = [0xF4, 0xD4, 0x54, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.set_flag(FLAG_T, false);

        cpu.step(&mut bus);
        assert!(cpu.flag(FLAG_T));

        cpu.step(&mut bus);
        assert!(!cpu.flag(FLAG_T));
        assert!(cpu.clock_high_speed);

        cpu.step(&mut bus);
        assert!(!cpu.flag(FLAG_T));
        assert!(!cpu.clock_high_speed);
    }

    #[test]
    fn t_mode_and_writes_back_to_mpr1_x_and_keeps_a() {
        let program = [0xF4, 0x25, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x0F;
        cpu.x = 0x10;
        bus.write(0x0010, 0xA5);
        bus.write(0x2010, 0xF0);

        cpu.step(&mut bus); // SET
        assert!(cpu.flag(FLAG_T));

        cpu.step(&mut bus); // AND zp in T-mode
        assert_eq!(cpu.a, 0x0F, "T-mode AND should not modify A");
        assert_eq!(
            bus.read(0x2010),
            0xA0,
            "T-mode AND should write result to [MPR1:X]"
        );
        assert!(!cpu.flag(FLAG_ZERO));
        assert!(cpu.flag(FLAG_NEGATIVE));
        assert!(!cpu.flag(FLAG_T), "T flag should clear after ALU op");
    }

    #[test]
    fn t_mode_adc_writes_back_to_mpr1_x_and_clears_t() {
        let program = [0xF4, 0x65, 0x20, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x7F;
        cpu.x = 0x20;
        bus.write(0x0020, 0x10);
        cpu.set_flag(FLAG_CARRY, false);
        bus.write(0x2020, 0x20);

        cpu.step(&mut bus); // SET
        assert!(cpu.flag(FLAG_T));

        cpu.step(&mut bus); // ADC zp in T-mode
        assert_eq!(cpu.a, 0x7F, "T-mode ADC should not modify A");
        assert_eq!(bus.read(0x2020), 0x30);
        assert!(!cpu.flag(FLAG_ZERO));
        assert!(!cpu.flag(FLAG_T));
    }

    #[test]
    fn t_mode_ora_uses_operand_value_instead_of_a() {
        let program = [0xF4, 0x05, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x01;
        cpu.x = 0x10;
        bus.write(0x0010, 0x20);
        bus.write(0x2010, 0x40);

        cpu.step(&mut bus); // SET
        cpu.step(&mut bus); // ORA zp in T-mode
        assert_eq!(cpu.a, 0x01);
        assert_eq!(bus.read(0x2010), 0x60);
        assert!(!cpu.flag(FLAG_T));
    }

    #[test]
    fn t_mode_eor_uses_operand_value_instead_of_a() {
        let program = [0xF4, 0x45, 0x11, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0xF0;
        cpu.x = 0x11;
        bus.write(0x0011, 0xAA);
        bus.write(0x2011, 0x0F);

        cpu.step(&mut bus); // SET
        cpu.step(&mut bus); // EOR zp in T-mode
        assert_eq!(cpu.a, 0xF0);
        assert_eq!(bus.read(0x2011), 0xA5);
        assert!(!cpu.flag(FLAG_T));
    }

    #[test]
    fn t_mode_sbc_uses_operand_value_instead_of_a() {
        let program = [0xF4, 0xE5, 0x12, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x01;
        cpu.x = 0x12;
        cpu.set_flag(FLAG_CARRY, true);
        bus.write(0x0012, 0x20);
        bus.write(0x2012, 0x50);

        cpu.step(&mut bus); // SET
        cpu.step(&mut bus); // SBC zp in T-mode
        assert_eq!(cpu.a, 0x01);
        assert_eq!(bus.read(0x2012), 0x30);
        assert!(cpu.flag(FLAG_CARRY));
        assert!(!cpu.flag(FLAG_T));
    }

    #[test]
    fn t_mode_adc_overflow_flag_uses_transfer_operands() {
        let program = [0xF4, 0x65, 0x13, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x00;
        cpu.x = 0x13;
        cpu.set_flag(FLAG_CARRY, false);
        bus.write(0x0013, 0x01);
        bus.write(0x2013, 0x7F);

        cpu.step(&mut bus); // SET
        cpu.step(&mut bus); // ADC zp in T-mode
        assert_eq!(bus.read(0x2013), 0x80);
        assert!(cpu.flag(FLAG_OVERFLOW));
    }

    #[test]
    fn t_mode_adc_overflow_flag_clears_without_signed_wrap() {
        let program = [0xF4, 0x65, 0x14, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x00;
        cpu.x = 0x14;
        cpu.set_flag(FLAG_CARRY, false);
        bus.write(0x0014, 0x10);
        bus.write(0x2014, 0x20);

        cpu.step(&mut bus); // SET
        cpu.step(&mut bus); // ADC zp in T-mode
        assert_eq!(bus.read(0x2014), 0x30);
        assert!(!cpu.flag(FLAG_OVERFLOW));
    }

    #[test]
    fn t_mode_sbc_overflow_flag_uses_transfer_operands() {
        let program = [0xF4, 0xE5, 0x15, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0xFF;
        cpu.x = 0x15;
        cpu.set_flag(FLAG_CARRY, true);
        bus.write(0x0015, 0x01);
        bus.write(0x2015, 0x80);

        cpu.step(&mut bus); // SET
        cpu.step(&mut bus); // SBC zp in T-mode
        assert_eq!(bus.read(0x2015), 0x7F);
        assert!(cpu.flag(FLAG_OVERFLOW));
    }

    #[test]
    fn immediate_alu_op_clears_t_without_memory_transfer() {
        let program = [0xF4, 0x29, 0xF0, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x0F;
        cpu.x = 0x10;
        bus.write(0x2010, 0xAA);

        cpu.step(&mut bus); // SET
        assert!(cpu.flag(FLAG_T));

        cpu.step(&mut bus); // AND #imm
        assert_eq!(cpu.a, 0x00, "immediate AND should operate on A");
        assert_eq!(
            bus.read(0x2010),
            0xAA,
            "immediate op should not touch [MPR1:X]"
        );
        assert!(cpu.flag(FLAG_ZERO));
        assert!(!cpu.flag(FLAG_T));
    }

    #[test]
    fn bsr_pushes_return_address() {
        // BSR pushes PC-1 (last byte of instruction), matching JSR/RTS convention.
        // RTS adds 1 to popped address, so BSR at $8000 (2 bytes: 44 02)
        // returns to $8002 (the byte after the BSR instruction).
        let program = [0x44, 0x02, 0x00, 0xEA, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x8004);
        assert_eq!(cpu.sp, 0xFB);
        let lo = bus.read(0x01FC);
        let hi = bus.read(0x01FD);
        assert_eq!(lo, 0x01);
        assert_eq!(hi, 0x80);
    }

    #[test]
    fn st_ports_write_immediate_values() {
        let program = [0x03, 0xAA, 0x13, 0xBB, 0x23, 0xCC, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        cpu.step(&mut bus);
        cpu.step(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(bus.st_port(0), 0xAA);
        assert_eq!(bus.st_port(1), 0xBB);
        assert_eq!(bus.st_port(2), 0xCC);
    }

    #[test]
    fn stp_halts_cpu() {
        let program = [0xDB, 0xEA];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 3);
        assert!(cpu.halted);

        let next_cycles = cpu.step(&mut bus);
        assert_eq!(next_cycles, 0);
    }

    #[test]
    fn writing_mpr_via_memory_updates_mapping() {
        let program = [0xA9, 0x08, 0x8D, 0x80, 0xFF, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        // MPR registers at $FF80-$FFBF are only accessible when the
        // address maps to the hardware page.
        bus.set_mpr(7, 0xFF);

        cpu.step(&mut bus); // LDA #$08
        cpu.step(&mut bus); // STA $FF80

        assert_eq!(bus.mpr(0), 0x08);

        bus.load_rom_image(vec![0x11; PAGE_SIZE * 4]);

        assert_eq!(bus.read(0x0000), 0x11);
    }

    #[test]
    fn wai_pauses_until_irq() {
        let program = [0xCB, 0xEA, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write_u16(0xFFFA, 0x9000);
        bus.load(0x9000, &[0xEA, 0x00]);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 3);
        assert_eq!(cpu.pc, 0x8001);

        let idle_cycles = cpu.step(&mut bus);
        assert_eq!(idle_cycles, 0);
        assert_eq!(cpu.pc, 0x8001);

        bus.tick(64, true);
        bus.raise_irq(IRQ_REQUEST_TIMER);
        let irq_cycles = cpu.step(&mut bus);
        assert_eq!(irq_cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
        assert_eq!(cpu.pc, 0x9000);
    }

    #[test]
    fn irq_and_rti_restore_state() {
        let program = [0xEA, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write_u16(0xFFFA, 0x9000);
        bus.load(0x9000, &[0x40, 0x00]);

        cpu.status = FLAG_CARRY;
        bus.raise_irq(IRQ_REQUEST_TIMER);
        let irq_cycles = cpu.step(&mut bus);
        assert_eq!(irq_cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
        assert_eq!(cpu.pc, 0x9000);
        assert_eq!(cpu.sp, 0xFA);

        // Stack order: status pushed last at current SP+1 (0x01FB)
        assert_eq!(bus.read(0x01FB), FLAG_CARRY);
        assert_eq!(bus.read(0x01FC), 0x00); // PCL
        assert_eq!(bus.read(0x01FD), 0x80); // PCH

        let rti_cycles = cpu.step(&mut bus);
        assert_eq!(rti_cycles, 7); // HuC6280: RTI is 7 cycles
        assert_eq!(cpu.pc, 0x8000);
        assert_eq!(cpu.sp, 0xFD);
        assert!(cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn multiple_irq_sources_preserve_lower_priority() {
        let program = [0xEA, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write_u16(0xFFFA, 0x9000);
        bus.write_u16(0xFFF8, 0x9100);
        bus.write_u16(0xFFF6, 0x9200);
        bus.load(0x9000, &[0x40, 0x00]);
        bus.load(0x9100, &[0x40, 0x00]);
        bus.load(0x9200, &[0x40, 0x00]);

        cpu.status &= !FLAG_INTERRUPT_DISABLE;
        bus.raise_irq(IRQ_REQUEST_IRQ1 | IRQ_REQUEST_IRQ2 | IRQ_REQUEST_TIMER);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
        assert_eq!(cpu.pc, 0x9000);
        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
            IRQ_REQUEST_IRQ1
        );
        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ2,
            IRQ_REQUEST_IRQ2
        );

        let _ = cpu.step(&mut bus); // RTI from timer handler
        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
            IRQ_REQUEST_IRQ1
        );
        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ2,
            IRQ_REQUEST_IRQ2
        );

        let cycles = cpu.step(&mut bus); // service IRQ1
        assert_eq!(cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
        assert_eq!(cpu.pc, 0x9100);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
        assert_eq!(
            bus.pending_interrupts() & IRQ_REQUEST_IRQ2,
            IRQ_REQUEST_IRQ2
        );

        let _ = cpu.step(&mut bus); // RTI from IRQ1 handler
        let cycles = cpu.step(&mut bus); // service IRQ2
        assert_eq!(cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
        assert_eq!(cpu.pc, 0x9200);
        assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
    }

    #[test]
    fn reset_prefers_huc6280_reset_vector_slot() {
        let mut bus = Bus::new();
        bus.write_u16(0xFFFE, 0x8123);
        bus.write_u16(0xFFFC, 0x9000);

        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);

        assert_eq!(cpu.pc, 0x8123);
    }

    // --- T-mode SBC overflow: no overflow when signs are the same ---

    #[test]
    fn t_mode_sbc_no_overflow_when_signs_same() {
        // mem=0x50 (positive), value=0x10 (positive) => 0x50-0x10=0x40 (positive)
        // Same-sign operands subtracted: no overflow expected.
        let program = [0xF4, 0xE5, 0x16, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0xFF;
        cpu.x = 0x16;
        cpu.set_flag(FLAG_CARRY, true);
        bus.write(0x0016, 0x10); // value (subtrahend)
        bus.write(0x2016, 0x50); // mem (minuend)

        cpu.step(&mut bus); // SET
        cpu.step(&mut bus); // SBC zp in T-mode
        assert_eq!(bus.read(0x2016), 0x40);
        assert!(!cpu.flag(FLAG_OVERFLOW));
        assert!(cpu.flag(FLAG_CARRY));
    }

    // --- BCD mode tests ---

    #[test]
    fn adc_bcd_mode_basic_addition() {
        // 0x15 + 0x27 = 0x42 in BCD
        let program = [0x69, 0x27, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x15;
        cpu.set_flag(FLAG_DECIMAL, true);
        cpu.set_flag(FLAG_CARRY, false);

        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x42);
        assert!(!cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn adc_bcd_mode_carry_out() {
        // 0x99 + 0x01 = 0x00 with carry in BCD
        let program = [0x69, 0x01, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x99;
        cpu.set_flag(FLAG_DECIMAL, true);
        cpu.set_flag(FLAG_CARRY, false);

        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn adc_bcd_mode_with_carry_in() {
        // 0x58 + 0x01 + carry = 0x60 in BCD
        let program = [0x69, 0x01, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x58;
        cpu.set_flag(FLAG_DECIMAL, true);
        cpu.set_flag(FLAG_CARRY, true);

        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x60);
        assert!(!cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn sbc_bcd_mode_basic_subtraction() {
        // 0x42 - 0x15 = 0x27 in BCD
        let program = [0xE9, 0x15, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x42;
        cpu.set_flag(FLAG_DECIMAL, true);
        cpu.set_flag(FLAG_CARRY, true); // no borrow

        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x27);
        assert!(cpu.flag(FLAG_CARRY));
    }

    #[test]
    fn sbc_bcd_mode_borrow() {
        // 0x10 - 0x20 = 0x90 with borrow in BCD
        let program = [0xE9, 0x20, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x10;
        cpu.set_flag(FLAG_DECIMAL, true);
        cpu.set_flag(FLAG_CARRY, true); // no borrow

        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x90);
        assert!(!cpu.flag(FLAG_CARRY));
    }

    // --- TST N/V from memory value ---

    #[test]
    fn tst_nv_flags_come_from_memory_value() {
        // mask=0xFF, memory=0x40: AND=0x40 (non-zero) => Z=0
        // N from memory bit 7 = 0, V from memory bit 6 = 1
        let program = [0x83, 0xFF, 0x10, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x40);

        cpu.step(&mut bus);
        assert!(!cpu.flag(FLAG_ZERO));
        assert!(!cpu.flag(FLAG_NEGATIVE), "N = memory bit 7 = 0");
        assert!(cpu.flag(FLAG_OVERFLOW), "V = memory bit 6 = 1");
    }

    // --- SAX/SAY/SXY do not affect flags ---

    #[test]
    fn sax_say_sxy_do_not_affect_flags() {
        let program = [0x22, 0x42, 0x02, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x80;
        cpu.x = 0x00;
        cpu.y = 0x01;
        cpu.set_flag(FLAG_ZERO, false);
        cpu.set_flag(FLAG_NEGATIVE, false);

        cpu.step(&mut bus); // SAX: A=0x00, X=0x80
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.x, 0x80);
        assert!(!cpu.flag(FLAG_ZERO), "SAX should not set Z");
        assert!(!cpu.flag(FLAG_NEGATIVE), "SAX should not set N");

        cpu.step(&mut bus); // SAY: A=0x01, Y=0x00
        assert_eq!(cpu.a, 0x01);
        assert_eq!(cpu.y, 0x00);
        assert!(!cpu.flag(FLAG_ZERO), "SAY should not set Z");

        cpu.step(&mut bus); // SXY: X=0x00, Y=0x80
        assert_eq!(cpu.x, 0x00);
        assert_eq!(cpu.y, 0x80);
        assert!(!cpu.flag(FLAG_NEGATIVE), "SXY should not set N");
    }

    // --- WAI does not modify I flag ---

    #[test]
    fn wai_does_not_set_interrupt_disable() {
        let program = [0xCB, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.set_flag(FLAG_INTERRUPT_DISABLE, false);

        cpu.step(&mut bus); // WAI
        assert!(cpu.waiting);
        assert!(
            !cpu.flag(FLAG_INTERRUPT_DISABLE),
            "WAI should not modify I flag"
        );
    }

    // --- Interrupt clears D flag ---

    #[test]
    fn interrupt_clears_decimal_flag() {
        let program = [0xEA, 0x00];
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write_u16(VECTOR_IRQ2_BRK, 0x9000);
        bus.load(0x9000, &[0xEA, 0x00]);
        cpu.set_flag(FLAG_DECIMAL, true);
        cpu.set_flag(FLAG_INTERRUPT_DISABLE, false);

        bus.tick(64, true);
        bus.raise_irq(IRQ_REQUEST_TIMER);
        cpu.step(&mut bus);

        assert!(
            !cpu.flag(FLAG_DECIMAL),
            "Interrupt should clear D flag (65C02 behavior)"
        );
    }
}
