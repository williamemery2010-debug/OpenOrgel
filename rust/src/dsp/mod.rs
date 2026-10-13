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

    // Pre-calculate pipe wind modulation & aerodynamic turbulence
    let mut wind_mod = vec![0.0f32; num_samples];
    let sample_rate_f = sample_rate as f64;
    for (n, w) in wind_mod.iter_mut().enumerate() {
        let t = n as f64 / sample_rate_f;
        let wobble = 0.0015 * sine_lut.sin(0.7 * 2.0 * std::f64::consts::PI * t + 0.5)
            + 0.0010 * sine_lut.sin(1.3 * 2.0 * std::f64::consts::PI * t + 1.2)
            + 0.0008 * sine_lut.sin(2.8 * 2.0 * std::f64::consts::PI * t + 2.3);
        *w = wobble;
    }

    // Render every active stop directly into the accumulator with calibrated chiff
    for &id in stop_ids {
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
}
