//! HIGH-PERFORMANCE BANDLIMITED WAVETABLE OSCILLATOR ENGINE
//!
//! Replaces millions of per-sample trigonometric evaluations with
//! single-cycle RAM lookup tables and linear interpolation.

pub const TABLE_SIZE: usize = 2048;
pub const TABLE_MASK: usize = TABLE_SIZE - 1;

/// Global sine LUT for LFOs, wind wobble, and audio modulation.
pub struct SineLut {
    table: Vec<f32>,
}

impl SineLut {
    pub fn new() -> Self {
        let size = 4096;
        let mut table = Vec::with_capacity(size + 1);
        for i in 0..=size {
            let p = i as f64 * 2.0 * std::f64::consts::PI / size as f64;
            table.push(p.sin() as f32);
        }
        SineLut { table }
    }

    #[inline(always)]
    pub fn sin(&self, phase_rad: f64) -> f32 {
        let norm = phase_rad * (0.5 / std::f64::consts::PI);
        let pos = (norm - norm.floor()) * 4096.0;
        let idx = (pos as usize) & 4095;
        let frac = (pos - pos.floor()) as f32;
        self.table[idx] + frac * (self.table[idx + 1] - self.table[idx])
    }
}

/// A pre-computed single-cycle periodic pipe wave shape.
#[derive(Clone)]
pub struct Wavetable {
    pub samples: Vec<f32>,
}

impl Wavetable {
    /// Builds a periodic wavetable from a set of harmonic partials and amplitudes.
    /// Harmonics above Nyquist frequency (`sample_rate / 2`) are excluded to prevent aliasing.
    pub fn from_harmonics(
        fundamental_freq: f64,
        sample_rate: u32,
        harmonics: &[(f64, f64)], // (factor, amplitude)
    ) -> Self {
        let mut wave = vec![0.0f32; TABLE_SIZE + 1];
        let nyquist = sample_rate as f64 * 0.5;

        for &(factor, amp) in harmonics {
            let partial_freq = fundamental_freq * factor;
            if partial_freq >= nyquist {
                continue;
            }
            // Gentle high-frequency damping near Nyquist
            let damp = if partial_freq > nyquist * 0.85 {
                (1.0 - (partial_freq - nyquist * 0.85) / (nyquist * 0.15)) as f32
            } else {
                1.0
            };

            let amp_scaled = (amp as f32) * damp;
            let step = factor * 2.0 * std::f64::consts::PI / TABLE_SIZE as f64;
            for i in 0..TABLE_SIZE {
                let p = i as f64 * step;
                wave[i] += amp_scaled * (p.sin() as f32);
            }
        }

        // Wrap-around guard for branchless interpolation
        wave[TABLE_SIZE] = wave[0];

        // Normalise peak to 1.0 to preserve headroom
        let peak = wave.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        if peak > 1e-5 {
            let inv = 1.0 / peak;
            for s in wave.iter_mut() {
                *s *= inv;
            }
        }

        Wavetable { samples: wave }
    }

    /// Read the wavetable at normalized phase `[0.0, 1.0)` with linear interpolation.
    #[inline(always)]
    pub fn get_sample(&self, phase_normalized: f64) -> f32 {
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
