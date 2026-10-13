//! SOUNDFONT ENGINE & SAMPLE PARSER
//!
//! Loads SF2 SoundFont banks into memory and translates them into playable ranks.

use crate::classifier::{classify_division, classify_footage, Division, Footage};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Debug)]
pub struct SoundFontPreset {
    pub id: usize,
    pub font_id: usize,
    pub font_name: String,
    pub name: String,
    pub bank: i32,
    pub patch: i32,
    pub division: Division,
    pub footage: Footage,
}

pub struct LoadedFont {
    pub id: usize,
    pub name: String,
    pub path: PathBuf,
    pub sound_font: Arc<SoundFont>,
    pub presets: Vec<SoundFontPreset>,
    pub synth: Synthesizer,
    /// Channel map for currently active presets in this font: preset_id -> channel (0..15, skipping 9)
    pub active_channels: HashMap<usize, i32>,
    pub free_channels: Vec<i32>,
}

/// Manages loaded SF2 SoundFont banks and active synthesizer channels.
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
            let name = p.get_name().trim().to_string();
            let division = classify_division(&name);
            let footage = classify_footage(&name);
            discovered.push(SoundFontPreset {
                id: pid,
                font_id,
                font_name: font_name.clone(),
                name,
                bank: p.get_bank_number(),
                patch: p.get_patch_number(),
                division,
                footage,
            });
        }

        // Sort presets: Division first, then Footage, then Name
        discovered.sort_by(|a, b| {
            a.division
                .cmp(&b.division)
                .then(a.footage.cmp(&b.footage))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let mut synth = Synthesizer::new(&sf, &self.settings)
            .map_err(|e| format!("Failed to create synthesizer: {e:?}"))?;
        synth.set_master_volume(1.0);

        // 16 channels, excluding channel 9 (GM Percussion) so pitched stops don't become drums!
        let free_channels = (0..16).filter(|&c| c != 9).rev().collect();

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
                // If all free channels are exhausted, recycle the oldest one
                let chan = if let Some(c) = font.free_channels.pop() {
                    c
                } else if let Some((&first_id, &c)) = font.active_channels.iter().next() {
                    font.synth.note_off_all_channel(c, true);
                    font.active_channels.remove(&first_id);
                    self.active_presets.remove(&first_id);
                    c
                } else {
                    0
                };

                // Set bank and program via MIDI Control Change & Program Change
                font.synth.process_midi_message(chan, 0xB0, 0x00, preset.bank);
                font.synth.process_midi_message(chan, 0xC0, preset.patch, 0);
                // Set Volume and Expression to 127 (maximum audible gain)
                font.synth.process_midi_message(chan, 0xB0, 0x07, 127);
                font.synth.process_midi_message(chan, 0xB0, 0x0B, 127);

                font.active_channels.insert(preset_id, chan);
                self.active_presets.insert(preset_id);
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
    pub fn note_on(&mut self, key: u8, velocity: u8) {
        if self.active_presets.is_empty() {
            return;
        }
        for font in &mut self.fonts {
            for &chan in font.active_channels.values() {
                font.synth.note_on(chan, key as i32, velocity.max(1) as i32);
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

    /// Render synthesized audio directly into the live bus for unified reverb and limiting.
    pub fn render_into_live(&mut self, live: &mut [f32]) -> bool {
        if self.active_presets.is_empty() {
            return false;
        }
        let frames = live.len();
        if self.temp_left.len() < frames {
            self.temp_left.resize(frames, 0.0);
            self.temp_right.resize(frames, 0.0);
        }
        let left = &mut self.temp_left[..frames];
        let right = &mut self.temp_right[..frames];

        let active_count = self.active_presets.len().max(1) as f32;
        // Equal volume calibration: matches native pipe loudness and prevents headroom blowing
        let gain = 1.0 / active_count;
        let mut produced_audio = false;

        for font in &mut self.fonts {
            if font.active_channels.is_empty() {
                continue;
            }
            left.fill(0.0);
            right.fill(0.0);
            font.synth.render(left, right);

            for i in 0..frames {
                let sample = (left[i] + right[i]) * 0.5 * gain;
                if sample.abs() > 1e-5 {
                    produced_audio = true;
                }
                live[i] += sample;
            }
        }
        produced_audio
    }

    /// Render synthesized audio and add it directly into `out` (mono).
    pub fn render_into(&mut self, out: &mut [f32], master_volume: f32) {
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

        let gain = master_volume * 0.95;

        for font in &mut self.fonts {
            if font.active_channels.is_empty() {
                continue;
            }
            left.fill(0.0);
            right.fill(0.0);
            font.synth.render(left, right);

            for i in 0..frames {
                // Average left and right for mono mix bus
                out[i] += (left[i] + right[i]) * 0.5 * gain;
            }
        }
    }

    /// Extracts a lightweight, thread-safe snapshot of all active fonts and channels
    /// to synthesize offline audio without holding the real-time audio mutex.
    pub fn offline_snapshot(&self) -> SoundFontOfflineSnapshot {
        let mut fonts = Vec::new();
        for f in &self.fonts {
            if f.active_channels.is_empty() {
                continue;
            }
            let mut active_presets = Vec::new();
            let mut active_channels = Vec::new();
            for (&pid, &chan) in &f.active_channels {
                if let Some(p) = f.presets.iter().find(|p| p.id == pid) {
                    active_presets.push(p.clone());
                    active_channels.push((pid, chan));
                }
            }
            if !active_channels.is_empty() {
                fonts.push(OfflineFontSnapshot {
                    sound_font: Arc::clone(&f.sound_font),
                    active_presets,
                    active_channels,
                });
            }
        }
        SoundFontOfflineSnapshot {
            fonts,
        }
    }
}

#[derive(Clone)]
pub struct OfflineFontSnapshot {
    pub sound_font: Arc<SoundFont>,
    pub active_presets: Vec<SoundFontPreset>,
    pub active_channels: Vec<(usize, i32)>,
}

pub struct SoundFontOfflineSnapshot {
    pub fonts: Vec<OfflineFontSnapshot>,
}

impl SoundFontOfflineSnapshot {
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Offline timeline synthesis using fixed block processing with dedicated synthesizer instances.
    pub fn render_midi_offline(&self, events: &[(f64, bool, u8)], total_frames: usize) -> Vec<f32> {
        if self.fonts.is_empty() || events.is_empty() || total_frames == 0 {
            return vec![0.0; total_frames];
        }

        let mut output = vec![0.0f32; total_frames];
        const BLOCK_SIZE: usize = 1024;
        let mut left_block = vec![0.0f32; BLOCK_SIZE];
        let mut right_block = vec![0.0f32; BLOCK_SIZE];

        let mut settings = SynthesizerSettings::new(SAMPLE_RATE as i32);
        settings.block_size = BLOCK_SIZE;
        settings.maximum_polyphony = 128;

        for font in &self.fonts {
            if font.active_channels.is_empty() {
                continue;
            }

            let Ok(mut synth) = Synthesizer::new(&font.sound_font, &settings) else {
                continue;
            };
            synth.set_master_volume(1.0);

            // Configure channels with Bank Select (CC 0), Program Change, and Volume/Expression 127
            for &(pid, chan) in &font.active_channels {
                if let Some(preset) = font.active_presets.iter().find(|p| p.id == pid) {
                    synth.process_midi_message(chan, 0xB0, 0x00, preset.bank);
                    synth.process_midi_message(chan, 0xC0, preset.patch, 0);
                    synth.process_midi_message(chan, 0xB0, 0x07, 127);
                    synth.process_midi_message(chan, 0xB0, 0x0B, 127);
                }
            }

            let mut event_idx = 0;
            let mut current_frame = 0;
            let sample_rate = SAMPLE_RATE as f64;

            while current_frame < total_frames {
                let block_len = BLOCK_SIZE.min(total_frames - current_frame);
                let block_end_time = (current_frame + block_len) as f64 / sample_rate;

                while event_idx < events.len() && events[event_idx].0 < block_end_time {
                    let (_, on, note) = events[event_idx];
                    for &(_, chan) in &font.active_channels {
                        if on {
                            synth.note_on(chan, note as i32, 110);
                        } else {
                            synth.note_off(chan, note as i32);
                        }
                    }
                    event_idx += 1;
                }

                left_block[..block_len].fill(0.0);
                right_block[..block_len].fill(0.0);
                synth.render(&mut left_block[..block_len], &mut right_block[..block_len]);

                for i in 0..block_len {
                    // Balanced SoundFont volume normalized against native pipe levels
                    output[current_frame + i] += (left_block[i] + right_block[i]) * 0.5 * 0.95;
                }
                current_frame += block_len;
            }
        }

        output
    }
}
