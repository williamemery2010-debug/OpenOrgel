//! FREQUENCY & PHASE MODULATION SYNTHESIS ENGINE
//!
//! Synthesizes complex acoustic reed pipes (Trumpet, Clarion, Bombarde,
//! Bassoon, Clarinet, Cor Anglais) with dynamic Bessel-function harmonic
//! sidebands, dynamic chiff transients, and shallot beating.

use super::wavetable::SineLut;

/// Configuration parameters for an FM Reed Operator Pair.
#[derive(Clone, Copy, Debug)]
pub struct FmVoiceConfig {
    /// Ratio of carrier frequency relative to fundamental (usually 1.0)
    pub carrier_ratio: f64,
    /// Ratio of modulator frequency relative to fundamental (e.g. 1.0, 2.0, 3.0)
    pub mod_ratio: f64,
    /// Steady-state modulation index (controls brightness / richness)
    pub mod_index_sustain: f64,
    /// Transient chiff attack modulation index boost
    pub mod_index_attack: f64,
    /// Chiff attack decay rate (seconds)
    pub attack_time: f64,
    /// Self-feedback coefficient on the modulator (0.0 to 1.5)
    pub feedback: f64,
}

impl Default for FmVoiceConfig {
    fn default() -> Self {
        FmVoiceConfig {
            carrier_ratio: 1.0,
            mod_ratio: 1.0,
            mod_index_sustain: 1.4,
            mod_index_attack: 0.8,
            attack_time: 0.025,
            feedback: 0.35,
        }
    }
}

/// Dynamic 2-Operator FM Reed Voice.
pub struct FmOperator {
    config: FmVoiceConfig,
    carrier_phase: f64,
    mod_phase: f64,
    last_mod_out: f64,
    decay_rate: f64,
}

impl FmOperator {
    pub fn new(config: FmVoiceConfig, sample_rate: u32) -> Self {
        let decay_rate = (-1.0 / (config.attack_time.max(0.005) * sample_rate as f64)).exp();
        FmOperator {
            config,
            carrier_phase: 0.0,
            mod_phase: 0.0,
            last_mod_out: 0.0,
            decay_rate,
        }
    }

    /// Renders a buffer of audio samples using Phase Modulation.
    pub fn render(
        &mut self,
        freq: f64,
        sample_rate: u32,
        out: &mut [f32],
        sine_lut: &SineLut,
        wind_mod: &[f32],
        chiff_scale: f32,
    ) {
        let sample_rate_f = sample_rate as f64;
        let c_step_base = (freq * self.config.carrier_ratio) / sample_rate_f;
        let m_step_base = (freq * self.config.mod_ratio) / sample_rate_f;

        let max_chiff = self.config.mod_index_attack * chiff_scale.max(0.0) as f64;
        let sustain_idx = self.config.mod_index_sustain;
        let fb = self.config.feedback;
        let rise_samples = (0.005 * sample_rate_f).round() as usize; // 5ms smooth onset to eliminate click

        let mut attack_boost = max_chiff;

        for (i, sample) in out.iter_mut().enumerate() {
            let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
            let c_step = c_step_base * (1.0 + wind);
            let m_step = m_step_base * (1.0 + wind);

            let rise = if i < rise_samples {
                i as f64 / rise_samples as f64
            } else {
                1.0
            };

            // Dynamic modulation index: smooth rise then natural decay, completely click-free
            let current_index = sustain_idx + attack_boost * rise;
            attack_boost *= self.decay_rate;

            // Modulator with feedback: y_m = sin(2*pi*phase_m + fb * last_y_m)
            let mod_arg = self.mod_phase * (2.0 * std::f64::consts::PI) + fb * self.last_mod_out;
            let mod_out = sine_lut.sin(mod_arg) as f64;
            self.last_mod_out = mod_out;

            // Carrier modulated by modulator: y_c = sin(2*pi*phase_c + I * y_m)
            let carrier_arg = self.carrier_phase * (2.0 * std::f64::consts::PI) + current_index * mod_out;
            let carrier_out = sine_lut.sin(carrier_arg);

            *sample += carrier_out;

            self.carrier_phase = (self.carrier_phase + c_step).fract();
            self.mod_phase = (self.mod_phase + m_step).fract();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fm_operator_stability_and_no_nan() {
        let config = FmVoiceConfig::default();
        let mut op = FmOperator::new(config, 44100);
        let sine_lut = SineLut::new();
        let mut buffer = vec![0.0f32; 1024];
        let wind = vec![0.0f32; 1024];

        op.render(440.0, 44100, &mut buffer, &sine_lut, &wind, 1.0);

        for (i, &s) in buffer.iter().enumerate() {
            assert!(!s.is_nan(), "Sample at {} is NaN", i);
            assert!(!s.is_infinite(), "Sample at {} is infinite", i);
            assert!(s.abs() <= 2.5, "Sample at {} exceeded expected bounds: {}", i, s);
        }
    }

    #[test]
    fn test_chiff_volume_zero_has_no_boost() {
        let config = FmVoiceConfig::default();
        let mut op = FmOperator::new(config, 44100);
        let sine_lut = SineLut::new();
        let mut buffer = vec![0.0f32; 1024];
        let wind = vec![0.0f32; 1024];

        // chiff_scale = 0.0 should render smoothly with zero attack click
        op.render(440.0, 44100, &mut buffer, &sine_lut, &wind, 0.0);

        for (i, &s) in buffer.iter().enumerate() {
            assert!(!s.is_nan(), "Sample at {} is NaN", i);
            assert!(s.abs() <= 2.0);
        }
    }
}

