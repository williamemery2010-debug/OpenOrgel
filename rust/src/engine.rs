//! CORE ENGINE - tone bank, live voices, MIDI playback state and the mixer.
//!
//! Direct descendant of the Python host (tone cache, `audio_callback`,
//! `_generate_audio_buffer`) but with 100% less GIL and 100% more borrow checker.
//! apple text go brrr

use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crate::ffi::{self, Reverb};

pub const SAMPLE_RATE: u32 = 44_100;
/// Standard cached tone length. Anything shorter is sliced out of this.
pub const TONE_SECONDS: f64 = 3.5;
pub const ATTACK_SECONDS: f64 = 0.12;
pub const RELEASE_SECONDS: f64 = 0.1;
pub const LIVE_NOTE_LOW: u8 = 36;
pub const LIVE_NOTE_HIGH: u8 = 96;
pub const REVERB_ROOM: f32 = 0.86;
pub const REVERB_WET: f32 = 0.40;
/// Live output is scaled before the tanh soft limiter to dodge polyphonic crunch.
const LIVE_PRE_LIMIT_GAIN: f32 = 0.35;
/// Seconds the reverb is kept running after the last live voice dies.
const REVERB_TAIL_SECONDS: usize = 6;

/// (start sample, end sample, midi note)
pub type NoteSpan = (usize, usize, u8);

pub fn midi_to_freq(a4: f64, note: u8) -> f64 {
    a4 * 2f64.powf((note as f64 - 69.0) / 12.0)
}

// ---------------------------------------------------------------------------
// TONE BANK: per-stop tone cache (the RAM half of the dual-layer cache matrix)
// ---------------------------------------------------------------------------

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct ToneKey {
    freq_centi: i64,
    samples: usize,
    stop: i32, // -1 = the default sine, for when no stops are drawn
}

#[derive(Default)]
pub struct ToneBank {
    map: RwLock<HashMap<ToneKey, Arc<Vec<f32>>>>,
}

impl ToneBank {
    /// One stop (or the default sine) at a frequency. Returned buffer is at least
    /// `duration` long; callers slice what they need.
    pub fn stop_tone(&self, freq: f64, duration: f64, stop: Option<i32>) -> Arc<Vec<f32>> {
        let cache_duration = if duration <= TONE_SECONDS {
            TONE_SECONDS
        } else {
            duration
        };
        let key = ToneKey {
            freq_centi: (freq * 100.0).round() as i64,
            samples: (SAMPLE_RATE as f64 * cache_duration) as usize,
            stop: stop.unwrap_or(-1),
        };
        if let Some(hit) = self.map.read().get(&key) {
            return hit.clone();
        }
        let ids: Vec<i32> = stop.into_iter().collect();
        let data = Arc::new(ffi::generate_tone(freq, cache_duration, SAMPLE_RATE, &ids));
        self.map.write().insert(key, data.clone());
        data
    }

    pub fn clear(&self) {
        self.map.write().clear();
    }

    /// Sum of every drawn stop, trimmed to exactly `duration` seconds (offline render).
    /// menthol - SUPERCONDUCTING TONE ACCUMULATION
    pub fn summed_tone(&self, freq: f64, duration: f64, stops: &[i32]) -> Vec<f32> {
        let n = (SAMPLE_RATE as f64 * duration) as usize;
        let mut acc = vec![0.0f32; n];
        if stops.is_empty() {
            // No physical stops drawn: return absolute silence so SoundFonts sound unadulterated!
            return acc;
        }
        let mut add = |tone: &[f32]| {
            for (a, b) in acc.iter_mut().zip(tone.iter()) {
                *a += *b;
            }
        };
        for &s in stops {
            add(&self.stop_tone(freq, duration, Some(s)));
        }
        acc
    }

    /// Live voice tone: all stops at the standard 3.5 s length, averaged so that
    /// pulling more stops does not blow the headroom.
    /// why code hard
    pub fn live_tone(&self, freq: f64, stops: &[i32]) -> Vec<f32> {
        let n = (SAMPLE_RATE as f64 * TONE_SECONDS) as usize;
        let mut acc = vec![0.0f32; n];
        if stops.is_empty() {
            // No physical stops drawn: return silence for live keys
            return acc;
        }
        let count = stops.len().max(1) as f32;
        let mut add = |tone: &[f32]| {
            for (a, b) in acc.iter_mut().zip(tone.iter()) {
                *a += *b;
            }
        };
        for &s in stops {
            add(&self.stop_tone(freq, TONE_SECONDS, Some(s)));
        }
        for a in acc.iter_mut() {
            *a /= count;
        }
        acc
    }
}

// ---------------------------------------------------------------------------
// SHARED STATE
// ---------------------------------------------------------------------------

struct Voice {
    tone: Arc<Vec<f32>>,
    phase: usize,
    released: bool,
    release_start: usize,
}

#[derive(Default)]
pub struct PlaybackState {
    pub audio: Option<Arc<Vec<f32>>>,
    pub idx: usize,
    pub playing: bool,
    pub paused: bool,
    pub notes: Arc<Vec<NoteSpan>>,
    pub starts: Arc<Vec<usize>>,
}

pub struct Rendered {
    pub audio: Vec<f32>,
    pub notes: Vec<NoteSpan>,
}

pub struct Engine {
    pub bank: ToneBank,
    live_tones: RwLock<HashMap<u8, Arc<Vec<f32>>>>,
    voices: Mutex<HashMap<u8, Voice>>,
    pub playback: Mutex<PlaybackState>,
    stops: Mutex<Vec<i32>>,
    a4: Mutex<f64>,
    volume: AtomicU32,
    generation: AtomicU64,
    rendering: AtomicBool,
    render_requested: AtomicBool,
    current_midi: Mutex<Option<PathBuf>>,
    /// Set by the audio thread when a song runs off its own end.
    pub song_finished: AtomicBool,
    // menthol - SOUNDFONT TRANSCENDENCE MATRIX
    pub soundfont: Arc<Mutex<crate::soundfont::SoundFontManager>>,
}

impl Engine {
    pub fn new() -> Arc<Engine> {
        Arc::new(Engine {
            bank: ToneBank::default(),
            live_tones: RwLock::new(HashMap::new()),
            voices: Mutex::new(HashMap::new()),
            playback: Mutex::new(PlaybackState::default()),
            stops: Mutex::new(Vec::new()),
            a4: Mutex::new(440.0),
            volume: AtomicU32::new(1.0f32.to_bits()),
            generation: AtomicU64::new(0),
            rendering: AtomicBool::new(false),
            render_requested: AtomicBool::new(false),
            current_midi: Mutex::new(None),
            song_finished: AtomicBool::new(false),
            soundfont: Arc::new(Mutex::new(crate::soundfont::SoundFontManager::new())),
        })
    }

    // ----- parameters -----

    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_stops(&self, ids: Vec<i32>) {
        *self.stops.lock() = ids;
    }

    pub fn stops(&self) -> Vec<i32> {
        self.stops.lock().clone()
    }

    pub fn set_a4(&self, hz: f64) {
        *self.a4.lock() = hz;
    }

    pub fn a4(&self) -> f64 {
        *self.a4.lock()
    }

    /// Stops or tuning changed: throw away stale live tones, rebuild in the
    /// background and, if a song is playing, re-render it with the new voices.
    pub fn refresh(self: &Arc<Self>) {
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.live_tones.write().clear();

        let me = self.clone();
        let stops = self.stops();
        let a4 = self.a4();
        std::thread::spawn(move || {
            (LIVE_NOTE_LOW..=LIVE_NOTE_HIGH)
                .into_par_iter()
                .for_each(|note| {
                    if me.generation.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    if me.live_tones.read().contains_key(&note) {
                        return;
                    }
                    let tone = Arc::new(me.bank.live_tone(midi_to_freq(a4, note), &stops));
                    let mut w = me.live_tones.write();
                    if me.generation.load(Ordering::SeqCst) == gen {
                        w.insert(note, tone);
                    }
                });
        });

        self.request_rerender();
    }

    pub fn clear_caches(self: &Arc<Self>) {
        ffi::clear_engine_cache();
        self.bank.clear();
        self.refresh();
    }

    // ----- live MIDI -----

    pub fn note_on(&self, note: u8) {
        let stops = self.stops();
        if !stops.is_empty() {
            let cached = self.live_tones.read().get(&note).cloned();
            let tone = match cached {
                Some(t) => t,
                None => {
                    let gen = self.generation.load(Ordering::SeqCst);
                    let a4 = self.a4();
                    let t = Arc::new(self.bank.live_tone(midi_to_freq(a4, note), &stops));
                    let mut w = self.live_tones.write();
                    if self.generation.load(Ordering::SeqCst) == gen {
                        w.insert(note, t.clone());
                    }
                    t
                }
            };
            self.voices.lock().insert(
                note,
                Voice {
                    tone,
                    phase: 0,
                    released: false,
                    release_start: 0,
                },
            );
        }
        // apple text go brrr
        self.soundfont.lock().note_on(note, 100);
    }

    pub fn note_off(&self, note: u8) {
        if let Some(v) = self.voices.lock().get_mut(&note) {
            if !v.released {
                v.released = true;
                v.release_start = v.phase;
            }
        }
        self.soundfont.lock().note_off(note);
    }

    pub fn all_notes_off(&self) {
        self.voices.lock().clear();
        self.soundfont.lock().all_notes_off();
    }

    // ----- playback control -----

    pub fn start_playback(&self, rendered: Rendered, path: PathBuf) {
        let mut notes = rendered.notes;
        notes.sort_by_key(|n| n.0);
        let starts: Vec<usize> = notes.iter().map(|n| n.0).collect();
        *self.current_midi.lock() = Some(path);
        let mut pb = self.playback.lock();
        pb.audio = Some(Arc::new(rendered.audio));
        pb.notes = Arc::new(notes);
        pb.starts = Arc::new(starts);
        pb.idx = 0;
        pb.playing = true;
        pb.paused = false;
        self.song_finished.store(false, Ordering::SeqCst);
    }

    pub fn stop_playback(&self) {
        let mut pb = self.playback.lock();
        pb.playing = false;
        pb.paused = false;
        pb.idx = 0;
        *self.current_midi.lock() = None;
    }

    pub fn pause(&self) {
        let mut pb = self.playback.lock();
        if pb.playing {
            pb.paused = true;
        }
    }

    pub fn resume(&self) {
        let mut pb = self.playback.lock();
        if pb.playing {
            pb.paused = false;
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playback.lock().playing
    }

    pub fn is_paused(&self) -> bool {
        self.playback.lock().paused
    }

    pub fn is_rendering(&self) -> bool {
        self.rendering.load(Ordering::SeqCst)
    }

    /// (position seconds, total seconds)
    pub fn progress(&self) -> (f32, f32) {
        let pb = self.playback.lock();
        let total = pb.audio.as_ref().map(|a| a.len()).unwrap_or(0);
        (
            pb.idx as f32 / SAMPLE_RATE as f32,
            total as f32 / SAMPLE_RATE as f32,
        )
    }

    /// Re-render the current song in the background (stops/tuning changed mid-song).
    pub fn request_rerender(self: &Arc<Self>) {
        if !self.is_playing() || self.current_midi.lock().is_none() {
            return;
        }
        if self.rendering.swap(true, Ordering::SeqCst) {
            self.render_requested.store(true, Ordering::SeqCst);
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || loop {
            let path = me.current_midi.lock().clone();
            if let Some(p) = path {
                let stops = me.stops();
                let a4 = me.a4();
                match render_midi(&me, &p, &stops, a4) {
                    Ok(r) => me.swap_audio(r),
                    Err(e) => eprintln!("Background render failed: {e}"),
                }
            }
            if me.render_requested.swap(false, Ordering::SeqCst) {
                continue;
            }
            me.rendering.store(false, Ordering::SeqCst);
            // A request could have slipped in between the check and the store.
            if me.render_requested.swap(false, Ordering::SeqCst)
                && !me.rendering.swap(true, Ordering::SeqCst)
            {
                continue;
            }
            break;
        });
    }

    // menthol - SEAMLESS AUDIO TIMELINE MORPHING
    fn swap_audio(&self, r: Rendered) {
        let mut notes = r.notes;
        notes.sort_by_key(|n| n.0);
        let starts: Vec<usize> = notes.iter().map(|n| n.0).collect();
        let mut pb = self.playback.lock();
        if let Some(old_audio) = pb.audio.as_ref() {
            let cur = pb.idx;
            let xfade_len = 1024
                .min(r.audio.len().saturating_sub(cur))
                .min(old_audio.len().saturating_sub(cur));
            if xfade_len > 0 {
                let mut merged = r.audio;
                for i in 0..xfade_len {
                    let old_w = 1.0 - (i as f32 / xfade_len as f32);
                    let new_w = i as f32 / xfade_len as f32;
                    merged[cur + i] = old_audio[cur + i] * old_w + merged[cur + i] * new_w;
                }
                pb.audio = Some(Arc::new(merged));
            } else {
                pb.audio = Some(Arc::new(r.audio));
            }
        } else {
            pb.audio = Some(Arc::new(r.audio));
        }
        pb.notes = Arc::new(notes);
        pb.starts = Arc::new(starts);
    }

    /// Pitch classes currently sounding (song + live keys), plus "is A4 sounding
    /// during playback" for the servo.
    pub fn visual_state(&self) -> ([bool; 12], bool) {
        let mut pcs = [false; 12];
        let mut a4 = false;
        {
            let pb = self.playback.lock();
            if pb.playing && !pb.starts.is_empty() {
                let cur = pb.idx;
                let limit = pb.starts.partition_point(|&s| s <= cur);
                let lower_target = cur.saturating_sub(6 * SAMPLE_RATE as usize);
                let lower = pb.starts.partition_point(|&s| s < lower_target);
                for i in lower..limit {
                    let (s, e, n) = pb.notes[i];
                    if s <= cur && cur <= e {
                        pcs[(n % 12) as usize] = true;
                        if n == 69 {
                            a4 = true;
                        }
                    }
                }
            }
        }
        for (&n, _) in self.voices.lock().iter() {
            pcs[(n % 12) as usize] = true;
            if n == 69 {
                a4 = true;
            }
        }
        (pcs, a4 && self.is_playing())
    }
}

// ---------------------------------------------------------------------------
// MIXER (lives on the audio thread; renders mono @ SAMPLE_RATE)
// ---------------------------------------------------------------------------

pub struct Mixer {
    engine: Arc<Engine>,
    reverb: Reverb,
    live: Vec<f32>,
    attack_tbl: Vec<f32>,
    release_tbl: Vec<f32>,
    tail_left: usize,
    done: Vec<u8>,
}

impl Mixer {
    pub fn new(engine: Arc<Engine>) -> Self {
        let attack_n = (ATTACK_SECONDS * SAMPLE_RATE as f64) as usize;
        let release_n = (RELEASE_SECONDS * SAMPLE_RATE as f64) as usize;
        let attack_tbl = (0..attack_n)
            .map(|i| (i as f64 * std::f64::consts::FRAC_PI_2 / attack_n as f64).sin() as f32)
            .collect();
        let release_tbl = (0..release_n)
            .map(|i| (i as f64 * std::f64::consts::FRAC_PI_2 / release_n as f64).cos() as f32)
            .collect();
        Mixer {
            engine,
            reverb: Reverb::new(SAMPLE_RATE, REVERB_ROOM),
            live: vec![0.0; 4096],
            attack_tbl,
            release_tbl,
            tail_left: 0,
            done: Vec::with_capacity(16),
        }
    }

    /// Fill `out` with the next mono block. Allocation-free once warmed up.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len();
        let vol = self.engine.volume();

        // --- rendered MIDI playback ---
        {
            let mut pb = self.engine.playback.lock();
            let PlaybackState {
                audio,
                idx,
                playing,
                paused,
                ..
            } = &mut *pb;
            if *playing && !*paused {
                if let Some(audio) = audio.as_ref() {
                    let remaining = audio.len().saturating_sub(*idx);
                    let n = remaining.min(frames);
                    for i in 0..n {
                        out[i] = audio[*idx + i] * vol;
                    }
                    *idx += n;
                    if n < frames {
                        *playing = false;
                        self.engine.song_finished.store(true, Ordering::SeqCst);
                    }
                }
            }
        }

        // --- live organ voices ---
        if self.live.len() < frames {
            self.live.resize(frames, 0.0);
        }
        let live = &mut self.live[..frames];
        live.fill(0.0);

        let rel_n = self.release_tbl.len();
        let mut any = false;
        {
            let mut voices = self.engine.voices.lock();
            if !voices.is_empty() {
                any = true;
                self.done.clear();
                for (&note, v) in voices.iter_mut() {
                    let wave = &v.tone[..];
                    let len = wave.len();
                    if !v.released {
                        let sustain_end = len.saturating_sub(rel_n);
                        let mut loop_start = (2.0 * SAMPLE_RATE as f64) as usize;
                        if loop_start >= sustain_end {
                            loop_start = 0;
                        }
                        for l in live.iter_mut() {
                            if v.phase >= sustain_end {
                                v.phase = loop_start;
                            }
                            let mut s = wave[v.phase];
                            if v.phase < self.attack_tbl.len() {
                                s *= self.attack_tbl[v.phase];
                            }
                            *l += s;
                            v.phase += 1;
                        }
                    } else {
                        let mut dead = false;
                        for l in live.iter_mut() {
                            if v.phase >= len {
                                dead = true;
                                break;
                            }
                            let rel_idx = v.phase - v.release_start;
                            if rel_idx >= rel_n {
                                dead = true;
                                break;
                            }
                            *l += wave[v.phase] * self.release_tbl[rel_idx];
                            v.phase += 1;
                        }
                        if dead || v.phase - v.release_start >= rel_n {
                            self.done.push(note);
                        }
                    }
                }
                for n in &self.done {
                    voices.remove(n);
                }
            }
        }

        if any {
            self.tail_left = REVERB_TAIL_SECONDS * SAMPLE_RATE as usize;
        }
        if self.tail_left > 0 {
            self.reverb.process(live, REVERB_WET);
            self.tail_left = self.tail_left.saturating_sub(frames);
            for i in 0..frames {
                out[i] += (live[i] * LIVE_PRE_LIMIT_GAIN).tanh() * vol;
            }
        }

        // --- soundfont rendered PCM ---
        // why code hard
        self.engine.soundfont.lock().render_into(out, vol);
    }
}

// ---------------------------------------------------------------------------
// OFFLINE MIDI RENDERER (Load & Play, Export WAV, background re-render)
// ---------------------------------------------------------------------------

enum Kind {
    Tempo(f64),
    On(u8),
    Off(u8),
}

/// Parse a MIDI file into absolute-time (seconds) note on/off events,
/// honouring tempo changes the way `mido` does.
fn collect_events(bytes: &[u8]) -> Result<Vec<(f64, bool, u8)>, String> {
    use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

    let smf = Smf::parse(bytes).map_err(|e| format!("Could not parse MIDI file: {e}"))?;

    // seconds per tick, as a function of the current tempo (microseconds / beat)
    let secs_per_tick = |tempo_us: f64| -> f64 {
        match smf.header.timing {
            Timing::Metrical(tpb) => tempo_us / 1_000_000.0 / tpb.as_int().max(1) as f64,
            Timing::Timecode(fps, sub) => 1.0 / (fps.as_f32() as f64 * sub.max(1) as f64),
        }
    };

    let mut raw: Vec<(u64, Kind)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u64 = 0;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                    raw.push((tick, Kind::Tempo(t.as_int() as f64)))
                }
                TrackEventKind::Midi { message, .. } => match message {
                    MidiMessage::NoteOn { key, vel } => {
                        if vel.as_int() > 0 {
                            raw.push((tick, Kind::On(key.as_int())));
                        } else {
                            raw.push((tick, Kind::Off(key.as_int())));
                        }
                    }
                    MidiMessage::NoteOff { key, .. } => raw.push((tick, Kind::Off(key.as_int()))),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    raw.sort_by_key(|e| e.0); // stable: ties keep track order, like mido's merge

    let mut tempo = 500_000.0;
    let mut last_tick = 0u64;
    let mut t = 0.0f64;
    let mut events = Vec::new();
    for (tick, kind) in raw {
        t += (tick - last_tick) as f64 * secs_per_tick(tempo);
        last_tick = tick;
        match kind {
            Kind::Tempo(us) => tempo = us,
            Kind::On(n) => events.push((t, true, n)),
            Kind::Off(n) => events.push((t, false, n)),
        }
    }
    Ok(events)
}

pub fn render_midi(
    engine: &Engine,
    path: &Path,
    stops: &[i32],
    a4: f64,
) -> Result<Rendered, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read MIDI file: {e}"))?;
    let events = collect_events(&bytes)?;
    if events.is_empty() {
        return Err("That MIDI file has no notes in it.".to_string());
    }

    // Pass 1: longest sounding duration per pitch, so each pitch is synthesised once.
    let mut max_dur: HashMap<u8, f64> = HashMap::new();
    let mut active: HashMap<u8, Vec<f64>> = HashMap::new();
    for &(t, on, note) in &events {
        if on {
            active.entry(note).or_default().push(t);
        } else if let Some(starts) = active.get_mut(&note) {
            if !starts.is_empty() {
                let start = starts.remove(0);
                let total = (t - start) + RELEASE_SECONDS;
                let e = max_dur.entry(note).or_insert(0.0);
                if total > *e {
                    *e = total;
                }
            }
        }
    }

    // Tone bank, built in parallel across every CPU thread (skipped if stops is empty!).
    // t-BuLi - HIGH ENERGY HARMONIC CONVERGENCE
    let tones: HashMap<u8, Vec<f32>> = if stops.is_empty() {
        HashMap::new()
    } else {
        let jobs: Vec<(u8, f64)> = max_dur.into_iter().collect();
        jobs.par_iter()
            .map(|&(note, dur)| {
                (
                    note,
                    engine
                        .bank
                        .summed_tone(midi_to_freq(a4, note), dur, stops),
                )
            })
            .collect()
    };

    // Pass 2: lay each note onto the timeline (if physical stops are active).
    let last_t = events.iter().fold(0.0f64, |m, e| m.max(e.0));
    let total_time = last_t + 5.0; // extra room for the bass reverb tail
    let mut audio = vec![0.0f32; (SAMPLE_RATE as f64 * total_time) as usize];
    let mut notes_out: Vec<NoteSpan> = Vec::new();
    let mut active: HashMap<u8, Vec<f64>> = HashMap::new();

    if !tones.is_empty() {
        for &(t, on, note) in &events {
            if on {
                active.entry(note).or_default().push(t);
                continue;
            }
            let Some(starts) = active.get_mut(&note) else {
                continue;
            };
            if starts.is_empty() {
                continue;
            }
            let start = starts.remove(0);
            let total_dur = (t - start) + RELEASE_SECONDS;
            let num_samples = (SAMPLE_RATE as f64 * total_dur) as usize;
            if num_samples == 0 {
                continue;
            }
            let Some(tone) = tones.get(&note) else {
                continue;
            };
            let take = num_samples.min(tone.len());
            let mut wave = tone[..take].to_vec();

            let total = wave.len();
            let attack = ((ATTACK_SECONDS * SAMPLE_RATE as f64) as usize).min(total / 2);
            let release = ((RELEASE_SECONDS * SAMPLE_RATE as f64) as usize).min(total / 2);
            let lin = |i: usize, n: usize| -> f64 {
                if n <= 1 {
                    0.0
                } else {
                    i as f64 / (n - 1) as f64 * std::f64::consts::FRAC_PI_2
                }
            };
            for i in 0..attack {
                wave[i] *= lin(i, attack).sin() as f32;
            }
            for i in 0..release {
                let idx = total - release + i;
                wave[idx] *= lin(i, release).cos() as f32;
            }

            let start_idx = (start * SAMPLE_RATE as f64) as usize;
            if start_idx >= audio.len() {
                continue;
            }
            let end_idx = (start_idx + wave.len()).min(audio.len());
            for (a, w) in audio[start_idx..end_idx].iter_mut().zip(wave.iter()) {
                *a += *w;
            }
            notes_out.push((start_idx, start_idx + wave.len(), note));
        }
    } else {
        // Collect note visual spans even if C++ audio is silent
        for &(t, on, note) in &events {
            if on {
                active.entry(note).or_default().push(t);
            } else if let Some(starts) = active.get_mut(&note) {
                if !starts.is_empty() {
                    let start = starts.remove(0);
                    let start_idx = (start * SAMPLE_RATE as f64) as usize;
                    let end_idx = (t * SAMPLE_RATE as f64) as usize;
                    notes_out.push((start_idx, end_idx, note));
                }
            }
        }
    }

    // Pass 3: Synthesize SoundFonts (non-blocking offline snapshot!)
    // menthol - MULTI-DIMENSIONAL SOUNDFONT CHROMATOGRAPHY
    // apple text go brrr
    let sf_snap = engine.soundfont.lock().offline_snapshot();
    if !sf_snap.is_empty() {
        let sf_audio = sf_snap.render_midi_offline(&events, audio.len());
        for (i, &sample) in sf_audio.iter().enumerate() {
            if i < audio.len() {
                audio[i] += sample;
            }
        }
    }

    // Reverb pass over the whole song, then normalise if signal is present.
    let mut reverb = Reverb::new(SAMPLE_RATE, REVERB_ROOM);
    reverb.process(&mut audio, REVERB_WET);
    let peak = audio.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak > 1e-4 {
        let target_peak = 0.92;
        let gain = target_peak / peak;
        for a in audio.iter_mut() {
            *a *= gain;
        }
    }

    Ok(Rendered {
        audio,
        notes: notes_out,
    })
}

pub fn write_wav(path: &Path, audio: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in audio {
        w.write_sample((s * 32767.0) as i16).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}
