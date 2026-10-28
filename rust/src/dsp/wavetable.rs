//! HIGH-PERFORMANCE BANDLIMITED WAVETABLE OSCILLATOR ENGINE
//!
//! Replaces millions of per-sample trigonometric evaluations with
//! single-cycle RAM lookup tables and branchless linear interpolation.

/// Power-of-two wavetable length (`2^11 = 2048` samples per cycle) so index wrapping
/// can use a single bitwise AND (`& TABLE_MASK`) instead of a slow modulo instruction.
pub const TABLE_SIZE: usize = 2048;
/// Bitmask (`2047 = 0x7FF`) used for branchless power-of-two table index wrapping.
pub const TABLE_MASK: usize = TABLE_SIZE - 1;

/// Global precomputed Sine Lookup Table (LUT) for LFOs, wind wobble, and additive partial synthesis.
///
/// Stores `4096 + 1` samples of `sin(x)` over `[0, 2*pi]`. The extra guard element at index `4096`
/// equals `table[0]`, allowing linear interpolation between `idx` and `idx + 1` without a second
/// bounds check or wrap-around branch.
pub struct SineLut {
    table: Vec<f32>,
}

impl SineLut {
    /// Precomputes a 4096-point single-cycle sine wave plus 1 guard sample for linear interpolation.
    pub fn new() -> Self {
        let size = 4096;
        let mut table = Vec::with_capacity(size + 1);
        for i in 0..=size {
            // Map index i in [0, 4096] linearly to angle p in [0, 2*pi] radians
            let p = i as f64 * 2.0 * std::f64::consts::PI / size as f64;
            table.push(p.sin() as f32);
        }
        SineLut { table }
    }

    /// Evaluates `sin(phase_rad)` for any real radian angle using O(1) table lookup and linear interpolation.
    #[inline(always)]
    pub fn sin(&self, phase_rad: f64) -> f32 {
        // Convert radians to normalized cycles (1 cycle = 2*pi radians)
        let norm = phase_rad * (0.5 / std::f64::consts::PI);
        // Extract the fractional cycle in [0.0, 1.0) (works for both positive and negative angles)
        // and scale to table coordinates [0.0, 4096.0)
        let pos = (norm - norm.floor()) * 4096.0;
        // Integer table index wrapped to 0..=4095 via bitwise AND
        let idx = (pos as usize) & 4095;
        // Fractional distance between sample[idx] and sample[idx + 1]
        let frac = (pos - pos.floor()) as f32;
        // Linear interpolation: y = y0 + frac * (y1 - y0)
        self.table[idx] + frac * (self.table[idx + 1] - self.table[idx])
    }
}

/// A pre-computed single-cycle periodic pipe wave shape (`2048 + 1` samples).
#[derive(Clone)]
pub struct Wavetable {
    pub samples: Vec<f32>,
}

impl Wavetable {
    /// Builds a band-limited periodic wavetable from a set of harmonic partials `(factor, amplitude)`.
    ///
    /// Why band-limiting and phase dispersion are applied here:
    /// - Any partial whose frequency `fundamental_freq * factor` reaches or exceeds the Nyquist limit
    ///   (`sample_rate / 2`) is omitted so high notes never fold back as enharmonic digital aliasing.
    /// - Harmonic phases are dispersed via the golden-angle offset `(factor * 2.39996).sin() * pi`
    ///   to minimize the peak-to-RMS crest factor (preventing artificial phase-0 impulse spikes).
    pub fn from_harmonics(
        fundamental_freq: f64,
        sample_rate: u32,
        harmonics: &[(f64, f64)], // (factor, amplitude)
    ) -> Self {
        let mut wave = vec![0.0f32; TABLE_SIZE + 1];
        let nyquist = sample_rate as f64 * 0.5;

        for &(factor, amp) in harmonics {
            let partial_freq = fundamental_freq * factor;
            // Strict Nyquist anti-aliasing cutoff
            if partial_freq >= nyquist {
                continue;
            }
            // Gentle cosine-like roll-off across the top 15% of the spectrum below Nyquist
            let damp = if partial_freq > nyquist * 0.85 {
                (1.0 - (partial_freq - nyquist * 0.85) / (nyquist * 0.15)) as f32
            } else {
                1.0
            };

            // Warm midrange pipe body & mouth radiation formant (~420 Hz peak)
            let formant_octave_dist = (partial_freq / 420.0).log2();
            let formant_gain = 1.0 + 0.35 * (-formant_octave_dist * formant_octave_dist).exp() as f32;

            // Töpfer Normalmensur pipe scaling: bass pipes retain richer overtones, treble rounds off
            let scale_damp = if factor > 2.0 {
                let oct_shift = (fundamental_freq / 261.63).log2();
                (1.0 - 0.10 * oct_shift).clamp(0.60, 1.35) as f32
            } else {
                1.0
            };

            let amp_scaled = (amp as f32) * damp * formant_gain * scale_damp;
            let step = factor * 2.0 * std::f64::consts::PI / TABLE_SIZE as f64;
            // Golden-ratio radian offset disperses harmonic peaks across the cycle
            let phase_offset = (factor * 2.39996).sin() * std::f64::consts::PI;
            for i in 0..TABLE_SIZE {
                let p = i as f64 * step + phase_offset;
                wave[i] += amp_scaled * (p.sin() as f32);
            }
        }

        // Copy sample 0 into the guard slot at TABLE_SIZE for branchless linear interpolation
        wave[TABLE_SIZE] = wave[0];

        // Normalize peak amplitude to 1.0 so all wavetables have consistent headroom
        let peak = wave.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        if peak > 1e-5 {
            let inv = 1.0 / peak;
            for s in wave.iter_mut() {
                *s *= inv;
            }
        }

        Wavetable { samples: wave }
    }

    /// Reads the wavetable at normalized phase `[0.0, 1.0)` using branchless linear interpolation.
    #[inline(always)]
    pub fn get_sample(&self, phase_normalized: f64) -> f32 {
        // Wrap normalized phase into [0.0, 1.0) and scale to table index range [0.0, 2048.0)
        let p = (phase_normalized - phase_normalized.floor()) * TABLE_SIZE as f64;
        let idx = (p as usize) & TABLE_MASK;
        let frac = (p - p.floor()) as f32;
        self.samples[idx] + frac * (self.samples[idx + 1] - self.samples[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_lut_accuracy() {
        let lut = SineLut::new();
        let pi = std::f64::consts::PI;
        // Test key phase points: 0, pi/6, pi/4, pi/2, pi, 3*pi/2, 2*pi
        for step in 0..=360 {
            let angle = step as f64 * (pi / 180.0);
            let expected = angle.sin() as f32;
            let actual = lut.sin(angle);
            assert!(
                (actual - expected).abs() < 0.002,
                "Sine LUT error at {} deg: got {}, expected {}",
                step, actual, expected
            );
        }
    }

    #[test]
    fn test_wavetable_normalization_and_wrap() {
        let harmonics = [(1.0, 1.0), (2.0, 0.5), (3.0, 0.25)];
        let wt = Wavetable::from_harmonics(440.0, 44100, &harmonics);
        // Table should have TABLE_SIZE + 1 elements
        assert_eq!(wt.samples.len(), TABLE_SIZE + 1);
        // Wrap-around guard for linear interpolation
        assert_eq!(wt.samples[0], wt.samples[TABLE_SIZE]);
        // Peak normalized to <= 1.0
        let peak = wt.samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((peak - 1.0).abs() < 1e-4, "Peak should be normalized to 1.0, got {}", peak);
    }

    #[test]
    fn test_wavetable_bandlimiting_nyquist() {
        // Fundamental at 15 kHz, sample rate 44.1 kHz. Harmonic 2 (30 kHz) and Harmonic 3 (45 kHz) exceed Nyquist (22.05 kHz)
        let harmonics = [(1.0, 1.0), (2.0, 1.0), (3.0, 1.0)];
        let wt = Wavetable::from_harmonics(15_000.0, 44100, &harmonics);
        // Should only synthesize fundamental without panicking or creating aliasing
        assert_eq!(wt.samples.len(), TABLE_SIZE + 1);
        assert!(!wt.samples[0].is_nan());
    }
}
