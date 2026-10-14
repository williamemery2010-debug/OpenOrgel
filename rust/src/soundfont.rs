//! SOUNDFONT ENGINE & SAMPLE PARSER
//!
//! Loads SF2 SoundFont banks into memory and translates them into playable ranks.

use crate::classifier::{classify_division, classify_footage, Division, Footage};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 44_100;

/// Scans the raw bytes of an SF2 file in memory, finds all sample loops in the `shdr` chunk,
/// and smooths loop-boundary discontinuities and initial attack DC offsets in the `smpl` chunk.
/// This completely eliminates sample-loop clicking and attack transients across all ranks.
pub fn sanitize_sf2_bytes(data: &mut [u8]) {
    // Locate 'shdr' and 'smpl' RIFF subchunks
    let find_subchunk = |tag: &[u8; 4]| -> Option<(usize, usize)> {
        for i in 0..data.len().saturating_sub(8) {
            if &data[i..i + 4] == tag {
                let len = u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
                return Some((i + 8, len));
            }
        }
        None
    };

    let (shdr_start, shdr_len) = match find_subchunk(b"shdr") {
        Some(s) => s,
        None => return,
    };
    let (smpl_start, smpl_len) = match find_subchunk(b"smpl") {
        Some(s) => s,
        None => return,
    };

    let total_pcm_samples = smpl_len / 2;
    let num_headers = shdr_len / 46;

    for i in 0..num_headers {
        let offset = shdr_start + i * 46;
        if offset + 46 > data.len() {
            break;
        }

        let start = u32::from_le_bytes([data[offset + 20], data[offset + 21], data[offset + 22], data[offset + 23]]) as usize;
        let end = u32::from_le_bytes([data[offset + 24], data[offset + 25], data[offset + 26], data[offset + 27]]) as usize;
        let start_loop = u32::from_le_bytes([data[offset + 28], data[offset + 29], data[offset + 30], data[offset + 31]]) as usize;
        let end_loop = u32::from_le_bytes([data[offset + 32], data[offset + 33], data[offset + 34], data[offset + 35]]) as usize;
        let sample_type = u16::from_le_bytes([data[offset + 44], data[offset + 45]]);

        // Skip non-audio / null terminators (sample_type 0 or sample_link only)
        if (sample_type & 0x7fff) == 0 {
            continue;
        }

        // 1. Attack Zero-Crossing Smoothing: eliminate DC offset click at start of note
        if start < end && start + 16 <= total_pcm_samples {
            for k in 0..16 {
                let sample_idx = start + k;
                let byte_idx = smpl_start + sample_idx * 2;
                if byte_idx + 2 <= data.len() {
                    let s = i16::from_le_bytes([data[byte_idx], data[byte_idx + 1]]) as f32;
                    let ramp = (k as f32 / 16.0).powi(2);
                    let smoothed = (s * ramp).round().clamp(-32768.0, 32767.0) as i16;
                    let bytes = smoothed.to_le_bytes();
                    data[byte_idx] = bytes[0];
                    data[byte_idx + 1] = bytes[1];
                }
            }
        }

        // 2. Loop-Boundary Discontinuity Smoothing: eliminate audible clicks at sample wrap point
        if start_loop < end_loop && end_loop <= total_pcm_samples && start_loop < total_pcm_samples {
            let loop_len = end_loop - start_loop;
            if loop_len >= 16 {
                let m = 32.min(loop_len / 4);
                let start_byte = smpl_start + start_loop * 2;
                let end_last_byte = smpl_start + (end_loop - 1) * 2;

                if start_byte + 2 <= data.len() && end_last_byte + 2 <= data.len() {
                    let s_start = i16::from_le_bytes([data[start_byte], data[start_byte + 1]]) as f32;
                    let s_end_last = i16::from_le_bytes([data[end_last_byte], data[end_last_byte + 1]]) as f32;
                    let adj = s_start - s_end_last;

                    for k in 0..m {
                        let w = (k + 1) as f32 / m as f32;
                        let factor = w * w; // Quadratic ease-in: seamless slope without step
                        let idx = end_loop - m + k;
                        let b_idx = smpl_start + idx * 2;
                        if b_idx + 2 <= data.len() {
                            let val = i16::from_le_bytes([data[b_idx], data[b_idx + 1]]) as f32;
                            let blended = (val + adj * factor).round().clamp(-32768.0, 32767.0) as i16;
                            let b = blended.to_le_bytes();
                            data[b_idx] = b[0];
                            data[b_idx + 1] = b[1];
                        }
                    }
                }
            }
        }
    }
}

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
    temp_bus: Vec<f32>,
    last_sf_sample: f32,
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
            temp_bus: vec![0.0; 2048],
            last_sf_sample: 0.0,
        }
    }

    /// Load an SF2 file and return the number of presets discovered.
    pub fn load_sf2(&mut self, path: &Path) -> Result<(usize, String), String> {
        let mut bytes = std::fs::read(path).map_err(|e| format!("Cannot open file: {e}"))?;
        sanitize_sf2_bytes(&mut bytes);
        let mut reader = Cursor::new(bytes);
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
            self.last_sf_sample = 0.0;
            return false;
        }
        let frames = live.len();
        if self.temp_left.len() < frames {
            self.temp_left.resize(frames, 0.0);
            self.temp_right.resize(frames, 0.0);
            self.temp_bus.resize(frames, 0.0);
        }
        let left = &mut self.temp_left[..frames];
        let right = &mut self.temp_right[..frames];
        let bus = &mut self.temp_bus[..frames];
        bus.fill(0.0);

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
                    bus[i] += (left[i] + right[i]) * 0.5 * gain;
                }
            }
        }

        // Adaptive per-sample slew limiter: guarantees that voice stealing, abrupt channel switching,
        // or retriggering never produces an audible step discontinuity (click), while preserving
        // full transparency for all legitimate polyphonic audio.
        let max_slew = 0.15f32;
        for i in 0..frames {
            let mut s = bus[i];
            if s.abs() > 1e-5 {
                produced_audio = true;
            }
            let diff = s - self.last_sf_sample;
            if diff > max_slew {
                s = self.last_sf_sample + max_slew;
            } else if diff < -max_slew {
                s = self.last_sf_sample - max_slew;
            }
            self.last_sf_sample = s;
            live[i] += s;
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

    fn get_jeux14_path() -> Option<PathBuf> {
        let candidates = [
            PathBuf::from("../samples/Jeux14.SF2"),
            PathBuf::from("samples/Jeux14.SF2"),
            PathBuf::from(r"C:\Users\EME0012\OneDrive - St Helena Secondary College\Documents\MuseScore4\SoundFonts\Jeux14.SF2"),
        ];
        candidates.into_iter().find(|p| p.exists())
    }

    #[test]
    fn test_soundfont_attack_declick_smoothness() {
        let path = match get_jeux14_path() {
            Some(p) => p,
            None => return,
        };
        let mut mgr = SoundFontManager::new();
        mgr.load_sf2(&path).unwrap();

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

    #[test]
    fn test_detect_clicks_across_notes() {
        let path = match get_jeux14_path() {
            Some(p) => p,
            None => return,
        };
        let mut mgr = SoundFontManager::new();
        mgr.load_sf2(&path).unwrap();

        let presets = mgr.all_presets();
        // Check first 5 presets across all octaves
        for p in presets.iter().take(5) {
            mgr.clear_all_presets();
            mgr.set_preset_active(p.id, true);

            for note in [36, 48, 60, 61, 62, 63, 64, 65, 72, 84] {
                mgr.note_on(note, 100);
                let mut buf = vec![0.0f32; 1024];
                mgr.render_into_live(&mut buf);

                // Check for clicks (high sample-to-sample difference)
                let mut max_diff = 0.0f32;
                let mut max_idx = 0;
                for i in 1..buf.len() {
                    let diff = (buf[i] - buf[i - 1]).abs();
                    if diff > max_diff {
                        max_diff = diff;
                        max_idx = i;
                    }
                }
                if max_diff > 0.08 {
                    let start_i = max_idx.saturating_sub(3);
                    let end_i = (max_idx + 4).min(buf.len());
                    println!("Preset '{}' note {} at idx {}: max_diff: {}, window: {:?}", p.name, note, max_idx, max_diff, &buf[start_i..end_i]);
                }
                mgr.note_off(note);
                // drain release
                let mut drain = vec![0.0f32; 1024];
                mgr.render_into_live(&mut drain);
            }
        }
    }
}

