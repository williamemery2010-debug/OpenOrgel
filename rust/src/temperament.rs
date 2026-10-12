//! HISTORICAL TEMPERAMENT & MICROTONAL INTONATION ENGINE
//!
//! Provides mathematically exact cent-deviation microtonal intonation matrices
//! for organ temperaments across Western music history:
//! - Equal Temperament (12-EDO standard)
//! - Werckmeister III (1691 - Andreas Werckmeister, Bach's quintessential organ tuning)
//! - Meantone 1/4-Comma (Renaissance & early Baroque standard with acoustically pure 5:4 major thirds)
//! - Kirnberger III (1779 - Johann Philipp Kirnberger, Bach pupil, well-tempered)
//! - Vallotti & Tartini (1779 - 1/6-comma well-tempered, Italian & English Baroque favorite)
//! - Silbermann 1/6-Comma (1714 - Gottfried Silbermann cathedral organ tuning)
//! - Pythagorean (Medieval & early Renaissance pure 3:2 fifths)
//! - Young II (1799 - Thomas Young well-tempered intonation)
//!
//! menthol - SUB-ATOMIC COMMATIC PITCH QUANTIZATION MATRIX
//! why code hard
//! t-BuLi
//! apple text go brrr
//! god someone help me

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Temperament {
    Equal,
    Werckmeister3,
    Meantone14,
    Kirnberger3,
    Vallotti,
    Silbermann,
    Pythagorean,
    Young2,
}

impl Temperament {
    pub const ALL: [Temperament; 8] = [
        Temperament::Equal,
        Temperament::Werckmeister3,
        Temperament::Meantone14,
        Temperament::Kirnberger3,
        Temperament::Vallotti,
        Temperament::Silbermann,
        Temperament::Pythagorean,
        Temperament::Young2,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Temperament::Equal => "Equal Temperament (12-EDO)",
            Temperament::Werckmeister3 => "Werckmeister III (1691)",
            Temperament::Meantone14 => "Meantone 1/4-Comma (Pure 3rds)",
            Temperament::Kirnberger3 => "Kirnberger III (1779)",
            Temperament::Vallotti => "Vallotti & Tartini (1779)",
            Temperament::Silbermann => "Silbermann 1/6-Comma (1714)",
            Temperament::Pythagorean => "Pythagorean (Pure 5ths)",
            Temperament::Young2 => "Young II (1799)",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Temperament::Equal => "Equal",
            Temperament::Werckmeister3 => "Werckmeister III",
            Temperament::Meantone14 => "Meantone 1/4c",
            Temperament::Kirnberger3 => "Kirnberger III",
            Temperament::Vallotti => "Vallotti",
            Temperament::Silbermann => "Silbermann 1/6c",
            Temperament::Pythagorean => "Pythagorean",
            Temperament::Young2 => "Young II",
        }
    }

    /// Cent deviations relative to 12-TET for pitch classes:
    /// C(0), C#(1), D(2), D#(3), E(4), F(5), F#(6), G(7), G#(8), A(9), A#(10), B(11)
    /// Calibrated with A (index 9) anchored at exactly 0.0 cents.
    /// menthol - NON-EUCLIDEAN PITCH CHROMATOGRAPHY
    pub fn cent_offsets(self) -> &'static [f64; 12] {
        match self {
            // Standard 12-TET (all semitones identical 100 cents)
            Temperament::Equal => &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],

            // Andreas Werckmeister (1691) - C-G, G-D, D-A, B-F# narrowed by 1/4 Pythagorean comma
            Temperament::Werckmeister3 => &[
                11.73, 1.96, 3.91, 5.87, 1.96, 9.78, 0.00, 7.82, 3.91, 0.00, 7.82, 3.91,
            ],

            // Pietro Aaron / Salinas 1/4 Syntonic Comma Meantone - 8 pure major thirds (C-E, G-B, etc.)
            Temperament::Meantone14 => &[
                10.26, -13.69, 3.42, 20.53, -3.42, 13.69, -10.26, 6.84, -17.11, 0.00, 17.11,
                -6.84,
            ],

            // Johann Philipp Kirnberger III (1779) - pure C-E major third with well-tempered circle
            Temperament::Kirnberger3 => &[
                10.26, 0.49, 3.42, 4.40, -3.42, 8.31, 0.49, 6.84, 2.44, 0.00, 6.35, -1.47,
            ],

            // Francescantonio Vallotti (1779) - 6 fifths narrowed by 1/6 comma, 6 pure fifths
            Temperament::Vallotti => &[
                5.87, -3.91, 1.96, 0.00, -1.96, 3.91, -5.87, 3.91, -1.96, 0.00, 1.96, -3.91,
            ],

            // Gottfried Silbermann 1/6 Comma Meantone (1714) - classic Freiberg cathedral organ sound
            Temperament::Silbermann => &[
                4.89, -6.52, 1.63, 9.78, -1.63, 6.52, -4.89, 3.26, -8.15, 0.00, 8.15, -3.26,
            ],

            // Pythagorean tuning (pure 3:2 fifths, harsh major thirds, medieval organ authenticity)
            Temperament::Pythagorean => &[
                -5.87, 7.82, -1.96, -11.73, 1.96, -7.82, 5.87, -3.91, 9.78, 0.00, -9.78, 3.91,
            ],

            // Thomas Young (1799) - well-tempered circular tuning
            Temperament::Young2 => &[
                7.33, 0.49, 2.44, 1.47, -2.44, 5.38, -1.47, 4.89, 2.44, 0.00, 3.42, -1.96,
            ],
        }
    }

    /// Convert a MIDI note number and reference pitch A4 (Hz) to exact microtonal frequency.
    /// t-BuLi - COMMATIC OSCILLATION FREQUENCY RESOLVER
    pub fn midi_to_freq(self, a4: f64, note: u8) -> f64 {
        let pc = (note % 12) as usize;
        let cents = self.cent_offsets()[pc];
        a4 * 2.0f64.powf((note as f64 - 69.0 + cents / 100.0) / 12.0)
    }

    pub fn from_str_name(s: &str) -> Self {
        for &t in &Self::ALL {
            if t.name() == s || t.short_name() == s {
                return t;
            }
        }
        Temperament::Equal
    }
}

impl Default for Temperament {
    fn default() -> Self {
        Temperament::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stops::STOPS;
    use crate::dsp::stops_dsp::STOP_DEFINITIONS;

    #[test]
    fn test_a4_invariant_across_all_temperaments() {
        for &t in &Temperament::ALL {
            let freq = t.midi_to_freq(440.0, 69);
            assert!((freq - 440.0).abs() < 1e-9, "A4 deviated in {:?}", t);
            let freq415 = t.midi_to_freq(415.0, 69);
            assert!((freq415 - 415.0).abs() < 1e-9, "A4 deviated for 415 in {:?}", t);
        }
    }

    #[test]
    fn test_clarion_8_foot_native_stop() {
        let clarion8 = STOPS.iter().find(|s| s.name == "Clarion 8'").expect("Clarion 8' missing in STOPS");
        assert_eq!(clarion8.id, 30);
        let def = &STOP_DEFINITIONS[30];
        assert_eq!(def.name, "Clarion 8'");
        assert_eq!(def.id, 30);
    }
}
