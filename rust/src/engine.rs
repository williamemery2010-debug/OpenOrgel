//! CORE ENGINE - tone bank, live voices, MIDI playback state and the mixer.
//!
//! Direct descendant of the Python host (tone cache, `audio_callback`,
//! `_generate_audio_buffer`) but with 100% less GIL and 100% more borrow checker.

use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crate::dsp::{self, AcousticVenue, Reverb};

pub const SAMPLE_RATE: u32 = 44_100;
/// Standard cached tone length. Anything shorter is sliced out of this.
pub const TONE_SECONDS: f64 = 3.5;
pub const ATTACK_SECONDS: f64 = 0.12;
pub const RELEASE_SECONDS: f64 = 0.1;
pub const LIVE_NOTE_LOW: u8 = 36;
pub const LIVE_NOTE_HIGH: u8 = 96;
#[allow(dead_code)]
pub const REVERB_ROOM: f32 = 0.81;
#[allow(dead_code)]
pub const REVERB_WET: f32 = 0.35;
/// Live output is scaled before the tanh soft limiter to dodge polyphonic crunch.
const LIVE_PRE_LIMIT_GAIN: f32 = 0.35;
/// Seconds the reverb is kept running after the last live voice dies.
const REVERB_TAIL_SECONDS: usize = 6;

/// (start sample, end sample, midi note)
pub type NoteSpan = (usize, usize, u8);

use crate::temperament::Temperament;

#[allow(dead_code)]
pub fn midi_to_freq(a4: f64, note: u8, temperament: Temperament) -> f64 {
    temperament.midi_to_freq(a4, note)
}

// ---------------------------------------------------------------------------
// TONE BANK: per-stop tone cache (the RAM half of the dual-layer cache matrix)
// ---------------------------------------------------------------------------

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct ToneKey {
    freq_centi: i64,
    samples: usize,
    stop: i32, // -1 = the default sine, for when no stops are drawn
    chiff_centi: i64,
}

#[derive(Default)]
pub struct ToneBank {
    map: RwLock<HashMap<ToneKey, Arc<Vec<f32>>>>,
}

impl ToneBank {
    /// One stop (or the default sine) at a frequency. Returned buffer is at least
    /// `duration` long; callers slice what they need.
    pub fn stop_tone(
        &self,
        freq: f64,
        duration: f64,
        stop: Option<i32>,
        chiff_vol: f32,
        num_stops: usize,
    ) -> Arc<Vec<f32>> {
        let cache_duration = if duration <= TONE_SECONDS {
            TONE_SECONDS
        } else {
            duration
        };
        let key = ToneKey {
            freq_centi: (freq * 100.0).round() as i64,
            samples: (SAMPLE_RATE as f64 * cache_duration) as usize,
            stop: stop.unwrap_or(-1),
            chiff_centi: (chiff_vol * 100.0).round() as i64,
        };
        if let Some(hit) = self.map.read().get(&key) {
            return hit.clone();
        }
        let ids: Vec<i32> = stop.into_iter().collect();
        // Energy normalization: balances transient attack across active ranks
        let norm = 1.0 / (num_stops.max(1) as f32).sqrt();
        let eff_chiff = (chiff_vol * norm).clamp(0.0, 1.5);
        let data = Arc::new(dsp::generate_tone(freq, cache_duration, SAMPLE_RATE, &ids, eff_chiff));
        self.map.write().insert(key, data.clone());
        data
    }

    pub fn clear(&self) {
        self.map.write().clear();
    }

    /// Sum of every drawn stop, trimmed to exactly `duration` seconds (offline render).
    /// Automatically applies energy normalization so adding stops does not blow out headroom or transients.
    pub fn summed_tone(&self, freq: f64, duration: f64, stops: &[i32], chiff_vol: f32) -> Vec<f32> {
        let n = (SAMPLE_RATE as f64 * duration) as usize;
        let mut acc = vec![0.0f32; n];
        if stops.is_empty() {
            // No physical stops drawn: return absolute silence so SoundFonts sound unadulterated!
            return acc;
        }
        let stop_norm = 1.0 / (stops.len() as f32).sqrt().max(1.0);
        let mut add = |tone: &[f32], w: f32| {
            for (a, b) in acc.iter_mut().zip(tone.iter()) {
                *a += *b * stop_norm * w;
            }
        };
        for &s in stops {
            let w = dsp::stops_dsp::stop_registration_weight(s);
            add(&self.stop_tone(freq, duration, Some(s), chiff_vol, stops.len()), w);
        }
        acc
    }

    /// Live voice tone: all stops at the standard 3.5 s length, averaged so that
    /// pulling more stops does not blow the headroom.
    pub fn live_tone(&self, freq: f64, stops: &[i32], chiff_vol: f32) -> Vec<f32> {
        let n = (SAMPLE_RATE as f64 * TONE_SECONDS) as usize;
        let mut acc = vec![0.0f32; n];
        if stops.is_empty() {
            // No physical stops drawn: return silence for live keys
            return acc;
        }
        let count = stops.len().max(1) as f32;
        let mut add = |tone: &[f32], w: f32| {
            for (a, b) in acc.iter_mut().zip(tone.iter()) {
                *a += *b * w;
            }
        };
        for &s in stops {
            let w = dsp::stops_dsp::stop_registration_weight(s);
            add(&self.stop_tone(freq, TONE_SECONDS, Some(s), chiff_vol, stops.len()), w);
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

/// Half-width (samples) of the correlation window used to pick loop points.
pub const LOOP_MATCH_HALF_WINDOW: usize = 96;
/// Length (samples) of the equal-power blend placed just before the loop end.
pub const LOOP_XFADE: usize = 64;
/// Length (samples) of the anti-click fade applied when playback stops, pauses or resumes.
pub const PLAYBACK_FADE_SAMPLES: usize = 2205; // 50 ms @ 44.1 kHz

/// Finds a seamless sustain loop inside a rendered tone buffer.
///
/// The loop start is fixed at 4/7 of the buffer (2.0 s of a 3.5 s tone). The loop end is
/// the candidate sample whose surrounding window (+/- `LOOP_MATCH_HALF_WINDOW` samples)
/// has the smallest squared error against the window around the loop start. Because
/// the stop tones are periodic, the minimum lands on a whole number of pitch periods,
/// so jumping from `end` back to `start` is continuous in value and in slope.
/// Returns `(start, end)` with `start < end <= wave.len()`.
pub fn find_loop_points(wave: &[f32]) -> (usize, usize) {
    let len = wave.len();
    let w = LOOP_MATCH_HALF_WINDOW;
    let start = len * 4 / 7;
    let e_lo = start + len / 4;
    let e_hi = len.saturating_sub(w + 1);
    if len < 8 * w || start < w || e_lo >= e_hi {
        return (0, len);
    }
    let reference = &wave[start - w..=start + w];
    let mut best_cost = f32::MAX;
    let mut best_end = e_hi;
    for e in e_lo..=e_hi {
        let candidate = &wave[e - w..=e + w];
        let mut cost = 0.0f32;
        for (a, b) in reference.iter().zip(candidate.iter()) {
            let d = a - b;
            cost += d * d;
            if cost >= best_cost {
                break;
            }
        }
        if cost < best_cost {
            best_cost = cost;
            best_end = e;
        }
    }
    (start, best_end)
}

/// A cached live tone together with its matched sustain loop.
pub struct LiveTone {
    pub data: Vec<f32>,
    pub loop_start: usize,
    pub loop_end: usize,
}

impl LiveTone {
    pub fn new(data: Vec<f32>) -> Self {
        let (loop_start, loop_end) = find_loop_points(&data);
        LiveTone {
            data,
            loop_start,
            loop_end,
        }
    }
}

struct Voice {
    tone: Arc<LiveTone>,
    /// Read position inside the tone buffer (wraps through the sustain loop).
    pos: usize,
    /// Samples since key-down; drives the attack envelope independently of `pos`.
    age: usize,
    released: bool,
    /// Samples since key-up; drives the release envelope.
    rel_count: usize,
}

impl Voice {
    fn new(tone: Arc<LiveTone>) -> Self {
        Voice {
            tone,
            pos: 0,
            age: 0,
            released: false,
            rel_count: 0,
        }
    }

    fn release(&mut self) {
        if !self.released {
            self.released = true;
            self.rel_count = 0;
        }
    }
}

/// Sounding voices. `ghosts` holds voices that were retriggered while still
/// sounding: they finish their release instead of being cut off mid-waveform.
#[derive(Default)]
struct VoiceBank {
    active: HashMap<u8, Voice>,
    ghosts: Vec<Voice>,
}

/// Renders one voice into `live`. Returns true once the voice has fully decayed.
///
/// The output is `tone * attack(age) * release(rel_count)`. Both envelopes are
/// evaluated on every sample, so a key released during its attack starts its
/// release from the current (partial) amplitude rather than from full level.
fn render_voice(
    v: &mut Voice,
    live: &mut [f32],
    attack_tbl: &[f32],
    release_tbl: &[f32],
    xfade_tbl: &[f32],
) -> bool {
    let tone = v.tone.clone();
    let wave = &tone.data[..];
    let (ls, le) = (tone.loop_start, tone.loop_end);
    let loop_len = le - ls;
    let rel_n = release_tbl.len();
    let xf = if ls >= xfade_tbl.len() && loop_len >= 2 * xfade_tbl.len() {
        xfade_tbl.len()
    } else {
        0
    };

    for l in live.iter_mut() {
        if v.released && v.rel_count >= rel_n {
            return true;
        }
        let mut s = wave[v.pos];
        if xf > 0 && v.pos + xf >= le {
            // Blend the last `xf` samples with the matching samples one loop earlier,
            // so the jump back to `ls` is exactly continuous.
            let w = xfade_tbl[v.pos + xf - le];
            s = (1.0 - w) * s + w * wave[v.pos - loop_len];
        }
        if v.age < attack_tbl.len() {
            s *= attack_tbl[v.age];
        }
        if v.released {
            s *= release_tbl[v.rel_count];
            v.rel_count += 1;
        }
        *l += s;
        v.age = v.age.saturating_add(1);
        v.pos += 1;
        if v.pos >= le {
            v.pos = ls;
        }
    }
    v.released && v.rel_count >= rel_n
}

/// Remaining samples of a song that was interrupted by stop, pause or silence.
/// The mixer plays these out under a short fade so the cut never lands mid-waveform.
pub struct FadeTail {
    audio: Arc<Vec<f32>>,
    idx: usize,
    done: usize,
}

#[derive(Default)]
pub struct PlaybackState {
    pub audio: Option<Arc<Vec<f32>>>,
    pub idx: usize,
    pub playing: bool,
    pub paused: bool,
    pub notes: Arc<Vec<NoteSpan>>,
    pub starts: Arc<Vec<usize>>,
    pub fade_tail: Option<FadeTail>,
    /// Samples of fade-in still to apply (set on resume).
    pub fade_in_left: usize,
}

impl PlaybackState {
    /// Captures the audible audio at the current position as a fade-out tail.
    fn begin_fade_out(&mut self) {
        if self.playing && !self.paused && self.fade_tail.is_none() {
            if let Some(a) = self.audio.as_ref() {
                self.fade_tail = Some(FadeTail {
                    audio: a.clone(),
                    idx: self.idx,
                    done: 0,
                });
            }
        }
    }
}

pub struct Rendered {
    pub audio: Vec<f32>,
    pub notes: Vec<NoteSpan>,
}

pub struct Engine {
    pub bank: ToneBank,
    live_tones: RwLock<HashMap<u8, Arc<LiveTone>>>,
    voices: Mutex<VoiceBank>,
    pub playback: Mutex<PlaybackState>,
    stops: Mutex<Vec<i32>>,
    a4: Mutex<f64>,
    temperament: Mutex<Temperament>,
    venue: Mutex<AcousticVenue>,
    volume: AtomicU32,
    chiff_volume: AtomicU32,
    generation: AtomicU64,
    rendering: AtomicBool,
    render_requested: AtomicBool,
    current_midi: Mutex<Option<PathBuf>>,
    /// Set by the audio thread when a song runs off its own end.
    pub song_finished: AtomicBool,
    pub soundfont: Arc<Mutex<crate::soundfont::SoundFontManager>>,
}

impl Engine {
    pub fn new() -> Arc<Engine> {
        Arc::new(Engine {
            bank: ToneBank::default(),
            live_tones: RwLock::new(HashMap::new()),
            voices: Mutex::new(VoiceBank::default()),
            playback: Mutex::new(PlaybackState::default()),
            stops: Mutex::new(Vec::new()),
            a4: Mutex::new(440.0),
            temperament: Mutex::new(Temperament::Equal),
            venue: Mutex::new(AcousticVenue::HamerHall),
            volume: AtomicU32::new(1.0f32.to_bits()),
            chiff_volume: AtomicU32::new(0.20f32.to_bits()),
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

    pub fn set_chiff_volume(&self, v: f32) {
        self.chiff_volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn chiff_volume(&self) -> f32 {
        f32::from_bits(self.chiff_volume.load(Ordering::Relaxed))
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

    pub fn set_temperament(&self, t: Temperament) {
        *self.temperament.lock() = t;
    }

    pub fn temperament(&self) -> Temperament {
        *self.temperament.lock()
    }

    pub fn set_venue(&self, v: AcousticVenue) {
        *self.venue.lock() = v;
    }

    pub fn venue(&self) -> AcousticVenue {
        *self.venue.lock()
    }

    /// Stops or tuning changed: throw away stale live tones, rebuild in the
    /// background and, if a song is playing, re-render it with the new voices.
    pub fn refresh(self: &Arc<Self>) {
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.live_tones.write().clear();

        let me = self.clone();
        let stops = self.stops();
        let a4 = self.a4();
        let temp = self.temperament();
        let chiff = self.chiff_volume();
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
                    let tone = Arc::new(LiveTone::new(me.bank.live_tone(temp.midi_to_freq(a4, note), &stops, chiff)));
                    let mut w = me.live_tones.write();
                    if me.generation.load(Ordering::SeqCst) == gen {
                        w.insert(note, tone);
                    }
                });
        });

        self.request_rerender();
    }

    pub fn clear_caches(self: &Arc<Self>) {
        dsp::clear_engine_cache();
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
                    let temp = self.temperament();
                    let chiff = self.chiff_volume();
                    let t = Arc::new(LiveTone::new(self.bank.live_tone(temp.midi_to_freq(a4, note), &stops, chiff)));
                    let mut w = self.live_tones.write();
                    if self.generation.load(Ordering::SeqCst) == gen {
                        w.insert(note, t.clone());
                    }
                    t
                }
            };
            let mut bank = self.voices.lock();
            // A key struck again while its previous voice is still sounding: let the old
            // voice finish its release rather than cutting it off mid-waveform.
            if let Some(mut old) = bank.active.remove(&note) {
                old.release();
                bank.ghosts.push(old);
            }
            bank.active.insert(note, Voice::new(tone));
        }
        self.soundfont.lock().note_on(note, 100);
    }

    pub fn note_off(&self, note: u8) {
        if let Some(v) = self.voices.lock().active.get_mut(&note) {
            v.release();
        }
        self.soundfont.lock().note_off(note);
    }

    /// Releases every live voice through its normal release envelope (no hard cut).
    pub fn all_notes_off(&self) {
        for v in self.voices.lock().active.values_mut() {
            v.release();
        }
        self.soundfont.lock().all_notes_off();
    }

    // ----- playback control -----

    pub fn start_playback(&self, rendered: Rendered, path: PathBuf) {
        let mut notes = rendered.notes;
        notes.sort_by_key(|n| n.0);
        let starts: Vec<usize> = notes.iter().map(|n| n.0).collect();
        *self.current_midi.lock() = Some(path);
        let mut pb = self.playback.lock();
        pb.begin_fade_out(); // a song already playing fades out under the new one
        pb.audio = Some(Arc::new(rendered.audio));
        pb.notes = Arc::new(notes);
        pb.starts = Arc::new(starts);
        pb.idx = 0;
        pb.playing = true;
        pb.paused = false;
        pb.fade_in_left = 0;
        self.song_finished.store(false, Ordering::SeqCst);
    }

    pub fn stop_playback(&self) {
        let mut pb = self.playback.lock();
        pb.begin_fade_out();
        pb.playing = false;
        pb.paused = false;
        pb.idx = 0;
        *self.current_midi.lock() = None;
    }

    pub fn pause(&self) {
        let mut pb = self.playback.lock();
        if pb.playing {
            pb.begin_fade_out();
            pb.paused = true;
        }
    }

    pub fn resume(&self) {
        let mut pb = self.playback.lock();
        if pb.playing {
            if pb.paused {
                pb.fade_tail = None;
                pb.fade_in_left = PLAYBACK_FADE_SAMPLES;
            }
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

    /// Silences the current song playback buffer when all stops are pushed in.
    /// The audible part is faded out first so the cut is not a click.
    pub fn silence_playback(&self) {
        let mut pb = self.playback.lock();
        pb.begin_fade_out();
        if let Some(audio) = pb.audio.as_ref() {
            pb.audio = Some(Arc::new(vec![0.0; audio.len()]));
        }
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
                let sf_active = me.soundfont.lock().active_preset_count();
                if stops.is_empty() && sf_active == 0 {
                    me.silence_playback();
                } else {
                    let a4 = me.a4();
                    let temp = me.temperament();
                    match render_midi(&me, &p, &stops, a4, temp) {
                        Ok(r) => me.swap_audio(r),
                        Err(e) => eprintln!("Background render failed: {e}"),
                    }
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
        for (&n, _) in self.voices.lock().active.iter() {
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
    /// Rising equal-power weights for the loop-end blend.
    xfade_tbl: Vec<f32>,
    /// 1 -> 0 quarter-cosine used to fade a stopped/paused song out.
    fade_out_tbl: Vec<f32>,
    /// 0 -> 1 quarter-sine used to fade a resumed song in.
    fade_in_tbl: Vec<f32>,
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
        let xfade_tbl = (0..LOOP_XFADE)
            .map(|k| {
                let t = (k + 1) as f64 / LOOP_XFADE as f64;
                (t * std::f64::consts::FRAC_PI_2).sin().powi(2) as f32
            })
            .collect();
        let fade_out_tbl = (0..PLAYBACK_FADE_SAMPLES)
            .map(|i| (i as f64 * std::f64::consts::FRAC_PI_2 / PLAYBACK_FADE_SAMPLES as f64).cos() as f32)
            .collect();
        let fade_in_tbl = (0..PLAYBACK_FADE_SAMPLES)
            .map(|i| (i as f64 * std::f64::consts::FRAC_PI_2 / PLAYBACK_FADE_SAMPLES as f64).sin() as f32)
            .collect();
        Mixer {
            reverb: Reverb::for_venue(SAMPLE_RATE, engine.venue()),
            engine,
            live: vec![0.0; 4096],
            attack_tbl,
            release_tbl,
            xfade_tbl,
            fade_out_tbl,
            fade_in_tbl,
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
                fade_tail,
                fade_in_left,
                ..
            } = &mut *pb;
            if *playing && !*paused {
                if let Some(audio) = audio.as_ref() {
                    let remaining = audio.len().saturating_sub(*idx);
                    let n = remaining.min(frames);
                    for i in 0..n {
                        let mut g = vol;
                        if *fade_in_left > 0 {
                            g *= self.fade_in_tbl[PLAYBACK_FADE_SAMPLES - *fade_in_left];
                            *fade_in_left -= 1;
                        }
                        out[i] = audio[*idx + i] * g;
                    }
                    *idx += n;
                    if n < frames {
                        *playing = false;
                        self.engine.song_finished.store(true, Ordering::SeqCst);
                    }
                }
            }
            // Song interrupted by stop/pause: play the remaining samples under a fade-out
            // so the waveform reaches zero instead of being chopped (audible click).
            let mut tail_finished = false;
            if let Some(tail) = fade_tail.as_mut() {
                for o in out.iter_mut() {
                    if tail.done >= self.fade_out_tbl.len() || tail.idx >= tail.audio.len() {
                        break;
                    }
                    *o += tail.audio[tail.idx] * self.fade_out_tbl[tail.done] * vol;
                    tail.idx += 1;
                    tail.done += 1;
                }
                tail_finished = tail.done >= self.fade_out_tbl.len() || tail.idx >= tail.audio.len();
            }
            if tail_finished {
                *fade_tail = None;
            }
        }

        // --- live organ voices ---
        if self.live.len() < frames {
            self.live.resize(frames, 0.0);
        }
        let live = &mut self.live[..frames];
        live.fill(0.0);

        let mut any = false;
        {
            let mut bank = self.engine.voices.lock();
            if !bank.active.is_empty() || !bank.ghosts.is_empty() {
                any = true;
                self.done.clear();
                for (&note, v) in bank.active.iter_mut() {
                    if render_voice(v, live, &self.attack_tbl, &self.release_tbl, &self.xfade_tbl) {
                        self.done.push(note);
                    }
                }
                for n in &self.done {
                    bank.active.remove(n);
                }
                bank.ghosts.retain_mut(|v| {
                    !render_voice(v, live, &self.attack_tbl, &self.release_tbl, &self.xfade_tbl)
                });
            }
        }

        // --- soundfont rendered PCM (rendered directly into live bus for unified acoustics & matched volume) ---
        let sf_active = self.engine.soundfont.lock().render_into_live(live);
        if sf_active {
            any = true;
        }

        // Dynamically update physical venue if changed
        let current_venue = self.engine.venue();
        if self.reverb.venue() != current_venue {
            self.reverb.set_venue(current_venue);
        }

        if any {
            self.tail_left = REVERB_TAIL_SECONDS * SAMPLE_RATE as usize;
        }
        if self.tail_left > 0 {
            let wet = self.reverb.wet_mix();
            self.reverb.process(live, wet);
            self.tail_left = self.tail_left.saturating_sub(frames);
            for i in 0..frames {
                out[i] += (live[i] * LIVE_PRE_LIMIT_GAIN).tanh() * vol;
            }
        }
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
    temperament: Temperament,
) -> Result<Rendered, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read MIDI file: {e}"))?;
    let events = collect_events(&bytes)?;
    if events.is_empty() {
        return Err("That MIDI file has no notes in it.".to_string());
    }

    let sf_snap = engine.soundfont.lock().offline_snapshot();
    if stops.is_empty() && sf_snap.is_empty() {
        return Err("No selected stops! Please draw at least one stop on Principal Organ or Page 2 to play.".to_string());
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
    let tones: HashMap<u8, Vec<f32>> = if stops.is_empty() {
        HashMap::new()
    } else {
        let chiff = engine.chiff_volume();
        let jobs: Vec<(u8, f64)> = max_dur.into_iter().collect();
        jobs.par_iter()
            .map(|&(note, dur)| {
                (
                    note,
                    engine
                        .bank
                        .summed_tone(temperament.midi_to_freq(a4, note), dur, stops, chiff),
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

    // Pass 3: Synthesize SoundFonts (non-blocking offline snapshot)
    if !sf_snap.is_empty() {
        let sf_audio = sf_snap.render_midi_offline(&events, audio.len());
        for (i, &sample) in sf_audio.iter().enumerate() {
            if i < audio.len() {
                audio[i] += sample;
            }
        }
    }

    // Physically modeled venue reverberation pass over the song
    let mut reverb = Reverb::for_venue(SAMPLE_RATE, engine.venue());
    let wet = reverb.wet_mix();
    reverb.process(&mut audio, wet);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope_tables() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let attack_n = (ATTACK_SECONDS * SAMPLE_RATE as f64) as usize;
        let release_n = (RELEASE_SECONDS * SAMPLE_RATE as f64) as usize;
        let attack = (0..attack_n)
            .map(|i| (i as f64 * std::f64::consts::FRAC_PI_2 / attack_n as f64).sin() as f32)
            .collect();
        let release = (0..release_n)
            .map(|i| (i as f64 * std::f64::consts::FRAC_PI_2 / release_n as f64).cos() as f32)
            .collect();
        let xfade = (0..LOOP_XFADE)
            .map(|k| {
                let t = (k + 1) as f64 / LOOP_XFADE as f64;
                (t * std::f64::consts::FRAC_PI_2).sin().powi(2) as f32
            })
            .collect();
        (attack, release, xfade)
    }

    fn max_step(x: &[f32]) -> f32 {
        x.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()))
    }

    #[test]
    fn test_loop_points_are_seamless_for_periodic_tone() {
        let n = (SAMPLE_RATE as f64 * TONE_SECONDS) as usize;
        let tone: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / SAMPLE_RATE as f64).sin() as f32)
            .collect();
        let (start, end) = find_loop_points(&tone);
        assert_eq!(start, n * 4 / 7);
        assert!(end > start + n / 4, "loop must be long enough to avoid audible repetition");
        assert!(end + LOOP_MATCH_HALF_WINDOW < n);
        // Jumping from `end` back to `start` must be continuous in value and in slope.
        let value_gap = (tone[end] - tone[start]).abs();
        let slope_gap = ((tone[end] - tone[end - 1]) - (tone[start] - tone[start - 1])).abs();
        assert!(value_gap < 0.005, "value discontinuity {value_gap}");
        assert!(slope_gap < 0.005, "slope discontinuity {slope_gap}");
    }

    #[test]
    fn test_sustained_voice_has_no_click_at_loop_wrap() {
        let n = (SAMPLE_RATE as f64 * TONE_SECONDS) as usize;
        let tone: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / SAMPLE_RATE as f64).sin() as f32)
            .collect();
        let (attack, release, xfade) = envelope_tables();
        let mut voice = Voice::new(Arc::new(LiveTone::new(tone)));
        let mut out = vec![0.0f32; 300_000]; // ~6.8 s: crosses the loop wrap more than once
        for chunk in out.chunks_mut(256) {
            assert!(!render_voice(&mut voice, chunk, &attack, &release, &xfade));
        }
        // A 440 Hz unit sine never steps more than 2*pi*440/44100 = 0.0627 per sample.
        let step = max_step(&out[attack.len() + 10..]);
        assert!(step < 0.075, "loop wrap produced a step of {step}");
    }

    #[test]
    fn test_release_during_attack_starts_from_current_amplitude() {
        let n = (SAMPLE_RATE as f64 * TONE_SECONDS) as usize;
        let (attack, release, xfade) = envelope_tables();
        let mut voice = Voice::new(Arc::new(LiveTone::new(vec![1.0f32; n])));
        let mut out = vec![0.0f32; 441]; // 10 ms: well inside the 120 ms attack
        render_voice(&mut voice, &mut out, &attack, &release, &xfade);
        let level_at_release = *out.last().unwrap();
        assert!(level_at_release < 0.2);

        voice.release();
        let mut tail = vec![0.0f32; release.len() + 64];
        let finished = render_voice(&mut voice, &mut tail, &attack, &release, &xfade);
        assert!(finished, "voice must report completion after its release");
        assert!((tail[0] - level_at_release).abs() < 0.01, "release must not jump to full level");
        assert!(max_step(&tail) < 0.01, "release produced a step of {}", max_step(&tail));
        assert_eq!(*tail.last().unwrap(), 0.0);
    }

    fn running_mixer() -> (Arc<Engine>, Mixer) {
        let engine = Engine::new();
        let mixer = Mixer::new(engine.clone());
        (engine, mixer)
    }

    fn render_samples(mixer: &mut Mixer, count: usize) -> Vec<f32> {
        let mut all = Vec::with_capacity(count);
        let mut block = vec![0.0f32; 256];
        while all.len() < count {
            mixer.render(&mut block);
            all.extend_from_slice(&block);
        }
        all.truncate(count);
        all
    }

    #[test]
    fn test_stop_playback_fades_to_silence_without_a_step() {
        let (engine, mut mixer) = running_mixer();
        let audio = vec![0.5f32; SAMPLE_RATE as usize * 2];
        engine.start_playback(Rendered { audio, notes: Vec::new() }, PathBuf::from("test.mid"));
        let warm = render_samples(&mut mixer, 4096);
        assert!((warm[4095] - 0.5).abs() < 1e-6);

        engine.stop_playback();
        let after = render_samples(&mut mixer, 4608);
        assert!(after[0] > 0.499, "fade must start from the current level");
        assert!(max_step(&after) < 0.002, "stop produced a step of {}", max_step(&after));
        assert!(after[..PLAYBACK_FADE_SAMPLES].windows(2).all(|w| w[1] <= w[0] + 1e-6));
        assert!(after[PLAYBACK_FADE_SAMPLES + 1..].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn test_pause_and_resume_ramp_without_a_step() {
        let (engine, mut mixer) = running_mixer();
        let audio = vec![0.5f32; SAMPLE_RATE as usize * 2];
        engine.start_playback(Rendered { audio, notes: Vec::new() }, PathBuf::from("test.mid"));
        render_samples(&mut mixer, 4096);

        engine.pause();
        let paused = render_samples(&mut mixer, 4608);
        assert!(max_step(&paused) < 0.002);
        assert_eq!(*paused.last().unwrap(), 0.0);

        engine.resume();
        let resumed = render_samples(&mut mixer, 4608);
        assert!(resumed[0] < 0.01, "resume must start near zero, got {}", resumed[0]);
        assert!(max_step(&resumed) < 0.002);
        assert!((resumed[4607] - 0.5).abs() < 1e-6);
    }
}
