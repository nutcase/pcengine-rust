const PSG_CLOCK_HZ: u32 = 7_159_090 / 2;
const AUDIO_SAMPLE_RATE: u32 = 44_100;

pub(crate) const PSG_REG_COUNT: usize = 32;
pub(crate) const PSG_CHANNEL_COUNT: usize = 6;
pub(crate) const PSG_WAVE_SIZE: usize = 32;
pub(crate) const PSG_REG_CH_SELECT: usize = 0x00;
pub(crate) const PSG_REG_MAIN_BALANCE: usize = 0x01;
pub(crate) const PSG_REG_FREQ_LO: usize = 0x02;
pub(crate) const PSG_REG_FREQ_HI: usize = 0x03;
pub(crate) const PSG_REG_CH_CONTROL: usize = 0x04;
pub(crate) const PSG_REG_CH_BALANCE: usize = 0x05;
pub(crate) const PSG_REG_WAVE_DATA: usize = 0x06;
pub(crate) const PSG_REG_NOISE_CTRL: usize = 0x07;
pub(crate) const PSG_REG_LFO_FREQ: usize = 0x08;
pub(crate) const PSG_REG_LFO_CTRL: usize = 0x09;
pub(crate) const PSG_REG_TIMER_LO: usize = 0x18;
pub(crate) const PSG_REG_TIMER_HI: usize = 0x19;
pub(crate) const PSG_REG_TIMER_CTRL: usize = 0x1A;
pub(crate) const PSG_CTRL_ENABLE: u8 = 0x01;
pub(crate) const PSG_CTRL_IRQ_ENABLE: u8 = 0x02;
const PSG_STATUS_IRQ: u8 = 0x80;
pub(crate) const PSG_CH_CTRL_VOLUME_MASK: u8 = 0x1F;
pub(crate) const PSG_CH_CTRL_DDA: u8 = 0x40;
pub(crate) const PSG_CH_CTRL_KEY_ON: u8 = 0x80;
pub(crate) const PSG_NOISE_ENABLE: u8 = 0x80;
const PSG_NOISE_FREQ_MASK: u8 = 0x1F;
const PSG_PHASE_FRAC_BITS: u32 = 12;
const PSG_PHASE_FRAC_MASK: u32 = (1 << PSG_PHASE_FRAC_BITS) - 1;
const PSG_PERIOD_ENTRIES: usize = 0x1000;
// Output gain: Mednafen uses base * 8/6 ≈ 1.33x per channel.
// With 6 channels at max (15 * 65536 each), max mix = 5,898,240.
// Gain 340: (5,898,240 * 340) >> 16 = 30,600 (within i16 range).
const PSG_OUTPUT_GAIN: i32 = 340;

/// Logarithmic volume table (Mednafen-compatible).
/// Index = attenuation level (0 = full volume, 31 = silence).
/// Each step ≈ 1.5 dB: multiplier = 1.0 / pow(2, 0.25 * level).
/// Values are fixed-point with 16 fractional bits.
fn psg_db_table() -> &'static [i32; 32] {
    static TABLE: std::sync::OnceLock<[i32; 32]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0i32; 32];
        for vl in 0..32 {
            if vl == 31 {
                table[vl] = 0; // muted
            } else if vl == 0 {
                table[vl] = 65536; // 1.0 in fixed-point
            } else {
                let multiplier = 1.0 / f64::powf(2.0, 0.25 * vl as f64);
                table[vl] = (multiplier * 65536.0) as i32;
            }
        }
        table
    })
}

/// Maps 4-bit balance register values (0-15) to 5-bit volume range (0-31).
/// 0 = muted (maps to 0), 15 = full volume (maps to 31).
fn psg_balance_scale_tab() -> &'static [u8; 16] {
    static TABLE: std::sync::OnceLock<[u8; 16]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u8; 16];
        for n in 0..16u8 {
            if n == 0 {
                table[n as usize] = 0;
            } else {
                // Scale 1-15 to 3-31 (matching Mednafen: n*2 + 1 for non-zero)
                table[n as usize] = (n * 2 + 1).min(31);
            }
        }
        table
    })
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub(crate) struct PsgChannel {
    pub(crate) frequency: u16,
    pub(crate) phase_step: u32,
    pub(crate) control: u8,
    pub(crate) balance: u8,
    pub(crate) noise_control: u8,
    pub(crate) phase: u32,
    pub(crate) wave_pos: u8,
    pub(crate) wave_write_pos: u8,
    pub(crate) dda_sample: u8,
    pub(crate) noise_lfsr: u32, // 18-bit LFSR (HuC6280 reference)
    pub(crate) noise_phase: u32,
}

impl Default for PsgChannel {
    fn default() -> Self {
        Self {
            frequency: 0,
            phase_step: 1,
            control: 0,
            balance: 0xFF,
            noise_control: 0,
            phase: 0,
            wave_pos: 0,
            wave_write_pos: 0,
            dda_sample: 0x10,
            noise_lfsr: 1, // 18-bit LFSR initial value (Mednafen reference)
            noise_phase: 0,
        }
    }
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub(crate) struct Psg {
    regs: [u8; PSG_REG_COUNT],
    select: u8,
    current_channel: usize,
    pub(crate) main_balance: u8,
    lfo_frequency: u8,
    lfo_control: u8,
    accumulator: u32,
    irq_pending: bool,
    pub(crate) channels: [PsgChannel; PSG_CHANNEL_COUNT],
    pub(crate) waveform_ram: [u8; PSG_CHANNEL_COUNT * PSG_WAVE_SIZE],
    /// First-order IIR low-pass filter state for anti-aliasing.
    lpf_state: f64,
}

impl Psg {
    pub(crate) fn new() -> Self {
        Self {
            regs: [0; PSG_REG_COUNT],
            select: 0,
            current_channel: 0,
            main_balance: 0xFF,
            lfo_frequency: 0,
            lfo_control: 0,
            accumulator: 0,
            irq_pending: false,
            channels: [PsgChannel::default(); PSG_CHANNEL_COUNT],
            waveform_ram: [0; PSG_CHANNEL_COUNT * PSG_WAVE_SIZE],
            lpf_state: 0.0,
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn write_address(&mut self, value: u8) {
        self.select = value;
    }

    pub(crate) fn write_data(&mut self, value: u8) {
        let index = self.select as usize;
        if index < PSG_REG_COUNT {
            self.regs[index] = value;
            self.write_register(index, value);
        }
        if index >= PSG_REG_COUNT {
            self.write_wave_ram(index - PSG_REG_COUNT, value);
        }
        self.select = self.select.wrapping_add(1);
    }

    pub(crate) fn read_address(&self) -> u8 {
        self.select
    }

    pub(crate) fn read_data(&mut self) -> u8 {
        let index = self.select as usize;
        let value = if index < PSG_REG_COUNT {
            self.regs[index]
        } else {
            let wave_index = index - PSG_REG_COUNT;
            self.waveform_ram[wave_index % self.waveform_ram.len()]
        };
        self.select = self.select.wrapping_add(1);
        value
    }

    pub(crate) fn write_direct(&mut self, index: usize, value: u8) {
        if index < PSG_REG_COUNT {
            self.regs[index] = value;
            self.write_register(index, value);
        } else {
            self.write_wave_ram(index - PSG_REG_COUNT, value);
        }
    }

    pub(crate) fn read_direct(&mut self, index: usize) -> u8 {
        if index < PSG_REG_COUNT {
            self.regs[index]
        } else {
            let wave_index = index - PSG_REG_COUNT;
            self.waveform_ram[wave_index % self.waveform_ram.len()]
        }
    }

    pub(crate) fn read_status(&mut self) -> u8 {
        let mut status = 0;
        if self.irq_pending {
            status |= PSG_STATUS_IRQ;
        }
        status
    }

    fn write_register(&mut self, index: usize, value: u8) {
        match index {
            PSG_REG_CH_SELECT => {
                self.current_channel = (value as usize) & 0x07;
                if self.current_channel >= PSG_CHANNEL_COUNT {
                    self.current_channel = PSG_CHANNEL_COUNT - 1;
                }
            }
            PSG_REG_MAIN_BALANCE => {
                self.main_balance = value;
            }
            PSG_REG_FREQ_LO => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                channel.frequency = (channel.frequency & 0x0F00) | value as u16;
                channel.phase_step = Self::phase_step_for_period(channel.frequency);
            }
            PSG_REG_FREQ_HI => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                channel.frequency = (channel.frequency & 0x00FF) | (((value & 0x0F) as u16) << 8);
                channel.phase_step = Self::phase_step_for_period(channel.frequency);
            }
            PSG_REG_CH_CONTROL => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                let previous = channel.control;
                channel.control = value;
                if previous & PSG_CH_CTRL_DDA != 0 && value & PSG_CH_CTRL_DDA == 0 {
                    // Hardware resets the waveform index when DDA is cleared.
                    channel.wave_write_pos = 0;
                    channel.wave_pos = 0;
                }
                if previous & PSG_CH_CTRL_KEY_ON == 0 && value & PSG_CH_CTRL_KEY_ON != 0 {
                    channel.phase = 0;
                    channel.wave_pos = channel.wave_write_pos;
                    channel.noise_phase = 0;
                    channel.noise_lfsr = 1;
                }
            }
            PSG_REG_CH_BALANCE => {
                self.channels[self.current_channel].balance = value;
            }
            PSG_REG_WAVE_DATA => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                let sample = value & 0x1F;
                if channel.control & PSG_CH_CTRL_DDA != 0 {
                    channel.dda_sample = sample;
                }
                if channel.control & PSG_CH_CTRL_KEY_ON == 0 {
                    // Games commonly upload wave tables with KEY OFF and DDA toggled.
                    // Accept writes whenever KEY is off so both patterns work.
                    let write_pos = channel.wave_write_pos as usize & (PSG_WAVE_SIZE - 1);
                    let index = ch * PSG_WAVE_SIZE + write_pos;
                    self.waveform_ram[index] = sample;
                    channel.wave_write_pos = channel.wave_write_pos.wrapping_add(1) & 0x1F;
                }
            }
            PSG_REG_NOISE_CTRL => {
                if self.current_channel >= 4 {
                    self.channels[self.current_channel].noise_control = value;
                }
            }
            PSG_REG_LFO_FREQ => {
                self.lfo_frequency = value;
            }
            PSG_REG_LFO_CTRL => {
                self.lfo_control = value;
            }
            PSG_REG_TIMER_LO | PSG_REG_TIMER_HI => {
                self.accumulator = 0;
            }
            PSG_REG_TIMER_CTRL => {
                if value & PSG_CTRL_ENABLE == 0 {
                    self.irq_pending = false;
                }
            }
            _ => {}
        }
    }

    fn timer_period(&self) -> u16 {
        let lo = self.regs[PSG_REG_TIMER_LO] as u16;
        let hi = self.regs[PSG_REG_TIMER_HI] as u16;
        (hi << 8) | lo
    }

    fn enabled(&self) -> bool {
        let ctrl = self.regs[PSG_REG_TIMER_CTRL];
        self.timer_period() != 0 && (ctrl & PSG_CTRL_ENABLE != 0)
    }

    pub(crate) fn tick(&mut self, cycles: u32) -> bool {
        if !self.enabled() {
            return false;
        }
        if self.irq_pending {
            return false;
        }

        self.accumulator = self.accumulator.saturating_add(cycles);
        let period = self.timer_period() as u32;
        if period == 0 {
            return false;
        }
        if self.accumulator >= period {
            self.accumulator %= period.max(1);
            if self.regs[PSG_REG_TIMER_CTRL] & PSG_CTRL_IRQ_ENABLE != 0 {
                self.irq_pending = true;
                return true;
            }
        }
        false
    }

    pub(crate) fn acknowledge(&mut self) {
        self.irq_pending = false;
    }

    #[inline]
    fn phase_step_for_period(period: u16) -> u32 {
        Self::phase_step_table()[(period & 0x0FFF) as usize]
    }

    #[inline]
    fn phase_step_table() -> &'static [u32; PSG_PERIOD_ENTRIES] {
        static TABLE: std::sync::OnceLock<[u32; PSG_PERIOD_ENTRIES]> = std::sync::OnceLock::new();
        TABLE.get_or_init(|| {
            let mut table = [1u32; PSG_PERIOD_ENTRIES];
            for (period, slot) in table.iter_mut().enumerate() {
                let divider = if period == 0 {
                    0x1000_u64
                } else {
                    period as u64
                };
                *slot = ((((PSG_CLOCK_HZ as u64) << PSG_PHASE_FRAC_BITS)
                    / (divider * AUDIO_SAMPLE_RATE as u64))
                    .max(1)) as u32;
            }
            table
        })
    }

    pub(crate) fn generate_sample(&mut self) -> i16 {
        self.advance_waveforms();
        let mut mix: i32 = 0;
        for channel_index in 0..PSG_CHANNEL_COUNT {
            let state = self.channels[channel_index];
            mix += self.sample_channel(channel_index, state);
        }
        // sample_channel() returns values with 16 fractional bits.
        // Per-channel max = 15 * 65536 = 983,040; 6-channel max = 5,898,240.
        // Apply gain and shift: (mix * gain) >> 16.
        // With PSG_OUTPUT_GAIN=256: max = (5,898,240 * 256) >> 16 = 23,040.
        let scaled = ((mix as i64 * PSG_OUTPUT_GAIN as i64) >> 16) as i32;
        let clamped = scaled.clamp(i16::MIN as i32, i16::MAX as i32) as f64;
        // First-order IIR low-pass filter (~14 kHz cutoff at 44.1 kHz).
        // alpha ≈ 2*pi*fc / (2*pi*fc + fs) ≈ 0.67 for fc=14000, fs=44100
        const LPF_ALPHA: f64 = 0.67;
        self.lpf_state = self.lpf_state + LPF_ALPHA * (clamped - self.lpf_state);
        self.lpf_state as i16
    }

    fn advance_waveforms(&mut self) {
        let lfo_mod = self.lfo_modulation();
        let lfo_enabled = self.lfo_enabled();
        for idx in 0..PSG_CHANNEL_COUNT {
            let ch = &mut self.channels[idx];
            if ch.control & PSG_CH_CTRL_KEY_ON == 0 {
                continue;
            }
            if ch.control & PSG_CH_CTRL_DDA != 0 {
                continue;
            }
            if idx >= 4 && ch.noise_control & PSG_NOISE_ENABLE != 0 {
                // HuC6280 noise generator (Mednafen reference):
                // - 18-bit LFSR with taps at bits 0, 1, 11, 12, 17
                // - Noise period: NF = noisectrl & 0x1F
                //   raw = 31 - NF; if raw==0 then period=64, else period = raw * 128
                // - LFSR steps once per `period` PSG clock cycles
                // Use 16-bit fixed-point accumulator for precision.
                let nf = (ch.noise_control & PSG_NOISE_FREQ_MASK) as u32;
                let raw = 31u32.saturating_sub(nf);
                let period = if raw == 0 { 64u64 } else { raw as u64 * 128 };
                let noise_step =
                    ((PSG_CLOCK_HZ as u64) << 16) / (period * AUDIO_SAMPLE_RATE as u64);
                ch.noise_phase = ch.noise_phase.wrapping_add(noise_step.max(1) as u32);
                let steps = (ch.noise_phase >> 16) as usize;
                ch.noise_phase &= 0xFFFF;
                for _ in 0..steps {
                    let lfsr = ch.noise_lfsr;
                    let feedback =
                        ((lfsr >> 0) ^ (lfsr >> 1) ^ (lfsr >> 11) ^ (lfsr >> 12) ^ (lfsr >> 17))
                            & 0x01;
                    ch.noise_lfsr = (lfsr >> 1) | (feedback << 17);
                    if ch.noise_lfsr == 0 {
                        ch.noise_lfsr = 1;
                    }
                }
                continue;
            }

            let step_fp = if idx == 0 && lfo_enabled {
                let effective_period = (ch.frequency as i32 + lfo_mod).clamp(0, 0x0FFF) as u16;
                Self::phase_step_for_period(effective_period)
            } else {
                ch.phase_step.max(1)
            };
            let phase = ch.phase.wrapping_add(step_fp);
            let step = (phase >> PSG_PHASE_FRAC_BITS) as u8;
            ch.phase = phase & PSG_PHASE_FRAC_MASK;
            if step != 0 {
                ch.wave_pos = ch.wave_pos.wrapping_add(step) & (PSG_WAVE_SIZE as u8 - 1);
            }
        }
    }

    fn sample_channel(&self, channel: usize, state: PsgChannel) -> i32 {
        if state.control & PSG_CH_CTRL_KEY_ON == 0 {
            return 0;
        }
        let raw = if state.control & PSG_CH_CTRL_DDA != 0 {
            state.dda_sample as i32 - 0x10
        } else if channel >= 4 && state.noise_control & PSG_NOISE_ENABLE != 0 {
            if state.noise_lfsr & 0x01 == 0 {
                0x0F
            } else {
                -0x10
            }
        } else {
            let base = channel * PSG_WAVE_SIZE;
            let offset = (state.wave_pos as usize) & (PSG_WAVE_SIZE - 1);
            let wave_index = base + offset;
            self.waveform_ram[wave_index] as i32 - 0x10
        };
        if raw == 0 {
            return 0;
        }

        // Logarithmic volume mixing (Mednafen-compatible).
        // Combine channel volume, channel balance, and main balance as
        // attenuation indices (additive in dB domain).
        let db_table = psg_db_table();
        let scale_tab = psg_balance_scale_tab();

        // Channel volume: 5-bit, 31=max(0 attenuation), 0=min(31 attenuation)
        let al = 0x1F_u8.wrapping_sub(state.control & PSG_CH_CTRL_VOLUME_MASK);

        // Channel balance: 4-bit per side, scaled to 5-bit range
        let bal_l = 0x1F - scale_tab[((state.balance >> 4) & 0x0F) as usize];
        let bal_r = 0x1F - scale_tab[(state.balance & 0x0F) as usize];

        // Main balance: 4-bit per side, scaled to 5-bit range
        let gbal_l = 0x1F - scale_tab[((self.main_balance >> 4) & 0x0F) as usize];
        let gbal_r = 0x1F - scale_tab[(self.main_balance & 0x0F) as usize];

        // Sum attenuations (clamped to 31 = silence)
        let vol_l = ((al as u16 + bal_l as u16 + gbal_l as u16).min(0x1F)) as usize;
        let vol_r = ((al as u16 + bal_r as u16 + gbal_r as u16).min(0x1F)) as usize;

        // Apply logarithmic volume (fixed-point 16.16).
        // Return with 16 fractional bits intact; generate_sample() shifts after
        // accumulating all channels and applying the output gain.
        let left = raw as i64 * db_table[vol_l] as i64;
        let right = raw as i64 * db_table[vol_r] as i64;
        ((left + right) / 2) as i32
    }

    fn lfo_enabled(&self) -> bool {
        self.lfo_control & 0x80 != 0
    }

    fn lfo_modulation(&self) -> i32 {
        if !self.lfo_enabled() {
            return 0;
        }
        let depth_shift = (self.lfo_control & 0x03) as i32;
        let speed_bias = (self.lfo_frequency & 0x0F) as i32;
        let ch1 = self.channels[1];
        let base = PSG_WAVE_SIZE;
        let offset = (ch1.wave_pos as usize) & (PSG_WAVE_SIZE - 1);
        let raw = self.waveform_ram[base + offset] as i32 - 0x10;
        (raw << depth_shift) + speed_bias
    }

    fn write_wave_ram(&mut self, addr: usize, value: u8) {
        let index = addr % self.waveform_ram.len();
        self.waveform_ram[index] = value & 0x1F;
    }
}
