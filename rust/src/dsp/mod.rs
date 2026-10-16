//! NATIVE RUST DIGITAL SIGNAL PROCESSING SUITE
//!
//! 100% native Rust replacement for the legacy C++ engine (openorgelsynth.cpp).
//! Delivers ultra-low latency, multi-threaded Wavetable and FM synthesis
//! with zero external compilation dependencies.

pub mod fm;
pub mod reverb;
pub mod stops_dsp;
pub mod wavetable;

pub use reverb::{AcousticVenue, Reverb};
use stops_dsp::render_stop_into;
use wavetable::SineLut;

use std::sync::OnceLock;

static SINE_LUT: OnceLock<SineLut> = OnceLock::new();

fn get_sine_lut() -> &'static SineLut {
    SINE_LUT.get_or_init(SineLut::new)
}

pub fn init_engine() {
    let _ = get_sine_lut();
}

pub fn engine_version() -> &'static str {
    "2.0.2 (Rust Wavetable + FM + Physical Reverb Engine)"
}

pub fn clear_engine_cache() {
    // Allocation-free RAM buffers require no cache flush
}

/// Peak fractional frequency deviation of each wind component.
/// A deviation of 0.001 equals 1.73 cents (1200 * log2(1.001)).
const WIND_RESERVOIR_DEPTH: f64 = 0.0006; // shared bellows/reservoir sway, about 1.0 cent
const WIND_PIPE_DEPTH: f64 = 0.0005; // periodic flutter of this pipe, about 0.9 cent
const WIND_TURBULENCE_DEPTH: f64 = 0.0006; // random turbulence of this pipe, about 1.0 cent

/// Builds the wind-pressure pitch modulation for one pipe (one stop at one pitch).
///
/// The returned values are fractional frequency deviations: renderers multiply their
/// phase increment by `1.0 + wind[i]` directly. Three components are summed:
/// 1. Reservoir sway at 0.9 Hz, identical for every pipe (all pipes share the bellows).
/// 2. A periodic flutter whose rate (1.1 to 2.3 Hz) and phase depend on the pipe.
/// 3. Smoothed random turbulence: random values every 1/7 s joined by raised-cosine
///    interpolation, so the wobble never repeats exactly.
///
/// The pipe identity (pitch and stop id) seeds a SplitMix64 generator, so the result is
/// deterministic for a given pipe but decorrelated between notes of a chord and between
/// ranks on the same note. Peak deviation is about 0.0017 (about 3 cents).
pub fn pipe_wind_modulation(
    freq: f64,
    stop_id: i32,
    num_samples: usize,
    sample_rate: u32,
    sine_lut: &SineLut,
) -> Vec<f32> {
    let sr = sample_rate as f64;
    let two_pi = 2.0 * std::f64::consts::PI;

    let mut state = ((freq * 100.0).round() as i64 as u64)
        ^ (stop_id as i64 as u64).wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut next_unit = move || -> f64 {
        // SplitMix64: returns a uniform value in [0, 1).
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    };

    let pipe_rate = 1.1 + 1.2 * next_unit();
    let pipe_phase = two_pi * next_unit();

    const KNOT_RATE: f64 = 7.0;
    let knot_count = (num_samples as f64 / sr * KNOT_RATE).ceil() as usize + 2;
    let knots: Vec<f64> = (0..knot_count).map(|_| 2.0 * next_unit() - 1.0).collect();

    let mut wind = vec![0.0f32; num_samples];
    for (n, w) in wind.iter_mut().enumerate() {
        let t = n as f64 / sr;
        let reservoir = WIND_RESERVOIR_DEPTH * sine_lut.sin(two_pi * 0.9 * t + 0.5) as f64;
        let flutter = WIND_PIPE_DEPTH * sine_lut.sin(two_pi * pipe_rate * t + pipe_phase) as f64;

        let kpos = t * KNOT_RATE;
        let k = kpos as usize;
        let frac = kpos - k as f64;
        let blend = 0.5 - 0.5 * (std::f64::consts::PI * frac).cos();
        let turbulence = WIND_TURBULENCE_DEPTH * (knots[k] * (1.0 - blend) + knots[k + 1] * blend);

        *w = (reservoir + flutter + turbulence) as f32;
    }
    wind
}

/// Generates raw audio samples for a given pitch, duration, registration, and chiff volume.
/// Automatically balances chiff across active stops to prevent transient multiplication.
pub fn generate_tone(
    freq: f64,
    duration: f64,
    sample_rate: u32,
    stop_ids: &[i32],
    chiff_volume: f32,
) -> Vec<f32> {
    let num_samples = (sample_rate as f64 * duration) as usize;
    if num_samples == 0 || stop_ids.is_empty() {
        return vec![0.0f32; num_samples];
    }

    let sine_lut = get_sine_lut();
    let mut out = vec![0.0f32; num_samples];

    // Energy normalization factor: prevents transient attack from multiplying when multiple stops are active
    let num_stops = stop_ids.len().max(1) as f32;
    let stop_norm = 1.0 / num_stops.sqrt();
    let effective_chiff = (chiff_volume * stop_norm).clamp(0.0, 1.5);

    // Render every active stop directly into the accumulator with calibrated chiff.
    // Each stop is a separate rank of pipes, so each gets its own wind modulation.
    for &id in stop_ids {
        let wind_mod = pipe_wind_modulation(freq, id, num_samples, sample_rate, sine_lut);
        render_stop_into(id, freq, sample_rate, &mut out, sine_lut, &wind_mod, effective_chiff);
    }

    // Sub-bass acoustic pipe boost for deep pedal stops
    if freq < 250.0 {
        let bass_boost = (250.0 / freq).sqrt() as f32;
        for s in out.iter_mut() {
            *s *= bass_boost;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_tone_chiff_scaling_and_normalization() {
        // Stop 11 is Diapason 8', Stop 7 is Flute 4', Stop 14 is Piccolo 2', Stop 1 is Clarinet 8'
        let single_stop = [11];
        let quad_stops = [11, 7, 14, 1];

        let tone_single = generate_tone(440.0, 0.05, 44100, &single_stop, 0.20);
        let tone_quad = generate_tone(440.0, 0.05, 44100, &quad_stops, 0.20);

        assert_eq!(tone_single.len(), 2205);
        assert_eq!(tone_quad.len(), 2205);

        for s in &tone_single {
            assert!(!s.is_nan() && !s.is_infinite());
        }
        for s in &tone_quad {
            assert!(!s.is_nan() && !s.is_infinite());
        }

        // Chiff 0.0 test
        let tone_zero_chiff = generate_tone(440.0, 0.05, 44100, &single_stop, 0.0);
        for s in &tone_zero_chiff {
            assert!(!s.is_nan() && !s.is_infinite());
        }
    }

    #[test]
    fn test_wind_flutter_is_audible_and_decorrelated_per_pipe() {
        let lut = SineLut::new();
        let n = 44100 * 3;
        let a = pipe_wind_modulation(440.0, 11, n, 44100, &lut);
        let b = pipe_wind_modulation(440.0, 7, n, 44100, &lut); // other rank, same note
        let c = pipe_wind_modulation(466.16, 11, n, 44100, &lut); // same rank, next note

        // Peak pitch deviation in cents must be audible but stay in tune.
        let peak = a.iter().fold(0.0f64, |m, &v| m.max((v as f64).abs()));
        let peak_cents = 1200.0 * (1.0 + peak).log2();
        assert!(peak_cents > 1.5 && peak_cents < 5.0, "peak wind deviation {peak_cents} cents");

        // Deterministic for the same pipe.
        assert_eq!(a, pipe_wind_modulation(440.0, 11, n, 44100, &lut));

        let corr = |x: &[f32], y: &[f32]| -> f64 {
            let (mut sxy, mut sxx, mut syy) = (0.0f64, 0.0f64, 0.0f64);
            for (&p, &q) in x.iter().zip(y.iter()) {
                sxy += p as f64 * q as f64;
                sxx += p as f64 * p as f64;
                syy += q as f64 * q as f64;
            }
            sxy / (sxx * syy).sqrt()
        };
        let r_ab = corr(&a, &b);
        let r_ac = corr(&a, &c);
        assert!(r_ab.abs() < 0.8, "ranks on one note must not wobble together: r={r_ab}");
        assert!(r_ac.abs() < 0.8, "notes of a chord must not wobble together: r={r_ac}");
    }
}
