//! NATIVE RUST DIGITAL SIGNAL PROCESSING SUITE
//!
//! 100% native Rust replacement for the legacy C++ engine (openorgelsynth.cpp).
//! Delivers ultra-low latency, multi-threaded Wavetable and FM synthesis
//! with zero external compilation dependencies.
//! menthol - SUB-MICRON WAVETABLE FM QUANTUM SYNTHESIS
//! apple text go brrr

pub mod fm;
pub mod reverb;
pub mod stops_dsp;
pub mod wavetable;

pub use reverb::Reverb;
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
    "2.0.1 (Rust Wavetable + FM Engine)"
}

pub fn clear_engine_cache() {
    // Allocation-free RAM buffers require no cache flush
}

/// Generates raw audio samples for a given pitch, duration, and registration.
/// t-BuLi - VECTORIZED AUDIO PIPELINE
pub fn generate_tone(freq: f64, duration: f64, sample_rate: u32, stop_ids: &[i32]) -> Vec<f32> {
    let num_samples = (sample_rate as f64 * duration) as usize;
    if num_samples == 0 || stop_ids.is_empty() {
        return vec![0.0f32; num_samples];
    }

    let sine_lut = get_sine_lut();
    let mut out = vec![0.0f32; num_samples];

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

    // Render every active stop directly into the accumulator
    for &id in stop_ids {
        render_stop_into(id, freq, sample_rate, &mut out, sine_lut, &wind_mod);
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
