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
pub const DECLICK_SAMPLES: usize = 256; // ~5.8 ms smooth onset window

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelSlot {
    pub synth_idx: usize,
    pub channel: i32,
}

pub struct LoadedFont {
    pub id: usize,
    pub name: String,
    pub path: PathBuf,
    pub sound_font: Arc<SoundFont>,
    pub presets: Vec<SoundFontPreset>,
    pub synths: Vec<Synthesizer>,
    /// Channel map for currently active presets in this font: preset_id -> ChannelSlot
    pub active_channels: HashMap<usize, ChannelSlot>,
    pub free_channels: Vec<ChannelSlot>,
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
    attack_declick_remaining: usize,
    attack_declick_pos: usize,
    last_live_sample: f32,
}

impl SoundFontManager {
    pub fn new() -> Self {
        let mut settings = SynthesizerSettings::new(SAMPLE_RATE as i32);
        settings.block_size = 64;
        settings.maximum_polyphony = 128;
        settings.enable_reverb_and_chorus = false;
        Self {
            fonts: Vec::new(),
            next_font_id: 1,
            next_preset_id: 1,
            active_presets: HashSet::new(),
            settings,
            temp_left: vec![0.0; 2048],
            temp_right: vec![0.0; 2048],
            attack_declick_remaining: 0,
            attack_declick_pos: 0,
            last_live_sample: 0.0,
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

        // 16 channels per synthesizer, excluding channel 9 (GM Percussion) so pitched stops don't become drums
        let free_channels: Vec<ChannelSlot> = (0..16)
            .filter(|&c| c != 9)
            .rev()
            .map(|c| ChannelSlot { synth_idx: 0, channel: c })
            .collect();

        let loaded = LoadedFont {
            id: font_id,
            name: font_name.clone(),
            path: path.to_path_buf(),
            sound_font: sf,
            presets: discovered,
            synths: vec![synth],
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
                // Dynamically allocate new Synthesizers when all 15 channels on existing instances are filled.
                // This removes the 15-stop limit and allows arbitrary numbers of stops to be drawn simultaneously.
                let slot = if let Some(s) = font.free_channels.pop() {
                    s
                } else {
                    let new_synth_idx = font.synths.len();
                    if let Ok(mut new_synth) = Synthesizer::new(&font.sound_font, &self.settings) {
                        new_synth.set_master_volume(1.0);
                        font.synths.push(new_synth);
                        for c in (0..16).filter(|&c| c != 9).rev() {
                            font.free_channels.push(ChannelSlot { synth_idx: new_synth_idx, channel: c });
                        }
                        font.free_channels.pop().unwrap()
                    } else {
                        ChannelSlot { synth_idx: 0, channel: 0 }
                    }
                };

                if slot.synth_idx < font.synths.len() {
                    let synth = &mut font.synths[slot.synth_idx];
                    synth.process_midi_message(slot.channel, 0xB0, 0x00, preset.bank);
                    synth.process_midi_message(slot.channel, 0xC0, preset.patch, 0);
                    synth.process_midi_message(slot.channel, 0xB0, 0x07, 127);
                    synth.process_midi_message(slot.channel, 0xB0, 0x0B, 127);
                }

                font.active_channels.insert(preset_id, slot);
                self.active_presets.insert(preset_id);
                break;
            }
        }
    }

    fn deactivate_preset(&mut self, preset_id: usize) {
        self.active_presets.remove(&preset_id);
        for font in &mut self.fonts {
            if let Some(slot) = font.active_channels.remove(&preset_id) {
                if slot.synth_idx < font.synths.len() {
                    font.synths[slot.synth_idx].note_off_all_channel(slot.channel, false);
                }
                font.free_channels.push(slot);
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
        // Initialize attack de-click envelope whenever a new note starts
        self.attack_declick_remaining = DECLICK_SAMPLES;
        self.attack_declick_pos = 0;

        for font in &mut self.fonts {
            for slot in font.active_channels.values() {
                if slot.synth_idx < font.synths.len() {
                    font.synths[slot.synth_idx].note_on(slot.channel, key as i32, velocity.max(1) as i32);
                }
            }
        }
    }

    pub fn note_off(&mut self, key: u8) {
        if self.active_presets.is_empty() {
            return;
        }
        for font in &mut self.fonts {
            for slot in font.active_channels.values() {
                if slot.synth_idx < font.synths.len() {
                    font.synths[slot.synth_idx].note_off(slot.channel, key as i32);
                }
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for font in &mut self.fonts {
            for synth in &mut font.synths {
                synth.note_off_all(false);
            }
        }
    }

    /// Render synthesized audio directly into the live bus for unified acoustics and limiting.
    pub fn render_into_live(&mut self, live: &mut [f32]) -> bool {
        if self.active_presets.is_empty() {
            self.last_live_sample = 0.0;
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
        // Energy normalization (1 / sqrt(N)): keeps total loudness consistent across arbitrary registrations
        let gain = 1.0 / active_count.sqrt();
        let mut produced_audio = false;

        for font in &mut self.fonts {
            if font.active_channels.is_empty() {
                continue;
            }
            for synth in &mut font.synths {
                left.fill(0.0);
                right.fill(0.0);
                synth.render(left, right);

                for i in 0..frames {
                    let sample = (left[i] + right[i]) * 0.5 * gain;
                    if sample.abs() > 1e-5 {
                        produced_audio = true;
                    }
                    live[i] += sample;
                }
            }
        }

        // Attack De-Clicker Smoothing Envelope: eliminates transient clicks at the start of every note
        if self.attack_declick_remaining > 0 {
            let n = frames.min(self.attack_declick_remaining);
            let last_sample = self.last_live_sample;
            for i in 0..n {
                let t = self.attack_declick_pos as f32 / DECLICK_SAMPLES as f32;
                // S-curve half-sine squared: guarantees C^0 continuity and C^1 zero derivative at onset
                let ramp = (t * std::f32::consts::FRAC_PI_2).sin().powi(2);
                live[i] = last_sample * (1.0 - ramp) + live[i] * ramp;
                self.attack_declick_pos += 1;
            }
            self.attack_declick_remaining = self.attack_declick_remaining.saturating_sub(n);
        }

        if frames > 0 {
            self.last_live_sample = live[frames - 1];
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

        let active_count = self.active_presets.len().max(1) as f32;
        let gain = master_volume * 0.95 / active_count.sqrt();

        for font in &mut self.fonts {
            if font.active_channels.is_empty() {
                continue;
            }
            for synth in &mut font.synths {
                left.fill(0.0);
                right.fill(0.0);
                synth.render(left, right);

                for i in 0..frames {
                    out[i] += (left[i] + right[i]) * 0.5 * gain;
                }
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
            let mut active_slots = Vec::new();
            for (&pid, &slot) in &f.active_channels {
                if let Some(p) = f.presets.iter().find(|p| p.id == pid) {
                    active_presets.push(p.clone());
                    active_slots.push((pid, slot));
                }
            }
            if !active_slots.is_empty() {
                fonts.push(OfflineFontSnapshot {
                    sound_font: Arc::clone(&f.sound_font),
                    active_presets,
                    active_slots,
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
    pub active_slots: Vec<(usize, ChannelSlot)>,
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
        settings.enable_reverb_and_chorus = false;

        for font in &self.fonts {
            if font.active_slots.is_empty() {
                continue;
            }

            let num_synths = font.active_slots.iter().map(|s| s.1.synth_idx).max().map(|m| m + 1).unwrap_or(1);
            let mut synths = Vec::with_capacity(num_synths);
            for _ in 0..num_synths {
                let Ok(mut synth) = Synthesizer::new(&font.sound_font, &settings) else {
                    continue;
                };
                synth.set_master_volume(1.0);
                synths.push(synth);
            }
            if synths.is_empty() {
                continue;
            }

            // Configure each channel on its assigned synthesizer
            for &(pid, slot) in &font.active_slots {
                if slot.synth_idx < synths.len() {
                    if let Some(preset) = font.active_presets.iter().find(|p| p.id == pid) {
                        let synth = &mut synths[slot.synth_idx];
                        synth.process_midi_message(slot.channel, 0xB0, 0x00, preset.bank);
                        synth.process_midi_message(slot.channel, 0xC0, preset.patch, 0);
                        synth.process_midi_message(slot.channel, 0xB0, 0x07, 127);
                        synth.process_midi_message(slot.channel, 0xB0, 0x0B, 127);
                    }
                }
            }

            let mut event_idx = 0;
            let mut current_frame = 0;
            let sample_rate = SAMPLE_RATE as f64;
            let active_count = font.active_slots.len().max(1) as f32;
            let gain = 0.95 / active_count.sqrt();

            while current_frame < total_frames {
                let block_len = BLOCK_SIZE.min(total_frames - current_frame);
                let block_end_time = (current_frame + block_len) as f64 / sample_rate;

                while event_idx < events.len() && events[event_idx].0 < block_end_time {
                    let (_, on, note) = events[event_idx];
                    for &(_, slot) in &font.active_slots {
                        if slot.synth_idx < synths.len() {
                            let synth = &mut synths[slot.synth_idx];
                            if on {
                                synth.note_on(slot.channel, note as i32, 110);
                            } else {
                                synth.note_off(slot.channel, note as i32);
                            }
                        }
                    }
                    event_idx += 1;
                }

                for synth in &mut synths {
                    left_block[..block_len].fill(0.0);
                    right_block[..block_len].fill(0.0);
                    synth.render(&mut left_block[..block_len], &mut right_block[..block_len]);

                    for i in 0..block_len {
                        output[current_frame + i] += (left_block[i] + right_block[i]) * 0.5 * gain;
                    }
                }
                current_frame += block_len;
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soundfont_unlimited_presets_exceeding_15_channels() {
        let path = Path::new(r"C:\Users\EME0012\OneDrive - St Helena Secondary College\Documents\MuseScore4\SoundFonts\Jeux14.SF2");
        if !path.exists() {
            return;
        }
        let mut mgr = SoundFontManager::new();
        let (count, _) = mgr.load_sf2(path).unwrap();
        assert!(count >= 20);

        let presets = mgr.all_presets();
        for p in presets.iter().take(25) {
            mgr.set_preset_active(p.id, true);
        }

        // Successfully supports 25+ stops without evicting or capping at 15
        assert_eq!(mgr.active_preset_count(), 25);
    }

    #[test]
    fn test_soundfont_attack_declick_smoothness() {
        let path = Path::new(r"C:\Users\EME0012\OneDrive - St Helena Secondary College\Documents\MuseScore4\SoundFonts\Jeux14.SF2");
        if !path.exists() {
            return;
        }
        let mut mgr = SoundFontManager::new();
        mgr.load_sf2(path).unwrap();

        let presets = mgr.all_presets();
        if let Some(p) = presets.first() {
            mgr.set_preset_active(p.id, true);
            mgr.note_on(60, 100);

            let mut live = vec![0.0f32; 128];
            mgr.render_into_live(&mut live);

            // First sample is guaranteed zero from silence with no step impulse
            assert!(live[0].abs() < 1e-6, "Sample 0 must be 0 to prevent click, got {}", live[0]);
            for s in &live {
                assert!(!s.is_nan() && !s.is_infinite());
            }
        }
    }
}

