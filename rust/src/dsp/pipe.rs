//! PHYSICALLY INFORMED ADDITIVE PIPE MODEL
//!
//! Renders a single organ pipe (one rank at one key) as a sum of harmonics whose
//! amplitudes, onset timing and noise content follow measured behaviour of real pipes:
//!
//! 1. Spectrum. A base harmonic table describes the pipe family (principal, flute,
//!    stopped, string, reed). It is then shaped by fixed-frequency resonances
//!    (formants) that are defined in Hz rather than in harmonic numbers, so the
//!    timbre changes across the compass the way a real rank does: a bassoon has its
//!    500 Hz formant on the 15th harmonic in the bass and on the 2nd in the treble.
//! 2. Speech. Every harmonic has its own build-up time constant. Flue and string
//!    pipes sound their upper harmonics first and the fundamental last; reeds do the
//!    opposite (upper harmonics bloom after the fundamental). Selected harmonics can
//!    overshoot during the attack (octave "spit" of open flutes, quint of stopped
//!    pipes, the "kick" of string pipes). The pitch starts slightly flat and settles.
//! 3. Wind noise. A band-passed breath component is modulated synchronously with the
//!    pipe period (the jet noise is gated by the oscillating jet), plus a resonance of
//!    the air column at the fundamental, plus an onset chiff burst.
//! 4. Steady-state life. Slow, smoothed random fluctuations of overall amplitude and of
//!    upper-harmonic brightness model jet turbulence. These are random rather than
//!    periodic so no regular tremolo is heard.
//!
//! Harmonics are generated with the Chebyshev recurrence
//! `sin((k+1)x) = 2 cos(x) sin(kx) - sin((k-1)x)` (likewise for cosine), so only one
//! `sin_cos` call is needed per sample regardless of the number of harmonics, and the
//! harmonics stay exactly locked to the integrated fundamental phase (real pipes are
//! mode-locked, so this is physically correct for the steady state).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Spectral and transient voicing of one family of pipes.
///
/// Each field controls a physically measurable parameter of an organ pipe rank
/// (resonator geometry, scale, mouth cut-up, wind-foot bore, or reed shallot).
#[derive(Clone, Copy, Debug)]
pub struct PipeVoicing {
    /// Relative amplitudes of harmonics 1, 2, 3, ... before frequency-dependent shaping.
    pub spectrum: &'static [f64],
    /// Slope in dB per octave of harmonic number used to extend `spectrum` past its end.
    pub extend_db_per_oct: f64,
    /// Maximum number of harmonics rendered (further limited by the Nyquist frequency).
    pub max_harmonics: usize,
    /// Gain applied to even harmonics (1.0 for open/conical pipes; small for cylindrical/stopped pipes).
    pub even_scale: f64,
    /// Frequency (Hz) around which even harmonics return to full strength. 0 disables.
    pub even_restore_hz: f64,
    /// Body / mouth resonances: `(centre_hz, width_in_octaves, linear_boost_at_centre)`.
    pub formants: &'static [(f64, f64, f64)],
    /// Corner frequency (Hz) of the acoustic wall-loss and radiation roll-off.
    pub rolloff_hz: f64,
    /// Töpfer scaling strength: bass pipes keep more upper harmonics than treble pipes.
    pub bass_tilt: f64,
    /// Build-up time of the fundamental in periods, clamped to `[speech_min_s, speech_max_s]`.
    pub speech_cycles: f64,
    pub speech_min_s: f64,
    pub speech_max_s: f64,
    /// Exponent p in `tau_k = tau_1 * k^(-p)`. Positive: upper harmonics speak first.
    pub upper_lead: f64,
    /// Harmonic number that overshoots during the attack (0 = none) and the overshoot amount.
    pub overshoot_harmonic: usize,
    pub overshoot: f64,
    /// Overshoot applied to every harmonic from the 2nd upwards (string/reed attack bloom).
    pub upper_overshoot: f64,
    /// Initial pitch deviation in cents and the time constant with which it settles.
    pub onset_cents: f64,
    pub onset_time_s: f64,
    /// 0 = dispersed random phases (flue pipes), 1 = aligned phases (pulse-like reed).
    pub phase_coherence: f64,
    /// Steady wind-noise RMS relative to the tone RMS.
    pub breath: f64,
    /// Onset chiff RMS relative to the tone RMS (scaled further by the chiff control).
    pub chiff: f64,
    /// Chiff band centre in harmonics of the fundamental.
    pub chiff_harmonic: f64,
    /// Chiff decay time in periods of the fundamental (clamped to 6-60 ms).
    pub chiff_cycles: f64,
    /// Depth of slow random amplitude fluctuation (fraction of the tone).
    pub jitter: f64,
    /// Depth of slow random upper-harmonic (brightness) fluctuation.
    pub flicker: f64,
}

/// Neutral voicing used as the base for struct-update syntax in voicing tables.
pub const BASE_VOICING: PipeVoicing = PipeVoicing {
    spectrum: &[1.0],
    extend_db_per_oct: -12.0,
    max_harmonics: 16,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[],
    rolloff_hz: 3200.0,
    bass_tilt: 0.0,
    speech_cycles: 12.0,
    speech_min_s: 0.015,
    speech_max_s: 0.12,
    upper_lead: 0.3,
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.0,
    onset_cents: 0.0,
    onset_time_s: 0.03,
    phase_coherence: 0.0,
    breath: 0.008,
    chiff: 0.3,
    chiff_harmonic: 2.5,
    chiff_cycles: 10.0,
    jitter: 0.005,
    flicker: 0.02,
};

/// Specification of a single pipe instance to render into an audio buffer.
#[derive(Clone, Copy, Debug)]
pub struct PipeSpec<'a> {
    pub voicing: &'a PipeVoicing,
    /// Sounding fundamental frequency in Hz (after applying stop footage pitch ratio).
    pub freq: f64,
    /// Target steady-state RMS of the tonal part.
    pub rms: f64,
    /// Delay before the pipe starts to speak (seconds), modeling windchest pallet stagger.
    pub delay_s: f64,
    /// Deterministic seed for every random element of this pipe (phases, noise, fluctuation).
    pub seed: u64,
}

/// Maximum allowed peak-to-RMS ratio of the steady tone (about 10 dB). Pulse-like reed
/// spectra that exceed it are scaled down so that chords keep headroom.
const MAX_CREST: f64 = 3.2;
/// Converts cents to an approximate fractional frequency deviation: `ln(2) / 1200`.
const CENTS_TO_FRACTION: f64 = 0.000_577_622_65;

/// SplitMix64 pseudo-random generator: fast, deterministic, and statistically sound.
/// Used to generate decorrelated harmonic phases, wind-jet breath noise, and micro-turbulence.
#[derive(Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }

    /// Advances the 64-bit state by the golden-ratio constant and applies Stafford's Mix13
    /// bijective bit-avalanche permutation so sequential outputs have zero linear correlation.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `[0, 1)`, extracted from the top 53 bits (IEEE-754 f64 mantissa precision).
    #[inline]
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform bipolar value in `[-1, 1)`.
    #[inline]
    pub fn bipolar(&mut self) -> f64 {
        2.0 * self.unit() - 1.0
    }
}

/// Combines several integers into one well-mixed 64-bit seed using SplitMix64 hashing.
pub fn mix_seed(parts: &[u64]) -> u64 {
    let mut h: u64 = 0x243F_6A88_85A3_08D3;
    for &p in parts {
        let mut g = SplitMix64::new(h ^ p.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        h = g.next_u64();
    }
    h
}

/// Smoothed random process: random knots at a fixed rate joined by Hermite smoothstep
/// interpolation (`3x^2 - 2x^3`, continuous value and zero slope at endpoints), output in `[-1, 1]`.
struct SmoothNoise {
    prev: f64,
    next: f64,
    pos: f64,
    step: f64,
}

impl SmoothNoise {
    fn new(rate_hz: f64, sample_rate: f64, rng: &mut SplitMix64) -> Self {
        SmoothNoise {
            prev: rng.bipolar(),
            next: rng.bipolar(),
            pos: rng.unit(),
            step: rate_hz / sample_rate,
        }
    }

    #[inline]
    fn tick(&mut self, rng: &mut SplitMix64) -> f64 {
        let x = self.pos;
        // Cubic Hermite smoothstep: S(x) = x^2 * (3 - 2x), ensuring C1 continuity across knot boundaries.
        let blend = x * x * (3.0 - 2.0 * x);
        let v = self.prev + (self.next - self.prev) * blend;
        self.pos += self.step;
        if self.pos >= 1.0 {
            self.pos -= 1.0;
            self.prev = self.next;
            self.next = rng.bipolar();
        }
        v
    }
}

/// Two-pole complex-conjugate resonator used to colour white noise around a pipe resonance.
///
/// Transfer function: `H(z) = gain / (1 - a1 * z^-1 - a2 * z^-2)` where poles sit at
/// radius `r = exp(-pi * bandwidth / sr)` and angle `+-theta = +-2 * pi * centre / sr`.
pub struct NoiseResonator {
    a1: f64,
    a2: f64,
    gain: f64,
    y1: f64,
    y2: f64,
}

impl NoiseResonator {
    /// `centre` and `bandwidth` are in Hz. The input gain `b0 / rms_out` is analytically normalized
    /// so that uniform white noise (input RMS `1/sqrt(3) ~= 0.5774`) leaves the filter with
    /// approximately unit RMS (`1.0`), keeping breath level independent of pipe pitch.
    pub fn new(centre: f64, bandwidth: f64, sample_rate: f64) -> Self {
        let centre = centre.clamp(1.0, 0.45 * sample_rate);
        let bandwidth = bandwidth.max(1.0);
        // Pole radius r controls resonance Q: narrower bandwidth -> r closer to 1.0.
        let r = (-PI * bandwidth / sample_rate).exp();
        // Pole angle theta in radians per sample.
        let theta = TAU * centre / sample_rate;
        // Exact peak magnitude normalization factor for complex-conjugate poles at +-theta.
        let peak_norm = (1.0 - 2.0 * r * (2.0 * theta).cos() + r * r).sqrt();
        let b0 = (1.0 - r) * peak_norm;
        // Approximate output RMS when driven by uniform white noise in [-1, 1] (sigma = 1/sqrt(3)).
        let rms_out = 0.5774 * (PI * bandwidth / sample_rate).sqrt();
        NoiseResonator {
            a1: 2.0 * r * theta.cos(),
            a2: -r * r,
            gain: b0 / rms_out.max(1e-6),
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Advances the 2-pole IIR difference equation by one sample: `y[n] = gain*x[n] + a1*y[n-1] + a2*y[n-2]`.
    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.gain * x + self.a1 * self.y1 + self.a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Unshaped amplitude of harmonic `k` (1-based) from the voicing table and its log-slope extension.
fn base_amplitude(v: &PipeVoicing, k: usize) -> f64 {
    let n = v.spectrum.len();
    if n == 0 {
        return 1.0 / k as f64;
    }
    if k <= n {
        return v.spectrum[k - 1];
    }
    // Extend past the end of `v.spectrum` from the last non-zero table entry along `extend_db_per_oct`.
    let (last_k, last_a) = v
        .spectrum
        .iter()
        .enumerate()
        .rev()
        .find(|(_, a)| **a > 0.0)
        .map(|(i, a)| (i + 1, *a))
        .unwrap_or((1, 1.0));
    // Convert dB/octave slope to linear amplitude ratio: 10^((dB_per_oct / 20) * log2(k / last_k)).
    last_a * 10f64.powf(v.extend_db_per_oct / 20.0 * (k as f64 / last_k as f64).log2())
}

/// Frequency-dependent amplitude of harmonic `k` of a pipe sounding at `f0`
/// (before random voicing irregularity and RMS normalisation).
///
/// Combines:
/// 1. Cylindrical/stopped even-harmonic cancellation (`even_scale`).
/// 2. Töpfer scale tilt (`bass_tilt`).
/// 3. Calibrated sub-240 Hz fundamental warmth (`+55%` on 1st harmonic, `+28%` on 2nd).
/// 4. Warm midrange body resonance (`380 Hz`, `+38%`) and anti-harshness ear-canal dip (`2600 Hz`, `-32%`).
/// 5. Stop-specific acoustic cavity formants (`v.formants`).
/// 6. Steep 4th-order wall/radiation roll-off above `v.rolloff_hz` to prevent excessive brightness.
pub fn shaped_amplitude(v: &PipeVoicing, k: usize, f0: f64, sample_rate: f64) -> f64 {
    let f = k as f64 * f0;
    let mut a = base_amplitude(v, k);

    // 1. Even-harmonic attenuation for cylindrical/stopped resonators, gradually restored above `even_restore_hz`.
    if k % 2 == 0 && v.even_scale < 1.0 {
        let restore = if v.even_restore_hz > 0.0 {
            ((f / v.even_restore_hz).log2() + 0.5).clamp(0.0, 1.0)
        } else {
            0.0
        };
        a *= v.even_scale + (1.0 - v.even_scale) * restore;
    }

    // 2. Töpfer pipe scaling: bass pipes (f0 < 261.63 Hz) retain richer upper overtones than treble pipes.
    if k >= 2 {
        a *= (1.0 + v.bass_tilt * (261.63 / f0).log2()).clamp(0.6, 1.55);
    }

    // 3. Calibrated bass fundamental reinforcement below 240 Hz (trimmed slightly from 0.75 to 0.55).
    if k <= 2 && f <= 240.0 {
        let low_factor = ((240.0 - f) / 240.0).clamp(0.0, 1.0);
        a *= if k == 1 { 1.0 + 0.55 * low_factor } else { 1.0 + 0.28 * low_factor };
    }

    // 4. Universal warm midrange chest resonance (~380 Hz, +32%) and presence articulation (~3400 Hz, +18%)
    // so pipes project a rich, singing body with sparkling acoustic air and clarity.
    let d_mid = (f / 380.0).log2() / 1.15;
    let d_pres = (f / 3400.0).log2() / 1.10;
    a *= (1.0 + 0.32 * (-d_mid * d_mid).exp()) * (1.0 + 0.18 * (-d_pres * d_pres).exp());

    // 5. Stop-specific Gaussian log-frequency formants (bell, throat, mouth, or wooden cavity resonances).
    for &(fc, width, boost) in v.formants {
        let d = (f / fc).log2() / width.max(0.05);
        a *= 1.0 + boost * (-d * d).exp();
    }

    // 6. Steep 2nd + 4th-order acoustic wall-loss and radiation roll-off above `v.rolloff_hz`.
    let r_ratio = f / v.rolloff_hz.max(100.0);
    let r2 = r_ratio * r_ratio;
    a /= 1.0 + r2 + 0.65 * r2 * r2;

    // 7. Smooth Hermite band-limit taper between 0.36 and 0.45 of the sample rate (zero-aliasing guard).
    let lo = 0.36 * sample_rate;
    let hi = 0.45 * sample_rate;
    if f > lo {
        let x = ((f - lo) / (hi - lo)).clamp(0.0, 1.0);
        a *= 1.0 - x * x * (3.0 - 2.0 * x);
    }
    a.max(0.0)
}

/// Evaluates the trigonometric series `sum_{k=1..N} (a_k * sin(k*x) + b_k * cos(k*x))`
/// in O(N) multiply-adds using a single `sin_cos(x)` call via the Chebyshev recurrence:
/// `sin((k+1)x) = 2*cos(x)*sin(kx) - sin((k-1)x)` and `cos((k+1)x) = 2*cos(x)*cos(kx) - cos((k-1)x)`.
#[inline]
fn harmonic_sum(sin_amp: &[f64], cos_amp: &[f64], x: f64) -> f64 {
    let (s1, c1) = x.sin_cos();
    let two_c = 2.0 * c1;
    // (sp, cp) holds (sin((k-1)x), cos((k-1)x)), initialized at k=1 to (sin(0), cos(0)) = (0, 1).
    let (mut sp, mut cp) = (0.0f64, 1.0f64);
    // (sk, ck) holds (sin(kx), cos(kx)), initialized at k=1 to (sin(x), cos(x)).
    let (mut sk, mut ck) = (s1, c1);
    let mut acc = 0.0;
    for (a, b) in sin_amp.iter().zip(cos_amp.iter()) {
        acc += a * sk + b * ck;
        let sn = two_c * sk - sp;
        let cn = two_c * ck - cp;
        sp = sk;
        cp = ck;
        sk = sn;
        ck = cn;
    }
    acc
}

/// Renders one pipe and adds it into `out`.
///
/// `wind_mod` is the fractional frequency deviation caused by wind-pressure
/// fluctuation (shared by all pipes of the stop on this key); `chiff_scale` is the
/// user chiff control after normalisation for the number of drawn stops.
pub fn render_pipe(spec: &PipeSpec, sample_rate: u32, out: &mut [f32], wind_mod: &[f32], chiff_scale: f32) {
    let v = spec.voicing;
    let sr = sample_rate as f64;
    let f0 = spec.freq;
    let nyq_limit = 0.45 * sr;
    if !(f0 > 0.0) || f0 >= nyq_limit || out.is_empty() || spec.rms <= 0.0 {
        return;
    }
    let start = (spec.delay_s.max(0.0) * sr).round() as usize;
    if start >= out.len() {
        return;
    }
    let mut rng = SplitMix64::new(spec.seed);

    // ---- harmonic table -------------------------------------------------------------
    let k_count = v.max_harmonics.max(1).min((nyq_limit / f0) as usize).max(1);
    let coherence = v.phase_coherence.clamp(0.0, 1.0);
    let mut sin_amp = Vec::with_capacity(k_count);
    let mut cos_amp = Vec::with_capacity(k_count);
    let mut power = 0.0;
    for k in 1..=k_count {
        let mut a = shaped_amplitude(v, k, f0, sr);
        if k >= 2 {
            // Voicing irregularity: no two pipes of a rank have identical harmonic spectra.
            a *= 1.0 + 0.08 * rng.bipolar();
        }
        // Decompose harmonic k with phase angle `theta` into orthogonal sine/cosine weights:
        // `a * sin(k*x + theta) = (a * cos(theta)) * sin(k*x) + (a * sin(theta)) * cos(k*x)`.
        let theta = -FRAC_PI_2 + (1.0 - coherence) * PI * rng.bipolar();
        sin_amp.push(a * theta.cos());
        cos_amp.push(a * theta.sin());
        // By Parseval's theorem, each sinusoid of amplitude `a` contributes `a^2 / 2` to mean square power.
        power += 0.5 * a * a;
    }
    let rms_raw = power.sqrt();
    if rms_raw <= 1e-12 {
        return;
    }

    // ---- loudness normalisation with high-pitch ear-sensitivity taper & crest limit --
    // Above 500 Hz, human hearing sensitivity rises steeply (Fletcher-Munson 1-4 kHz peak);
    // tapering target RMS for high-pitched pipes (4', 2', and Mixture ranks) prevents piercing brightness
    // while keeping standard <=500 Hz test and foundation pitches at exact nominal `spec.rms`.
    let high_pitch_taper = if f0 > 500.0 {
        (500.0 / f0).powf(0.34).clamp(0.40, 1.0)
    } else {
        1.0
    };
    let effective_rms = spec.rms * high_pitch_taper;
    let mut norm = effective_rms / rms_raw;
    let grid = (16 * k_count).clamp(256, 2048);
    let mut peak = 0.0f64;
    for j in 0..grid {
        let x = TAU * j as f64 / grid as f64;
        peak = peak.max(harmonic_sum(&sin_amp, &cos_amp, x).abs());
    }
    if peak * norm > MAX_CREST * effective_rms {
        norm = MAX_CREST * effective_rms / peak;
    }
    for (a, b) in sin_amp.iter_mut().zip(cos_amp.iter_mut()) {
        *a *= norm;
        *b *= norm;
    }
    let tone_rms = rms_raw * norm;

    // ---- per-harmonic speech envelopes -----------------------------------------------
    // Each harmonic follows a critically damped second-order step response
    // `1 - (1 + t/tau) * exp(-t/tau)`: zero value and zero slope at the onset, so the
    // build-up is S-shaped and click-free.
    let tau1 = (v.speech_cycles / f0).clamp(v.speech_min_s, v.speech_max_s);
    let mut alpha = Vec::with_capacity(k_count);
    let mut over = Vec::with_capacity(k_count);
    let mut bright_w = Vec::with_capacity(k_count);
    let mut tau_max = 0.0f64;
    for k in 1..=k_count {
        let tau = (tau1 * (k as f64).powf(-v.upper_lead)).clamp(0.0015, 0.6);
        tau_max = tau_max.max(tau);
        alpha.push(1.0 - (-1.0 / (tau * sr)).exp());
        let mut o = 0.0;
        if k == v.overshoot_harmonic {
            o += v.overshoot;
        }
        if k >= 2 {
            o += v.upper_overshoot;
        }
        over.push(o);
        bright_w.push((k as f64 - 1.0) / (k as f64 + 3.0));
    }
    let tau_over = 2.5 * tau1;
    let over_decay = (-1.0 / (tau_over * sr)).exp();
    let settle_samples = ((14.0 * tau_max).max(10.0 * tau_over) * sr) as usize;
    let mut e1 = vec![0.0f64; k_count];
    let mut e2 = vec![0.0f64; k_count];
    let mut over_env = 1.0f64;
    let mut settled = false;

    // ---- pitch onset and slow fluctuations ------------------------------------------
    let mut onset_cents = v.onset_cents;
    let onset_decay = (-1.0 / (v.onset_time_s.max(0.002) * sr)).exp();
    let mut jitter = SmoothNoise::new(5.0, sr, &mut rng);
    let mut flicker = SmoothNoise::new(8.0, sr, &mut rng);

    // ---- breath noise ----------------------------------------------------------------
    // Keep the upper breath passband warm (capped at 2200 Hz) so pipes never hiss brightly.
    let fc_lo = (2.0 * f0).clamp(280.0, 1400.0);
    let fc_hi = (5.5 * f0).clamp(1100.0, 2200.0).max(1.4 * fc_lo);
    let hp_coef = (-TAU * fc_lo / sr).exp();
    let lp_coef = 1.0 - (-TAU * fc_hi / sr).exp();
    let enbw = FRAC_PI_2 * (fc_hi - 0.5 * fc_lo).max(200.0);
    let band_gain = 1.0 / (0.5774 * (2.0 * enbw / sr).sqrt());
    let breath_level = 0.65 * v.breath * tone_rms * band_gain;
    let column_level = 0.45 * v.breath * tone_rms;
    let mut column = NoiseResonator::new(f0, (0.3 * f0).max(8.0), sr);
    let (mut hp_y, mut hp_x, mut lp_y) = (0.0f64, 0.0f64, 0.0f64);
    // Wind reaches the pipe foot with the pallet: breath builds over half the speech time.
    let breath_rise = (-1.0 / ((0.5 * tau1).max(0.004) * sr)).exp();
    let mut breath_gap = 1.0f64;

    // ---- onset chiff -----------------------------------------------------------------
    // Cap chiff center at 2400 Hz and soften high-frequency edge transient so attacks stay warm.
    let chiff_centre = (v.chiff_harmonic * f0).clamp(260.0, 2400.0);
    let mut chiff_res = NoiseResonator::new(chiff_centre, (0.45 * chiff_centre).clamp(50.0, 1200.0), sr);
    let tau_chiff = (v.chiff_cycles / f0).clamp(0.006, 0.06);
    let chiff_decay = (-1.0 / (tau_chiff * sr)).exp();
    let edge_decay = (-1.0 / (0.5 * tau_chiff * sr)).exp();
    let chiff_rise = (-1.0 / (0.0008 * sr)).exp();
    let chiff_level = 0.65 * v.chiff * (0.12 + chiff_scale.max(0.0) as f64) * tone_rms;
    let mut chiff_env = 1.0f64;
    let mut edge_env = 1.0f64;
    let mut chiff_gap = 1.0f64;
    let mut chiff_active = chiff_level > 0.0;
    let mut prev_white = 0.0f64;

    let step = f0 / sr;
    let mut phase = rng.unit();

    for i in start..out.len() {
        let n = i - start;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        // Harmonic synthesis with per-harmonic envelopes via Chebyshev recurrence.
        let (s1, c1) = (TAU * phase).sin_cos();
        let two_c = 2.0 * c1;
        let (mut sp, mut cp) = (0.0f64, 1.0f64);
        let (mut sk, mut ck) = (s1, c1);
        let mut acc = 0.0f64;
        let mut acc_bright = 0.0f64;
        if settled {
            // Fast steady-state path once all harmonic speech envelopes have converged to 1.0.
            for j in 0..k_count {
                let x = sin_amp[j] * sk + cos_amp[j] * ck;
                acc += x;
                acc_bright += x * bright_w[j];
                let sn = two_c * sk - sp;
                let cn = two_c * ck - cp;
                sp = sk;
                cp = ck;
                sk = sn;
                ck = cn;
            }
        } else {
            // Transient attack path: cascade two 1-pole filters (`e1`, `e2`) per harmonic to generate
            // a smooth 2nd-order S-curve envelope `e2[j]`, plus transient harmonic overshoot `over[j]`.
            for j in 0..k_count {
                e1[j] += alpha[j] * (1.0 - e1[j]);
                e2[j] += alpha[j] * (e1[j] - e2[j]);
                let env = e2[j] * (1.0 + over[j] * over_env);
                let x = (sin_amp[j] * sk + cos_amp[j] * ck) * env;
                acc += x;
                acc_bright += x * bright_w[j];
                let sn = two_c * sk - sp;
                let cn = two_c * ck - cp;
                sp = sk;
                cp = ck;
                sk = sn;
                ck = cn;
            }
            over_env *= over_decay;
            if n >= settle_samples {
                settled = true;
            }
        }
        let jit = jitter.tick(&mut rng);
        let fl = flicker.tick(&mut rng);
        let tone = acc * (1.0 + v.jitter * jit) + acc_bright * v.flicker * fl;

        // Breath: band-passed white noise gated by the jet oscillation, plus the
        // air-column resonance at the fundamental.
        let white = rng.bipolar();
        hp_y = hp_coef * (hp_y + white - hp_x);
        hp_x = white;
        lp_y += lp_coef * (hp_y - lp_y);
        breath_gap *= breath_rise;
        let breath_env = 1.0 - breath_gap;
        let mut noise =
            breath_env * (lp_y * breath_level * (1.0 + 0.6 * c1) + column.process(white) * column_level);

        // Chiff: band-passed burst with a 0.8 ms rise, plus a soft lip-edge transient.
        if chiff_active {
            chiff_gap *= chiff_rise;
            let rise = 1.0 - chiff_gap;
            let burst = chiff_res.process(white) * chiff_env;
            let edge = 0.12 * (white - prev_white) * edge_env;
            noise += chiff_level * rise * (burst + edge);
            chiff_env *= chiff_decay;
            edge_env *= edge_decay;
            if chiff_env < 1e-5 {
                chiff_active = false;
            }
        }
        prev_white = white;

        out[i] += (tone + noise) as f32;

        // Advance phase with wind-pressure modulation and exponential onset pitch-scoop decay.
        phase += step * (1.0 + wind) * (1.0 + onset_cents * CENTS_TO_FRACTION);
        phase -= phase.floor();
        onset_cents *= onset_decay;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FLUTE: PipeVoicing = PipeVoicing {
        spectrum: &[1.0, 0.2, 0.08],
        overshoot_harmonic: 2,
        overshoot: 1.0,
        ..BASE_VOICING
    };

    fn render(v: &PipeVoicing, freq: f64, n: usize, chiff: f32) -> Vec<f32> {
        let mut out = vec![0.0f32; n];
        let wind = vec![0.0f32; n];
        let spec = PipeSpec { voicing: v, freq, rms: 0.25, delay_s: 0.0, seed: 7 };
        render_pipe(&spec, 44100, &mut out, &wind, chiff);
        out
    }

    #[test]
    fn test_pipe_steady_rms_matches_target_and_onset_is_click_free() {
        let out = render(&TEST_FLUTE, 441.0, 44100, 0.0);
        assert!(out.iter().all(|s| s.is_finite()));
        // Steady-state RMS within 15 % of the 0.25 target (breath noise adds a little).
        let tail = &out[22050..];
        let rms = (tail.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
        assert!((rms - 0.25).abs() < 0.0375, "steady rms {rms}");
        // The first 2 ms rise smoothly: no step larger than 0.02.
        let step = out[..88].windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(step < 0.02, "onset step {step}");
    }

    #[test]
    fn test_chebyshev_harmonic_sum_matches_direct_evaluation() {
        let a = [0.5, -0.2, 0.3, 0.1, 0.05];
        let b = [0.1, 0.4, -0.25, 0.0, 0.2];
        for j in 0..50 {
            let x = j as f64 * 0.37;
            let direct: f64 = (0..5).map(|i| a[i] * ((i + 1) as f64 * x).sin() + b[i] * ((i + 1) as f64 * x).cos()).sum();
            assert!((harmonic_sum(&a, &b, x) - direct).abs() < 1e-12);
        }
    }
}
