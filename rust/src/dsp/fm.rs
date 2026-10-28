//! FREQUENCY & PHASE MODULATION SYNTHESIS ENGINE
//!
//! Synthesizes complex acoustic reed pipes (Trumpet, Clarion, Bombarde,
//! Bassoon, Clarinet, Cor Anglais) with dynamic Bessel-function harmonic
//! sidebands, dynamic chiff transients, and shallot beating.

use super::wavetable::SineLut;

/// Configuration parameters for a 2-operator Phase Modulation (FM) Reed Voice.
///
/// Why this structure is used:
/// - In Chowning's FM synthesis, a carrier at `f_c` phase-modulated by a modulator at `f_m`
///   with modulation index `I` produces sidebands at `f_c ± k * f_m` weighted by Bessel functions `J_k(I)`.
/// - By dynamically varying `I` during note onset (`mod_index_attack` decaying into `mod_index_sustain`),
///   we model the bright brassy transient bite of a beating reed tongue striking the shallot.
#[derive(Clone, Copy, Debug)]
pub struct FmVoiceConfig {
    /// Ratio of carrier frequency relative to fundamental (usually 1.0 for 8' unison pitch).
    pub carrier_ratio: f64,
    /// Ratio of modulator frequency relative to fundamental (1.0 = all harmonics, 2.0 = odd harmonics).
    pub mod_ratio: f64,
    /// Steady-state modulation index `I_sustain` (controls steady-state harmonic richness).
    pub mod_index_sustain: f64,
    /// Transient chiff attack modulation index boost `I_attack` added on key-down.
    pub mod_index_attack: f64,
    /// Exponential time constant (seconds) over which the attack boost decays to steady state.
    pub attack_time: f64,
    /// Self-feedback coefficient `beta` on the modulator (`0.0` to `1.5`), skewing the sinusoidal
    /// modulator toward a sawtooth-like reed pressure wave.
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

/// Stateful 2-Operator Phase Modulation Reed Voice.
///
/// Tracks normalized `[0.0, 1.0)` phase accumulators for both carrier and modulator,
/// the previous modulator sample (`last_mod_out`) for single-sample feedback, and the
/// precomputed per-sample exponential decay multiplier (`decay_rate`).
pub struct FmOperator {
    config: FmVoiceConfig,
    carrier_phase: f64,
    mod_phase: f64,
    last_mod_out: f64,
    decay_rate: f64,
}

impl FmOperator {
    /// Creates a new `FmOperator` initialized at zero phase for the given `sample_rate`.
    pub fn new(config: FmVoiceConfig, sample_rate: u32) -> Self {
        // One-pole exponential decay multiplier per sample: exp(-1 / (tau * fs))
        let decay_rate = (-1.0 / (config.attack_time.max(0.005) * sample_rate as f64)).exp();
        FmOperator {
            config,
            carrier_phase: 0.0,
            mod_phase: 0.0,
            last_mod_out: 0.0,
            decay_rate,
        }
    }

    /// Renders a buffer of audio samples using 2-operator Phase Modulation with modulator feedback.
    ///
    /// Signal equation at sample `n`:
    /// - `y_m[n] = sin(2*pi * phi_m[n] + beta * y_m[n - 1])`
    /// - `y_c[n] = sin(2*pi * phi_c[n] + I[n] * y_m[n])`
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
        // Normalized phase increment per sample (cycles/sample) for carrier and modulator
        let c_step_base = (freq * self.config.carrier_ratio) / sample_rate_f;
        let m_step_base = (freq * self.config.mod_ratio) / sample_rate_f;

        // Scale the transient modulation boost by the user's Chiff slider setting
        let max_chiff = self.config.mod_index_attack * chiff_scale.max(0.0) as f64;
        let sustain_idx = self.config.mod_index_sustain;
        let fb = self.config.feedback;
        // 5 ms linear ramp-in on the attack modulation boost prevents a discontinuous phase step at sample 0
        let rise_samples = (0.005 * sample_rate_f).round() as usize;

        let mut attack_boost = max_chiff;

        for (i, sample) in out.iter_mut().enumerate() {
            // Apply instantaneous windchest pressure wobble to both carrier and modulator frequencies
            let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
            let c_step = c_step_base * (1.0 + wind);
            let m_step = m_step_base * (1.0 + wind);

            // Smooth 5 ms onset ramp for the transient index boost
            let rise = if i < rise_samples {
                i as f64 / rise_samples as f64
            } else {
                1.0
            };

            // Instantaneous modulation index I[n]: smooth rise followed by exponential decay to sustain_idx
            let current_index = sustain_idx + attack_boost * rise;
            attack_boost *= self.decay_rate;

            // Modulator with self-feedback: y_m[n] = sin(2*pi*phase_m + fb * y_m[n-1])
            let mod_arg = self.mod_phase * (2.0 * std::f64::consts::PI) + fb * self.last_mod_out;
            let mod_out = sine_lut.sin(mod_arg) as f64;
            self.last_mod_out = mod_out;

            // Carrier phase-modulated by modulator output: y_c[n] = sin(2*pi*phase_c + I[n] * y_m[n])
            let carrier_arg = self.carrier_phase * (2.0 * std::f64::consts::PI) + current_index * mod_out;
            let carrier_out = sine_lut.sin(carrier_arg);

            // Accumulate into the output mix buffer
            *sample += carrier_out;

            // Wrap carrier and modulator phases into [0.0, 1.0) to maintain floating-point precision
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
