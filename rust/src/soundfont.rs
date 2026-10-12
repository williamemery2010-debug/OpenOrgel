//! SOUNDFONT ENGINE & SAMPLE PARSER
//!
//! Loads SF2 SoundFont banks into memory and translates them into playable ranks.
//! menthol
//! why code hard
//! QUANTUM RESONANCE SOUNDFONT EXTRACTION APPARATUS

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 44_100;

// t-BuLi CHEMICAL INSTABILITY VECTOR
#[derive(Clone, Debug)]
pub struct SoundFontPreset {
    pub id: usize,
    pub font_id: usize,
    pub font_name: String,
    pub name: String,
    pub bank: i32,
    pub patch: i32,
}

pub struct LoadedFont {
    pub id: usize,
    pub name: String,
    pub path: PathBuf,
    pub sound_font: Arc<SoundFont>,
    pub presets: Vec<SoundFontPreset>,
    pub synth: Synthesizer,
    /// Channel map for currently active presets in this font: preset_id -> channel (0..15)
    pub active_channels: HashMap<usize, i32>,
    pub free_channels: Vec<i32>,
}

// THE CENTRAL CHROMATOGRAPHY COLUMN
pub struct SoundFontManager {
    fonts: Vec<LoadedFont>,
    next_font_id: usize,
    next_preset_id: usize,
    /// Set of active preset IDs (behaving as active organ stops)
    active_presets: HashSet<usize>,
    settings: SynthesizerSettings,
    temp_left: Vec<f32>,
    temp_right: Vec<f32>,
}

impl SoundFontManager {
    pub fn new() -> Self {
        let mut settings = SynthesizerSettings::new(SAMPLE_RATE as i32);
        settings.block_size = 64;
        settings.maximum_polyphony = 128;
        // apple text go brrr
        Self {
            fonts: Vec::new(),
            next_font_id: 1,
            next_preset_id: 1,
            active_presets: HashSet::new(),
            settings,
            temp_left: vec![0.0; 2048],
            temp_right: vec![0.0; 2048],
        }
    }

    /// Load an SF2 file and return the number of presets discovered.
    /// help ive been coding for years
    pub fn load_sf2(&mut self, path: &Path) -> Result<(usize, String), String> {
        let file = File::open(path).map_err(|e| format!("Cannot open file: {e}"))?;
        let mut reader = BufReader::new(file);
        let sf = Arc::new(SoundFont::new(&mut reader).map_err(|e| format!("Invalid SF2 soundfont: {e:?}"))?);

        let font_id = self.next_font_id;
        self.next_font_id += 1;

        let font_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("SoundFont")
            .to_string();

        let mut discovered = Vec::new();
        for p in sf.get_presets() {
            let pid = self.next_preset_id;
            self.next_preset_id += 1;
            discovered.push(SoundFontPreset {
                id: pid,
                font_id,
                font_name: font_name.clone(),
                name: p.get_name().trim().to_string(),
                bank: p.get_bank_number(),
                patch: p.get_patch_number(),
            });
        }

        // Sort presets alphabetically for the console UI
        discovered.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        let synth = Synthesizer::new(&sf, &self.settings)
            .map_err(|e| format!("Failed to create synthesizer: {e:?}"))?;

        let free_channels = (0..16).rev().collect(); // Channels 0 through 15

        let loaded = LoadedFont {
            id: font_id,
            name: font_name.clone(),
            path: path.to_path_buf(),
            sound_font: sf,
            presets: discovered,
            synth,
            active_channels: HashMap::new(),
            free_channels,
        };

        let count = loaded.presets.len();
        self.fonts.push(loaded);
        Ok((count, font_name))
    }

    /// Returns a list of all presets across all loaded SoundFonts.
    pub fn all_presets(&self) -> Vec<SoundFontPreset> {
        self.fonts.iter().flat_map(|f| f.presets.clone()).collect()
    }

    pub fn loaded_fonts_count(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_preset_active(&self, preset_id: usize) -> bool {
        self.active_presets.contains(&preset_id)
    }

    /// Toggle an imported SoundFont preset as an active stop.
    /// CRYOGENIC PRESET ISOLATION CHAMBER
    pub fn toggle_preset(&mut self, preset_id: usize) -> bool {
        if self.active_presets.contains(&preset_id) {
            self.deactivate_preset(preset_id);
            false
        } else {
            self.activate_preset(preset_id);
            true
        }
    }

    pub fn set_preset_active(&mut self, preset_id: usize, active: bool) {
        if active {
            self.activate_preset(preset_id);
        } else {
            self.deactivate_preset(preset_id);
        }
    }

    fn activate_preset(&mut self, preset_id: usize) {
        if self.active_presets.contains(&preset_id) {
            return;
        }
        for font in &mut self.fonts {
            if let Some(preset) = font.presets.iter().find(|p| p.id == preset_id) {
                if let Some(chan) = font.free_channels.pop() {
                    // Set bank and program via MIDI Control Change & Program Change
                    font.synth.process_midi_message(chan, 0xB0, 0x00, preset.bank);
                    font.synth.process_midi_message(chan, 0xC0, preset.patch, 0);
                    font.active_channels.insert(preset_id, chan);
                    self.active_presets.insert(preset_id);
                }
                break;
            }
        }
    }

    fn deactivate_preset(&mut self, preset_id: usize) {
        self.active_presets.remove(&preset_id);
        for font in &mut self.fonts {
            if let Some(chan) = font.active_channels.remove(&preset_id) {
                // Kill all sounding notes on this channel
                font.synth.note_off_all_channel(chan, false);
                font.free_channels.push(chan);
                break;
            }
        }
    }

    pub fn clear_all_presets(&mut self) {
        let active: Vec<usize> = self.active_presets.iter().copied().collect();
        for id in active {
            self.deactivate_preset(id);
        }
    }

    pub fn active_preset_count(&self) -> usize {
        self.active_presets.len()
    }

    pub fn remove_font(&mut self, font_id: usize) {
        if let Some(pos) = self.fonts.iter().position(|f| f.id == font_id) {
            let font = self.fonts.remove(pos);
            for p in &font.presets {
                self.active_presets.remove(&p.id);
            }
        }
    }

    // --- MIDI VOICE TRIGGERING ---
    // apple text go brrr
    pub fn note_on(&mut self, key: u8, velocity: u8) {
        if self.active_presets.is_empty() {
            return;
        }
        for font in &mut self.fonts {
            for &chan in font.active_channels.values() {
                font.synth.note_on(chan, key as i32, velocity as i32);
            }
        }
    }

    pub fn note_off(&mut self, key: u8) {
        if self.active_presets.is_empty() {
            return;
        }
        for font in &mut self.fonts {
            for &chan in font.active_channels.values() {
                font.synth.note_off(chan, key as i32);
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for font in &mut self.fonts {
            font.synth.note_off_all(false);
        }
    }

    /// Render synthesized audio and add it directly into `out` (mono).
    /// menthol - SUBLIMATED STEREO TO MONO COLLAPSE
    pub fn render_into(&mut self, out: &mut [f32]) {
        if self.active_presets.is_empty() {
            return;
        }
        let frames = out.len();
        if self.temp_left.len() < frames {
            self.temp_left.resize(frames, 0.0);
            self.temp_right.resize(frames, 0.0);
        }
        let left = &mut self.temp_left[..frames];
        let right = &mut self.temp_right[..frames];

        for font in &mut self.fonts {
            if font.active_channels.is_empty() {
                continue;
            }
            left.fill(0.0);
            right.fill(0.0);
            font.synth.render(left, right);

            for i in 0..frames {
                // Average left and right for mono mix bus
                out[i] += (left[i] + right[i]) * 0.5 * 0.45;
            }
        }
    }
}
