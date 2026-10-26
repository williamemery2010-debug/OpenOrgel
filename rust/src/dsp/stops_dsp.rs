//! ORGAN STOP ACOUSTIC REGISTRY & DSP RENDERING PIPELINE
//!
//! Maps each console stop to a synthesis model: an additive flue pipe (harmonic
//! table, open diapason or stopped pipe), an FM reed operator, or an acoustic sample.

use std::io::Cursor;
use std::sync::OnceLock;
use hound::WavReader;

use super::fm::{FmOperator, FmVoiceConfig};
use super::pipe::{self, PipeSpec, PipeVoicing};
use super::wavetable::SineLut;

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

#[derive(Clone, Copy, Debug)]
pub struct CompoundRank {
    pub voicing: &'static PipeVoicing,
    pub pitch_ratio: f64,
    pub gain: f32,
    pub detune_cents: f64,
}

#[derive(Clone)]
pub enum StopSynthesisModel {
    /// Flue rank defined by a harmonic table (kept for backward compatibility).
    Wavetable {
        harmonics: &'static [(f64, f64)],
        dampening: f64,
        is_celeste: bool,
    },
    /// Complex acoustic reed pipes: Frequency & Phase Modulation (kept for backward compatibility).
    FmReed(FmVoiceConfig),
    /// Real acoustic recording: linear pitch interpolated loop with seamless crossfade.
    Sampled {
        is_clarion: bool,
        harmonic_factor: f64,
        base_freq: f64,
    },
    /// Stopped (covered) wooden flue pipe: additive odd-partial model with wind noise.
    StoppedFlue { pitch_ratio: f64 },
    /// Open cylindrical metal flue pipe (Open Diapason / Principal / Montre):
    /// full integer harmonic series with singing octave warmth, acoustic phase dispersion,
    /// vocal mouth radiation formant (~850 Hz), articulate languid speech transient ("chirrup"),
    /// independent harmonic micro-drift, and physical air-column breath turbulence.
    OpenDiapason { pitch_ratio: f64 },
    /// Physically modeled organ pipe rank (Chebyshev harmonic synthesis, dynamic speech build-up,
    /// acoustic cavity formants, physical air-column breath turbulence, and articulate speech chiff).
    PhysicalPipe {
        voicing: &'static PipeVoicing,
        pitch_ratio: f64,
        gain: f32,
    },
    /// Multi-rank compound stop (Cornets, Mixtures, Undulating Celestes).
    Compound {
        ranks: &'static [CompoundRank],
    },
}

pub struct StopDefinition {
    pub name: &'static str,
    pub id: i32,
    pub model: StopSynthesisModel,
}

// ---------------------------------------------------------------------------
// PHYSICAL ACOUSTIC PIPE VOICING PRESETS
// ---------------------------------------------------------------------------

// CLARINET / CROMORNE:
// Cylindrical resonator suppresses even harmonics (hollow woody color)
// Mouth/throat formant around 1450 Hz and 2800 Hz, rapid tongue speech
pub const VOICING_CLARINET: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.08, 0.75, 0.05, 0.50, 0.04, 0.32, 0.03, 0.18, 0.02, 0.10],
    extend_db_per_oct: -14.0,
    max_harmonics: 22,
    even_scale: 0.12,        // Characteristic hollow odd-harmonic dominance of cylindrical resonators
    even_restore_hz: 2400.0, // Upper partials recover symmetry above pipe cutoff
    formants: &[(1450.0, 0.8, 1.2), (2800.0, 1.0, 0.6)], // Throat & shallot resonance
    rolloff_hz: 4800.0,
    bass_tilt: 0.20,
    speech_cycles: 8.0,
    speech_min_s: 0.012,
    speech_max_s: 0.060,
    upper_lead: 0.10,
    overshoot_harmonic: 3,   // 3rd harmonic woody bite on attack
    overshoot: 0.25,
    upper_overshoot: 0.0,
    onset_cents: -5.0,       // Reed tongue starts slightly flat and pulls into pitch in 22ms
    onset_time_s: 0.022,
    phase_coherence: 0.72,   // High phase coherence of striking reed tongue
    breath: 0.008,
    chiff: 0.18,
    chiff_harmonic: 3.0,
    chiff_cycles: 8.0,
    jitter: 0.006,
    flicker: 0.025,
};

// BASSOON / FAGOTTO 16':
// Slender conical resonator, dark reedy body with throat resonance at ~480 Hz
pub const VOICING_BASSOON: PipeVoicing = PipeVoicing {
    spectrum: &[0.70, 1.0, 0.85, 0.65, 0.50, 0.40, 0.30, 0.22, 0.16, 0.12, 0.08, 0.05],
    extend_db_per_oct: -15.0,
    max_harmonics: 24,
    even_scale: 1.0,         // Conical pipes resonate all harmonics
    even_restore_hz: 0.0,
    formants: &[(480.0, 0.9, 1.4), (1100.0, 1.2, 0.7)], // Dark throat & boot cavity formant
    rolloff_hz: 3800.0,
    bass_tilt: 0.40,
    speech_cycles: 14.0,
    speech_min_s: 0.030,
    speech_max_s: 0.110,
    upper_lead: -0.15,       // Fundamental develops first, upper buzz blooms
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.10,
    onset_cents: -8.0,
    onset_time_s: 0.038,
    phase_coherence: 0.80,
    breath: 0.006,
    chiff: 0.15,
    chiff_harmonic: 2.5,
    chiff_cycles: 10.0,
    jitter: 0.008,
    flicker: 0.030,
};

// CONTRABASSOON 32':
// Sub-bass 32' pedal reed, thunderous low-frequency drive
pub const VOICING_CONTRABASSOON: PipeVoicing = PipeVoicing {
    spectrum: &[0.85, 1.0, 0.90, 0.75, 0.60, 0.48, 0.38, 0.28, 0.20, 0.14, 0.10],
    extend_db_per_oct: -16.0,
    max_harmonics: 28,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(320.0, 1.1, 1.6), (750.0, 1.3, 0.8)],
    rolloff_hz: 2800.0,
    bass_tilt: 0.50,
    speech_cycles: 16.0,
    speech_min_s: 0.045,
    speech_max_s: 0.140,
    upper_lead: -0.20,
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.12,
    onset_cents: -10.0,
    onset_time_s: 0.050,
    phase_coherence: 0.82,
    breath: 0.005,
    chiff: 0.12,
    chiff_harmonic: 2.0,
    chiff_cycles: 12.0,
    jitter: 0.012,
    flicker: 0.035,
};

// BOMBARDE 16':
// Grand French chorus reed of immense fiery brilliance and power
pub const VOICING_BOMBARDE: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.95, 0.88, 0.80, 0.72, 0.65, 0.58, 0.50, 0.42, 0.36, 0.30, 0.24, 0.18],
    extend_db_per_oct: -10.0,
    max_harmonics: 32,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(850.0, 0.8, 1.3), (1750.0, 0.9, 1.0), (3200.0, 1.2, 0.5)], // Wide bell flare
    rolloff_hz: 6000.0,
    bass_tilt: 0.35,
    speech_cycles: 10.0,
    speech_min_s: 0.020,
    speech_max_s: 0.075,
    upper_lead: 0.05,
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.18,   // Fiery attack bite
    onset_cents: -6.0,
    onset_time_s: 0.028,
    phase_coherence: 0.85,
    breath: 0.007,
    chiff: 0.22,
    chiff_harmonic: 3.5,
    chiff_cycles: 8.0,
    jitter: 0.008,
    flicker: 0.030,
};

// OPHICLEIDE 16':
// Heavy English cathedral solo reed (Willis / Lewis style) with deep authority
pub const VOICING_OPHICLEIDE: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.90, 0.78, 0.68, 0.55, 0.45, 0.35, 0.26, 0.18, 0.12, 0.08],
    extend_db_per_oct: -13.0,
    max_harmonics: 26,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(620.0, 1.0, 1.5), (1350.0, 1.1, 0.9)], // Round foundational power
    rolloff_hz: 4500.0,
    bass_tilt: 0.45,
    speech_cycles: 12.0,
    speech_min_s: 0.025,
    speech_max_s: 0.085,
    upper_lead: 0.0,
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.12,
    onset_cents: -6.0,
    onset_time_s: 0.030,
    phase_coherence: 0.78,
    breath: 0.006,
    chiff: 0.18,
    chiff_harmonic: 3.0,
    chiff_cycles: 9.0,
    jitter: 0.007,
    flicker: 0.025,
};

// COR ANGLAIS 8':
// Double-conical pear-shaped resonator with plaintive, poignant nasal woodwind color
pub const VOICING_COR_ANGLAIS: PipeVoicing = PipeVoicing {
    spectrum: &[0.80, 0.90, 1.0, 0.60, 0.50, 0.35, 0.25, 0.18, 0.12, 0.08],
    extend_db_per_oct: -14.0,
    max_harmonics: 22,
    even_scale: 0.70,
    even_restore_hz: 2000.0,
    formants: &[(600.0, 0.7, 1.4), (1800.0, 0.8, 1.1)], // Pear bell nasal resonance
    rolloff_hz: 4200.0,
    bass_tilt: 0.25,
    speech_cycles: 9.0,
    speech_min_s: 0.015,
    speech_max_s: 0.055,
    upper_lead: 0.10,
    overshoot_harmonic: 3,
    overshoot: 0.20,
    upper_overshoot: 0.08,
    onset_cents: -4.0,
    onset_time_s: 0.024,
    phase_coherence: 0.70,
    breath: 0.008,
    chiff: 0.20,
    chiff_harmonic: 3.0,
    chiff_cycles: 8.0,
    jitter: 0.006,
    flicker: 0.025,
};

// OBOE / HAUTBOIS 8':
// Slender conical solo reed, refined, singing and expressive
pub const VOICING_OBOE: PipeVoicing = PipeVoicing {
    spectrum: &[0.85, 1.0, 0.80, 0.65, 0.48, 0.35, 0.24, 0.16, 0.10, 0.06],
    extend_db_per_oct: -13.0,
    max_harmonics: 22,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(1050.0, 0.75, 1.3), (2300.0, 0.9, 0.8)],
    rolloff_hz: 4800.0,
    bass_tilt: 0.20,
    speech_cycles: 9.0,
    speech_min_s: 0.014,
    speech_max_s: 0.050,
    upper_lead: 0.12,
    overshoot_harmonic: 2,
    overshoot: 0.22,
    upper_overshoot: 0.08,
    onset_cents: -4.0,
    onset_time_s: 0.020,
    phase_coherence: 0.75,
    breath: 0.008,
    chiff: 0.22,
    chiff_harmonic: 3.0,
    chiff_cycles: 8.0,
    jitter: 0.006,
    flicker: 0.022,
};

// VOX HUMANA 8':
// Short capped cylindrical resonator producing distinct vocal formant
pub const VOICING_VOX_HUMANA: PipeVoicing = PipeVoicing {
    spectrum: &[0.70, 0.45, 1.0, 0.35, 0.80, 0.25, 0.50, 0.18, 0.30, 0.10],
    extend_db_per_oct: -12.0,
    max_harmonics: 20,
    even_scale: 0.40,
    even_restore_hz: 2600.0,
    formants: &[(780.0, 0.6, 1.8), (2500.0, 0.8, 1.2)], // Vowel formant
    rolloff_hz: 4000.0,
    bass_tilt: 0.15,
    speech_cycles: 7.0,
    speech_min_s: 0.012,
    speech_max_s: 0.045,
    upper_lead: 0.15,
    overshoot_harmonic: 3,
    overshoot: 0.25,
    upper_overshoot: 0.10,
    onset_cents: -5.0,
    onset_time_s: 0.018,
    phase_coherence: 0.70,
    breath: 0.010,
    chiff: 0.20,
    chiff_harmonic: 3.0,
    chiff_cycles: 7.0,
    jitter: 0.009,
    flicker: 0.028,
};

// VIOL 4':
// Narrow scale string pipe with rich harmonic development and bowing scrape
pub const VOICING_VIOL_4: PipeVoicing = PipeVoicing {
    spectrum: &[
        1.0, 0.85, 0.75, 0.65, 0.55, 0.48, 0.40, 0.34, 0.28, 0.22,
        0.18, 0.14, 0.10, 0.08, 0.06, 0.04,
    ],
    extend_db_per_oct: -10.0,
    max_harmonics: 28,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(2200.0, 1.1, 1.2), (3600.0, 1.3, 0.8)], // Narrow string body resonance
    rolloff_hz: 6500.0,
    bass_tilt: 0.25,
    speech_cycles: 12.0,
    speech_min_s: 0.014,
    speech_max_s: 0.060,
    upper_lead: 0.40,        // Upper harmonics speak first (rosin scrape)
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.24,   // String attack "fizz"
    onset_cents: -4.5,
    onset_time_s: 0.025,
    phase_coherence: 0.15,
    breath: 0.014,           // Audible air-jet breath
    chiff: 0.38,
    chiff_harmonic: 4.0,
    chiff_cycles: 10.0,
    jitter: 0.005,
    flicker: 0.022,
};

// VOIX CELESTE 8':
// Narrow string pipe for undulating celeste chorus
pub const VOICING_CELESTE_8: PipeVoicing = PipeVoicing {
    spectrum: &[
        1.0, 0.82, 0.70, 0.58, 0.48, 0.38, 0.30, 0.24, 0.18, 0.14, 0.10, 0.07,
    ],
    extend_db_per_oct: -11.0,
    max_harmonics: 24,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(2100.0, 1.0, 1.1), (3400.0, 1.2, 0.7)],
    rolloff_hz: 6000.0,
    bass_tilt: 0.25,
    speech_cycles: 13.0,
    speech_min_s: 0.016,
    speech_max_s: 0.065,
    upper_lead: 0.35,
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.20,
    onset_cents: -4.0,
    onset_time_s: 0.028,
    phase_coherence: 0.15,
    breath: 0.012,
    chiff: 0.32,
    chiff_harmonic: 3.8,
    chiff_cycles: 10.0,
    jitter: 0.005,
    flicker: 0.020,
};

// FLUTE 4':
// Open singing wooden/metal flute (Flûte Harmonique / Waldflöte)
pub const VOICING_FLUTE_4: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.38, 0.10, 0.03, 0.01],
    extend_db_per_oct: -18.0,
    max_harmonics: 12,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(800.0, 0.9, 0.4)], // Singing mouth cut-up
    rolloff_hz: 4200.0,
    bass_tilt: 0.18,
    speech_cycles: 10.0,
    speech_min_s: 0.012,
    speech_max_s: 0.045,
    upper_lead: 0.25,
    overshoot_harmonic: 2,   // Octave spit on speech onset
    overshoot: 0.35,
    upper_overshoot: 0.0,
    onset_cents: -3.0,
    onset_time_s: 0.020,
    phase_coherence: 0.10,
    breath: 0.012,
    chiff: 0.40,
    chiff_harmonic: 3.0,
    chiff_cycles: 9.0,
    jitter: 0.005,
    flicker: 0.018,
};

// CRYSTAL FLUTE 4' (Glassy):
// Ethereal, glassy, pure flute with airy speech
pub const VOICING_CRYSTAL_FLUTE_4: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.22, 0.15, 0.02, 0.08, 0.01, 0.03],
    extend_db_per_oct: -16.0,
    max_harmonics: 14,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(2400.0, 0.8, 0.9)], // Glassy sheen formant
    rolloff_hz: 5500.0,
    bass_tilt: 0.15,
    speech_cycles: 9.0,
    speech_min_s: 0.010,
    speech_max_s: 0.040,
    upper_lead: 0.20,
    overshoot_harmonic: 3,   // Twelfth glassy chime
    overshoot: 0.25,
    upper_overshoot: 0.0,
    onset_cents: -2.5,
    onset_time_s: 0.018,
    phase_coherence: 0.05,
    breath: 0.016,           // More airy breath presence
    chiff: 0.48,             // Silvery chiff
    chiff_harmonic: 4.5,
    chiff_cycles: 8.0,
    jitter: 0.005,
    flicker: 0.020,
};

// OTTAVINO 2':
// Sparkling open metal Italian principal / flute 2'
pub const VOICING_OTTAVINO_2: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.45, 0.18, 0.08, 0.03, 0.01],
    extend_db_per_oct: -16.0,
    max_harmonics: 12,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(1200.0, 0.9, 0.4)],
    rolloff_hz: 5200.0,
    bass_tilt: 0.15,
    speech_cycles: 8.0,
    speech_min_s: 0.008,
    speech_max_s: 0.032,
    upper_lead: 0.25,
    overshoot_harmonic: 2,
    overshoot: 0.30,
    upper_overshoot: 0.0,
    onset_cents: -2.0,
    onset_time_s: 0.016,
    phase_coherence: 0.05,
    breath: 0.012,
    chiff: 0.38,
    chiff_harmonic: 3.5,
    chiff_cycles: 8.0,
    jitter: 0.005,
    flicker: 0.018,
};

// PICCOLO 2':
// Sweet French orchestral piccolo
pub const VOICING_PICCOLO_2: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.30, 0.06, 0.02],
    extend_db_per_oct: -20.0,
    max_harmonics: 10,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(1400.0, 1.0, 0.3)],
    rolloff_hz: 5000.0,
    bass_tilt: 0.12,
    speech_cycles: 8.0,
    speech_min_s: 0.008,
    speech_max_s: 0.030,
    upper_lead: 0.20,
    overshoot_harmonic: 2,
    overshoot: 0.25,
    upper_overshoot: 0.0,
    onset_cents: -2.0,
    onset_time_s: 0.015,
    phase_coherence: 0.05,
    breath: 0.014,
    chiff: 0.42,
    chiff_harmonic: 3.0,
    chiff_cycles: 8.0,
    jitter: 0.005,
    flicker: 0.018,
};

// PRINCIPAL / DIAPASON RANK (used in Mixtures and Choruses)
pub const VOICING_PRINCIPAL: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.58, 0.28, 0.16, 0.085, 0.048, 0.024, 0.015, 0.009, 0.005],
    extend_db_per_oct: -14.0,
    max_harmonics: 16,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(850.0, 0.85, 0.35)], // Mouth radiation formant
    rolloff_hz: 4800.0,
    bass_tilt: 0.22,
    speech_cycles: 11.0,
    speech_min_s: 0.012,
    speech_max_s: 0.045,
    upper_lead: 0.28,
    overshoot_harmonic: 2,
    overshoot: 0.25,
    upper_overshoot: 0.0,
    onset_cents: -3.5,
    onset_time_s: 0.020,
    phase_coherence: 0.10,
    breath: 0.010,
    chiff: 0.40,
    chiff_harmonic: 3.2,
    chiff_cycles: 9.0,
    jitter: 0.005,
    flicker: 0.020,
};

// CORNET FLUTE RANK (wide-scale, round, vocal flute)
pub const VOICING_CORNET_FLUTE: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.42, 0.14, 0.05, 0.015],
    extend_db_per_oct: -16.0,
    max_harmonics: 12,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(900.0, 0.8, 0.5)],
    rolloff_hz: 4500.0,
    bass_tilt: 0.20,
    speech_cycles: 10.0,
    speech_min_s: 0.010,
    speech_max_s: 0.040,
    upper_lead: 0.20,
    overshoot_harmonic: 2,
    overshoot: 0.28,
    upper_overshoot: 0.0,
    onset_cents: -2.5,
    onset_time_s: 0.018,
    phase_coherence: 0.08,
    breath: 0.012,
    chiff: 0.35,
    chiff_harmonic: 3.0,
    chiff_cycles: 8.0,
    jitter: 0.005,
    flicker: 0.018,
};

// ---------------------------------------------------------------------------
// COMPOUND STOP RANKS DEFINITIONS
// ---------------------------------------------------------------------------

pub const CORNET_V_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_CORNET_FLUTE, pitch_ratio: 1.0, gain: 0.52, detune_cents: 0.0 }, // 8' Bourdon/Flûte
    CompoundRank { voicing: &VOICING_CORNET_FLUTE, pitch_ratio: 2.0, gain: 0.44, detune_cents: 0.0 }, // 4' Flûte
    CompoundRank { voicing: &VOICING_CORNET_FLUTE, pitch_ratio: 3.0, gain: 0.46, detune_cents: 0.0 }, // 2-2/3' Nazard
    CompoundRank { voicing: &VOICING_CORNET_FLUTE, pitch_ratio: 4.0, gain: 0.38, detune_cents: 0.0 }, // 2' Quarte de Nazard
    CompoundRank { voicing: &VOICING_CORNET_FLUTE, pitch_ratio: 5.0, gain: 0.40, detune_cents: 0.0 }, // 1-3/5' Tierce
];

pub const MIXTURE_IV_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 3.0, gain: 0.42, detune_cents: 0.0 }, // 19th (1-1/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.38, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.32, detune_cents: 0.0 }, // 26th (2/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 8.0, gain: 0.28, detune_cents: 0.0 }, // 29th (1/2')
];

pub const PLEIN_JEU_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 2.0, gain: 0.48, detune_cents: 0.0 }, // 15th (2')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 3.0, gain: 0.42, detune_cents: 0.0 }, // 19th (1-1/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.38, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.32, detune_cents: 0.0 }, // 26th (2/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 8.0, gain: 0.26, detune_cents: 0.0 }, // 29th (1/2')
];

pub const CYMBALE_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.46, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.38, detune_cents: 0.0 }, // 26th (2/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 8.0, gain: 0.32, detune_cents: 0.0 }, // 29th (1/2')
];

pub const SCHARF_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 3.0, gain: 0.42, detune_cents: 0.0 }, // 19th (1-1/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.38, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 5.0, gain: 0.34, detune_cents: 0.0 }, // 17th (Tierce 1-3/5')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.30, detune_cents: 0.0 }, // 26th (2/3')
];

pub const VOIX_CELESTE_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_CELESTE_8, pitch_ratio: 1.0, gain: 0.62, detune_cents: 0.0 },
    CompoundRank { voicing: &VOICING_CELESTE_8, pitch_ratio: 1.0, gain: 0.62, detune_cents: 5.5 }, // Undulating beating rank
];

pub const STOP_DEFINITIONS: &[StopDefinition] = &[
    // 0: Oboe 8'
    StopDefinition {
        name: "Oboe 8'",
        id: 0,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_OBOE,
            pitch_ratio: 1.0,
            gain: 1.0,
        },
    },
    // 1: Clarinet 8'
    StopDefinition {
        name: "Clarinet 8'",
        id: 1,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_CLARINET,
            pitch_ratio: 1.0,
            gain: 1.0,
        },
    },
    // 2: Bassoon 16'
    StopDefinition {
        name: "Bassoon 16'",
        id: 2,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_BASSOON,
            pitch_ratio: 0.5,
            gain: 1.0,
        },
    },
    // 3: Bombarde 16'
    StopDefinition {
        name: "Bombarde 16'",
        id: 3,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_BOMBARDE,
            pitch_ratio: 0.5,
            gain: 1.05,
        },
    },
    // 4: Ophicleide 16'
    StopDefinition {
        name: "Ophicleide 16'",
        id: 4,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_OPHICLEIDE,
            pitch_ratio: 0.5,
            gain: 1.05,
        },
    },
    // 5: Ottavino 2'
    StopDefinition {
        name: "Ottavino 2'",
        id: 5,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_OTTAVINO_2,
            pitch_ratio: 4.0,
            gain: 0.95,
        },
    },
    // 6: Cor Anglais 8'
    StopDefinition {
        name: "Cor Anglais 8'",
        id: 6,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_COR_ANGLAIS,
            pitch_ratio: 1.0,
            gain: 1.0,
        },
    },
    // 7: Flute 4'
    StopDefinition {
        name: "Flute 4'",
        id: 7,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_FLUTE_4,
            pitch_ratio: 2.0,
            gain: 0.95,
        },
    },
    // 8: Clarinet 4'
    StopDefinition {
        name: "Clarinet 4'",
        id: 8,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_CLARINET,
            pitch_ratio: 2.0,
            gain: 0.95,
        },
    },
    // 9: Viol 4'
    StopDefinition {
        name: "Viol 4'",
        id: 9,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_VIOL_4,
            pitch_ratio: 2.0,
            gain: 0.95,
        },
    },
    // 10: Contrabassoon 32'
    StopDefinition {
        name: "Contrabassoon 32'",
        id: 10,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_CONTRABASSOON,
            pitch_ratio: 0.25,
            gain: 1.10,
        },
    },
    // 11: Diapason 8' (Open Diapason / Montre 8' / Principal 8')
    StopDefinition {
        name: "Diapason 8'",
        id: 11,
        model: StopSynthesisModel::OpenDiapason { pitch_ratio: 1.0 },
    },
    // 12: Crystal Flute 4' (Glassy)
    StopDefinition {
        name: "Crystal Flute 4' (Glassy)",
        id: 12,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_CRYSTAL_FLUTE_4,
            pitch_ratio: 2.0,
            gain: 0.92,
        },
    },
    // 13: Cornet V 8'
    StopDefinition {
        name: "Cornet V 8'",
        id: 13,
        model: StopSynthesisModel::Compound {
            ranks: CORNET_V_RANKS,
        },
    },
    // 14: Piccolo 2'
    StopDefinition {
        name: "Piccolo 2'",
        id: 14,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_PICCOLO_2,
            pitch_ratio: 4.0,
            gain: 0.92,
        },
    },
    // 15: Mixture IV
    StopDefinition {
        name: "Mixture IV",
        id: 15,
        model: StopSynthesisModel::Compound {
            ranks: MIXTURE_IV_RANKS,
        },
    },
    // 16: Vox Humana 8'
    StopDefinition {
        name: "Vox Humana 8'",
        id: 16,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_VOX_HUMANA,
            pitch_ratio: 1.0,
            gain: 1.0,
        },
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
        model: StopSynthesisModel::Compound {
            ranks: CYMBALE_RANKS,
        },
    },
    // 22: Plein Jeu Mixture
    StopDefinition {
        name: "Plein Jeu Mixture",
        id: 22,
        model: StopSynthesisModel::Compound {
            ranks: PLEIN_JEU_RANKS,
        },
    },
    // 23: Scharf Mixture
    StopDefinition {
        name: "Scharf Mixture",
        id: 23,
        model: StopSynthesisModel::Compound {
            ranks: SCHARF_RANKS,
        },
    },
    // 24: Voix Celeste 8'
    StopDefinition {
        name: "Voix Celeste 8'",
        id: 24,
        model: StopSynthesisModel::Compound {
            ranks: VOIX_CELESTE_RANKS,
        },
    },
    // 25: Acoustic Flue 8'
    StopDefinition {
        name: "Acoustic Flue 8'",
        id: 25,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 1.0,
            base_freq: 433.326,
        },
    },
    // 26: Clarion 4'
    StopDefinition {
        name: "Clarion 4'",
        id: 26,
        model: StopSynthesisModel::Sampled {
            is_clarion: true,
            harmonic_factor: 2.0,
            base_freq: 444.160,
        },
    },
    // 27: Acoustic Flue 16'
    StopDefinition {
        name: "Acoustic Flue 16'",
        id: 27,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 0.5,
            base_freq: 433.326,
        },
    },
    // 28: Acoustic Flue 4'
    StopDefinition {
        name: "Acoustic Flue 4'",
        id: 28,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 2.0,
            base_freq: 433.326,
        },
    },
    // 29: Acoustic Flue 2'
    StopDefinition {
        name: "Acoustic Flue 2'",
        id: 29,
        model: StopSynthesisModel::Sampled {
            is_clarion: false,
            harmonic_factor: 4.0,
            base_freq: 433.326,
        },
    },
    // 30: Clarion 8'
    StopDefinition {
        name: "Clarion 8'",
        id: 30,
        model: StopSynthesisModel::Sampled {
            is_clarion: true,
            harmonic_factor: 1.0,
            base_freq: 444.160,
        },
    },
    // 31: Gedeckt 2'
    StopDefinition {
        name: "Gedeckt 2'",
        id: 31,
        model: StopSynthesisModel::StoppedFlue { pitch_ratio: 4.0 },
    },
];

/// Render a single physically modeled pipe rank into `out`.
fn render_physical_pipe(
    stop_id: i32,
    freq: f64,
    sample_rate: u32,
    voicing: &'static PipeVoicing,
    pitch_ratio: f64,
    gain: f32,
    out: &mut [f32],
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    let seed = pipe::mix_seed(&[stop_id as u64, (freq * 100.0).round() as u64, 0x85A3_08D3]);
    let spec = PipeSpec {
        voicing,
        freq: freq * pitch_ratio,
        rms: 0.24 * gain as f64,
        delay_s: 0.0,
        seed,
    };
    pipe::render_pipe(&spec, sample_rate, out, wind_mod, chiff_scale);
}

/// Render a compound multi-rank stop (Cornet V, Mixture IV, Plein Jeu, Cymbale, Scharf, Celeste).
fn render_compound_stop(
    stop_id: i32,
    freq: f64,
    sample_rate: u32,
    ranks: &[CompoundRank],
    out: &mut [f32],
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    let base_seed = pipe::mix_seed(&[stop_id as u64, (freq * 100.0).round() as u64]);
    for (idx, rank) in ranks.iter().enumerate() {
        // Classical organ break logic:
        // When pitch exceeds 6500 Hz, break back an octave so it stays within musical singing range
        let mut ratio = rank.pitch_ratio;
        while ratio * freq > 6500.0 && ratio > 1.0 {
            ratio *= 0.5;
        }

        let detune_ratio = (rank.detune_cents * (std::f64::consts::LN_2 / 1200.0)).exp();
        let rank_freq = freq * ratio * detune_ratio;
        let rank_seed = pipe::mix_seed(&[base_seed, idx as u64, 0x9E37_79B9_7F4A_7C15]);

        let spec = PipeSpec {
            voicing: rank.voicing,
            freq: rank_freq,
            rms: 0.22 * rank.gain as f64,
            delay_s: 0.0,
            seed: rank_seed,
        };
        pipe::render_pipe(&spec, sample_rate, out, wind_mod, chiff_scale);
    }
}

/// Renders a real acoustic pipe recording with pitch interpolation, seamless crossfade looping, and wind modulation.
fn render_sampled_pipe(
    is_clarion: bool,
    harmonic_factor: f64,
    base_freq: f64,
    freq: f64,
    sample_rate: u32,
    out: &mut [f32],
    wind_mod: &[f32],
) {
    let samples = get_embedded_samples();
    // Measured loop points:
    // Clarion: f0 = 444.160 Hz, steady region loop 3.61s to 8.35s
    // Stopped Flue: f0 = 433.326 Hz, steady region loop 0.96s to 2.28s
    let (pcm, src_rate, loop_start, loop_end) = if is_clarion {
        (&samples.clarion, samples.clarion_rate, 159264usize, 368107usize)
    } else {
        (&samples.flue, samples.flue_rate, 42336usize, 100715usize)
    };
    if pcm.is_empty() {
        return;
    }

    let rank_freq = freq * harmonic_factor;
    let pitch_ratio = rank_freq / base_freq;
    let base_step = pitch_ratio * (src_rate as f64 / sample_rate as f64);

    let len = pcm.len();
    let l_start = loop_start.min(len.saturating_sub(2048));
    let l_end = loop_end.min(len.saturating_sub(1)).max(l_start + 4096);
    let loop_len = l_end - l_start;
    const XFADE: usize = 1024;
    let fade_in_samples = (0.006 * sample_rate as f64).round() as usize;

    let mut pos = 0.0f64;
    for (n, out_sample) in out.iter_mut().enumerate() {
        let wind = if n < wind_mod.len() { wind_mod[n] as f64 } else { 0.0 };
        let step = base_step * (1.0 + wind);

        let sample_val = if pos < (l_end - XFADE) as f64 {
            let i0 = pos as usize;
            let frac = (pos - i0 as f64) as f32;
            let i1 = (i0 + 1).min(len - 1);
            (1.0 - frac) * pcm[i0] + frac * pcm[i1]
        } else {
            // In or beyond crossfade zone
            let loop_pos = if pos < l_end as f64 {
                pos
            } else {
                let over = pos - l_end as f64;
                l_start as f64 + (over % loop_len as f64)
            };

            // Equal-power crossfade between the end of the loop and the start of the loop
            if loop_pos >= (l_end - XFADE) as f64 {
                let k = (loop_pos - (l_end - XFADE) as f64) / XFADE as f64;
                let theta = k * std::f64::consts::FRAC_PI_2;
                let w_start = theta.sin() as f32;
                let w_end = theta.cos() as f32;

                let i0 = (loop_pos as usize).min(len - 1);
                let i1 = (i0 + 1).min(len - 1);
                let frac_i = (loop_pos - i0 as f64) as f32;
                let s_end = (1.0 - frac_i) * pcm[i0] + frac_i * pcm[i1];

                let start_pos = l_start as f64 + (loop_pos - (l_end - XFADE) as f64);
                let j0 = (start_pos as usize).min(len - 1);
                let j1 = (j0 + 1).min(len - 1);
                let frac_j = (start_pos - j0 as f64) as f32;
                let s_start = (1.0 - frac_j) * pcm[j0] + frac_j * pcm[j1];

                w_end * s_end + w_start * s_start
            } else {
                let i0 = (loop_pos as usize).min(len - 1);
                let i1 = (i0 + 1).min(len - 1);
                let frac = (loop_pos - i0 as f64) as f32;
                (1.0 - frac) * pcm[i0] + frac * pcm[i1]
            }
        };

        let attack = if n < fade_in_samples {
            (n as f32 / fade_in_samples as f32).sin()
        } else {
            1.0
        };

        *out_sample += sample_val * attack;
        pos += step;
    }
}

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
        StopSynthesisModel::PhysicalPipe { voicing, pitch_ratio, gain } => {
            render_physical_pipe(stop_id, freq, sample_rate, voicing, *pitch_ratio, *gain, out, wind_mod, chiff_scale);
        }
        StopSynthesisModel::Compound { ranks } => {
            render_compound_stop(stop_id, freq, sample_rate, ranks, out, wind_mod, chiff_scale);
        }
        StopSynthesisModel::OpenDiapason { pitch_ratio } => {
            render_open_diapason(freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::StoppedFlue { pitch_ratio } => {
            render_stopped_flue(freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::Sampled { is_clarion, harmonic_factor, base_freq } => {
            render_sampled_pipe(*is_clarion, *harmonic_factor, *base_freq, freq, sample_rate, out, wind_mod);
        }
        StopSynthesisModel::Wavetable {
            harmonics,
            is_celeste,
            ..
        } => {
            let seed = 0xA511_E9B3u32 ^ (stop_id as u32).wrapping_mul(0x9E37_79B9);
            if *is_celeste {
                render_additive_flue(freq, harmonics, sample_rate, out, sine_lut, wind_mod, chiff_scale, seed, 0.62);
                render_additive_flue(
                    freq * 1.0035,
                    harmonics,
                    sample_rate,
                    out,
                    sine_lut,
                    wind_mod,
                    chiff_scale,
                    seed ^ 0x5A69_7887,
                    0.62,
                );
            } else {
                render_additive_flue(freq, harmonics, sample_rate, out, sine_lut, wind_mod, chiff_scale, seed, 1.0);
            }
        }
        StopSynthesisModel::FmReed(cfg) => {
            let mut fm = FmOperator::new(*cfg, sample_rate);
            fm.render(freq, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
    }
}


/// Partial table for a stopped (covered) wooden flue pipe.
/// Tuple: (harmonic number, relative amplitude, onset boost, drift rate Hz, drift depth).
///
/// A pipe closed at one end resonates at odd multiples of its fundamental only, so the
/// even partials are present only as faint residue from the mouth and stopper. The 3rd
/// partial (a twelfth above the fundamental) gives the Gedackt its warm hollow color.
/// `onset boost` is set to 0.0 so speech transients are purely band-passed breath noise,
/// completely avoiding artificial tonal beeps or chirps.
const STOPPED_PARTIALS: &[(f64, f64, f64, f64, f64)] = &[
    (1.0, 1.000, 0.0, 0.31, 0.015),
    (2.0, 0.016, 0.0, 0.53, 0.050),
    (3.0, 0.200, 0.0, 0.47, 0.050),
    (4.0, 0.010, 0.0, 0.71, 0.060),
    (5.0, 0.062, 0.0, 0.83, 0.060),
    (6.0, 0.005, 0.0, 0.59, 0.060),
    (7.0, 0.026, 0.0, 1.07, 0.070),
    (9.0, 0.010, 0.0, 0.97, 0.070),
    (11.0, 0.004, 0.0, 1.21, 0.080),
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

/// Renders a living additive flue pipe with harmonic table, dispersed phases, independent
/// partial micro-drift, natural wind pitch modulation, and articulate band-passed noise chiff.
pub fn render_additive_flue(
    freq: f64,
    harmonics: &'static [(f64, f64)],
    sample_rate: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
    seed: u32,
    gain: f32,
) {
    let sr = sample_rate as f64;
    if freq <= 0.0 || out.is_empty() {
        return;
    }
    let nyquist = sr * 0.5;
    let two_pi = 2.0 * std::f64::consts::PI;
    let chiff = chiff_scale.max(0.0) as f64;

    // Töpfer Normalmensur acoustic scaling
    let oct_shift = (freq / 261.63).log2();

    // Build active partials with mouth formant and high-frequency roll-off
    let mut partials: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(harmonics.len());
    let mut amp_sum = 0.0;
    for (i, &(factor, base_amp)) in harmonics.iter().enumerate() {
        let pf = freq * factor;
        if pf >= nyquist * 0.95 {
            continue;
        }
        let scale_damp = if factor > 2.0 {
            (1.0 - 0.08 * oct_shift).clamp(0.65, 1.35)
        } else {
            1.0
        };
        // Pipe mouth radiation formant (~800 Hz)
        let formant_octave_dist = (pf / 800.0).log2();
        let formant_gain = 1.0 + 0.30 * (-formant_octave_dist * formant_octave_dist).exp();
        // Acoustic wall & dissipation damping
        let dissipation = 1.0 / (1.0 + (pf / 5000.0).powi(2));

        let amp = base_amp * scale_damp * formant_gain * dissipation;
        amp_sum += amp;

        // Dispersed initial phase angle so harmonics do not peak synchronously at phase 0
        let phase_off = (factor * 2.39996).sin() * std::f64::consts::PI;
        let drift_rate = 0.35 + 0.30 * ((i as f64 * 1.618).fract());
        let drift_depth = 0.018 + 0.015 * ((i as f64 * 2.718).fract());
        let lfo_phase = ((i as f64 + 1.0) * 0.77).fract() * two_pi;

        partials.push((factor, amp, phase_off, drift_rate, drift_depth, lfo_phase));
    }
    let norm = 0.85 / amp_sum.max(1e-6);

    // Physical breath resonator tuned to pipe mouth speech frequency
    let speech_freq = (freq * 1.8).clamp(300.0, 2600.0);
    let mut breath_filter = NoiseResonator::new(speech_freq, (speech_freq * 0.45).clamp(90.0, 750.0), sr);

    let mut rng: u32 = seed ^ ((freq * 100.0) as u32);
    if rng == 0 { rng = 1; }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let spit_decay = (-1.0 / (0.028 * sr)).exp(); // 28 ms chiff decay
    let edge_decay = (-1.0 / (0.014 * sr)).exp(); // 14 ms lip edge transient
    let mut spit_env = 1.0f64;
    let mut edge_env = 1.0f64;
    let spit_level = 0.035 + 0.22 * chiff;
    let edge_level = 0.012 + 0.08 * chiff;
    let hiss_level = 0.007; // subtle continuous air column breath

    let step = freq / sr;
    let mut phase = 0.0f64;
    let mut prev_white = 0.0f64;
    let tone_attack_len = (0.015 * sr).max(1.0);

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        let mut tone = 0.0f64;
        let tone_attack = (i as f64 / tone_attack_len).min(1.0);
        for &(factor, amp, phase_off, drift_rate, drift_depth, lfo_phase) in &partials {
            let drift = 1.0 + drift_depth * sine_lut.sin(two_pi * drift_rate * t + lfo_phase) as f64;
            let partial_phase = two_pi * (factor * phase) + phase_off;
            tone += amp * tone_attack * drift * sine_lut.sin(partial_phase) as f64;
        }

        let white = next_noise();
        let breath = breath_filter.process(white);
        let edge = (white - prev_white) * edge_level * edge_env;
        prev_white = white;
        let noise = breath * (hiss_level + spit_level * spit_env) + edge;

        *sample += ((norm * tone + noise) as f32) * gain;

        phase = (phase + step * (1.0 + wind)).fract();
        spit_env *= spit_decay;
        edge_env *= edge_decay;
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

    let spit_decay = (-1.0 / (0.045 * sr)).exp();
    let edge_decay = (-1.0 / (0.018 * sr)).exp();
    let spit_level = 0.05 + 0.30 * chiff;
    let edge_level = 0.02 + 0.12 * chiff;
    let hiss_level = 0.012;

    let step = f0 / sr;
    let mut phase = 0.0f64;
    let mut spit_env = 1.0f64;
    let mut edge_env = 1.0f64;
    let mut prev_white = 0.0f64;

    let tone_attack_len = (0.015 * sr).max(1.0);

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        let mut tone = 0.0f64;
        let tone_attack = (i as f64 / tone_attack_len).min(1.0);
        for &(k, amp, _boost, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            tone += amp * tone_attack * drift * sine_lut.sin(two_pi * k * phase) as f64;
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

        // wind_mod is a direct fractional frequency deviation (see dsp::pipe_wind_modulation).
        phase = (phase + step * (1.0 + wind)).fract();
        spit_env *= spit_decay;
        edge_env *= edge_decay;
    }
}

/// Harmonic series specification for an authentic Open Diapason (Principal / Montre 8').
/// Tuple: (harmonic number, relative amplitude, phase_offset_rad, onset_boost, drift_rate_hz, drift_depth).
///
/// An open cylindrical metal flue pipe produces the full integer harmonic series (both odd and even).
/// The 2nd harmonic (octave 4') provides singing open warmth and vocal presence;
/// the 3rd (twelfth 2-2/3') and 4th (fifteenth 2') give foundational pipe clarity;
/// higher partials provide silvery pipe brilliance.
/// Phase dispersion offsets prevent unnatural phase coherence spikes (which cause artificial buzzy synth tones).
pub const DIAPASON_PARTIALS: &[(f64, f64, f64, f64, f64, f64)] = &[
    (1.0, 1.000, 0.00, 0.0, 0.28, 0.012), // Fundamental 8' (full, noble, foundational body)
    (2.0, 0.580, 1.35, 0.0, 0.42, 0.020), // Octave 4' (vital singing open pipe warmth)
    (3.0, 0.280, 2.70, 0.0, 0.55, 0.025), // Twelfth 2-2/3' (vocal pipe presence)
    (4.0, 0.160, 0.90, 0.0, 0.68, 0.030), // Fifteenth 2' (clarity and definition)
    (5.0, 0.085, 3.14, 0.0, 0.79, 0.035), // Tierce 1-3/5' (rich spotted metal warmth)
    (6.0, 0.048, 1.80, 0.0, 0.88, 0.040), // Nineteenth 1-1/3' (silvery brilliance)
    (7.0, 0.024, 0.45, 0.0, 0.95, 0.045), // Septième (mild natural overtone color)
    (8.0, 0.015, 2.20, 0.0, 1.05, 0.050), // Twenty-second 1'
    (9.0, 0.009, 1.10, 0.0, 1.15, 0.050),
    (10.0, 0.005, 2.85, 0.0, 1.25, 0.050),
];

/// Renders an authentic Open Diapason (Principal / Montre 8') open cylindrical flue pipe.
///
/// Models the full integer harmonic series with singing octave warmth, acoustic phase dispersion,
/// vocal mouth radiation formant (~850 Hz), articulate band-passed breath noise speech chiff,
/// independent harmonic micro-drift, and physical air-column breath turbulence.
pub fn render_open_diapason(
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

    // Töpfer Normalmensur acoustic scaling:
    // Bass pipes below Middle C gain foundational round warmth and weight;
    // Treble pipes round off gently into pure, singing cathedral tones.
    let bass_tilt = (1.0 + 0.22 * (261.63 / f0).log2()).clamp(0.75, 1.45);

    // Pre-calculate partials with mouth formant and high-frequency dissipation
    let mut partials: Vec<(f64, f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(DIAPASON_PARTIALS.len());
    let mut amp_sum = 0.0;
    for (i, &(k, a, phase_off, boost, rate, depth)) in DIAPASON_PARTIALS.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.9 {
            continue;
        }
        let mut amp = a;
        if k >= 2.0 {
            amp *= bass_tilt;
        }

        // Acoustic mouth cut-up radiation formant (~850 Hz peak, typical of open diapasons)
        let formant_octave_dist = (pf / 850.0).log2();
        let formant_gain = 1.0 + 0.35 * (-formant_octave_dist * formant_octave_dist * 0.75).exp();
        amp *= formant_gain;

        // Open metal pipe acoustic dissipation above 4.5 kHz
        amp /= 1.0 + (pf / 4500.0).powi(2);

        amp_sum += amp;
        partials.push((k, amp, phase_off, boost, rate, depth, 0.65 * i as f64 + 0.25));
    }
    let norm = 0.88 / amp_sum.max(1e-6);

    // Physical air-column breath resonators:
    // Low resonator at pipe fundamental f0 (bandwidth 0.25 f0)
    let mut breath_low = NoiseResonator::new(f0, f0 * 0.25, sr);
    // Upper vocal resonance at octave 2*f0 or cut-up formant
    let breath_center = (f0 * 2.0).min(900.0);
    let mut breath_mid = if breath_center < nyquist * 0.9 {
        Some(NoiseResonator::new(breath_center, breath_center * 0.40, sr))
    } else {
        None
    };

    // Deterministic PRNG keyed to pitch for repeatable, artifact-free synthesis
    let mut rng: u32 = ((f0 * 100.0) as u32) ^ 0x55AA_A55A;
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    // Speech dynamics: genuine band-passed breath noise and labium edge burst
    let spit_decay = (-1.0 / (0.030 * sr)).exp();  // 30 ms languid vortex
    let edge_decay = (-1.0 / (0.014 * sr)).exp();  // 14 ms high-frequency lip edge
    let spit_level = 0.04 + 0.28 * chiff;
    let edge_level = 0.015 + 0.10 * chiff;
    let hiss_level = 0.009; // quiet, warm continuous air column presence

    let step = f0 / sr;
    let mut phase = 0.0f64;
    let mut spit_env = 1.0f64;
    let mut edge_env = 1.0f64;
    let mut prev_white = 0.0f64;
    let tone_attack_len = (0.018 * sr).max(1.0);

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        let mut tone = 0.0f64;
        let tone_attack = (i as f64 / tone_attack_len).min(1.0);
        for &(k, amp, phase_off, _boost, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            // Phase dispersion + partial oscillation
            let partial_phase = two_pi * (k * phase) + phase_off;
            tone += amp * tone_attack * drift * sine_lut.sin(partial_phase) as f64;
        }

        let white = next_noise();
        let mut breath = 0.65 * breath_low.process(white);
        if let Some(res) = breath_mid.as_mut() {
            breath += 0.45 * res.process(white);
        }
        let edge = (white - prev_white) * edge_level * edge_env;
        prev_white = white;
        let noise = breath * (hiss_level + spit_level * spit_env) + edge;

        *sample += (norm * tone + noise) as f32;

        phase = (phase + step * (1.0 + wind)).fract();
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
        for &id in &[17, 18, 19, 20, 31] {
            for &f in &[16.35, 65.41, 261.63, 1046.5, 4186.0] {
                let mut out = vec![0.0f32; 4410];
                render_stop_into(id, f, 44100, &mut out, &sine_lut, &wind, 0.2);
                assert!(out.iter().all(|s| s.is_finite() && s.abs() < 2.0), "stop {id} at {f} Hz");
                assert!(out.iter().any(|s| s.abs() > 1e-4), "stop {id} at {f} Hz must not be silent");
            }
        }
    }

    #[test]
    fn test_gedeckt_2_foot_sounds_two_octaves_above_key() {
        // Key frequency 441 Hz; the 2' rank must sound its fundamental two octaves up at 1764 Hz.
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];
        let mut out = vec![0.0f32; 22050];
        render_stop_into(31, 441.0, 44100, &mut out, &sine_lut, &wind, 0.2);

        let window = &out[11025..21025];
        let sounding = tone_magnitude(window, 1764.0, 44100.0);
        let at_key = tone_magnitude(window, 441.0, 44100.0);
        assert!(sounding > 15.0 * at_key, "2' fundamental must be at 4x key pitch: {sounding} vs {at_key}");
    }

    #[test]
    fn test_diapason_8_harmonic_registry_structure() {
        let diapason = &STOP_DEFINITIONS[11];
        assert_eq!(diapason.name, "Diapason 8'");
        assert_eq!(diapason.id, 11);

        match &diapason.model {
            StopSynthesisModel::OpenDiapason { pitch_ratio } => {
                assert!((pitch_ratio - 1.0).abs() < 1e-4, "Diapason 8' must have 1.0 pitch ratio");
                let has_fundamental = DIAPASON_PARTIALS.iter().any(|h| (h.0 - 1.0).abs() < 1e-4);
                let has_octave = DIAPASON_PARTIALS.iter().any(|h| (h.0 - 2.0).abs() < 1e-4);
                let has_twelfth = DIAPASON_PARTIALS.iter().any(|h| (h.0 - 3.0).abs() < 1e-4);
                let has_fifteenth = DIAPASON_PARTIALS.iter().any(|h| (h.0 - 4.0).abs() < 1e-4);

                assert!(has_fundamental, "Diapason 8' requires fundamental partial");
                assert!(has_octave, "Diapason 8' requires 2nd harmonic octave for singing flue warmth");
                assert!(has_twelfth, "Diapason 8' requires 3rd harmonic twelfth");
                assert!(has_fifteenth, "Diapason 8' requires 4th harmonic fifteenth");
                assert!(DIAPASON_PARTIALS.len() >= 8, "Diapason 8' requires rich overtone spectrum up to 8th partial or beyond");
            }
            _ => panic!("Diapason 8' must be an OpenDiapason model"),
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

    #[test]
    fn test_diapason_8_spectrum_and_singing_octave() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];
        let mut out = vec![0.0f32; 22050];
        render_stop_into(11, 441.0, 44100, &mut out, &sine_lut, &wind, 0.2);

        assert!(out.iter().all(|s| s.is_finite()));
        let steady_peak = out[11025..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(steady_peak < 1.0, "Diapason steady-state peak must leave headroom, got {steady_peak}");

        let window = &out[11025..21025];
        let m1 = tone_magnitude(window, 441.0, 44100.0); // Fundamental
        let m2 = tone_magnitude(window, 882.0, 44100.0); // Octave 4'
        let m3 = tone_magnitude(window, 1323.0, 44100.0); // Twelfth 2-2/3'

        assert!(m1 > 1.2 * m2, "Fundamental must dominate over octave: m1={m1}, m2={m2}");
        assert!(m2 > 0.30 * m1, "Octave must be singing and prominent for authentic open flue: m1={m1}, m2={m2}");
        assert!(m3 > 0.10 * m1, "Twelfth must provide vocal body: m1={m1}, m3={m3}");
    }

    #[test]
    fn test_physical_reed_stability_and_resonator_formants() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];

        // Test Oboe 8' (0), Clarinet 8' (1), Bassoon 16' (2), Bombarde 16' (3),
        // Ophicleide 16' (4), Cor Anglais 8' (6), Contrabassoon 32' (10), Vox Humana 8' (16)
        let reed_ids = [0, 1, 2, 3, 4, 6, 10, 16];
        for &id in &reed_ids {
            let mut out = vec![0.0f32; 22050];
            let test_freq = if id == 10 { 65.41 } else if id == 2 || id == 3 || id == 4 { 110.0 } else { 220.0 };
            render_stop_into(id, test_freq, 44100, &mut out, &sine_lut, &wind, 0.2);

            assert!(out.iter().all(|s| s.is_finite()), "Stop {id} produced non-finite samples");
            let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak < 2.5, "Stop {id} peak {peak} exceeded bounded dynamic range");

            let steady_rms = (out[11025..].iter().map(|s| s * s).sum::<f32>() / 11025.0).sqrt();
            assert!(steady_rms > 0.02, "Stop {id} steady RMS {steady_rms} is too quiet");
        }

        // Clarinet 8' (id 1) has cylindrical resonator physics: strong 1st and 3rd harmonics
        let mut cl_out = vec![0.0f32; 22050];
        render_stop_into(1, 441.0, 44100, &mut cl_out, &sine_lut, &wind, 0.0);
        let window = &cl_out[11025..21025];
        let m1 = tone_magnitude(window, 441.0, 44100.0);
        let m3 = tone_magnitude(window, 1323.0, 44100.0);
        assert!(m1 > 0.05, "Clarinet fundamental must be present: {m1}");
        assert!(m3 > 0.01, "Clarinet 3rd harmonic must be present: {m3}");
    }

    #[test]
    fn test_compound_chorus_mixture_breaks_and_multi_ranks() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];

        // 1. Cornet V 8' (stop 13) has 5 ranks: 8', 4', 2-2/3', 2', 1-3/5'
        let mut cornet_out = vec![0.0f32; 22050];
        // At key freq 220.5 Hz: 220.5, 441.0, 661.5, 882.0, 1102.5 Hz
        render_stop_into(13, 220.5, 44100, &mut cornet_out, &sine_lut, &wind, 0.2);
        assert!(cornet_out.iter().all(|s| s.is_finite()));

        let window = &cornet_out[11025..21025];
        let m_8 = tone_magnitude(window, 220.5, 44100.0);
        let m_4 = tone_magnitude(window, 441.0, 44100.0);
        let m_twelfth = tone_magnitude(window, 661.5, 44100.0);
        let m_fifteenth = tone_magnitude(window, 882.0, 44100.0);
        let m_tierce = tone_magnitude(window, 1102.5, 44100.0);

        assert!(m_8 > 0.01, "Cornet 8' rank missing: {m_8}");
        assert!(m_4 > 0.01, "Cornet 4' rank missing: {m_4}");
        assert!(m_twelfth > 0.01, "Cornet 2-2/3' tierce rank missing: {m_twelfth}");
        assert!(m_fifteenth > 0.01, "Cornet 2' rank missing: {m_fifteenth}");
        assert!(m_tierce > 0.01, "Cornet 1-3/5' tierce rank missing: {m_tierce}");

        // 2. Mixture IV (stop 15) breaks test at treble notes
        // At high key pitch (e.g. 1500 Hz), ranks above 6500 Hz must break down by octaves
        let mut mix_out = vec![0.0f32; 4410];
        render_stop_into(15, 1500.0, 44100, &mut mix_out, &sine_lut, &wind, 0.2);
        assert!(mix_out.iter().all(|s| s.is_finite()));
        let peak = mix_out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.05 && peak < 2.5, "Mixture IV treble peak out of bounds: {peak}");
    }

    #[test]
    fn test_sampled_stops_clarion_and_flue_pitch_and_loop_continuity() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 400_000];

        // 1. Clarion 8' (stop 30) pitch accuracy and long playback loop
        let mut clarion_out = vec![0.0f32; 400_000];
        // Render at base fundamental 444.16 Hz
        render_stop_into(30, 444.16, 44100, &mut clarion_out, &sine_lut, &wind, 0.2);

        assert!(clarion_out.iter().all(|s| s.is_finite()));
        // Check energy across steady-state zone and loop crossing (around sample 368,000)
        let mid_rms = (clarion_out[100_000..120_000].iter().map(|s| s * s).sum::<f32>() / 20_000.0).sqrt();
        let post_loop_rms = (clarion_out[375_000..395_000].iter().map(|s| s * s).sum::<f32>() / 20_000.0).sqrt();
        assert!(mid_rms > 0.05, "Clarion mid playback RMS too quiet: {mid_rms}");
        assert!(post_loop_rms > 0.05, "Clarion post-loop playback RMS too quiet: {post_loop_rms}");

        // Check continuity across crossfade zone: no sudden discontinuous step
        let max_delta = clarion_out[365_000..372_000]
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, |m, d| m.max(d));
        assert!(max_delta < 0.25, "Clarion loop crossfade has step discontinuity: {max_delta}");

        // 2. Stopped Flue 8' (stop 25)
        let mut flue_out = vec![0.0f32; 120_000];
        render_stop_into(25, 433.326, 44100, &mut flue_out, &sine_lut, &wind, 0.2);
        assert!(flue_out.iter().all(|s| s.is_finite()));
        let flue_rms = (flue_out[100_000..115_000].iter().map(|s| s * s).sum::<f32>() / 15_000.0).sqrt();
        assert!(flue_rms > 0.05, "Flue post-loop playback RMS too quiet: {flue_rms}");
    }
}

