use super::Psg;
use super::channel::PsgChannel;
use super::tables::*;

impl Psg {
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
                phase_step_for_period(effective_period)
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
}
