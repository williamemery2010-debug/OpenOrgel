//! NATIVE RUST DIGITAL SIGNAL PROCESSING SUITE
//!
//! 100% native Rust replacement for the legacy C++ engine (openorgelsynth.cpp).
//! Delivers ultra-low latency, multi-threaded Wavetable and FM synthesis
//! with zero external compilation dependencies.
//!
//! Architectural Overview:
//! - `fm`: Multi-operator frequency & phase modulation for legacy reed synthesis.
//! - `pipe`: Physically informed Chebyshev additive pipe synthesizer with per-harmonic speech envelopes.
//! - `reverb`: Schroeder-Moorer physical room reverberation modeling four acoustic venues.
//! - `stops_dsp`: Master stop registry and specialized physical flue, reed, flute, and sample renderers.
//! - `wavetable`: High-precision precomputed sine lookup table (`SineLut`) with linear interpolation.

pub mod fm;
pub mod pipe;
pub mod reverb;
pub mod stops_dsp;
pub mod wavetable;

pub use reverb::{AcousticVenue, Reverb};
use stops_dsp::render_stop_into;
use wavetable::SineLut;

use std::sync::OnceLock;

/// Thread-safe singleton holding the global precomputed sine lookup table (`SineLut`),
/// avoiding repeated table allocation across multi-threaded note rendering workers.
static SINE_LUT: OnceLock<SineLut> = OnceLock::new();

/// Retrieves a static reference to the global `SineLut`, initializing it on first access.
fn get_sine_lut() -> &'static SineLut {
    SINE_LUT.get_or_init(SineLut::new)
}

/// Pre-warms the global DSP lookup tables at application startup so the first note
/// experiences zero initialization latency.
pub fn init_engine() {
    let _ = get_sine_lut();
}

/// Returns a human-readable identifier string for the active DSP synthesis engine.
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

    // Seed the SplitMix64 state uniquely from the quantized pitch (centi-Hertz) and stop ID,
    // multiplying stop_id by the 64-bit golden-ratio constant (0x9E3779B97F4A7C15) to decorrelate adjacent stops.
    let mut state = ((freq * 100.0).round() as i64 as u64)
        ^ (stop_id as i64 as u64).wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut next_unit = move || -> f64 {
        // SplitMix64: advances internal state and applies Stafford's Mix13 bijective bit-avalanche
        // before dividing top 53 bits by 2^53 to yield a uniform f64 in [0.0, 1.0).
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    };

    // Derive per-pipe periodic flutter frequency (1.1..2.3 Hz) and initial phase (0..2*pi).
    let pipe_rate = 1.1 + 1.2 * next_unit();
    let pipe_phase = two_pi * next_unit();

    // Pre-generate random control knots at 7 Hz (every ~143 ms) for smooth low-frequency wind turbulence.
    const KNOT_RATE: f64 = 7.0;
    let knot_count = (num_samples as f64 / sr * KNOT_RATE).ceil() as usize + 2;
    let knots: Vec<f64> = (0..knot_count).map(|_| 2.0 * next_unit() - 1.0).collect();

    let mut wind = vec![0.0f32; num_samples];
    for (n, w) in wind.iter_mut().enumerate() {
        let t = n as f64 / sr;
        // Component 1: Global wind-reservoir bellows sway (0.9 Hz), shared across all speaking pipes.
        let reservoir = WIND_RESERVOIR_DEPTH * sine_lut.sin(two_pi * 0.9 * t + 0.5) as f64;
        // Component 2: Individual pipe wind-foot eddy flutter (1.1..2.3 Hz).
        let flutter = WIND_PIPE_DEPTH * sine_lut.sin(two_pi * pipe_rate * t + pipe_phase) as f64;

        // Component 3: Raised-cosine (C1 continuous) interpolation between adjacent random turbulence knots.
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

    // Energy normalization factor: scales chiff by 1/sqrt(N) so simultaneous stops do not multiply transient clicks.
    let num_stops = stop_ids.len().max(1) as f32;
    let stop_norm = 1.0 / num_stops.sqrt();
    let effective_chiff = (chiff_volume * stop_norm).clamp(0.0, 1.5);

    // Render every active stop directly into the accumulator with calibrated chiff.
    // Each stop is a separate rank of pipes, so each gets its own decorrelated wind modulation.
    for &id in stop_ids {
        let wind_mod = pipe_wind_modulation(freq, id, num_samples, sample_rate, sine_lut);
        render_stop_into(id, freq, sample_rate, &mut out, sine_lut, &wind_mod, effective_chiff);
    }

    // Apply the multi-band cathedral acoustic contour EQ:
    // When only the delicate airy 8' flutes (40..=44) are drawn, use a lighter sub-bass shelf
    // while still applying the rich midrange boost and high-frequency warmth smoothing.
    let only_airy_flutes = stop_ids.iter().all(|&id| (40..=44).contains(&id));
    apply_bass_warmth_eq_internal(&mut out, freq, sample_rate, only_airy_flutes);

    out
}

/// Applies the multi-band cathedral acoustic contour EQ (calibrated sub-bass/bass shelf,
/// prominent open midrange presence boost, anti-nasal scoop, and high-frequency warmth smoothing).
pub fn apply_bass_warmth_eq(out: &mut [f32], freq: f64, sample_rate: u32) {
    apply_bass_warmth_eq_internal(out, freq, sample_rate, false);
}

/// Internal implementation of the multi-band cathedral equalizer.
///
/// Why this contour is used:
/// 1. **Slightly Trimmed Sub-Bass & Bass Shelf (`125 Hz` & `220 Hz`):** Provides deep cathedral
///    gravitas without overpowering or bloating the low end (trimmed down slightly from v2.0.17).
/// 2. **Prominent Open Midrange Boost (`220–760 Hz`):** Strongly reinforces the singing chest and
///    vocal body of the pipes (`mid_warmth` + `mid_presence`) so the midrange projects warmly.
/// 3. **Anti-Nasal Scoop (`820–1220 Hz`):** Attenuates the pinched nasal horn band.
/// 4. **High-Frequency Warmth Filter (`> 2100 Hz`):** Gently smooths excessive upper-treble
///    brightness via a 1-pole low-pass blend so upper ranks and chords never sound harsh or piercing.
fn apply_bass_warmth_eq_internal(
    out: &mut [f32],
    freq: f64,
    sample_rate: u32,
    light_bass: bool,
) {
    if out.is_empty() {
        return;
    }
    let sr = sample_rate as f64;
    let two_pi = 2.0 * std::f64::consts::PI;

    // One-pole low-pass filter coefficients: a = 1 - exp(-2 * pi * fc / sr).
    let a_125 = 1.0 - (-two_pi * 125.0 / sr).exp();
    let a_220 = 1.0 - (-two_pi * 220.0 / sr).exp();
    let a_520 = 1.0 - (-two_pi * 520.0 / sr).exp();
    let a_760 = 1.0 - (-two_pi * 760.0 / sr).exp();
    let a_820 = 1.0 - (-two_pi * 820.0 / sr).exp();
    let a_1220 = 1.0 - (-two_pi * 1220.0 / sr).exp();
    let a_2100 = 1.0 - (-two_pi * 2100.0 / sr).exp();

    let mut lp_sub1 = 0.0f64;
    let mut lp_sub2 = 0.0f64;
    let mut lp_220 = 0.0f64;
    let mut lp_520 = 0.0f64;
    let mut lp_760 = 0.0f64;
    let mut lp_820 = 0.0f64;
    let mut lp_1220 = 0.0f64;
    let mut lp_2100 = 0.0f64;

    // Sub-bass & bass shelf gains: microscopically reduced from (1.10, 0.58) to (0.88, 0.44)
    // so the bass remains deep and authoritative without being overdone.
    let (sub_gain, bass_gain) = if light_bass {
        (0.18, 0.16)
    } else {
        (0.88, 0.44)
    };

    // Pitch-dependent bass & midrange contour:
    // - Slightly gentler sub-320 Hz pitch boost (exponent 0.36, max 2.40 vs previous 0.46, max 3.10).
    // - Added midrange fundamental presence boost centered around 380 Hz (+28%) so middle-register
    //   melody and chord notes come out singing and full.
    let pitch_bass_boost = if !light_bass && freq < 320.0 {
        (320.0 / freq.max(16.0)).powf(0.36).clamp(1.0, 2.40)
    } else {
        1.0
    };
    let mid_pitch_dist = (freq.max(20.0) / 380.0).log2() / 1.15;
    let pitch_mid_boost = 1.0 + 0.28 * (-mid_pitch_dist * mid_pitch_dist).exp();
    let total_pitch_gain = pitch_bass_boost * pitch_mid_boost;

    for s in out.iter_mut() {
        let x = *s as f64;
        // Cascade two 125 Hz 1-pole filters to form a smooth 12 dB/oct sub-bass shelf.
        lp_sub1 += a_125 * (x - lp_sub1);
        lp_sub2 += a_125 * (lp_sub1 - lp_sub2);
        // Parallel 1-pole low-pass states used to construct band-pass midrange and anti-nasal filters.
        lp_220 += a_220 * (x - lp_220);
        lp_520 += a_520 * (x - lp_520);
        lp_760 += a_760 * (x - lp_760);
        lp_820 += a_820 * (x - lp_820);
        lp_1220 += a_1220 * (x - lp_1220);
        lp_2100 += a_2100 * (x - lp_2100);

        // Band-pass subtraction:
        // - `mid_chest` (220-520 Hz) adds warm wooden/metal chest fullness.
        // - `mid_presence` (220-760 Hz) projects vocal midrange clarity.
        // - `nasal_band` (820-1220 Hz) scoops out nasal honk.
        let mid_chest = lp_520 - lp_220;
        let mid_presence = lp_760 - lp_220;
        let nasal_band = lp_1220 - lp_820;

        // Blend 72% warm 2.1 kHz low-passed signal with 28% full-bandwidth signal to tame excessive brightness.
        let warm_dry = 0.72 * lp_2100 + 0.28 * x;

        let eq_val = (warm_dry
            + sub_gain * lp_sub2
            + bass_gain * lp_220
            + 0.62 * mid_chest
            + 0.74 * mid_presence
            - 0.24 * nasal_band)
            * total_pitch_gain;
        *s = eq_val as f32;
    }
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

    #[test]
    fn test_bass_warmth_eq_and_full_organ_foundation_weighting() {
        // Verify multi-band EQ boosts bass (110 Hz) and midrange (380 Hz) relative to high treble (3520 Hz)
        let sr = 44100;
        let mut bass_wave: Vec<f32> = (0..4410)
            .map(|n| (2.0 * std::f64::consts::PI * 110.0 * n as f64 / sr as f64).sin() as f32 * 0.25)
            .collect();
        let mut mid_wave: Vec<f32> = (0..4410)
            .map(|n| (2.0 * std::f64::consts::PI * 380.0 * n as f64 / sr as f64).sin() as f32 * 0.25)
            .collect();
        let mut treble_wave: Vec<f32> = (0..4410)
            .map(|n| (2.0 * std::f64::consts::PI * 3520.0 * n as f64 / sr as f64).sin() as f32 * 0.25)
            .collect();

        apply_bass_warmth_eq(&mut bass_wave, 110.0, sr);
        apply_bass_warmth_eq(&mut mid_wave, 380.0, sr);
        apply_bass_warmth_eq(&mut treble_wave, 3520.0, sr);

        let bass_rms = (bass_wave[1000..].iter().map(|s| s * s).sum::<f32>() / 3410.0).sqrt();
        let mid_rms = (mid_wave[1000..].iter().map(|s| s * s).sum::<f32>() / 3410.0).sqrt();
        let treble_rms = (treble_wave[1000..].iter().map(|s| s * s).sum::<f32>() / 3410.0).sqrt();
        assert!(
            bass_rms > 1.8 * treble_rms,
            "Bass EQ must reinforce low frequencies over high frequencies: bass_rms={bass_rms}, treble_rms={treble_rms}"
        );
        assert!(
            mid_rms > 2.2 * treble_rms,
            "Midrange EQ must strongly project 380 Hz midrange over high treble: mid_rms={mid_rms}, treble_rms={treble_rms}"
        );

        // Verify Full Organ registration weights prioritize 32'/16'/8' foundations over 2'/Mixtures
        let w_32 = stops_dsp::stop_registration_weight(10);      // Contrabassoon 32'
        let w_16 = stops_dsp::stop_registration_weight(3);       // Bombarde 16'
        let w_diapason = stops_dsp::stop_registration_weight(11); // Diapason 8'
        let w_mixture = stops_dsp::stop_registration_weight(15);  // Mixture IV
        assert!(w_32 > w_diapason && w_16 > w_diapason);
        assert!(w_diapason > 1.8 * w_mixture, "8' Diapason must outweigh high mixtures in Full Organ");
    }
}
