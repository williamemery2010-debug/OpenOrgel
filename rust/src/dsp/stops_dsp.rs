//! ORGAN STOP ACOUSTIC REGISTRY & DSP RENDERING PIPELINE
//!
//! Maps each of the 30 console stops to either a high-speed Wavetable profile,
//! an FM Reed Operator, or a high-fidelity Acoustic Sample.

use std::io::Cursor;
use std::sync::OnceLock;
use hound::WavReader;

use super::fm::{FmOperator, FmVoiceConfig};
use super::wavetable::{SineLut, Wavetable};

static CLARION_WAV_BYTES: &[u8] = include_bytes!("../../../clarion.wav");
static FLUE_WAV_BYTES: &[u8] = include_bytes!("../../../stoppedflue.wav");

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
    // 11: Diapason 8'
    StopDefinition {
        name: "Diapason 8'",
        id: 11,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(1.0, 1.0), (3.0, 0.35), (5.0, 0.05)],
            dampening: 0.08,
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
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(1.0, 1.0), (3.0, 0.5), (5.0, 0.2), (7.0, 0.08), (9.0, 0.03), (11.0, 0.01)],
            dampening: 0.08,
            is_celeste: false,
        },
    },
    // 18: Gedeckt 4'
    StopDefinition {
        name: "Gedeckt 4'",
        id: 18,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(2.0, 1.0), (6.0, 0.5), (10.0, 0.2), (14.0, 0.08), (18.0, 0.03), (22.0, 0.01)],
            dampening: 0.02,
            is_celeste: false,
        },
    },
    // 19: Gedeckt 16'
    StopDefinition {
        name: "Gedeckt 16'",
        id: 19,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(0.5, 1.0), (1.5, 0.5), (2.5, 0.2), (3.5, 0.08), (4.5, 0.03), (5.5, 0.01)],
            dampening: 0.08,
            is_celeste: false,
        },
    },
    // 20: Gedeckt 32'
    StopDefinition {
        name: "Gedeckt 32'",
        id: 20,
        model: StopSynthesisModel::Wavetable {
            harmonics: &[(0.25, 1.0), (0.75, 0.5), (1.25, 0.2), (1.75, 0.08), (2.25, 0.03), (2.75, 0.01)],
            dampening: 0.08,
            is_celeste: false,
        },
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

            // Detuned pipe chorus: center (1.0), detune high (+0.15%), detune low (-0.15%)
            let chorus_rates = [1.0, 1.0015, 0.9985];
            let chorus_gains = [1.0f32, 0.35f32, 0.35f32];

            let sample_rate_f = sample_rate as f64;
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
        }
        StopSynthesisModel::FmReed(cfg) => {
            let mut fm = FmOperator::new(*cfg, sample_rate);
            fm.render(freq, sample_rate, out, sine_lut, wind_mod, chiff_scale);
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
