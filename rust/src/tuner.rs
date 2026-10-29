//! REAL-TIME MICROPHONE CHROMATIC PITCH DETECTOR & BLOWER PRESSURE PID CONTROLLER
//!
//! Captures live audio from the laptop's microphone via `cpal`, estimates the fundamental
//! pipe frequency `f0` using the McLeod Pitch Method (Normalized Square Difference Function,
//! NSDF) with 3-point parabolic sub-sample interpolation, maps the pitch against the active
//! Master Tuning (`A4`) and Historical Temperament (`Equal`, `Werckmeister III`, `Meantone`, etc.),
//! and regulates the centrifugal organ blower's MOSFET PWM duty cycle (Arduino Pin 3, with
//! ON-ON switch on Pin 8) starting from 50.0% nominal power in 0.5% steps (or PID mode)
//! with a 500 ms anti-oscillation settling delay.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SizedSample, StreamConfig};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::temperament::Temperament;

/// MIDI note number for E-flat 5 (`Eb5` / `D#5`), the primary reference tuning pipe
/// specified for regulating the centrifugal organ blower's windchest pressure.
/// In 12-TET at `A4 = 440 Hz`, `Eb5` (MIDI 75) has a nominal frequency of `622.25397 Hz`.
pub const EB5_MIDI_NOTE: u8 = 75;

/// Nominal starting PWM duty cycle (%) for the centrifugal blower motor controller on Pin 3.
pub const NOMINAL_PWM_PERCENT: f32 = 50.0;

/// Single-step PWM adjustment increment (%) applied when the pipe is Flat (`+0.5%`) or Sharp (`-0.5%`).
pub const PWM_STEP_PERCENT: f32 = 0.5;

/// Default anti-oscillation delay (`500 ms`) between consecutive blower PWM adjustments.
/// Prevents closed-loop hunting caused by the physical inertia of the centrifugal blower
/// impeller and windchest reservoir.
pub const DEFAULT_ADJUSTMENT_DELAY_MS: u64 = 500;

/// Deadband tolerance in cents (`±3.5 cents`). When the detected pitch error is within
/// `[-3.5, +3.5]` cents of the target frequency, the pipe is considered in tune and no
/// PWM adjustment is sent.
pub const IN_TUNE_DEADBAND_CENTS: f64 = 3.5;

/// Chromatic pitch class names using flat/sharp organ notation (with `Eb` explicitly labeled).
pub const CHROMATIC_NOTE_NAMES: [&str; 12] = [
    "C", "C#/Db", "D", "Eb (D#)", "E", "F", "F#/Gb", "G", "G#/Ab", "A", "Bb (A#)", "B",
];

/// Operating mode for the chromatic pitch detector and blower regulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunerMode {
    /// Locks specifically onto the `Eb5` reference pipe (`MIDI 75`, with optional octave folding
    /// so an 8', 4', or 16' `Eb` pipe at `Eb3`, `Eb4`, `Eb5`, or `Eb6` is also recognized as `Eb`).
    TargetEb5,
    /// Full chromatic mode: automatically detects the nearest semitone (`C0..B7`) in the active
    /// Master Tuning (`A4`) and Historical Temperament, allowing any pipe on the windchest to
    /// be tuned or used as the blower pressure reference.
    ChromaticAuto,
}

impl TunerMode {
    /// Human-readable label for the GUI selector.
    pub fn label(self) -> &'static str {
        match self {
            TunerMode::TargetEb5 => "Target Pipe: Eb5 (D#5)",
            TunerMode::ChromaticAuto => "Chromatic (Any Pipe Note)",
        }
    }
}

/// Direction of the most recent blower PWM correction command sent to the Arduino.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlowerAdjustment {
    /// No signal above the microphone noise gate.
    WaitingForSignal,
    /// Pipe pitch is within `±IN_TUNE_DEADBAND_CENTS` of the target frequency; hold current PWM.
    InTuneHold,
    /// Pipe pitch is Flat (`< -deadband`): blower PWM increased by `+0.5%` (Serial command `'F'`).
    FlatIncreasePwm,
    /// Pipe pitch is Sharp (`> +deadband`): blower PWM decreased by `-0.5%` (Serial command `'S'`).
    SharpDecreasePwm,
}

/// Snapshot of the real-time pitch detector and blower controller state for GUI rendering.
#[derive(Clone, Debug)]
pub struct TunerStatus {
    /// Whether a clear periodic pipe tone is currently detected above the RMS noise gate.
    pub signal_detected: bool,
    /// RMS amplitude of the incoming microphone signal (`0.0` to `1.0`).
    pub rms_level: f32,
    /// Detected fundamental frequency `f0` in Hz (`0.0` when silent).
    pub detected_freq_hz: f64,
    /// Target reference frequency in Hz for the matched note under the current `A4` & `Temperament`.
    pub target_freq_hz: f64,
    /// MIDI note number of the matched target note (e.g. `75` for `Eb5`).
    pub matched_midi_note: u8,
    /// Formatted note name including octave (e.g. `"Eb5 (D#5)"`).
    pub matched_note_name: String,
    /// Pitch deviation in cents: `1200 * log2(detected_freq_hz / target_freq_hz)`.
    /// Negative = Flat (needs more wind pressure / higher PWM); Positive = Sharp (needs less PWM).
    pub cents_error: f64,
    /// Normalized autocorrelation clarity/confidence (`0.0` to `1.0`) from the NSDF peak.
    pub confidence: f32,
    /// Current blower motor PWM duty cycle (`0.0%` to `100.0%`, starting at `50.0%` nominal).
    pub current_pwm_percent: f32,
    /// Live centrifugal blower motor current in Amperes reported by IBT-2 BTS7960 IS pin on Pin A0.
    pub motor_current_amps: f32,
    /// Most recent control action taken by the regulator.
    pub last_action: BlowerAdjustment,
    /// Hardware blower switch state reported by Arduino Pin 8 (`None` if not yet reported).
    pub hardware_switch_on: Option<bool>,
}

impl Default for TunerStatus {
    fn default() -> Self {
        TunerStatus {
            signal_detected: false,
            rms_level: 0.0,
            detected_freq_hz: 0.0,
            target_freq_hz: 622.25,
            matched_midi_note: EB5_MIDI_NOTE,
            matched_note_name: "Eb5 (D#5)".to_string(),
            cents_error: 0.0,
            confidence: 0.0,
            current_pwm_percent: NOMINAL_PWM_PERCENT,
            motor_current_amps: 0.0,
            last_action: BlowerAdjustment::WaitingForSignal,
            hardware_switch_on: None,
        }
    }
}

/// Formats a MIDI note number (`0..=127`) into a standard organ note string such as `"Eb5 (D#5)"`.
pub fn format_midi_note(midi_note: u8) -> String {
    let pc = (midi_note % 12) as usize;
    // MIDI note 60 is C4, so octave = (midi / 12) - 1
    let octave = (midi_note as i32 / 12) - 1;
    match pc {
        3 => format!("Eb{octave} (D#{octave})"),
        10 => format!("Bb{octave} (A#{octave})"),
        1 => format!("C#{octave}/Db{octave}"),
        6 => format!("F#{octave}/Gb{octave}"),
        8 => format!("G#{octave}/Ab{octave}"),
        _ => format!("{}{octave}", CHROMATIC_NOTE_NAMES[pc]),
    }
}

/// Estimates the fundamental frequency `f0` (in Hz) and harmonic clarity `(f0_hz, clarity)`
/// of a mono audio window using the **McLeod Pitch Method (NSDF)** with parabolic interpolation.
///
/// Why NSDF is used instead of simple zero-crossing or raw FFT peak picking:
/// - Organ pipes (especially flue and reed stops) often have a 2nd or 3rd harmonic that is
///   nearly as strong as the fundamental. Raw FFT peak picking can lock onto the wrong harmonic,
///   and zero-crossing counters jitter wildly when harmonics or wind noise are present.
/// - The Normalized Square Difference Function (NSDF):
///   `NSDF(tau) = 2 * r(tau) / (m(tau))`
///   where `r(tau) = sum_{n=0}^{W-1-tau} x[n] * x[n + tau]` and
///   `m(tau) = sum_{n=0}^{W-1-tau} (x[n]^2 + x[n + tau]^2)`
///   is strictly bounded in `[-1.0, +1.0]` and reaches a sharp peak near `+1.0` at the true
///   fundamental period `T0 = fs / f0`, even when upper harmonics are strong.
/// - Picking the **first** positive-region peak whose height exceeds `0.85 * max_peak`
///   guarantees that we lock onto the true fundamental `f0` rather than sub-harmonics (`f0 / 2`).
pub fn detect_pitch_nsdf(samples: &[f32], sample_rate: u32) -> Option<(f64, f32)> {
    let n = samples.len();
    if n < 512 || sample_rate == 0 {
        return None;
    }

    // 1. Compute signal mean (DC offset) and RMS energy to reject silence / background hum
    let mean = samples.iter().map(|&s| s as f64).sum::<f64>() / n as f64;
    let mut sum_sq = 0.0f64;
    // Remove DC offset and apply a nhẹ one-pole low-pass at ~2400 Hz to suppress high wind hiss
    let mut clean = Vec::with_capacity(n);
    let rc = 1.0 / (2.0 * std::f64::consts::PI * 2400.0);
    let dt = 1.0 / sample_rate as f64;
    let alpha = dt / (rc + dt);
    let mut lp_state = 0.0f64;

    for &s in samples {
        let centered = (s as f64) - mean;
        sum_sq += centered * centered;
        lp_state += alpha * (centered - lp_state);
        clean.push(lp_state);
    }

    let rms = (sum_sq / n as f64).sqrt();
    // Noise gate: ignore buffers quieter than -50 dBFS (RMS < 0.003)
    if rms < 0.003 {
        return None;
    }

    // 2. Define lag search bounds [tau_min, tau_max] corresponding to [55 Hz, 1800 Hz]
    //    This covers 8'/4'/2' organ pipes including Eb3 (155.6 Hz), Eb4 (311.1 Hz),
    //    Eb5 (622.25 Hz), and Eb6 (1244.5 Hz).
    let sr = sample_rate as f64;
    let tau_min = (sr / 1800.0).floor().max(4.0) as usize;
    let tau_max = ((sr / 55.0).ceil() as usize).min(n / 2);
    if tau_min >= tau_max {
        return None;
    }

    // 3. Evaluate NSDF(tau) across [0..=tau_max]
    //    We start from tau = 0 so we can detect the first zero-crossing after the lag-0 peak.
    let mut nsdf = vec![0.0f64; tau_max + 1];
    for tau in 1..=tau_max {
        let limit = n - tau;
        let mut acf = 0.0f64;
        let mut norm = 0.0f64;
        for i in 0..limit {
            let a = clean[i];
            let b = clean[i + tau];
            acf += a * b;
            norm += a * a + b * b;
        }
        if norm > 1e-12 {
            nsdf[tau] = (2.0 * acf) / norm;
        }
    }

    // 4. Advance past the initial lag-0 lobe until NSDF drops below 0.0
    let mut start_tau = tau_min;
    while start_tau < tau_max && nsdf[start_tau] > 0.0 {
        start_tau += 1;
    }
    if start_tau >= tau_max {
        return None;
    }

    // 5. Collect the highest local maximum inside each positive lobe of NSDF(tau)
    let mut peaks: Vec<(usize, f64)> = Vec::new();
    let mut in_positive_lobe = false;
    let mut best_tau = 0usize;
    let mut best_val = -1.0f64;

    for tau in start_tau..tau_max {
        let v = nsdf[tau];
        if v > 0.0 {
            if !in_positive_lobe {
                in_positive_lobe = true;
                best_tau = tau;
                best_val = v;
            } else if v > best_val {
                best_tau = tau;
                best_val = v;
            }
        } else if in_positive_lobe {
            peaks.push((best_tau, best_val));
            in_positive_lobe = false;
        }
    }
    if in_positive_lobe && best_val > 0.0 {
        peaks.push((best_tau, best_val));
    }

    if peaks.is_empty() {
        return None;
    }

    // 6. McLeod threshold selection: find the global maximum peak height `max_peak`,
    //    then select the FIRST peak that reaches at least `0.85 * max_peak`.
    //    This prevents sub-harmonic octave errors (where 2*T0 has a slightly higher numeric value).
    let max_peak = peaks.iter().fold(0.0f64, |m, &(_, v)| m.max(v));
    if max_peak < 0.55 {
        // Signal is aperiodic noise (e.g. pure wind hiss or room noise without a pipe tone)
        return None;
    }
    let threshold = 0.85 * max_peak;
    let &(chosen_tau, chosen_val) = peaks
        .iter()
        .find(|&&(_, v)| v >= threshold)
        .unwrap_or(&peaks[0]);

    // 7. Refine `chosen_tau` to sub-sample precision using 3-point parabolic interpolation
    //    around (chosen_tau - 1, chosen_tau, chosen_tau + 1).
    let refined_tau = if chosen_tau > 1 && chosen_tau < tau_max {
        let y_prev = nsdf[chosen_tau - 1];
        let y_curr = nsdf[chosen_tau];
        let y_next = nsdf[chosen_tau + 1];
        let denom = 2.0 * (2.0 * y_curr - y_prev - y_next);
        if denom.abs() > 1e-12 {
            let delta = (y_next - y_prev) / denom;
            (chosen_tau as f64) + delta.clamp(-0.5, 0.5)
        } else {
            chosen_tau as f64
        }
    } else {
        chosen_tau as f64
    };

    let freq_hz = sr / refined_tau;
    if !(50.0..=2000.0).contains(&freq_hz) {
        return None;
    }

    Some((freq_hz, chosen_val.clamp(0.0, 1.0) as f32))
}

/// Evaluates the target MIDI note, target frequency (Hz), and pitch error (in cents)
/// for a detected frequency `detected_hz` under the active `a4` reference pitch,
/// `temperament`, and `mode`.
///
/// How Temperament & Chromatic matching work together:
/// - In `TunerMode::TargetEb5`:
///   The primary target is `Eb5` (`MIDI 75`), whose exact frequency is computed via
///   `temperament.midi_to_freq(a4, 75)`. If the user's physical `Eb` pipe speaks in a lower
///   or higher octave (`Eb3 = 51`, `Eb4 = 63`, `Eb5 = 75`, `Eb6 = 87`), we pick the `Eb` octave
///   closest to `detected_hz` so 16', 8', 4', and 2' `Eb` ranks all work seamlessly!
/// - In `TunerMode::ChromaticAuto`:
///   We search all MIDI notes `24..=108` (`C1..C8`) using `temperament.midi_to_freq(a4, note)`
///   to find the note with the smallest absolute cent distance `|1200 * log2(detected / target)|`.
pub fn evaluate_chromatic_target(
    detected_hz: f64,
    a4: f64,
    temperament: Temperament,
    mode: TunerMode,
) -> (u8, f64, f64) {
    let candidates: &[u8] = match mode {
        // All octaves of Eb (Eb2=39, Eb3=51, Eb4=63, Eb5=75, Eb6=87) with Eb5 as primary
        TunerMode::TargetEb5 => &[39, 51, 63, EB5_MIDI_NOTE, 87],
        // Full chromatic range rendered on the organ
        TunerMode::ChromaticAuto => &[
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44,
            45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
            66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
            87, 88, 89, 90, 91, 92, 93, 94, 95, 96,
        ],
    };

    let mut best_note = EB5_MIDI_NOTE;
    let mut best_target_hz = temperament.midi_to_freq(a4, EB5_MIDI_NOTE);
    let mut best_cents = 1200.0 * (detected_hz / best_target_hz).log2();

    for &note in candidates {
        let target_hz = temperament.midi_to_freq(a4, note);
        let cents = 1200.0 * (detected_hz / target_hz).log2();
        if cents.abs() < best_cents.abs() {
            best_note = note;
            best_target_hz = target_hz;
            best_cents = cents;
        }
    }

    (best_note, best_target_hz, best_cents)
}

/// Closed-loop Blower Motor PWM Controller (supporting both the exact `±0.5%` step algorithm
/// and proportional-integral-derivative PID smoothing with a `500 ms` anti-oscillation delay).
pub struct BlowerController {
    /// Current PWM duty cycle (`0.0%` to `100.0%`), initialized at `50.0%` nominal pressure.
    pub current_pwm_percent: f32,
    /// Minimum time delay between consecutive PWM adjustments (default `500 ms`).
    pub adjustment_delay: Duration,
    /// Timestamp of the last PWM adjustment.
    last_adjustment: Option<Instant>,
    /// Integral error accumulator for optional PID smoothing.
    integral_error: f64,
    /// Previous cent error for derivative calculation.
    prev_error: f64,
    /// Whether to use fixed `0.5%` steps (`false` = PID scaled step, `true` = strict `0.5%` step per prompt).
    pub strict_half_percent_step: bool,
}

impl Default for BlowerController {
    fn default() -> Self {
        Self::new()
    }
}

impl BlowerController {
    /// Creates a new `BlowerController` starting at `50.0%` nominal PWM and `500 ms` delay.
    pub fn new() -> Self {
        BlowerController {
            current_pwm_percent: NOMINAL_PWM_PERCENT,
            adjustment_delay: Duration::from_millis(DEFAULT_ADJUSTMENT_DELAY_MS),
            last_adjustment: None,
            integral_error: 0.0,
            prev_error: 0.0,
            strict_half_percent_step: true,
        }
    }

    /// Resets the blower PWM to `50.0%` nominal pressure and clears PID state.
    pub fn reset_to_nominal(&mut self) {
        self.current_pwm_percent = NOMINAL_PWM_PERCENT;
        self.integral_error = 0.0;
        self.prev_error = 0.0;
        self.last_adjustment = None;
    }

    /// Evaluates a single pitch measurement (`cents_error = 1200 * log2(f_detected / f_target)`)
    /// at timestamp `now`.
    ///
    /// Returns `Some((command_char, new_pwm_percent, adjustment_kind))` if the `adjustment_delay`
    /// (`500 ms`) has elapsed and the pitch error exceeds `IN_TUNE_DEADBAND_CENTS`:
    /// - If Flat (`cents_error < -3.5`): increases PWM by `+0.5%` and returns `Some(('F', pwm, FlatIncreasePwm))`.
    /// - If Sharp (`cents_error > +3.5`): decreases PWM by `-0.5%` and returns `Some(('S', pwm, SharpDecreasePwm))`.
    /// - If In Tune (`|cents_error| <= 3.5`): returns `None` and maintains `current_pwm_percent`.
    pub fn update_from_cents_error(
        &mut self,
        cents_error: f64,
        now: Instant,
    ) -> (BlowerAdjustment, Option<char>) {
        // 1. Check if the pipe is already in tune within the deadband
        if cents_error.abs() <= IN_TUNE_DEADBAND_CENTS {
            self.prev_error = 0.0;
            return (BlowerAdjustment::InTuneHold, None);
        }

        // 2. Enforce the anti-oscillation delay (default 500 ms) between adjustments
        if let Some(last) = self.last_adjustment {
            if now.duration_since(last) < self.adjustment_delay {
                let pending = if cents_error < 0.0 {
                    BlowerAdjustment::FlatIncreasePwm
                } else {
                    BlowerAdjustment::SharpDecreasePwm
                };
                return (pending, None);
            }
        }

        // 3. Compute the PWM step:
        //    In strict 0.5% step mode (matching newconsole.ino), every adjustment changes PWM
        //    by exactly +0.5% (when Flat) or -0.5% (when Sharp).
        //    In PID mode, the step is scaled by Kp*e + Ki*integral + Kd*derivative.
        let step = if self.strict_half_percent_step {
            PWM_STEP_PERCENT
        } else {
            let dt = self.adjustment_delay.as_secs_f64().max(0.1);
            // Setpoint error: positive when pipe is Flat (needs more wind pressure)
            let err = -cents_error;
            self.integral_error = (self.integral_error + err * dt).clamp(-50.0, 50.0);
            let deriv = (err - self.prev_error) / dt;
            self.prev_error = err;
            let pid_out = 0.04 * err + 0.008 * self.integral_error + 0.005 * deriv;
            (pid_out.abs() as f32).clamp(0.25, 2.0)
        };

        self.last_adjustment = Some(now);

        if cents_error < -IN_TUNE_DEADBAND_CENTS {
            // Pitch is Flat -> increase blower PWM by 0.5% and send 'F' to Arduino
            self.current_pwm_percent = (self.current_pwm_percent + step).clamp(0.0, 100.0);
            (BlowerAdjustment::FlatIncreasePwm, Some('F'))
        } else {
            // Pitch is Sharp -> decrease blower PWM by 0.5% and send 'S' to Arduino
            self.current_pwm_percent = (self.current_pwm_percent - step).clamp(0.0, 100.0);
            (BlowerAdjustment::SharpDecreasePwm, Some('S'))
        }
    }
}

/// Shared state between the background `cpal` microphone capture callback and the main GUI thread.
struct SharedMicState {
    /// Circular ring buffer of recent mono microphone samples (`4096` samples ~ `85-93 ms`).
    ring_buf: Vec<f32>,
    /// Current write cursor in `ring_buf`.
    write_pos: usize,
    /// Sample rate of the active microphone input stream.
    sample_rate: u32,
    /// Exponentially smoothed fundamental frequency `f0` (Hz).
    smoothed_freq: f64,
}

/// Real-time Microphone Pitch Tuner & Blower Pressure Regulator manager.
pub struct MicrophoneTuner {
    /// Active `cpal` input stream (present when microphone capture is enabled).
    stream: Option<cpal::Stream>,
    /// Name of the active microphone device.
    pub device_name: String,
    /// Shared sample ring buffer written by the audio input callback.
    shared: Arc<Mutex<SharedMicState>>,
    /// Closed-loop blower PWM controller.
    pub controller: BlowerController,
    /// Active tuning mode (`TargetEb5` or `ChromaticAuto`).
    pub mode: TunerMode,
    /// Whether closed-loop serial PWM commands (`'F'` / `'S'`) are automatically transmitted to the Arduino.
    pub auto_regulate_pwm: bool,
    /// Latest computed status for GUI display.
    pub status: TunerStatus,
}

impl Default for MicrophoneTuner {
    fn default() -> Self {
        Self::new()
    }
}

impl MicrophoneTuner {
    /// Creates a new `MicrophoneTuner` (initially stopped until the user enables it or selects a mic).
    pub fn new() -> Self {
        MicrophoneTuner {
            stream: None,
            device_name: "Default Microphone".to_string(),
            shared: Arc::new(Mutex::new(SharedMicState {
                ring_buf: vec![0.0f32; 4096],
                write_pos: 0,
                sample_rate: 44100,
                smoothed_freq: 0.0,
            })),
            controller: BlowerController::new(),
            mode: TunerMode::TargetEb5,
            auto_regulate_pwm: true,
            status: TunerStatus::default(),
        }
    }

    /// Returns true if the microphone capture stream is currently running.
    pub fn is_listening(&self) -> bool {
        self.stream.is_some()
    }

    /// Lists all available audio input (microphone) device names on the system.
    pub fn list_input_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut names = Vec::new();
        if let Ok(devices) = host.input_devices() {
            for dev in devices {
                if let Ok(desc) = dev.description() {
                    names.push(desc.name().to_string());
                } else if let Ok(id) = dev.id() {
                    names.push(id.to_string());
                }
            }
        }
        names
    }

    /// Starts capturing audio from the specified microphone device (or the system default microphone
    /// if `preferred_name` is empty or `"Default Microphone"`).
    pub fn start(&mut self, preferred_name: Option<&str>) -> Result<(), String> {
        self.stop();

        let host = cpal::default_host();
        let device = if let Some(target) = preferred_name.filter(|s| !s.is_empty() && *s != "Default Microphone") {
            let mut found: Option<Device> = None;
            if let Ok(devices) = host.input_devices() {
                for d in devices {
                    let d_name = d
                        .description()
                        .map(|desc| desc.name().to_string())
                        .or_else(|_| d.id().map(|i| i.to_string()))
                        .unwrap_or_default();
                    if d_name == target {
                        found = Some(d);
                        break;
                    }
                }
            }
            found.or_else(|| host.default_input_device())
        } else {
            host.default_input_device()
        }
        .ok_or_else(|| "No microphone input device found on this laptop.".to_string())?;

        let dev_name = device
            .description()
            .map(|desc| desc.name().to_string())
            .or_else(|_| device.id().map(|i| i.to_string()))
            .unwrap_or_else(|_| "Default Microphone".to_string());

        let supported = device
            .default_input_config()
            .map_err(|e| format!("Failed to query microphone config: {e}"))?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = (config.channels as usize).max(1);

        {
            let mut guard = self.shared.lock();
            guard.sample_rate = sample_rate;
            guard.ring_buf.fill(0.0);
            guard.write_pos = 0;
            guard.smoothed_freq = 0.0;
        }

        let shared_ref = self.shared.clone();
        let stream = match sample_format {
            SampleFormat::F32 => build_input_stream::<f32>(&device, config, channels, shared_ref, |s| s)?,
            SampleFormat::F64 => build_input_stream::<f64>(&device, config, channels, shared_ref, |s| s as f32)?,
            SampleFormat::I16 => build_input_stream::<i16>(&device, config, channels, shared_ref, |s| s as f32 / 32768.0)?,
            SampleFormat::I32 => build_input_stream::<i32>(&device, config, channels, shared_ref, |s| s as f32 / 2_147_483_648.0)?,
            SampleFormat::U16 => build_input_stream::<u16>(&device, config, channels, shared_ref, |s| (s as f32 - 32768.0) / 32768.0)?,
            other => return Err(format!("Unsupported microphone sample format: {other}")),
        };

        stream.play().map_err(|e| format!("Failed to start microphone stream: {e}"))?;
        self.device_name = dev_name;
        self.stream = Some(stream);
        Ok(())
    }

    /// Stops the active microphone capture stream.
    pub fn stop(&mut self) {
        self.stream = None;
        self.status.signal_detected = false;
        self.status.rms_level = 0.0;
        self.status.detected_freq_hz = 0.0;
        self.status.last_action = BlowerAdjustment::WaitingForSignal;
    }

    /// Polls the latest 4096-sample microphone window, runs NSDF pitch detection, compares
    /// against the active `a4` reference and `temperament`, and updates the blower PWM controller.
    ///
    /// Returns `Some(serial_char)` (`'F'` or `'S'`) whenever a PWM step adjustment should be
    /// sent over Serial to the Arduino on Pin 3.
    pub fn poll_and_regulate(&mut self, a4: f64, temperament: Temperament) -> Option<char> {
        // Keep the target frequency updated even when silent so the GUI always shows the active Eb5 target
        let default_eb5_hz = temperament.midi_to_freq(a4, EB5_MIDI_NOTE);
        if !self.is_listening() {
            self.status.target_freq_hz = default_eb5_hz;
            self.status.current_pwm_percent = self.controller.current_pwm_percent;
            return None;
        }

        // Copy the latest ordered 4096-sample window out of the circular ring buffer
        let (window, sr, prev_smoothed) = {
            let guard = self.shared.lock();
            let len = guard.ring_buf.len();
            let wp = guard.write_pos;
            let mut ordered = Vec::with_capacity(len);
            ordered.extend_from_slice(&guard.ring_buf[wp..]);
            ordered.extend_from_slice(&guard.ring_buf[..wp]);
            (ordered, guard.sample_rate, guard.smoothed_freq)
        };

        // Compute RMS level for the GUI signal meter
        let rms = (window.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>()
            / window.len().max(1) as f64)
            .sqrt() as f32;
        self.status.rms_level = rms;

        let Some((raw_freq, confidence)) = detect_pitch_nsdf(&window, sr) else {
            self.status.signal_detected = false;
            self.status.detected_freq_hz = 0.0;
            self.status.confidence = 0.0;
            self.status.target_freq_hz = default_eb5_hz;
            self.status.matched_midi_note = EB5_MIDI_NOTE;
            self.status.matched_note_name = format_midi_note(EB5_MIDI_NOTE);
            self.status.last_action = BlowerAdjustment::WaitingForSignal;
            self.status.current_pwm_percent = self.controller.current_pwm_percent;
            return None;
        };

        // Exponential pitch smoothing (70% new + 30% previous when within 1 semitone)
        // to reject transient attack chiff while tracking pressure drift smoothly
        let smoothed_freq = if prev_smoothed > 0.0 && (raw_freq / prev_smoothed - 1.0).abs() < 0.06 {
            0.30 * prev_smoothed + 0.70 * raw_freq
        } else {
            raw_freq
        };
        self.shared.lock().smoothed_freq = smoothed_freq;

        let (matched_note, target_hz, cents_error) =
            evaluate_chromatic_target(smoothed_freq, a4, temperament, self.mode);

        // In TargetEb5 mode, only regulate the blower if the detected pitch is within ±250 cents
        // (2.5 semitones) of an Eb pipe so random speech or other notes don't mistrigger the blower
        let valid_for_blower = match self.mode {
            TunerMode::TargetEb5 => cents_error.abs() <= 250.0,
            TunerMode::ChromaticAuto => cents_error.abs() <= 60.0,
        };

        let mut command_to_send = None;
        let action = if valid_for_blower && self.auto_regulate_pwm {
            let (act, cmd) = self.controller.update_from_cents_error(cents_error, Instant::now());
            command_to_send = cmd;
            act
        } else if cents_error.abs() <= IN_TUNE_DEADBAND_CENTS {
            BlowerAdjustment::InTuneHold
        } else if cents_error < 0.0 {
            BlowerAdjustment::FlatIncreasePwm
        } else {
            BlowerAdjustment::SharpDecreasePwm
        };

        self.status.signal_detected = true;
        self.status.detected_freq_hz = smoothed_freq;
        self.status.target_freq_hz = target_hz;
        self.status.matched_midi_note = matched_note;
        self.status.matched_note_name = format_midi_note(matched_note);
        self.status.cents_error = cents_error;
        self.status.confidence = confidence;
        self.status.current_pwm_percent = self.controller.current_pwm_percent;
        self.status.last_action = action;

        command_to_send
    }
}

/// Helper that builds a typed `cpal` input stream and mixes multi-channel microphone frames
/// down to mono into the shared circular ring buffer.
fn build_input_stream<T>(
    device: &Device,
    config: StreamConfig,
    channels: usize,
    shared: Arc<Mutex<SharedMicState>>,
    to_f32: fn(T) -> f32,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mut guard = shared.lock();
                let len = guard.ring_buf.len();
                for frame in data.chunks(channels) {
                    let mut sum = 0.0f32;
                    for &ch in frame {
                        sum += to_f32(ch);
                    }
                    let mono = sum / (frame.len().max(1) as f32);
                    let wp = guard.write_pos;
                    guard.ring_buf[wp] = mono;
                    guard.write_pos = (wp + 1) % len;
                }
            },
            |err| eprintln!("Microphone input stream error: {err}"),
            None,
        )
        .map_err(|e| format!("Failed to build microphone input stream: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nsdf_pitch_detection_on_harmonic_eb5_organ_pipe() {
        // Synthesize a realistic Eb5 (622.25 Hz) organ pipe waveform with strong 2nd, 3rd, and 4th harmonics
        let sr = 44100u32;
        let f0 = 622.25397f64;
        let mut signal = vec![0.0f32; 4096];
        for (i, sample) in signal.iter_mut().enumerate() {
            let t = i as f64 / sr as f64;
            let p = 2.0 * std::f64::consts::PI * f0 * t;
            // Fundamental + strong 2nd (0.75) + 3rd (0.45) + 4th (0.25) harmonics
            let val = 1.0 * p.sin()
                + 0.75 * (2.0 * p + 0.8).sin()
                + 0.45 * (3.0 * p + 1.7).sin()
                + 0.25 * (4.0 * p + 2.4).sin();
            *sample = (val * 0.25) as f32;
        }

        let detected = detect_pitch_nsdf(&signal, sr).expect("Must detect Eb5 pipe pitch");
        let (freq_hz, confidence) = detected;
        let cents_err = (1200.0 * (freq_hz / f0).log2()).abs();
        assert!(
            cents_err < 1.5,
            "NSDF pitch detector must be accurate within 1.5 cents on harmonic Eb5 pipe: got {freq_hz:.2} Hz (err {cents_err:.2} cents)"
        );
        assert!(confidence > 0.85, "Confidence must be high for periodic organ tone: {confidence}");
    }

    #[test]
    fn test_blower_pwm_controller_flat_sharp_step_and_delay() {
        let mut ctrl = BlowerController::new();
        assert!((ctrl.current_pwm_percent - 50.0).abs() < 1e-5, "Must start at 50% nominal PWM");

        let t0 = Instant::now();

        // 1. Flat Eb5 (-18 cents): must increase PWM by +0.5% to 50.5% and emit 'F'
        let (act1, cmd1) = ctrl.update_from_cents_error(-18.0, t0);
        assert_eq!(act1, BlowerAdjustment::FlatIncreasePwm);
        assert_eq!(cmd1, Some('F'));
        assert!((ctrl.current_pwm_percent - 50.5).abs() < 1e-5);

        // 2. Poll again 200 ms later (< 500 ms delay): must NOT adjust PWM yet (anti-oscillation delay)
        let (act2, cmd2) = ctrl.update_from_cents_error(-18.0, t0 + Duration::from_millis(200));
        assert_eq!(act2, BlowerAdjustment::FlatIncreasePwm);
        assert_eq!(cmd2, None);
        assert!((ctrl.current_pwm_percent - 50.5).abs() < 1e-5);

        // 3. Poll at t0 + 550 ms (>= 500 ms delay): must increase PWM by another +0.5% to 51.0%
        let (act3, cmd3) = ctrl.update_from_cents_error(-12.0, t0 + Duration::from_millis(550));
        assert_eq!(act3, BlowerAdjustment::FlatIncreasePwm);
        assert_eq!(cmd3, Some('F'));
        assert!((ctrl.current_pwm_percent - 51.0).abs() < 1e-5);

        // 4. Pipe becomes Sharp (+14 cents) at t0 + 1100 ms: must decrease PWM by -0.5% to 50.5% and emit 'S'
        let (act4, cmd4) = ctrl.update_from_cents_error(14.0, t0 + Duration::from_millis(1100));
        assert_eq!(act4, BlowerAdjustment::SharpDecreasePwm);
        assert_eq!(cmd4, Some('S'));
        assert!((ctrl.current_pwm_percent - 50.5).abs() < 1e-5);

        // 5. Pipe is In Tune (+1.2 cents within ±3.5 cent deadband): must hold PWM steady
        let (act5, cmd5) = ctrl.update_from_cents_error(1.2, t0 + Duration::from_millis(1700));
        assert_eq!(act5, BlowerAdjustment::InTuneHold);
        assert_eq!(cmd5, None);
        assert!((ctrl.current_pwm_percent - 50.5).abs() < 1e-5);
    }

    #[test]
    fn test_chromatic_and_temperament_target_calculation() {
        // Verify Eb5 target adapts to both Equal Temperament and 1/4-Comma Meantone
        let eb5_equal = Temperament::Equal.midi_to_freq(440.0, EB5_MIDI_NOTE);
        let eb5_meantone = Temperament::Meantone14.midi_to_freq(440.0, EB5_MIDI_NOTE);
        assert!((eb5_equal - 622.25397).abs() < 0.01);
        assert!(eb5_meantone > eb5_equal + 5.0, "Meantone D#/Eb5 has +20.53 cent offset");

        // Evaluate slightly flat Eb5 (618.0 Hz) in TargetEb5 mode
        let (note, target_hz, cents) =
            evaluate_chromatic_target(618.0, 440.0, Temperament::Equal, TunerMode::TargetEb5);
        assert_eq!(note, EB5_MIDI_NOTE);
        assert!((target_hz - eb5_equal).abs() < 1e-5);
        assert!(cents < -10.0 && cents > -15.0);
    }
}
