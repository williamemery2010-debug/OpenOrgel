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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftFluteVariant {
    HarmonicFlute,
    Hohlflote,
    Melodia,
    FlautoDolce,
    Rohrflote,
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
    /// Wide-scale heavy wooden stopped Bourdon / Subbass (16' & 32') with deep sub-bass gravity
    /// and acoustic resultant reinforcement.
    BourdonFlue {
        pitch_ratio: f64,
        sub_harmonic: f64,
    },
    /// Large-scale open timber pedal/manual bass pipe (Open Wood 16') with thunderous fundamental
    /// and warm open octave body.
    OpenWoodBass { pitch_ratio: f64 },
    /// Soft, velvety 8' flute family (Harmonic Flute, Hohlflöte, Melodia, Flauto Dolce, Rohrflöte).
    SoftFlute {
        variant: SoftFluteVariant,
        pitch_ratio: f64,
    },
    /// Open cylindrical metal flue pipe (Open Diapason / Principal / Montre):
    /// full integer harmonic series with singing octave warmth, acoustic phase dispersion,
    /// warm chest/midrange resonances (~150 Hz & ~340 Hz), anti-nasal scoop (~900 Hz),
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
    /// Cylindrical quarter-wave Clarinet / Cromorne reed pipe:
    /// dominant odd-harmonic chalumeau/clarion spectrum with suppressed even partials,
    /// warm woody bell/bore formants (~680 Hz & ~1280 Hz), steep cylindrical cutoff (~1850 Hz),
    /// dispersed phases, independent partial micro-drift, and reed-aperture air breath.
    ClarinetReed { pitch_ratio: f64 },
    /// Dual-rank undulating Voix Céleste 8' (Salicional 8' + Unda Maris / Céleste 8'):
    /// asymmetric warm open flue-string ranks with organological beat-rate scaling,
    /// decorrelated per-pipe wind modulation, distinct phase dispersion, and soft flue speech.
    VoixCeleste { pitch_ratio: f64 },
    /// Multi-rank Cathedral Principal Chorus (Open Diapason 8' + Octave 4' + Twelfth 2-2/3' + Fifteenth 2'):
    /// four independent open cylindrical metal flue ranks with decorrelated wind modulation,
    /// footage-scaled speech transients, natural inter-rank tuning dispersion, and treble octave breaks.
    PrincipalChorus,
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

// VIOLONE 16' (Warm wooden/spotted-metal 16' string-principal bass with rich cellistic low-midrange):
pub const VOICING_VIOLONE_16: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.88, 0.62, 0.46, 0.34, 0.24, 0.16, 0.11, 0.07, 0.04],
    extend_db_per_oct: -13.0,
    max_harmonics: 18,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(160.0, 1.1, 1.5), (380.0, 1.0, 1.2)],
    rolloff_hz: 2600.0,
    bass_tilt: 0.32,
    speech_cycles: 12.0,
    speech_min_s: 0.022,
    speech_max_s: 0.080,
    upper_lead: 0.22,
    overshoot_harmonic: 2,
    overshoot: 0.18,
    upper_overshoot: 0.10,
    onset_cents: -3.5,
    onset_time_s: 0.026,
    phase_coherence: 0.18,
    breath: 0.010,
    chiff: 0.26,
    chiff_harmonic: 2.5,
    chiff_cycles: 9.0,
    jitter: 0.006,
    flicker: 0.020,
};

// CONTRA TROMBONE / POSAUNE 16' (Majestic full-length wooden/brass pedal & manual reed with deep bass foundation):
pub const VOICING_TROMBONE_16: PipeVoicing = PipeVoicing {
    spectrum: &[1.0, 0.96, 0.82, 0.68, 0.54, 0.42, 0.30, 0.20, 0.13, 0.08],
    extend_db_per_oct: -14.0,
    max_harmonics: 20,
    even_scale: 1.0,
    even_restore_hz: 0.0,
    formants: &[(150.0, 1.2, 1.7), (360.0, 1.1, 1.4)],
    rolloff_hz: 2800.0,
    bass_tilt: 0.42,
    speech_cycles: 12.0,
    speech_min_s: 0.024,
    speech_max_s: 0.090,
    upper_lead: -0.10,
    overshoot_harmonic: 0,
    overshoot: 0.0,
    upper_overshoot: 0.10,
    onset_cents: -5.5,
    onset_time_s: 0.030,
    phase_coherence: 0.72,
    breath: 0.006,
    chiff: 0.16,
    chiff_harmonic: 2.2,
    chiff_cycles: 9.0,
    jitter: 0.007,
    flicker: 0.024,
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
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 3.0, gain: 0.34, detune_cents: 0.0 }, // 19th (1-1/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.30, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.24, detune_cents: 0.0 }, // 26th (2/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 8.0, gain: 0.20, detune_cents: 0.0 }, // 29th (1/2')
];

pub const PLEIN_JEU_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 2.0, gain: 0.40, detune_cents: 0.0 }, // 15th (2')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 3.0, gain: 0.34, detune_cents: 0.0 }, // 19th (1-1/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.30, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.24, detune_cents: 0.0 }, // 26th (2/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 8.0, gain: 0.19, detune_cents: 0.0 }, // 29th (1/2')
];

pub const CYMBALE_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.36, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.28, detune_cents: 0.0 }, // 26th (2/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 8.0, gain: 0.22, detune_cents: 0.0 }, // 29th (1/2')
];

pub const SCHARF_RANKS: &[CompoundRank] = &[
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 3.0, gain: 0.34, detune_cents: 0.0 }, // 19th (1-1/3')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 4.0, gain: 0.30, detune_cents: 0.0 }, // 22nd (1')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 5.0, gain: 0.26, detune_cents: 0.0 }, // 17th (Tierce 1-3/5')
    CompoundRank { voicing: &VOICING_PRINCIPAL, pitch_ratio: 6.0, gain: 0.22, detune_cents: 0.0 }, // 26th (2/3')
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
        model: StopSynthesisModel::ClarinetReed { pitch_ratio: 1.0 },
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
        model: StopSynthesisModel::ClarinetReed { pitch_ratio: 2.0 },
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
        model: StopSynthesisModel::VoixCeleste { pitch_ratio: 1.0 },
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
    // 32: Principal Chorus
    StopDefinition {
        name: "Principal Chorus",
        id: 32,
        model: StopSynthesisModel::PrincipalChorus,
    },
    // 33: Bourdon 32'
    StopDefinition {
        name: "Bourdon 32'",
        id: 33,
        model: StopSynthesisModel::BourdonFlue {
            pitch_ratio: 0.25,
            sub_harmonic: 0.22,
        },
    },
    // 34: Bourdon 16'
    StopDefinition {
        name: "Bourdon 16'",
        id: 34,
        model: StopSynthesisModel::BourdonFlue {
            pitch_ratio: 0.5,
            sub_harmonic: 0.16,
        },
    },
    // 35: Double Diapason 16'
    StopDefinition {
        name: "Double Diapason 16'",
        id: 35,
        model: StopSynthesisModel::OpenDiapason { pitch_ratio: 0.5 },
    },
    // 36: Subbass 16'
    StopDefinition {
        name: "Subbass 16'",
        id: 36,
        model: StopSynthesisModel::BourdonFlue {
            pitch_ratio: 0.5,
            sub_harmonic: 0.28,
        },
    },
    // 37: Open Wood 16'
    StopDefinition {
        name: "Open Wood 16'",
        id: 37,
        model: StopSynthesisModel::OpenWoodBass { pitch_ratio: 0.5 },
    },
    // 38: Violone 16'
    StopDefinition {
        name: "Violone 16'",
        id: 38,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_VIOLONE_16,
            pitch_ratio: 0.5,
            gain: 1.25,
        },
    },
    // 39: Contra Trombone 16'
    StopDefinition {
        name: "Contra Trombone 16'",
        id: 39,
        model: StopSynthesisModel::PhysicalPipe {
            voicing: &VOICING_TROMBONE_16,
            pitch_ratio: 0.5,
            gain: 1.25,
        },
    },
    // 40: Harmonic Flute 8'
    StopDefinition {
        name: "Harmonic Flute 8'",
        id: 40,
        model: StopSynthesisModel::SoftFlute {
            variant: SoftFluteVariant::HarmonicFlute,
            pitch_ratio: 1.0,
        },
    },
    // 41: Hohlflöte 8'
    StopDefinition {
        name: "Hohlflöte 8'",
        id: 41,
        model: StopSynthesisModel::SoftFlute {
            variant: SoftFluteVariant::Hohlflote,
            pitch_ratio: 1.0,
        },
    },
    // 42: Melodia 8'
    StopDefinition {
        name: "Melodia 8'",
        id: 42,
        model: StopSynthesisModel::SoftFlute {
            variant: SoftFluteVariant::Melodia,
            pitch_ratio: 1.0,
        },
    },
    // 43: Flauto Dolce 8'
    StopDefinition {
        name: "Flauto Dolce 8'",
        id: 43,
        model: StopSynthesisModel::SoftFlute {
            variant: SoftFluteVariant::FlautoDolce,
            pitch_ratio: 1.0,
        },
    },
    // 44: Rohrflöte 8'
    StopDefinition {
        name: "Rohrflöte 8'",
        id: 44,
        model: StopSynthesisModel::SoftFlute {
            variant: SoftFluteVariant::Rohrflote,
            pitch_ratio: 1.0,
        },
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

/// Render a compound multi-rank stop (Cornet V, Mixture IV, Plein Jeu, Cymbale, Scharf).
fn render_compound_stop(
    stop_id: i32,
    freq: f64,
    sample_rate: u32,
    ranks: &[CompoundRank],
    out: &mut [f32],
    sine_lut: &SineLut,
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
        // Each speaking rank on the windchest has its own decorrelated aerodynamic wind flutter
        let rank_wind = super::pipe_wind_modulation(
            rank_freq,
            stop_id.wrapping_mul(31).wrapping_add(idx as i32 + 1),
            out.len(),
            sample_rate,
            sine_lut,
        );

        let spec = PipeSpec {
            voicing: rank.voicing,
            freq: rank_freq,
            rms: 0.22 * rank.gain as f64,
            delay_s: 0.0,
            seed: rank_seed,
        };
        pipe::render_pipe(&spec, sample_rate, out, &rank_wind, chiff_scale);
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
            render_compound_stop(stop_id, freq, sample_rate, ranks, out, sine_lut, chiff_scale);
        }
        StopSynthesisModel::ClarinetReed { pitch_ratio } => {
            render_clarinet_pipe(freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::VoixCeleste { pitch_ratio } => {
            render_voix_celeste(stop_id, freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::PrincipalChorus => {
            render_principal_chorus(stop_id, freq, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::OpenDiapason { pitch_ratio } => {
            render_open_diapason(freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::StoppedFlue { pitch_ratio } => {
            render_stopped_flue(freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::BourdonFlue { pitch_ratio, sub_harmonic } => {
            render_bourdon_flue(freq, *pitch_ratio, *sub_harmonic, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::OpenWoodBass { pitch_ratio } => {
            render_open_wood_bass(freq, *pitch_ratio, sample_rate, out, sine_lut, wind_mod, chiff_scale);
        }
        StopSynthesisModel::SoftFlute { variant, pitch_ratio } => {
            render_soft_flute(freq, *pitch_ratio, *variant, sample_rate, out, sine_lut, wind_mod, chiff_scale);
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
    (1.0, 1.000, 0.00, 0.0, 0.28, 0.012), // Fundamental 8' (deep, warm, open foundational body)
    (2.0, 0.620, 1.35, 0.0, 0.42, 0.018), // Octave 4' (strong open-pipe singing warmth)
    (3.0, 0.205, 2.70, 0.0, 0.55, 0.022), // Twelfth 2-2/3' (controlled to eliminate nasal honk)
    (4.0, 0.170, 0.90, 0.0, 0.68, 0.026), // Fifteenth 2' (open pipe clarity and roundness)
    (5.0, 0.052, 3.14, 0.0, 0.79, 0.030), // Tierce 1-3/5' (gentle spotted-metal warmth, non-nasal)
    (6.0, 0.038, 1.80, 0.0, 0.88, 0.034), // Nineteenth 1-1/3' (silvery open sheen)
    (7.0, 0.014, 0.45, 0.0, 0.95, 0.038), // Septième (trace natural overtone)
    (8.0, 0.011, 2.20, 0.0, 1.05, 0.042), // Twenty-second 1'
    (9.0, 0.005, 1.10, 0.0, 1.15, 0.045),
    (10.0, 0.003, 2.85, 0.0, 1.25, 0.045),
];

/// Organological registration balance weight for each stop in multi-stop and Full Organ (Tutti) mixes.
///
/// Weighting the 32'/16' pedal & manual bass ranks (1.50x-1.62x) and 8' unison foundations (1.22x-1.36x)
/// above 4' octaves (0.88x) and 2'/Mixtures (0.60x) preserves authentic cathedral gravitas and warm
/// midrange fullness in any registration.
pub fn stop_registration_weight(stop_id: i32) -> f32 {
    match stop_id {
        // 32' Sub-Bass Ranks (colossal cathedral gravity)
        10 | 20 | 33 => 1.62, // Contrabassoon 32', Gedeckt 32', Bourdon 32'
        // 16' Manual & Pedal Foundation Ranks
        2 | 3 | 4 | 19 | 27 | 34 | 35 | 36 | 37 | 38 | 39 => 1.50,
        // 8' Principal Foundation & Chorus Ranks
        11 | 32 => 1.36, // Diapason 8', Principal Chorus
        0 | 1 | 6 | 16 | 17 | 24 | 25 | 30 | 40 | 41 | 42 | 43 | 44 => 1.22,
        // 4' Octave & Solo Mutation Ranks
        7 | 8 | 9 | 12 | 13 | 18 | 26 | 28 => 0.88,
        // 2' Super-Octaves & High Mixtures (silvery crown without hollowing out the bass)
        5 | 14 | 15 | 21 | 22 | 23 | 29 | 31 => 0.60,
        _ => 1.0,
    }
}

/// Renders an authentic Open Diapason (Principal / Montre 8' & Double Diapason 16') open cylindrical flue pipe.
///
/// Voiced for a wide, open, warm, non-nasal cathedral character with deep low-low bass reinforcement
/// (< 160 Hz), rich open midrange chest body (~320 Hz), an acoustic anti-nasal scoop (~920 Hz),
/// phase-coherent sub-octave windchest resonance, per-harmonic S-curve speech envelopes, and warm
/// low-register air-column breath.
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

    // Gentle Töpfer scaling on upper partials so bass pipes do not become nasal or buzzy
    let bass_tilt = (1.0 + 0.14 * (261.63 / f0).log2()).clamp(0.82, 1.26);
    // Strong low-low bass & tenor fundamental reinforcement so Diapason has deep open authority
    let fund_weight = (1.0 + 0.34 * (320.0 / f0).log2().max(0.0)).clamp(1.0, 1.85);
    // Singing open octave reinforcement in the bass and midrange
    let oct_weight = (1.0 + 0.18 * (320.0 / f0).log2().max(0.0)).clamp(1.0, 1.40);

    // Tuple: (k, amp, phase_off, attack_samples, rate, depth, lfo_phase)
    let mut partials: Vec<(f64, f64, f64, f64, f64, f64, f64)> =
        Vec::with_capacity(DIAPASON_PARTIALS.len());
    let mut base_amp_sum = 0.0;
    for (i, &(k, a, phase_off, _boost, rate, depth)) in DIAPASON_PARTIALS.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.9 {
            continue;
        }
        base_amp_sum += a;

        let mut amp = if (k - 1.0).abs() < 0.1 {
            a * fund_weight
        } else if (k - 2.0).abs() < 0.1 {
            a * oct_weight
        } else {
            a * bass_tilt
        };

        // Deep sub-bass/bass chest warmth (~145 Hz) + rich open midrange body (~330 Hz)
        let d_sub = (pf / 145.0).log2() / 1.25;
        let d_mid = (pf / 330.0).log2() / 1.15;
        // Anti-nasal scoop centered around 920 Hz (eliminates pinched/nasal 700-1200 Hz honk)
        let d_nasal = (pf / 920.0).log2() / 0.72;
        let formant_gain = (1.0 + 0.42 * (-d_sub * d_sub).exp())
            * (1.0 + 0.36 * (-d_mid * d_mid).exp())
            * (1.0 - 0.18 * (-d_nasal * d_nasal).exp());
        amp *= formant_gain;

        // Smooth open metal wall dissipation above 3.3 kHz for round, warm openness
        amp /= 1.0 + (pf / 3300.0).powi(2);

        let attack_s = (0.022 * k.powf(-0.20)).clamp(0.010, 0.026);
        let attack_samples = (attack_s * sr).max(1.0);

        partials.push((
            k,
            amp,
            phase_off,
            attack_samples,
            rate,
            depth,
            0.65 * i as f64 + 0.25,
        ));
    }
    let norm = 0.58 / base_amp_sum.max(1e-6);

    // Phase-locked sub-octave wooden windchest coupling for warm 16'/32' acoustic gravity
    let sub_oct_gain = if f0 >= 42.0 && f0 <= 520.0 {
        0.13 * ((520.0 - f0) / 478.0).clamp(0.2, 1.0)
    } else {
        0.0
    };

    // Physical air-column breath resonators kept in the warm low-mid register (no high nasal whistle)
    let mut breath_low = NoiseResonator::new(f0, (f0 * 0.28).max(10.0), sr);
    let breath_center = (f0 * 1.5).min(460.0);
    let mut breath_mid = if breath_center < nyquist * 0.9 {
        Some(NoiseResonator::new(breath_center, (breath_center * 0.42).max(20.0), sr))
    } else {
        None
    };

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

    let spit_decay = (-1.0 / (0.030 * sr)).exp();
    let edge_decay = (-1.0 / (0.014 * sr)).exp();
    let spit_level = 0.032 + 0.22 * chiff;
    let edge_level = 0.012 + 0.08 * chiff;
    let hiss_level = 0.007;

    let mut onset_cents = -2.0f64;
    let onset_decay = (-1.0 / (0.018 * sr)).exp();
    const CENTS_TO_RATIO: f64 = 0.000_577_622_65;

    let step_main = f0 / sr;
    let mut phase_main = 0.0f64;
    let mut phase_sub = 0.0f64;
    let mut spit_env = 1.0f64;
    let mut edge_env = 1.0f64;
    let mut prev_white = 0.0f64;

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let n_f = i as f64;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        let mut tone = 0.0f64;
        for &(k, amp, phase_off, attack_samples, rate, depth, lfo_phase) in &partials {
            let u = (n_f / attack_samples).min(1.0);
            let env = 0.5 - 0.5 * (std::f64::consts::PI * u).cos();
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            let p_main = two_pi * (k * phase_main) + phase_off;
            tone += env * drift * amp * sine_lut.sin(p_main) as f64;
        }
        if sub_oct_gain > 0.0 {
            let u_sub = (n_f / (0.028 * sr)).min(1.0);
            let env_sub = 0.5 - 0.5 * (std::f64::consts::PI * u_sub).cos();
            tone += env_sub * sub_oct_gain * sine_lut.sin(two_pi * phase_sub + 0.45) as f64;
        }

        let white = next_noise();
        let mut breath = 0.72 * breath_low.process(white);
        if let Some(res) = breath_mid.as_mut() {
            breath += 0.35 * res.process(white);
        }
        let edge = (white - prev_white) * edge_level * edge_env;
        prev_white = white;
        let noise = breath * (hiss_level + spit_level * spit_env) + edge;

        *sample += (norm * tone + noise) as f32;

        let pitch_bend = 1.0 + onset_cents * CENTS_TO_RATIO;
        let delta = step_main * (1.0 + wind) * pitch_bend;
        phase_main = (phase_main + delta).fract();
        phase_sub = (phase_sub + 0.5 * delta).fract();
        onset_cents *= onset_decay;
        spit_env *= spit_decay;
        edge_env *= edge_decay;
    }
}

/// Partial table for wide-scale stopped wooden Bourdon 16' / 32' and Subbass 16' pipes.
/// Tuple: (harmonic_k, amplitude, phase_offset, drift_rate_hz, drift_depth).
pub const BOURDON_PARTIALS: &[(f64, f64, f64, f64, f64)] = &[
    (1.0, 1.000, 0.00, 0.24, 0.012), // Colossal stopped wooden fundamental
    (2.0, 0.065, 1.15, 0.37, 0.022), // Warm wide-mouth octave presence
    (3.0, 0.135, 2.30, 0.46, 0.028), // Dark, round wooden quint (twelfth)
    (4.0, 0.015, 0.65, 0.58, 0.032),
    (5.0, 0.030, 1.85, 0.71, 0.035), // Soft wooden tierce warmth
    (7.0, 0.008, 2.75, 0.89, 0.040),
];

/// Renders a heavy, wide-scale stopped wooden Bourdon 16' / 32' or Subbass 16' pipe.
///
/// Features deep sub-bass fundamental reinforcement, acoustic resultant / sub-harmonic gravity
/// coupling, warm wooden chest resonances (~95 Hz & ~240 Hz), steep wooden wall roll-off (~1100 Hz),
/// and a cushioned wooden wind-foot speech envelope.
pub fn render_bourdon_flue(
    freq: f64,
    pitch_ratio: f64,
    sub_harmonic: f64,
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

    let low_bass_boost = (1.0 + 0.42 * (180.0 / f0.max(16.0)).log2().max(0.0)).clamp(1.0, 2.10);

    let mut partials: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(BOURDON_PARTIALS.len());
    let mut base_sum = 0.0;
    for (i, &(k, base_a, phase_off, rate, depth)) in BOURDON_PARTIALS.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.88 {
            continue;
        }
        base_sum += base_a;
        let mut amp = if (k - 1.0).abs() < 0.1 {
            base_a * low_bass_boost
        } else {
            base_a
        };
        // Deep wooden cavity resonance (~95 Hz & ~240 Hz)
        let d_sub = (pf / 95.0).log2() / 1.3;
        let d_wood = (pf / 240.0).log2() / 1.1;
        amp *= (1.0 + 0.45 * (-d_sub * d_sub).exp()) * (1.0 + 0.28 * (-d_wood * d_wood).exp());
        // Thick wooden pipe wall damping above 1100 Hz
        let r = pf / 1100.0;
        amp /= 1.0 + r * r * r;

        partials.push((k, amp, phase_off, rate, depth, 0.73 * i as f64 + 0.35));
    }
    let norm = 0.68 / base_sum.max(1e-6);

    let mut breath_low = NoiseResonator::new(f0.max(20.0), (f0 * 0.30).max(8.0), sr);
    let mut rng: u32 = ((f0 * 100.0) as u32) ^ 0xB002_D016;
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let spit_decay = (-1.0 / (0.042 * sr)).exp();
    let spit_level = 0.035 + 0.20 * chiff;
    let hiss_level = 0.008;
    let mut spit_env = 1.0f64;

    let step = f0 / sr;
    let mut phase = 0.0f64;
    let mut phase_sub = 0.0f64;
    let attack_len = (0.028 * sr).max(1.0);

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
        let u = (i as f64 / attack_len).min(1.0);
        let env = 0.5 - 0.5 * (std::f64::consts::PI * u).cos();

        let mut tone = 0.0f64;
        for &(k, amp, phase_off, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            tone += amp * env * drift * sine_lut.sin(two_pi * k * phase + phase_off) as f64;
        }
        // Sub-harmonic / resultant bass reinforcement
        if f0 >= 38.0 {
            tone += sub_harmonic * env * sine_lut.sin(two_pi * phase_sub + 0.5) as f64;
        } else {
            // In the 32' sub-bass octave (< 38 Hz), reinforce 1st and 1.5x acoustic resultant
            tone += sub_harmonic * env * (0.70 * sine_lut.sin(two_pi * phase) as f64
                + 0.35 * sine_lut.sin(two_pi * 3.0 * phase_sub + 0.8) as f64);
        }

        let white = next_noise();
        let noise = breath_low.process(white) * (hiss_level + spit_level * spit_env) * env;
        *sample += (norm * tone + noise) as f32;

        let d_phase = step * (1.0 + wind);
        phase = (phase + d_phase).fract();
        phase_sub = (phase_sub + 0.5 * d_phase).fract();
        spit_env *= spit_decay;
    }
}

/// Renders a large-scale English Cathedral Open Wood 16' bass pipe.
pub fn render_open_wood_bass(
    freq: f64,
    pitch_ratio: f64,
    sample_rate: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    const OPEN_WOOD_PARTIALS: &[(f64, f64, f64, f64, f64)] = &[
        (1.0, 1.000, 0.00, 0.25, 0.012), // Thunderous open timber fundamental
        (2.0, 0.600, 1.20, 0.38, 0.018), // Warm open octave body
        (3.0, 0.165, 2.40, 0.51, 0.024), // Round twelfth
        (4.0, 0.125, 0.80, 0.64, 0.028), // Open double-octave
        (5.0, 0.042, 1.95, 0.77, 0.032),
        (6.0, 0.022, 2.85, 0.90, 0.036),
    ];

    let sr = sample_rate as f64;
    let f0 = freq * pitch_ratio;
    if f0 <= 0.0 || out.is_empty() {
        return;
    }
    let nyquist = sr * 0.5;
    let two_pi = 2.0 * std::f64::consts::PI;
    let chiff = chiff_scale.max(0.0) as f64;
    let bass_boost = (1.0 + 0.40 * (200.0 / f0.max(16.0)).log2().max(0.0)).clamp(1.0, 2.0);

    let mut partials = Vec::with_capacity(OPEN_WOOD_PARTIALS.len());
    let mut base_sum = 0.0;
    for (i, &(k, a, phase_off, rate, depth)) in OPEN_WOOD_PARTIALS.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.88 {
            continue;
        }
        base_sum += a;
        let mut amp = if k <= 2.0 { a * bass_boost } else { a };
        let d_chest = (pf / 120.0).log2() / 1.25;
        let d_mid = (pf / 290.0).log2() / 1.15;
        amp *= (1.0 + 0.44 * (-d_chest * d_chest).exp()) * (1.0 + 0.32 * (-d_mid * d_mid).exp());
        amp /= 1.0 + (pf / 1650.0).powi(2);
        partials.push((k, amp, phase_off, rate, depth, 0.67 * i as f64 + 0.2));
    }
    let norm = 0.66 / base_sum.max(1e-6);

    let mut breath_low = NoiseResonator::new(f0.max(20.0), (f0 * 0.28).max(8.0), sr);
    let mut rng: u32 = ((f0 * 100.0) as u32) ^ 0x09E4_800D;
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let spit_decay = (-1.0 / (0.036 * sr)).exp();
    let spit_level = 0.032 + 0.18 * chiff;
    let hiss_level = 0.008;
    let mut spit_env = 1.0f64;
    let step = f0 / sr;
    let mut phase = 0.0f64;
    let attack_len = (0.025 * sr).max(1.0);

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
        let u = (i as f64 / attack_len).min(1.0);
        let env = 0.5 - 0.5 * (std::f64::consts::PI * u).cos();

        let mut tone = 0.0f64;
        for &(k, amp, phase_off, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            tone += amp * env * drift * sine_lut.sin(two_pi * k * phase + phase_off) as f64;
        }
        let white = next_noise();
        let noise = breath_low.process(white) * (hiss_level + spit_level * spit_env) * env;
        *sample += (norm * tone + noise) as f32;

        phase = (phase + step * (1.0 + wind)).fract();
        spit_env *= spit_decay;
    }
}

/// Renders soft, warm, velvety 8' flute stops (Harmonic Flute 8', Hohlflöte 8', Melodia 8',
/// Flauto Dolce 8', and Rohrflöte 8').
pub fn render_soft_flute(
    freq: f64,
    pitch_ratio: f64,
    variant: SoftFluteVariant,
    sample_rate: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    const HARMONIC_FLUTE_SPEC: &[(f64, f64, f64)] = &[
        (1.0, 1.000, 0.00),
        (2.0, 0.520, 1.30), // Singing harmonic node octave
        (3.0, 0.105, 2.50),
        (4.0, 0.065, 0.85),
        (5.0, 0.020, 1.95),
        (6.0, 0.008, 2.90),
    ];
    const HOHLFLOTE_SPEC: &[(f64, f64, f64)] = &[
        (1.0, 1.000, 0.00), // Wide hollow wooden fundamental
        (2.0, 0.320, 1.45),
        (3.0, 0.195, 2.65), // Warm wooden twelfth
        (4.0, 0.050, 0.90),
        (5.0, 0.032, 2.10),
    ];
    const MELODIA_SPEC: &[(f64, f64, f64)] = &[
        (1.0, 1.000, 0.00), // Creamy romantic open wood flute
        (2.0, 0.440, 1.20),
        (3.0, 0.125, 2.40),
        (4.0, 0.075, 0.70),
        (5.0, 0.022, 1.80),
    ];
    const FLAUTO_DOLCE_SPEC: &[(f64, f64, f64)] = &[
        (1.0, 1.000, 0.00), // Whisper-soft inverted conical chamber flute
        (2.0, 0.290, 1.35),
        (3.0, 0.075, 2.55),
        (4.0, 0.035, 0.95),
    ];
    const ROHRFLOTE_SPEC: &[(f64, f64, f64)] = &[
        (1.0, 1.000, 0.00), // Chimney flute fundamental
        (2.0, 0.095, 1.15), // Open chimney even-harmonic trace
        (3.0, 0.165, 2.35), // Stopped twelfth
        (4.0, 0.035, 0.65),
        (5.0, 0.115, 1.85), // Signature sweet chimney 5th partial
        (6.0, 0.012, 2.75),
    ];

    let (spec, formant_hz, cutoff_hz, gain, attack_s) = match variant {
        SoftFluteVariant::HarmonicFlute => (HARMONIC_FLUTE_SPEC, 360.0, 2300.0, 0.56, 0.024),
        SoftFluteVariant::Hohlflote     => (HOHLFLOTE_SPEC,      280.0, 1750.0, 0.58, 0.026),
        SoftFluteVariant::Melodia       => (MELODIA_SPEC,        340.0, 2000.0, 0.56, 0.026),
        SoftFluteVariant::FlautoDolce   => (FLAUTO_DOLCE_SPEC,   310.0, 1650.0, 0.44, 0.030),
        SoftFluteVariant::Rohrflote     => (ROHRFLOTE_SPEC,      330.0, 2100.0, 0.54, 0.024),
    };

    let sr = sample_rate as f64;
    let f0 = freq * pitch_ratio;
    if f0 <= 0.0 || out.is_empty() {
        return;
    }
    let nyquist = sr * 0.5;
    let two_pi = 2.0 * std::f64::consts::PI;
    let chiff = chiff_scale.max(0.0) as f64;
    let fund_warmth = (1.0 + 0.25 * (300.0 / f0).log2().max(0.0)).clamp(1.0, 1.55);

    let mut partials = Vec::with_capacity(spec.len());
    let mut base_sum = 0.0;
    for (i, &(k, a, phase_off)) in spec.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.88 {
            continue;
        }
        base_sum += a;
        let mut amp = if (k - 1.0).abs() < 0.1 { a * fund_warmth } else { a };
        // Warm low-midrange flute body resonance + anti-nasal dip at 920 Hz
        let d_form = (pf / formant_hz).log2() / 1.15;
        let d_nasal = (pf / 920.0).log2() / 0.70;
        amp *= (1.0 + 0.36 * (-d_form * d_form).exp()) * (1.0 - 0.16 * (-d_nasal * d_nasal).exp());
        amp /= 1.0 + (pf / cutoff_hz).powi(2);

        let rate = 0.29 + 0.14 * i as f64;
        let depth = 0.014 + 0.005 * i as f64;
        partials.push((k, amp, phase_off, rate, depth, 0.77 * i as f64 + 0.3));
    }
    let norm = gain / base_sum.max(1e-6);

    let mut breath_low = NoiseResonator::new(f0.max(25.0), (f0 * 0.30).max(10.0), sr);
    let mut rng: u32 = ((f0 * 100.0) as u32) ^ (variant as u32).wrapping_mul(0x7F4A_7C15);
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let spit_decay = (-1.0 / (0.032 * sr)).exp();
    let spit_level = (0.022 + 0.14 * chiff) * gain;
    let hiss_level = 0.007 * gain;
    let mut spit_env = 1.0f64;
    let step = f0 / sr;
    let mut phase = 0.0f64;
    let attack_len = (attack_s * sr).max(1.0);

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };
        let u = (i as f64 / attack_len).min(1.0);
        let env = 0.5 - 0.5 * (std::f64::consts::PI * u).cos();

        let mut tone = 0.0f64;
        for &(k, amp, phase_off, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            tone += amp * env * drift * sine_lut.sin(two_pi * k * phase + phase_off) as f64;
        }

        let white = next_noise();
        let noise = breath_low.process(white) * (hiss_level + spit_level * spit_env) * env;
        *sample += (norm * tone + noise) as f32;

        phase = (phase + step * (1.0 + wind)).fract();
        spit_env *= spit_decay;
    }
}

/// Harmonic partial specification for a cylindrical quarter-wave Clarinet / Cromorne pipe.
/// Tuple: (harmonic_k, base_amplitude, phase_offset_rad, attack_tau_ms, drift_rate_hz, drift_depth).
///
/// A cylindrical pipe closed at the reed end acts as a quarter-wave resonator (L = lambda/4),
/// strongly supporting odd harmonics (1, 3, 5, 7, 9, 11) while cancelling even harmonics (2, 4, 6)
/// to trace bore-end reflections (< 2%). This odd-harmonic hollowness combined with a steep
/// acoustic bell cutoff around 1850 Hz produces the warm, woody chalumeau/clarion clarinet voice
/// and completely prevents sawtooth buzz.
pub const CLARINET_PARTIALS: &[(f64, f64, f64, f64, f64, f64)] = &[
    (1.0,  1.000, 0.00, 18.0, 0.31, 0.014), // Fundamental (deep, round chalumeau body)
    (2.0,  0.018, 1.57, 24.0, 0.47, 0.025), // Faint 2nd-harmonic bore asymmetry (< 2%, non-sawtooth)
    (3.0,  0.640, 2.35, 15.0, 0.43, 0.022), // 3rd harmonic (defining hollow woody twelfth)
    (4.0,  0.009, 0.85, 26.0, 0.61, 0.030), // Trace 4th harmonic
    (5.0,  0.290, 1.15, 20.0, 0.59, 0.028), // 5th harmonic (warm reed roundness)
    (6.0,  0.004, 2.90, 28.0, 0.73, 0.030), // Trace 6th harmonic
    (7.0,  0.115, 2.75, 24.0, 0.79, 0.035), // 7th harmonic (gentle woody reed definition)
    (9.0,  0.042, 0.60, 28.0, 0.97, 0.040), // 9th harmonic
    (11.0, 0.014, 1.95, 32.0, 1.13, 0.045), // 11th harmonic (upper clarion velvet)
];

/// Renders an authentic cylindrical quarter-wave Clarinet / Cromorne organ pipe (8' or 4').
///
/// Models strict odd-harmonic dominance, warm woody chalumeau/bell cavity formants (~680 Hz & ~1280 Hz),
/// steep cylindrical acoustic bell cutoff above 1850 Hz, dispersed partial phases, independent
/// per-partial micro-drift, reed-tongue onset pitch settle, and dual-resonator reed-aperture breath.
pub fn render_clarinet_pipe(
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

    // Bass chalumeau register retains fuller odd overtones; high clarion rounds into pure woody singing tone
    let bass_tilt = (1.0 + 0.18 * (261.63 / f0).log2()).clamp(0.72, 1.38);

    // (k, amp, phase_off, attack_len_samples, rate, depth, lfo_phase)
    let mut partials: Vec<(f64, f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(CLARINET_PARTIALS.len());
    let mut amp_sum = 0.0;
    for (i, &(k, base_a, phase_off, tau_ms, rate, depth)) in CLARINET_PARTIALS.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.88 {
            continue;
        }
        let mut amp = base_a;
        if k >= 3.0 {
            amp *= bass_tilt;
        }

        // Woody chalumeau bore resonance (~680 Hz) and throat/bell cavity resonance (~1280 Hz)
        let d_chalumeau = (pf / 680.0).log2() / 0.85;
        let d_bell = (pf / 1280.0).log2() / 0.75;
        let formant_gain = (1.0 + 0.42 * (-d_chalumeau * d_chalumeau).exp())
            * (1.0 + 0.28 * (-d_bell * d_bell).exp());
        amp *= formant_gain;

        // Steep cylindrical bore cutoff above 1850 Hz eliminates high-frequency sawtooth buzz
        let cutoff_ratio = pf / 1850.0;
        amp /= 1.0 + cutoff_ratio * cutoff_ratio * cutoff_ratio;

        amp_sum += amp;
        let attack_samples = (tau_ms * 0.001 * sr).max(1.0);
        partials.push((k, amp, phase_off, attack_samples, rate, depth, 0.83 * i as f64 + 0.4));
    }
    let norm = 0.84 / amp_sum.max(1e-6);

    // Reed-aperture air breath filtered at cylindrical pipe resonances (f0 and 3*f0)
    let mut breath_f0 = NoiseResonator::new(f0, (f0 * 0.30).max(12.0), sr);
    let mut breath_twelfth = if f0 * 3.0 < nyquist * 0.88 {
        Some(NoiseResonator::new(f0 * 3.0, (f0 * 0.65).max(30.0), sr))
    } else {
        None
    };

    let mut rng: u32 = ((f0 * 100.0) as u32) ^ 0xC1A2_19E7;
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    // Subtle reed-tongue onset articulation and woody 3rd-harmonic bloom
    let mut onset_cents = -3.0f64;
    let onset_decay = (-1.0 / (0.016 * sr)).exp();
    let woody_bloom_decay = (-1.0 / (0.032 * sr)).exp();
    let mut woody_bloom = 0.14 + 0.12 * chiff;

    let spit_decay = (-1.0 / (0.024 * sr)).exp();
    let mut spit_env = 1.0f64;
    let spit_level = 0.022 + 0.14 * chiff;
    let hiss_level = 0.008;

    let step = f0 / sr;
    let mut phase = 0.0f64;
    const CENTS_TO_RATIO: f64 = 0.000_577_622_65;

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        let mut tone = 0.0f64;
        let n_f = i as f64;
        for &(k, amp, phase_off, attack_samples, rate, depth, lfo_phase) in &partials {
            // Smooth S-curve (raised cosine) per-partial speech onset
            let u = (n_f / attack_samples).min(1.0);
            let env = 0.5 - 0.5 * (std::f64::consts::PI * u).cos();
            let bloom = if (k - 3.0).abs() < 0.1 { 1.0 + woody_bloom } else { 1.0 };
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            let partial_phase = two_pi * (k * phase) + phase_off;
            tone += amp * env * bloom * drift * sine_lut.sin(partial_phase) as f64;
        }

        let white = next_noise();
        let mut breath = 0.60 * breath_f0.process(white);
        if let Some(res) = breath_twelfth.as_mut() {
            breath += 0.50 * res.process(white);
        }
        let noise = breath * (hiss_level + spit_level * spit_env);

        *sample += (norm * tone + noise) as f32;

        let pitch_bend = 1.0 + onset_cents * CENTS_TO_RATIO;
        phase = (phase + step * (1.0 + wind) * pitch_bend).fract();
        onset_cents *= onset_decay;
        woody_bloom *= woody_bloom_decay;
        spit_env *= spit_decay;
    }
}

/// Rank I of Voix Céleste 8': Unison Salicional / Dulciana 8' open metal flue pipe.
/// Tuple: (harmonic_k, amplitude, phase_offset_rad, drift_rate_hz, drift_depth).
pub const SALICIONAL_CELESTE_PARTIALS: &[(f64, f64, f64, f64, f64)] = &[
    (1.0, 1.000, 0.00, 0.27, 0.014), // Warm 8' flue-string foundation
    (2.0, 0.620, 1.25, 0.39, 0.020), // Singing 4' octave warmth
    (3.0, 0.310, 2.60, 0.51, 0.025), // Gentle 2-2/3' vocal twelfth
    (4.0, 0.155, 0.75, 0.64, 0.030), // Silky 2' fifteenth
    (5.0, 0.076, 2.95, 0.77, 0.034), // Soft spotted-metal string-flue sheen
    (6.0, 0.038, 1.65, 0.89, 0.038),
    (7.0, 0.018, 0.40, 1.02, 0.042),
    (8.0, 0.008, 2.15, 1.14, 0.045),
    (9.0, 0.004, 1.05, 1.26, 0.045),
];

/// Rank II of Voix Céleste 8': Undulating Unda Maris / Céleste 8' companion pipe.
/// Voiced slightly softer and rounder with distinct harmonic phase dispersion so that
/// beating between Rank I and Rank II produces a warm cathedral shimmer rather than
/// a 100% comb-filter phaser cancellation notch.
pub const UNDA_MARIS_CELESTE_PARTIALS: &[(f64, f64, f64, f64, f64)] = &[
    (1.0, 1.000, 0.95, 0.33, 0.016),
    (2.0, 0.560, 2.45, 0.46, 0.022),
    (3.0, 0.265, 0.55, 0.58, 0.027),
    (4.0, 0.125, 1.90, 0.71, 0.032),
    (5.0, 0.058, 0.30, 0.84, 0.036),
    (6.0, 0.026, 2.75, 0.96, 0.040),
    (7.0, 0.011, 1.40, 1.09, 0.044),
    (8.0, 0.005, 2.90, 1.21, 0.046),
];

/// Renders a single open flue-string pipe for the Voix Céleste ensemble.
fn render_celeste_flue_rank(
    f0: f64,
    sample_rate: u32,
    partials_spec: &[(f64, f64, f64, f64, f64)],
    gain: f64,
    delay_s: f64,
    attack_s: f64,
    seed_salt: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    let sr = sample_rate as f64;
    if f0 <= 0.0 || out.is_empty() {
        return;
    }
    let start_idx = (delay_s.max(0.0) * sr).round() as usize;
    if start_idx >= out.len() {
        return;
    }
    let nyquist = sr * 0.5;
    let two_pi = 2.0 * std::f64::consts::PI;
    let chiff = chiff_scale.max(0.0) as f64;

    let bass_tilt = (1.0 + 0.20 * (261.63 / f0).log2()).clamp(0.75, 1.38);

    let mut partials: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(partials_spec.len());
    let mut amp_sum = 0.0;
    for (i, &(k, base_a, phase_off, rate, depth)) in partials_spec.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.88 {
            continue;
        }
        let mut amp = base_a;
        if k >= 2.0 {
            amp *= bass_tilt;
        }
        // Warm vocal open-flue mouth radiation formant (~780 Hz)
        let d_mouth = (pf / 780.0).log2() / 1.1;
        amp *= 1.0 + 0.28 * (-d_mouth * d_mouth).exp();

        // Smooth open flue wall dissipation above 2800 Hz (eliminates synth sawtooth buzz)
        amp /= 1.0 + (pf / 2800.0).powi(2);

        amp_sum += amp;
        let lfo_phase = (0.71 * i as f64 + (seed_salt & 0xFF) as f64 * 0.09) % two_pi;
        partials.push((k, amp, phase_off, rate, depth, lfo_phase));
    }
    let norm = (0.86 * gain) / amp_sum.max(1e-6);

    // Soft open-flue air-column breath resonators
    let mut breath_low = NoiseResonator::new(f0, (f0 * 0.28).max(10.0), sr);
    let mouth_center = (f0 * 2.0).min(850.0);
    let mut breath_mouth = if mouth_center < nyquist * 0.88 {
        Some(NoiseResonator::new(mouth_center, (mouth_center * 0.40).max(25.0), sr))
    } else {
        None
    };

    let mut rng: u32 = ((f0 * 100.0) as u32) ^ seed_salt;
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let spit_decay = (-1.0 / (0.032 * sr)).exp();
    let mut spit_env = 1.0f64;
    let spit_level = (0.025 + 0.16 * chiff) * gain;
    let hiss_level = 0.006 * gain;

    let step = f0 / sr;
    let mut phase = ((seed_salt as f64) * 0.000_137).fract();
    let attack_len = (attack_s * sr).max(1.0);

    for i in start_idx..out.len() {
        let n = i - start_idx;
        let t = n as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        // Cushioned S-curve flue speech onset
        let u = (n as f64 / attack_len).min(1.0);
        let env = 0.5 - 0.5 * (std::f64::consts::PI * u).cos();

        let mut tone = 0.0f64;
        for &(k, amp, phase_off, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
            let partial_phase = two_pi * (k * phase) + phase_off;
            tone += amp * env * drift * sine_lut.sin(partial_phase) as f64;
        }

        let white = next_noise();
        let mut breath = 0.65 * breath_low.process(white);
        if let Some(res) = breath_mouth.as_mut() {
            breath += 0.40 * res.process(white);
        }
        let noise = breath * (hiss_level + spit_level * spit_env) * env;

        out[i] += (norm * tone + noise) as f32;

        phase = (phase + step * (1.0 + wind)).fract();
        spit_env *= spit_decay;
    }
}

/// Renders an authentic Cavaillé-Coll / Cathedral Voix Céleste 8' stop:
/// pairs a warm unison Salicional 8' flue pipe with a gently undulating Unda Maris / Céleste 8'
/// flue pipe, each with independent windchest breathing, asymmetric voicing, and organological beat scaling.
pub fn render_voix_celeste(
    stop_id: i32,
    freq: f64,
    pitch_ratio: f64,
    sample_rate: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    let f0 = freq * pitch_ratio;
    if f0 <= 0.0 || out.is_empty() {
        return;
    }

    // Organological Celeste beat frequency curve (Dom Bédos / Cavaillé-Coll voicing):
    // Produces a serene ~0.8 Hz undulation in the bass, ~1.15 Hz at Middle C, ~1.5 Hz at A4,
    // and caps around 2.35 Hz in the high treble so it never warbles like a synth chorus LFO.
    let beat_hz = (0.65 + 0.0020 * f0).clamp(0.75, 2.35);
    let f_celeste = f0 + beat_hz;

    // Rank I: Unison Salicional 8' (fuller tonal anchor, uses primary pipe wind modulation)
    render_celeste_flue_rank(
        f0,
        sample_rate,
        SALICIONAL_CELESTE_PARTIALS,
        0.58,
        0.0,
        0.026,
        0x5A11_C10A,
        out,
        sine_lut,
        wind_mod,
        chiff_scale,
    );

    // Rank II: Undulating Céleste 8' (independent windchest pipe modulation & 6.5 ms pallet stagger)
    let celeste_wind = super::pipe_wind_modulation(
        f_celeste,
        stop_id.wrapping_add(101),
        out.len(),
        sample_rate,
        sine_lut,
    );
    render_celeste_flue_rank(
        f_celeste,
        sample_rate,
        UNDA_MARIS_CELESTE_PARTIALS,
        0.44,
        0.0065,
        0.032,
        0xCE1E_57E8,
        out,
        sine_lut,
        &celeste_wind,
        chiff_scale,
    );
}

/// Specification of one open diapason / principal rank inside the Cathedral Principal Chorus:
/// (pitch_ratio, rank_gain, detune_cents, attack_seconds, seed_salt).
pub const PRINCIPAL_CHORUS_RANKS: &[(f64, f64, f64, f64, u32)] = &[
    (1.0, 0.52,  0.0, 0.018, 0x1111_8008), // Open Diapason / Montre 8' (noble foundation)
    (2.0, 0.36,  1.2, 0.014, 0x2222_4004), // Octave / Prestant 4' (singing octave clarity)
    (3.0, 0.20, -1.0, 0.012, 0x3333_2667), // Twelfth / Quinte 2-2/3' (harmonic chorus binding)
    (4.0, 0.25,  1.6, 0.010, 0x4444_2002), // Super Octave / Fifteenth 2' (silvery crown)
];

/// Renders a single Open Diapason / Principal rank with custom tuning dispersion, speech time,
/// and phase seed for the multi-rank Principal Chorus.
fn render_principal_chorus_rank(
    f0: f64,
    sample_rate: u32,
    gain: f64,
    attack_s: f64,
    seed_salt: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    let sr = sample_rate as f64;
    if f0 <= 0.0 || out.is_empty() {
        return;
    }
    let nyquist = sr * 0.5;
    let two_pi = 2.0 * std::f64::consts::PI;
    let chiff = chiff_scale.max(0.0) as f64;

    let bass_tilt = (1.0 + 0.22 * (261.63 / f0).log2()).clamp(0.75, 1.45);
    let phase_rot = ((seed_salt & 0xFFFF) as f64 / 65535.0) * two_pi;

    let mut partials: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(DIAPASON_PARTIALS.len());
    let mut amp_sum = 0.0;
    for (i, &(k, a, phase_off, _boost, rate, depth)) in DIAPASON_PARTIALS.iter().enumerate() {
        let pf = f0 * k;
        if pf >= nyquist * 0.9 {
            continue;
        }
        let mut amp = a;
        if k >= 2.0 {
            amp *= bass_tilt;
        }
        // Warm low-midrange open body (~160 Hz & ~340 Hz) + anti-nasal scoop (~920 Hz)
        let d_sub = (pf / 160.0).log2() / 1.25;
        let d_mid = (pf / 340.0).log2() / 1.15;
        let d_nasal = (pf / 920.0).log2() / 0.72;
        let formant_gain = (1.0 + 0.36 * (-d_sub * d_sub).exp())
            * (1.0 + 0.30 * (-d_mid * d_mid).exp())
            * (1.0 - 0.16 * (-d_nasal * d_nasal).exp());
        amp *= formant_gain;
        amp /= 1.0 + (pf / 3800.0).powi(2);

        amp_sum += amp;
        let lfo_phase = (0.65 * i as f64 + phase_rot) % two_pi;
        partials.push((k, amp, phase_off + phase_rot, rate, depth, lfo_phase));
    }
    let norm = (0.88 * gain) / amp_sum.max(1e-6);

    let mut breath_low = NoiseResonator::new(f0, (f0 * 0.25).max(10.0), sr);
    let breath_center = (f0 * 2.0).min(900.0);
    let mut breath_mid = if breath_center < nyquist * 0.9 {
        Some(NoiseResonator::new(breath_center, (breath_center * 0.40).max(25.0), sr))
    } else {
        None
    };

    let mut rng: u32 = ((f0 * 100.0) as u32) ^ seed_salt;
    if rng == 0 {
        rng = 1;
    }
    let mut next_noise = move || -> f64 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let spit_decay = (-1.0 / (0.028 * sr)).exp();
    let edge_decay = (-1.0 / (0.013 * sr)).exp();
    let spit_level = (0.032 + 0.22 * chiff) * gain;
    let edge_level = (0.012 + 0.08 * chiff) * gain;
    let hiss_level = 0.007 * gain;

    let step = f0 / sr;
    let mut phase = ((seed_salt >> 16) as f64 / 65536.0).fract();
    let mut spit_env = 1.0f64;
    let mut edge_env = 1.0f64;
    let mut prev_white = 0.0f64;
    let tone_attack_len = (attack_s * sr).max(1.0);

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let wind = if i < wind_mod.len() { wind_mod[i] as f64 } else { 0.0 };

        let mut tone = 0.0f64;
        let tone_attack = (i as f64 / tone_attack_len).min(1.0);
        for &(k, amp, phase_off, rate, depth, lfo_phase) in &partials {
            let drift = 1.0 + depth * sine_lut.sin(two_pi * rate * t + lfo_phase) as f64;
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

/// Renders a complete 4-rank Cathedral Principal Chorus (Open Diapason 8' + Octave 4' +
/// Twelfth 2-2/3' + Super Octave 2') with independent wind modulation per rank,
/// footage-scaled speech transients, and classical organ octave break protection.
pub fn render_principal_chorus(
    stop_id: i32,
    freq: f64,
    sample_rate: u32,
    out: &mut [f32],
    sine_lut: &SineLut,
    wind_mod: &[f32],
    chiff_scale: f32,
) {
    if freq <= 0.0 || out.is_empty() {
        return;
    }
    for (idx, &(base_ratio, gain, detune_cents, attack_s, seed_salt)) in PRINCIPAL_CHORUS_RANKS.iter().enumerate() {
        // Classical organ break logic: fold upper chorus ranks down an octave when exceeding 5500 Hz
        let mut ratio = base_ratio;
        while ratio * freq > 5500.0 && ratio > 1.0 {
            ratio *= 0.5;
        }
        let detune_factor = (detune_cents * (std::f64::consts::LN_2 / 1200.0)).exp();
        let rank_freq = freq * ratio * detune_factor;

        if idx == 0 {
            render_principal_chorus_rank(
                rank_freq,
                sample_rate,
                gain,
                attack_s,
                seed_salt,
                out,
                sine_lut,
                wind_mod,
                chiff_scale,
            );
        } else {
            let rank_wind = super::pipe_wind_modulation(
                rank_freq,
                stop_id.wrapping_mul(17).wrapping_add(idx as i32 + 1),
                out.len(),
                sample_rate,
                sine_lut,
            );
            render_principal_chorus_rank(
                rank_freq,
                sample_rate,
                gain,
                attack_s,
                seed_salt,
                out,
                sine_lut,
                &rank_wind,
                chiff_scale,
            );
        }
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

    #[test]
    fn test_clarinet_cylindrical_odd_harmonic_hollow_woodwind_spectrum() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];
        let mut out = vec![0.0f32; 22050];
        // Render Clarinet 8' (Stop ID 1) at 441.0 Hz
        render_stop_into(1, 441.0, 44100, &mut out, &sine_lut, &wind, 0.2);

        assert!(out.iter().all(|s| s.is_finite()));
        let window = &out[11025..21025];
        let m1 = tone_magnitude(window, 441.0, 44100.0);  // 1st harmonic (fundamental)
        let m2 = tone_magnitude(window, 882.0, 44100.0);  // 2nd harmonic (even)
        let m3 = tone_magnitude(window, 1323.0, 44100.0); // 3rd harmonic (odd twelfth)
        let m4 = tone_magnitude(window, 1764.0, 44100.0); // 4th harmonic (even)
        let m5 = tone_magnitude(window, 2205.0, 44100.0); // 5th harmonic (odd tierce)

        // Cylindrical quarter-wave clarinet physics: odd harmonics (m1, m3, m5) strongly dominate
        // over suppressed even harmonics (m2, m4), proving mathematically that it is not a sawtooth wave.
        assert!(m1 > 0.15, "Clarinet fundamental must be strong and woody: m1={m1}");
        assert!(m3 > 0.15 * m1, "Clarinet 3rd harmonic twelfth must be prominent: m1={m1}, m3={m3}");
        assert!(m3 > 10.0 * m2, "Cylindrical bore must suppress 2nd harmonic relative to 3rd (not sawtooth): m3={m3}, m2={m2}");
        assert!(m5 > 5.0 * m4, "Cylindrical bore must suppress 4th harmonic relative to 5th: m5={m5}, m4={m4}");

        // Also verify Clarinet 4' (Stop ID 8) sounds an octave above key pitch
        let mut out4 = vec![0.0f32; 22050];
        render_stop_into(8, 220.5, 44100, &mut out4, &sine_lut, &wind, 0.2);
        let w4 = &out4[11025..21025];
        let m4_f0 = tone_magnitude(w4, 441.0, 44100.0);
        let m4_sub = tone_magnitude(w4, 220.5, 44100.0);
        assert!(m4_f0 > 15.0 * m4_sub, "Clarinet 4' must sound at 2x key frequency: {m4_f0} vs {m4_sub}");
    }

    #[test]
    fn test_voix_celeste_warm_flue_undulation_and_no_saw_buzz() {
        let sine_lut = SineLut::new();
        let n = 44100 * 2; // 2 seconds to observe gentle undulation
        let wind = vec![0.0f32; n];
        let mut out = vec![0.0f32; n];
        // Render Voix Celeste 8' (Stop ID 24) at 441.0 Hz
        render_stop_into(24, 441.0, 44100, &mut out, &sine_lut, &wind, 0.2);

        assert!(out.iter().all(|s| s.is_finite()));
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.15 && peak < 1.5, "Voix Celeste peak out of range: {peak}");

        // Spectral verification: warm open-flue foundation (1st & 2nd harmonics dominate, high buzz suppressed)
        let window = &out[22050..32050];
        let m1 = tone_magnitude(window, 441.0, 44100.0);
        let m2 = tone_magnitude(window, 882.0, 44100.0);
        let m12 = tone_magnitude(window, 441.0 * 12.0, 44100.0);
        assert!(m1 > 0.08, "Voix Celeste 8' fundamental must be warm and present: m1={m1}");
        assert!(m2 > 0.15 * m1, "Voix Celeste 8' octave must sing warmly: m1={m1}, m2={m2}");
        assert!(m1 > 25.0 * m12, "Voix Celeste must not have harsh high-frequency sawtooth buzz: m1={m1}, m12={m12}");

        // Verify asymmetric rank voicing prevents complete 0-amplitude comb-filter cancellation notches:
        // Every 100 ms window after speech onset must retain substantial RMS energy.
        for chunk in out[4410..].chunks_exact(4410) {
            let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / 4410.0).sqrt();
            assert!(rms > 0.04, "Voix Celeste suffered deep comb-filter cancellation notch: rms={rms}");
        }
    }

    #[test]
    fn test_principal_chorus_four_rank_plenum_and_breaks() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];
        let mut out = vec![0.0f32; 22050];

        // Verify Principal Chorus is registered at Stop ID 32
        assert_eq!(STOP_DEFINITIONS[32].name, "Principal Chorus");
        assert_eq!(STOP_DEFINITIONS[32].id, 32);

        // Render Principal Chorus (Stop ID 32) at 220.5 Hz (A3)
        // Speaking ranks: 8' (220.5 Hz), 4' (441.0 Hz), 2-2/3' (661.5 Hz), 2' (882.0 Hz)
        render_stop_into(32, 220.5, 44100, &mut out, &sine_lut, &wind, 0.2);
        assert!(out.iter().all(|s| s.is_finite()));
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.20 && peak < 2.0, "Principal Chorus peak out of bounds: {peak}");

        let window = &out[11025..21025];
        let m_8 = tone_magnitude(window, 220.5, 44100.0);
        let m_4 = tone_magnitude(window, 441.0, 44100.0);
        let m_twelfth = tone_magnitude(window, 661.5, 44100.0);
        let m_fifteenth = tone_magnitude(window, 882.0, 44100.0);

        assert!(m_8 > 0.08, "Principal Chorus 8' foundation rank missing: {m_8}");
        assert!(m_4 > 0.06, "Principal Chorus 4' octave rank missing: {m_4}");
        assert!(m_twelfth > 0.03, "Principal Chorus 2-2/3' twelfth rank missing: {m_twelfth}");
        assert!(m_fifteenth > 0.03, "Principal Chorus 2' fifteenth rank missing: {m_fifteenth}");

        // Verify high-treble octave break protection at 2000 Hz
        let mut high_out = vec![0.0f32; 4410];
        render_stop_into(32, 2000.0, 44100, &mut high_out, &sine_lut, &wind, 0.2);
        assert!(high_out.iter().all(|s| s.is_finite() && s.abs() < 2.0));
    }

    #[test]
    fn test_bourdon_and_16_32_ranks_deep_low_bass_gravity() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];

        // Verify Bourdon 32' (33), Bourdon 16' (34), Double Diapason 16' (35),
        // Subbass 16' (36), Open Wood 16' (37), Violone 16' (38), Contra Trombone 16' (39)
        for &id in &[33, 34, 35, 36, 37, 38, 39] {
            let mut out = vec![0.0f32; 22050];
            render_stop_into(id, 130.81, 44100, &mut out, &sine_lut, &wind, 0.2);
            assert!(out.iter().all(|s| s.is_finite()), "Stop {id} produced non-finite samples");
            let rms = (out[11025..].iter().map(|s| s * s).sum::<f32>() / 11025.0).sqrt();
            assert!(rms > 0.05, "16'/32' stop {id} steady RMS {rms} is too quiet");
        }

        // Verify Bourdon 16' (34) at key 441 Hz sounds its fundamental at 220.5 Hz
        let mut b16 = vec![0.0f32; 22050];
        render_stop_into(34, 441.0, 44100, &mut b16, &sine_lut, &wind, 0.2);
        let win16 = &b16[11025..21025];
        let f_16 = tone_magnitude(win16, 220.5, 44100.0);
        let f_key = tone_magnitude(win16, 441.0, 44100.0);
        assert!(f_16 > 6.0 * f_key, "Bourdon 16' fundamental at 220.5 Hz must dominate: {f_16} vs {f_key}");
    }

    #[test]
    fn test_soft_flute_8_stops_warm_velvety_spectrum() {
        let sine_lut = SineLut::new();
        let wind = vec![0.0f32; 22050];

        // Test Harmonic Flute 8' (40), Hohlflöte 8' (41), Melodia 8' (42),
        // Flauto Dolce 8' (43), and Rohrflöte 8' (44)
        for &id in &[40, 41, 42, 43, 44] {
            let mut out = vec![0.0f32; 22050];
            render_stop_into(id, 441.0, 44100, &mut out, &sine_lut, &wind, 0.2);
            assert!(out.iter().all(|s| s.is_finite()));
            let window = &out[11025..21025];
            let m1 = tone_magnitude(window, 441.0, 44100.0);
            let m2 = tone_magnitude(window, 882.0, 44100.0);
            let m8 = tone_magnitude(window, 441.0 * 8.0, 44100.0);
            assert!(m1 > 0.10, "Soft flute {id} fundamental must be warm and present: m1={m1}");
            assert!(m1 > 1.5 * m2, "Soft flute {id} fundamental must dominate octave: m1={m1}, m2={m2}");
            assert!(m1 > 30.0 * m8, "Soft flute {id} must have silky high rolloff: m1={m1}, m8={m8}");
        }
    }
}

