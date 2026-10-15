//! ORGAN STOP ACOUSTIC REGISTRY & DSP RENDERING PIPELINE
//!
//! Maps each of the 30 console stops to either a high-speed Wavetable profile,
//! an FM Reed Operator, or a high-fidelity Acoustic Sample.

use std::io::Cursor;
use std::sync::OnceLock;
use hound::WavReader;

use super::fm::{FmOperator, FmVoiceConfig};
use super::wavetable::{SineLut, Wavetable};

static CLARION_WAV_BYTES: &[u8] = include_bytes!("../../../samples/clarion.wav");
static FLUE_WAV_BYTES: &[u8] = include_bytes!("../../../samples/stoppedflue.wav");

pub struct EmbeddedSamples {
    pub clarion: Vec<f32>,
    pub clarion_rate: u32,
    pub flue: Vec<f32>,
    pub flue_rate: u32,
}

static SAMPLES: OnceLock<EmbeddedSamples> = OnceLock::new();

fn get_embedded_samples() -> &'static EmbeddedSamples {
    SAMPLES.get_or_init(|| {
        let decode_wav = |bytes: &[u8]| -> (Vec<f32>, u32) {
            let cursor = Cursor::new(bytes);
            if let Ok(mut reader) = WavReader::new(cursor) {
                let spec = reader.spec();
                let samples: Vec<f32> = reader
                    .samples::<i16>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / 32768.0)
                    .collect();
                (samples, spec.sample_rate)
            } else {
                (Vec::new(), 44100)
            }
        };

        let (clarion, clarion_rate) = decode_wav(CLARION_WAV_BYTES);
        let (flue, flue_rate) = decode_wav(FLUE_WAV_BYTES);
        EmbeddedSamples {
            clarion,
            clarion_rate,
            flue,
            flue_rate,
        }
    })
}

#[derive(Clone)]
pub enum StopSynthesisModel {
    /// Pure & chorus flue pipes: pre-baked bandlimited wavetable
    Wavetable {
        harmonics: &'static [(f64, f64)],
        dampening: f64,
        is_celeste: bool,
    },
    /// Complex acoustic reed pipes: Frequency & Phase Modulation
    FmReed(FmVoiceConfig),
    /// Real acoustic recording: linear pitch interpolated loop
    Sampled {
        is_clarion: bool,
        harmonic_factor: f64,
        base_freq: f64,
    },
    /// Stopped (covered) wooden flue pipe: additive odd-partial model with wind noise.
    /// `pitch_ratio` scales the key frequency (1.0 = 8', 2.0 = 4', 0.5 = 16', 0.25 = 32').
    StoppedFlue { pitch_ratio: f64 },
}

pub struct StopDefinition {
    pub name: &'static str,
    pub id: i32,
    pub model: StopSynthesisModel,
}

pub const STOP_DEFINITIONS: &[StopDefinition] = &[
    // 0: Oboe 8' (Legacy)
    StopDefinition {
        name: "Oboe 8'",
        id: 0,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 1.0,
            mod_ratio: 1.0,
            mod_index_sustain: 1.6,
            mod_index_attack: 0.7,
            attack_time: 0.02,
            feedback: 0.3,
        }),
    },
    // 1: Clarinet 8'
    StopDefinition {
        name: "Clarinet 8'",
        id: 1,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 1.0,
            mod_ratio: 2.0, // Suppresses even harmonics (hollow woodwind)
            mod_index_sustain: 1.4,
            mod_index_attack: 0.6,
            attack_time: 0.02,
            feedback: 0.2,
        }),
    },
    // 2: Bassoon 16'
    StopDefinition {
        name: "Bassoon 16'",
        id: 2,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 0.5,
            mod_ratio: 1.0,
            mod_index_sustain: 1.7,
            mod_index_attack: 0.8,
            attack_time: 0.03,
            feedback: 0.35,
        }),
    },
    // 3: Bombarde 16'
    StopDefinition {
        name: "Bombarde 16'",
        id: 3,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 0.5,
            mod_ratio: 1.0,
            mod_index_sustain: 2.3, // Heavy brass buzz
            mod_index_attack: 1.0,
            attack_time: 0.025,
            feedback: 0.5,
        }),
    },
    // 4: Ophicleide 16'
    StopDefinition {
        name: "Ophicleide 16'",
        id: 4,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 0.5,
            mod_ratio: 1.5,
            mod_index_sustain: 1.9,
            mod_index_attack: 0.9,
            attack_time: 0.028,
            feedback: 0.4,
        }),
    },
    // 5: Ottavino 2'
    StopDefinition {
        name: "Ottavino 2'",
        id: 5,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(4.0, 1.0), (8.0, 0.15), (12.0, 0.05)],
            dampening: 0.02,
            is_celeste: false,
        },
    },
    // 6: Cor Anglais 8'
    StopDefinition {
        name: "Cor Anglais 8'",
        id: 6,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 1.0,
            mod_ratio: 1.0,
            mod_index_sustain: 1.5,
            mod_index_attack: 0.7,
            attack_time: 0.02,
            feedback: 0.3,
        }),
    },
    // 7: Flute 4'
    StopDefinition {
        name: "Flute 4'",
        id: 7,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(2.0, 1.0), (4.0, 0.15), (6.0, 0.05)],
            dampening: 0.02,
            is_celeste: false,
        },
    },
    // 8: Clarinet 4'
    StopDefinition {
        name: "Clarinet 4'",
        id: 8,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 2.0,
            mod_ratio: 4.0,
            mod_index_sustain: 1.4,
            mod_index_attack: 0.6,
            attack_time: 0.015,
            feedback: 0.2,
        }),
    },
    // 9: Viol 4'
    StopDefinition {
        name: "Viol 4'",
        id: 9,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[
                (2.0, 1.0), (4.0, 0.7), (6.0, 0.6), (8.0, 0.5),
                (10.0, 0.4), (12.0, 0.3), (14.0, 0.2), (16.0, 0.1),
            ],
            dampening: 0.02,
            is_celeste: false,
        },
    },
    // 10: Contrabassoon 32'
    StopDefinition {
        name: "Contrabassoon 32'",
        id: 10,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 0.25,
            mod_ratio: 0.5,
            mod_index_sustain: 2.0,
            mod_index_attack: 1.0,
            attack_time: 0.04,
            feedback: 0.4,
        }),
    },
    // 11: Diapason 8' (Open Diapason / Montre 8' / Principal 8')
    StopDefinition {
        name: "Diapason 8'",
        id: 11,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[
                (1.0, 1.000), // Fundamental 8' (noble, full foundational tone)
                (2.0, 0.620), // Octave 4' (vital open cylindrical pipe singing warmth)
                (3.0, 0.380), // Twelfth 2-2/3' (vocal pipe body presence)
                (4.0, 0.240), // Fifteenth 2' (clarity, definition)
                (5.0, 0.140), // Tierce 1-3/5' (rich spotted metal warmth)
                (6.0, 0.085), // Nineteenth 1-1/3' (silvery brilliance)
                (7.0, 0.045), // Septième (mild natural overtone color)
                (8.0, 0.028), // Twenty-second 1'
                (9.0, 0.016),
                (10.0, 0.010),
            ],
            dampening: 0.04,
            is_celeste: false,
        },
    },
    // 12: Crystal Flute 4' (Glassy)
    StopDefinition {
        name: "Crystal Flute 4' (Glassy)",
        id: 12,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[
                (1.0, 0.4), (2.0, 1.03), (3.0, 0.08), (4.0, 0.03),
                (6.0, 0.1), (8.0, 0.01), (16.0, 0.03),
            ],
            dampening: 0.02,
            is_celeste: false,
        },
    },
    // 13: Cornet V 8'
    StopDefinition {
        name: "Cornet V 8'",
        id: 13,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(1.0, 1.0), (2.0, 0.8), (3.0, 0.9), (4.0, 0.7), (5.0, 0.8)],
            dampening: 0.08,
            is_celeste: false,
        },
    },
    // 14: Piccolo 2'
    StopDefinition {
        name: "Piccolo 2'",
        id: 14,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(4.0, 1.0), (8.0, 0.1), (12.0, 0.05), (16.0, 0.01)],
            dampening: 0.02,
            is_celeste: false,
        },
    },
    // 15: Mixture IV
    StopDefinition {
        name: "Mixture IV",
        id: 15,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(4.0, 1.0), (6.0, 0.8), (8.0, 0.6), (12.0, 0.4)],
            dampening: 0.08,
            is_celeste: false,
        },
    },
    // 16: Vox Humana 8'
    StopDefinition {
        name: "Vox Humana 8'",
        id: 16,
        model: StopSynthesisModel::FmReed(FmVoiceConfig {
            carrier_ratio: 1.0,
            mod_ratio: 1.0,
            mod_index_sustain: 1.4,
            mod_index_attack: 0.5,
            attack_time: 0.02,
            feedback: 0.3,
        }),
    },
    // 17: Gedeckt 8'
    StopDefinition {
        name: "Gedeckt 8'",
        id: 17,
        model: StopSynthesisModel::StoppedFlue { pitch_ratio: 1.0 },
    },
    // 18: Gedeckt 4'
    StopDefinition {
        name: "Gedeckt 4'",
        id: 18,
        model: StopSynthesisModel::StoppedFlue { pitch_ratio: 2.0 },
    },
    // 19: Gedeckt 16'
    StopDefinition {
        name: "Gedeckt 16'",
        id: 19,
        model: StopSynthesisModel::StoppedFlue { pitch_ratio: 0.5 },
    },
    // 20: Gedeckt 32'
    StopDefinition {
        name: "Gedeckt 32'",
        id: 20,
        model: StopSynthesisModel::StoppedFlue { pitch_ratio: 0.25 },
    },
    // 21: Cymbale Mixture
    StopDefinition {
        name: "Cymbale Mixture",
        id: 21,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(8.0, 1.0), (12.0, 0.8), (16.0, 0.6)],
            dampening: 0.08,
            is_celeste: false,
        },
    },
    // 22: Plein Jeu Mixture
    StopDefinition {
        name: "Plein Jeu Mixture",
        id: 22,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(2.0, 1.0), (3.0, 0.9), (4.0, 0.8), (6.0, 0.6), (8.0, 0.4)],
            dampening: 0.08,
            is_celeste: false,
        },
    },
    // 23: Scharf Mixture
    StopDefinition {
        name: "Scharf Mixture",
        id: 23,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(6.0, 1.0), (8.0, 0.9), (12.0, 0.7), (16.0, 0.5)],
            dampening: 0.08,
            is_celeste: false,
        },
    },
    // 24: Voix Celeste 8'
    StopDefinition {
        name: "Voix Celeste 8'",
        id: 24,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(1.0, 1.0)],
            dampening: 0.02,
            is_celeste: true,
        },
    },
    // 25: Acoustic Flue 8'
    StopDefinition {
        name: "Acoustic Flue 8'",
        id: 25,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 1.0,
            base_freq: 220.0,
        },
    },
    // 26: Clarion 4'
    StopDefinition {
        name: "Clarion 4'",
        id: 26,
        model: StopSynthesisModel::Sampled {
            is_clarion: true,
            harmonic_factor: 2.0,
            base_freq: 440.0,
        },
    },
    // 27: Acoustic Flue 16'
    StopDefinition {
        name: "Acoustic Flue 16'",
        id: 27,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 0.5,
            base_freq: 220.0,
        },
    },
    // 28: Acoustic Flue 4'
    StopDefinition {
        name: "Acoustic Flue 4'",
        id: 28,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 2.0,
            base_freq: 220.0,
        },
    },
    // 29: Acoustic Flue 2'
    StopDefinition {
        name: "Acoustic Flue 2'",
        id: 29,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 4.0,
            base_freq: 220.0,
        },
    },
    // 30: Clarion 8'
    StopDefinition {
        name: "Clarion 8'",
        id: 30,
        model: StopSynthesisModel::Sampled {
            is_clarion: true,
            harmonic_factor: 1.0,
            base_freq: 440.0,
        },
    },
];

/// Render a single stop's contribution into `out_buffer` using the optimal DSP engine.
pub fn render_stop_into(
    stop_id: i32,
    freq: f64,
    sample_rate: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    if stop_id < 0 || stop_id as usize >= STOP_DEFINITIONS.len() {
        return;
    }
    let def = &STOP_DEFINITIONS[stop_id as usize];

    match &def.model {
        StopSynthesisModel::Wavetable {
            harmonics,
            is_celeste,
            ..
        } => {
            let base_freq = if *is_celeste { freq * 1.003 } else { freq };
            let wt = Wavetable::from_harmonics(base_freq, sample_rate, harmonics);
            let sample_rate_f = sample_rate as f64;

            if *is_celeste {
                // Detuned undulating rank (Voix Céleste): dual detuned beating ranks
                let chorus_rates = [1.0, 1.0015, 0.9985];
                let chorus_gains = [0.8f32, 0.35f32, 0.35f32];
                for (&rate_mul, &gain) in chorus_rates.iter().zip(chorus_gains.iter()) {
                    let step = (base_freq * rate_mul) / sample_rate_f;
                    let mut phase = 0.0f64;
                    for (i, sample) in out.iter_mut().enumerate() {
                        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
                        let sample_val = wt.get_sample(phase);
                        *sample += sample_val * gain;
                        phase += step * (1.0 + wind * 0.0015);
                    }
                }
            } else {
                // Majestic, solid single flue pipe (Open Diapason, Principals, Flutes)
                // Real organ pipes resonate as a single acoustic air column with subtle wind jitter
                let step = base_freq / sample_rate_f;
                let mut phase = 0.0f64;
                for (i, sample) in out.iter_mut().enumerate() {
                    let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
                    let sample_val = wt.get_sample(phase);
                    *sample += sample_val;
                    phase += step * (1.0 + wind * 0.0020);
                }
            }

            // Pipe Languid Speech & Articulate Chiff for Flue Stops:
            // Models the initial turbulent vortex sheet striking the upper lip
            if chiff_scale > 0.001 {
                let chiff_samples = (0.022 * sample_rate_f).round() as usize; // 22 ms speech onset
                let decay_tau = (0.006 * sample_rate_f).max(1.0);
                let chiff_gain = chiff_scale * 0.16;
                for (i, sample) in out.iter_mut().enumerate().take(chiff_samples) {
                    let t = i as f64;
                    let env = (-t / decay_tau).exp() as f32;
                    let p3 = ((t * base_freq * 3.0 / sample_rate_f) * 2.0 * std::f64::consts::PI).sin() as f32;
                    let p5 = ((t * base_freq * 5.0 / sample_rate_f) * 2.0 * std::f64::consts::PI).sin() as f32;
                    let burst = (0.6 * p3 + 0.4 * p5) * env * chiff_gain;
                    *sample += burst;
                }
            }
        }
        StopSynthesisModel::FmReed(cfg) => {
            let mut fm = FmOperator::new(*cfg, sample_rate);
            fm.render(freq, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::StoppedFlue { pitch_ratio } => {
            render_stopped_flue(freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::Sampled {
            is_clarion,
            harmonic_factor,
            base_freq,
        } => {
            let samples = get_embedded_samples();
            let (pcm, src_rate) = if *is_clarion {
                (&samples.clarion, samples.clarion_rate)
            } else {
                (&samples.flue, samples.flue_rate)
            };
            if pcm.is_empty() {
                return;
            }

            let rank_freq = freq * harmonic_factor;
            let pitch_ratio = rank_freq / base_freq;
            let step = pitch_ratio * (src_rate as f64 / sample_rate as f64);

            let len = pcm.len();
            let loop_start = (0.2 * len as f64) as usize;
            let loop_end = (0.8 * len as f64) as usize;
            let loop_len = if loop_end > loop_start {
                loop_end - loop_start
            } else {
                len
            };

            // Smooth 5ms fade-in to eliminate initial DC offset click in sample files
            let fade_in_samples = (0.005 * sample_rate as f64).round() as usize;

            for (n, out_sample) in out.iter_mut().enumerate() {
                let mut pos = n as f64 * step;
                if pos >= loop_end as f64 {
                    let rem = pos - loop_start as f64;
                    pos = loop_start as f64 + (rem % loop_len as f64);
                }
                let i0 = pos as usize;
                let i1 = if i0 + 1 >= loop_end {
                    loop_start
                } else {
                    (i0 + 1).min(len.saturating_sub(1))
                };
                let frac = (pos - pos.floor()) as f32;
                let sample_val = (1.0 - frac) * pcm[i0.min(len.saturating_sub(1))] + frac * pcm[i1];
                let attack_ramp = if n < fade_in_samples {
                    (n as f32 / fade_in_samples as f32).sin()
                } else {
                    1.0
                };
                *out_sample += sample_val * attack_ramp;
            }
        }
    }
}

/// Partial table for a stopped (covered) wooden flue pipe.
/// Tuple: (harmonic number, relative amplitude, onset boost, drift rate Hz, drift depth).
///
/// A pipe closed at one end resonates at odd multiples of its fundamental only, so the
/// even partials are present only as faint residue from the mouth and stopper. The 3rd
/// partial (a twelfth above the fundamental) is the characteristic Gedackt "chirp".
/// `onset boost` makes the upper partials start stronger and settle (the pipe "speaks"
/// upper partials first) while the fundamental builds up slightly more slowly.
const STOPPED_PARTIALS: &[(f64, f64, f64, f64, f64)] = &[
    (1.0, 1.000, -0.30, 0.31, 0.015),
    (2.0, 0.016, 0.50, 0.53, 0.050),
    (3.0, 0.200, 1.60, 0.47, 0.050),
    (4.0, 0.010, 0.50, 0.71, 0.060),
    (5.0, 0.062, 2.20, 0.83, 0.060),
    (6.0, 0.005, 0.50, 0.59, 0.060),
    (7.0, 0.026, 2.50, 1.07, 0.070),
    (9.0, 0.010, 2.50, 0.97, 0.070),
    (11.0, 0.004, 2.50, 1.21, 0.080),
];

/// Two-pole resonator used to colour white noise around a pipe resonance.
struct NoiseResonator {
    a1: f64,
    a2: f64,
    gain: f64,
    y1: f64,
    y2: f64,
}

impl NoiseResonator {
    /// `centre` and `bandwidth` are in Hz. The gain is set so that uniform white noise
    /// (rms 1/sqrt(3)) leaves the filter with approximately unit rms, which keeps the
    /// breath level independent of the pitch of the pipe.
    fn new(centre: f64, bandwidth: f64, sample_rate: f64) -> Self {
        let r = (-std::f64::consts::PI * bandwidth / sample_rate).exp();
        let theta = 2.0 * std::f64::consts::PI * centre / sample_rate;
        let peak_norm = (1.0 - 2.0 * r * (2.0 * theta).cos() + r * r).sqrt();
        let b0 = (1.0 - r) * peak_norm;
        let rms_out = 0.5774 * (std::f64::consts::PI * bandwidth / sample_rate).sqrt();
        NoiseResonator {
            a1: 2.0 * r * theta.cos(),
            a2: -r * r,
            gain: b0 / rms_out.max(1e-6),
            y1: 0.0,
            y2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = self.gain * x + self.a1 * self.y1 + self.a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Renders a stopped wooden flue pipe (Gedeckt) into `out`.
///
/// The tone is additive: a dominant fundamental plus a weak odd-partial series, each
/// partial with its own slow amplitude drift so the spectrum is alive instead of a fixed
/// waveform. On top of that a real pipe has audible wind: a short broadband "spit" at the
/// onset (scaled by `chiff_scale`), then a quiet breath hiss concentrated around the
/// pipe resonances (fundamental and third partial).
pub fn render_stopped_flue(
    freq: f64,
    pitch_ratio: f64,
    sample_rate: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    let sr = sample_rate as f64;
    let f0 = freq * pitch_ratio;
    if f0 <= 0.0 || out.is_empty() {
        return;
    }
    let nyquist = sr * 0.5;
    let two_pi = 2.0 * std::f64::consts::PI;
    let chiff = chiff_scale.max(0.0) as f64;

    // Bass pipes keep a little more upper-partial colour, treble pipes are plainer.
    let bass_tilt = (1.0 + 0.25 * (261.63 / f0).log2()).clamp(0.7, 1.5);

    // (harmonic, amplitude, onset boost, drift rate, drift depth, drift phase)
    let mut partials: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(STOPPED_PARTIALS.len());
    let mut amp_sum = 0.0;
    for (i, &(k, a, boost, rate, depth)) in STOPPED_PARTIALS.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.9 {
            continue;
        }
        let mut amp = a;
        if k >= 3.0 {
            amp *= bass_tilt;
        }
        // Soft high-frequency roll-off from the wooden walls and the stopper.
        amp /= 1.0 + (pf / 3000.0).powi(2);
        amp_sum += amp;
        partials.push((k, amp, boost, rate, depth, 0.7 * i as f64 + 0.3));
    }
    let norm = 0.8 / amp_sum.max(1e-6);

    let mut breath_low = NoiseResonator::new(f0, f0 * 0.35, sr);
    let mut breath_high = if f0 * 3.0 < nyquist * 0.9 {
        Some(NoiseResonator::new(f0 * 3.0, f0 * 0.75, sr))
    } else {
        None
    };

    // Deterministic noise so that the same pitch always renders the same tone.
    let mut rng: u32 = ((f0 * 100.0) as u32) ^ 0x9E37_79B9;
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let speech = (0.6 + 2.0 * chiff).clamp(0.0, 1.6);
    let onset_decay = (-1.0 / (0.070 * sr)).exp();
    let spit_decay = (-1.0 / (0.045 * sr)).exp();
    let edge_decay = (-1.0 / (0.018 * sr)).exp();
    let spit_level = 0.05 + 0.30 * chiff;
    let edge_level = 0.02 + 0.12 * chiff;
    let hiss_level = 0.012;

    let step = f0 / sr;
    let mut phase = 0.0f64;
    let mut onset_env = 1.0f64;
    let mut spit_env = 1.0f64;
    let mut edge_env = 1.0f64;
    let mut prev_white = 0.0f64;

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        let mut tone = 0.0f64;
        for &(k, amp, boost, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            let speak = 1.0 + boost * speech * onset_env;
            tone += amp * speak * drift * sine_lut.sin(two_pi * k * phase) as f64;
        }

        let white = next_noise();
        let mut breath = 0.7 * breath_low.process(white);
        if let Some(res) = breath_high.as_mut() {
            breath += 0.5 * res.process(white);
        }
        let edge = (white - prev_white) * edge_level * edge_env;
        prev_white = white;
        let noise = breath * (hiss_level + spit_level * spit_env) + edge;

        *sample += (norm * tone + noise) as f32;

        phase = (phase + step * (1.0 + wind * 0.002)).fract();
        onset_env *= onset_decay;
        spit_env *= spit_decay;
        edge_env *= edge_decay;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Goertzel-style magnitude of `freq` over `x` (amplitude units).
    fn tone_magnitude(x: &[f32], freq: f64, sample_rate: f64) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (n, &s) in x.iter().enumerate() {
            let a = 2.0 * std::f64::consts::PI * freq * n as f64 / sample_rate;
            re += s as f64 * a.cos();
            im -= s as f64 * a.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    #[test]
    fn test_gedeckt_stopped_pipe_spectrum() {
        // 441 Hz at 44.1 kHz is exactly 100 samples per period, so the analysis window of
        // 10,000 samples holds a whole number of cycles of every harmonic.
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];
        let mut out = vec![0.0f32; 22050];
        render_stop_into(17, 441.0, 44100, &mut out, &sine_lut, &wind, 0.2);

        assert!(out.iter().all(|s| s.is_finite()));
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak < 1.5, "Onset peak must stay bounded, got {peak}");
        let steady_peak = out[11025..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(steady_peak < 1.0, "Steady-state peak must leave headroom, got {steady_peak}");

        let window = &out[11025..21025];
        let m1 = tone_magnitude(window, 441.0, 44100.0);
        let m2 = tone_magnitude(window, 882.0, 44100.0);
        let m3 = tone_magnitude(window, 1323.0, 44100.0);
        let m4 = tone_magnitude(window, 1764.0, 44100.0);
        let m5 = tone_magnitude(window, 2205.0, 44100.0);

        assert!(m1 > 5.0 * m3, "Fundamental must dominate: m1={m1}, m3={m3}");
        assert!(m3 > 0.08 * m1, "Third partial must be audible: m1={m1}, m3={m3}");
        assert!(m3 > 3.0 * m2.max(m4), "Odd partials must dominate even ones: m2={m2}, m3={m3}, m4={m4}");
        assert!(m3 > m5, "Partials must fall away with height: m3={m3}, m5={m5}");
    }

    #[test]
    fn test_gedeckt_16_foot_sounds_an_octave_below_key() {
        // Key frequency 882 Hz; the 16' rank must sound its fundamental at 441 Hz.
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];
        let mut out = vec![0.0f32; 22050];
        render_stop_into(19, 882.0, 44100, &mut out, &sine_lut, &wind, 0.2);

        let window = &out[11025..21025];
        let sounding = tone_magnitude(window, 441.0, 44100.0);
        let at_key = tone_magnitude(window, 882.0, 44100.0);
        assert!(sounding > 15.0 * at_key, "16' fundamental must be at half the key pitch: {sounding} vs {at_key}");
    }

    #[test]
    fn test_gedeckt_all_footages_render_finite_across_compass() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 4410];
        for id in 17..=20 {
            for &f in &[16.35, 65.41, 261.63, 1046.5, 4186.0] {
                let mut out = vec![0.0f32; 4410];
                render_stop_into(id, f, 44100, &mut out, &sine_lut, &wind, 0.2);
                assert!(out.iter().all(|s| s.is_finite() && s.abs() < 2.0), "stop {id} at {f} Hz");
                assert!(out.iter().any(|s| s.abs() > 1e-4), "stop {id} at {f} Hz must not be silent");
            }
        }
    }

    #[test]
    fn test_diapason_8_harmonic_registry_structure() {
        let diapason = &STOP_DEFINITIONS[11];
        assert_eq!(diapason.name, "Diapason 8'");
        assert_eq!(diapason.id, 11);

        match &diapason.model {
            StopSynthesisModel::Wavetable { harmonics, is_celeste, .. } => {
                // Must be an open flue pipe: contains both odd and even harmonics (1, 2, 3, 4, 5, etc.)
                assert!(!is_celeste, "Diapason 8' must be a foundational solo pipe, not a detuned celeste");
                let has_fundamental = harmonics.iter().any(|h| (h.0 - 1.0).abs() < 1e-4);
                let has_octave = harmonics.iter().any(|h| (h.0 - 2.0).abs() < 1e-4);
                let has_twelfth = harmonics.iter().any(|h| (h.0 - 3.0).abs() < 1e-4);
                let has_fifteenth = harmonics.iter().any(|h| (h.0 - 4.0).abs() < 1e-4);

                assert!(has_fundamental, "Diapason 8' requires fundamental partial");
                assert!(has_octave, "Diapason 8' requires 2nd harmonic octave for singing flue warmth");
                assert!(has_twelfth, "Diapason 8' requires 3rd harmonic twelfth");
                assert!(has_fifteenth, "Diapason 8' requires 4th harmonic fifteenth");
                assert!(harmonics.len() >= 8, "Diapason 8' requires rich overtone spectrum up to 8th partial or beyond");
            }
            _ => panic!("Diapason 8' must be a Wavetable model"),
        }
    }

    #[test]
    fn test_diapason_8_rendering_smoothness_and_chiff() {
        let sine_lut = SineLut::new();
        let wind_mod = vec![0.0f32; 1024];
        let mut out_normal = vec![0.0f32; 1024];
        let mut out_with_chiff = vec![0.0f32; 1024];

        render_stop_into(11, 440.0, 44100, &mut out_normal, &sine_lut, &wind_mod, 0.0);
        render_stop_into(11, 440.0, 44100, &mut out_with_chiff, &sine_lut, &wind_mod, 1.0);

        // Chiff adds articulate languid energy in the first 20ms
        let initial_normal_energy: f32 = out_normal[..100].iter().map(|s| s * s).sum();
        let initial_chiff_energy: f32 = out_with_chiff[..100].iter().map(|s| s * s).sum();
        assert!(
            initial_chiff_energy > initial_normal_energy,
            "Chiff must add articulate languid transient speech energy on onset"
        );

        // Must be numerically stable with no NaN or infinities
        for s in out_with_chiff {
            assert!(!s.is_nan() && !s.is_infinite());
        }
    }
}
