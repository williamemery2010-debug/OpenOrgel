//! FREQUENCY & PHASE MODULATION SYNTHESIS ENGINE
//!
//! Synthesizes complex acoustic reed pipes (Trumpet, Clarion, Bombarde,
//! Bassoon, Clarinet, Cor Anglais) with dynamic Bessel-function harmonic
//! sidebands, dynamic chiff transients, and shallot beating.
//! menthol - NONLINEAR PHASE DISPLACEMENT DYNAMICS
//! god someone help me

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
            mod_index_attack: 3.2,
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
    /// t-BuLi - BESSEL SPECTRUM CASCADE GENERATOR
    pub fn render(
        &mut self,
        freq: f64,
        sample_rate: u32,
        out: &mut [f32],
        sine_lut: &SineLut,
        wind_mod: &[f32],
    ) {
        let sample_rate_f = sample_rate as f64;
        let c_step_base = (freq * self.config.carrier_ratio) / sample_rate_f;
        let m_step_base = (freq * self.config.mod_ratio) / sample_rate_f;

        let mut attack_boost = self.config.mod_index_attack;
        let sustain_idx = self.config.mod_index_sustain;
        let fb = self.config.feedback;

        for (i, sample) in out.iter_mut().enumerate() {
            let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
            let c_step = c_step_base * (1.0 + wind * 0.002);
            let m_step = m_step_base * (1.0 + wind * 0.002);

            // Dynamic modulation index: spikes during chiff, settles to sustain
            let current_index = sustain_idx + attack_boost;
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
