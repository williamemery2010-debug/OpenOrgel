//! THE CONSOLE - egui front end for OpenOrgel.
//!
//! 1:1 feature parity with the old Tkinter console (stops, tuning, MIDI in,
//! pitch-class resonance wheel, load/export/pause/resume/stop, presets 1-9,
//! Arduino link) in a fraction of the RAM and none of the Python.

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Layout, Margin, Pos2, RichText, Sense, Shape, Stroke, StrokeKind, Vec2,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::audio::AudioOutput;
use crate::engine::{self, Engine, Rendered, SAMPLE_RATE};
use crate::dsp::{self, AcousticVenue};
use crate::midi_in::{self, MidiConnection};
use crate::serial::{ArduinoLink, SerialEvent};
use crate::classifier::{Division, Footage};
use crate::settings::Settings;
use crate::soundfont::SoundFontPreset;
use crate::stops::{self, Group, STOPS};
use crate::temperament::Temperament;

// ---- palette (carried over from the legacy console) ----
const DARK_BG: Color32 = Color32::from_rgb(0x0B, 0x0E, 0x14);
const PANEL_BG: Color32 = Color32::from_rgb(0x16, 0x19, 0x22);
const BORDER: Color32 = Color32::from_rgb(0x25, 0x2B, 0x3B);
const ACCENT: Color32 = Color32::from_rgb(0x3B, 0x82, 0xF6);
const ACCENT_HOVER: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
const TEXT_FG: Color32 = Color32::from_rgb(0xF3, 0xF4, 0xF6);
const TEXT_MUTED: Color32 = Color32::from_rgb(0x9C, 0xA3, 0xAF);
const GREEN: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
const GREEN_HOVER: Color32 = Color32::from_rgb(0x05, 0x96, 0x69);
const AMBER: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
const AMBER_HOVER: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
const RED: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);
const RED_HOVER: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);
const CHIP: Color32 = Color32::from_rgb(0x25, 0x2B, 0x3B);

const TUNINGS: [u32; 6] = [415, 432, 440, 441, 442, 466];
const PITCH_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

fn bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(bold_family()).size(size)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Rounded button with an animated hover colour (smoother than stock egui).
fn pill_button(
    ui: &mut egui::Ui,
    text: &str,
    base: Color32,
    hover: Color32,
    size: Vec2,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let id = resp.id.with("hover_anim");
    let t = ui.ctx().animate_bool_with_time(id, resp.hovered() && enabled, 0.12);
    let mut fill = lerp_color(base, hover, t);
    if resp.is_pointer_button_down_on() && enabled {
        fill = lerp_color(fill, Color32::BLACK, 0.15);
    }
    if !enabled {
        fill = lerp_color(base, DARK_BG, 0.6);
    }
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    let txt_color = if enabled { Color32::WHITE } else { TEXT_MUTED };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::new(13.0, bold_family()),
        txt_color,
    );
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Interactive Stop Tile / Card Button with tactile feedback and illuminated LED indicator.
fn stop_card_button(
    ui: &mut egui::Ui,
    name: &str,
    sub: &str,
    badge_pitch: &str,
    is_active: bool,
) -> egui::Response {
    let desired_size = Vec2::new(ui.available_width(), 48.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

    let bg = if is_active {
        if resp.hovered() {
            ACCENT_HOVER
        } else {
            ACCENT
        }
    } else {
        if resp.hovered() {
            CHIP
        } else {
            DARK_BG
        }
    };

    let border_color = if is_active {
        Color32::from_rgb(0x93, 0xC5, 0xFD)
    } else if resp.hovered() {
        Color32::from_rgb(0x4F, 0x56, 0x6B)
    } else {
        BORDER
    };

    ui.painter().rect_filled(rect, CornerRadius::same(7), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(7),
        Stroke::new(1.0, border_color),
        egui::StrokeKind::Inside,
    );

    // Glowing LED Indicator Dot
    let led_color = if is_active {
        GREEN
    } else {
        Color32::from_rgb(0x3B, 0x40, 0x50)
    };
    let led_center = Pos2::new(rect.min.x + 14.0, rect.center().y);
    ui.painter().circle_filled(led_center, 4.0, led_color);
    if is_active {
        ui.painter().circle_stroke(
            led_center,
            6.0,
            Stroke::new(1.0, Color32::from_rgba_premultiplied(0x34, 0xD3, 0x99, 110)),
        );
    }

    // Pitch Badge (e.g. "8'", "4'", "Mix") on the right
    if !badge_pitch.is_empty() {
        let chip_color = if is_active {
            Color32::from_rgb(0xDB, 0xEA, 0xFE)
        } else {
            TEXT_MUTED
        };
        ui.painter().text(
            Pos2::new(rect.max.x - 10.0, rect.min.y + 13.0),
            Align2::RIGHT_CENTER,
            badge_pitch,
            FontId::new(11.0, bold_family()),
            chip_color,
        );
    }

    // Stop Title Name
    let title_color = if is_active { Color32::WHITE } else { TEXT_FG };
    let title_pos = Pos2::new(rect.min.x + 26.0, rect.min.y + 8.0);
    ui.painter().text(
        title_pos,
        Align2::LEFT_TOP,
        name,
        FontId::new(12.0, bold_family()),
        title_color,
    );

    // Subtitle (Font name & Bank/Patch)
    let sub_color = if is_active {
        Color32::from_rgb(0xBF, 0xDB, 0xFE)
    } else {
        TEXT_MUTED
    };
    let sub_pos = Pos2::new(rect.min.x + 26.0, rect.min.y + 26.0);
    ui.painter().text(
        sub_pos,
        Align2::LEFT_TOP,
        sub,
        FontId::new(10.0, FontFamily::Proportional),
        sub_color,
    );

    resp
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(PANEL_BG)
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn inner_card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(DARK_BG)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

enum UiMsg {
    /// Dialog finished: Some(path) to render, None if cancelled.
    LoadPicked(Option<PathBuf>),
    RenderDone(Result<Rendered, String>, PathBuf),
    ExportStarted,
    ExportDone(Result<PathBuf, String>),
    ExportCancelled,
    ImportPicked(Vec<PathBuf>),
}

#[derive(PartialEq, Clone, Copy)]
enum Busy {
    Idle,
    Dialog,
    Synth,
    Export,
}

/// Console navigation tabs between native stops and imported soundfonts.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ConsoleTab {
    PrincipalOrgan,
    SoundFonts,
}

pub struct OrgelApp {
    engine: Arc<Engine>,
    audio: Result<AudioOutput, String>,
    settings: Settings,
    stop_on: Vec<bool>,
    groups: Vec<(Group, Vec<usize>)>,
    ordered: Vec<usize>,
    tuning: u32,
    volume: f32,
    midi_devices: Vec<String>,
    midi_selected: String,
    midi_conn: Option<MidiConnection>,
    link: ArduinoLink,
    last_a4: bool,
    toast: Option<(String, Instant, Color32)>,
    busy: Busy,
    tx: Sender<UiMsg>,
    rx: Receiver<UiMsg>,
    engine_version: String,
    pcs: [bool; 12],
    pub console_tab: ConsoleTab,
    pub sf_division: Option<Division>,
    pub sf_page: usize,
    pub sf_search: String,
    pub sf_presets: Vec<SoundFontPreset>,
    pub temperament: Temperament,
    pub venue: AcousticVenue,
}

impl OrgelApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        setup_visuals(&cc.egui_ctx);

        dsp::init_engine();
        let engine_version = dsp::engine_version().to_string();
        let engine = Engine::new();
        let settings = Settings::load();

        let mut stop_on = vec![false; STOPS.len()];
        if let Some(i) = stops::index_of(stops::DEFAULT_STOP) {
            stop_on[i] = true;
        }

        let tuning = if TUNINGS.contains(&settings.tuning) {
            settings.tuning
        } else {
            440
        };
        let temperament = Temperament::from_str_name(&settings.temperament);
        let venue = AcousticVenue::from_str_name(&settings.venue);
        let volume = settings.volume.clamp(0.0, 1.0);
        engine.set_a4(tuning as f64);
        engine.set_temperament(temperament);
        engine.set_venue(venue);
        engine.set_volume(volume);

        let audio = AudioOutput::start(engine.clone());
        if let Err(e) = &audio {
            eprintln!("Audio start failed: {e}");
        }

        let ctx = cc.egui_ctx.clone();
        let link = ArduinoLink::spawn(move || ctx.request_repaint());

        let (tx, rx) = unbounded();
        let mut app = OrgelApp {
            engine,
            audio,
            settings,
            stop_on,
            groups: stops::grouped(),
            ordered: stops::ordered_indices(),
            tuning,
            temperament,
            venue,
            volume,
            midi_devices: Vec::new(),
            midi_selected: "None".to_string(),
            midi_conn: None,
            link,
            last_a4: false,
            toast: None,
            busy: Busy::Idle,
            tx,
            rx,
            engine_version,
            pcs: [false; 12],
            console_tab: ConsoleTab::PrincipalOrgan,
            sf_division: None,
            sf_page: 0,
            sf_search: String::new(),
            sf_presets: Vec::new(),
        };
        app.apply_stops();
        app.refresh_midi_devices();
        let last = app.settings.last_midi_device.clone();
        if last != "None" && app.midi_devices.contains(&last) {
            app.select_midi(&last);
        }
        app
    }

    // ---------- helpers ----------

    fn toast(&mut self, msg: impl Into<String>, color: Color32) {
        self.toast = Some((msg.into(), Instant::now(), color));
    }

    fn active_ids(&self) -> Vec<i32> {
        STOPS
            .iter()
            .zip(self.stop_on.iter())
            .filter(|(_, on)| **on)
            .map(|(s, _)| s.id)
            .collect()
    }

    fn active_names(&self) -> Vec<String> {
        STOPS
            .iter()
            .zip(self.stop_on.iter())
            .filter(|(_, on)| **on)
            .map(|(s, _)| s.name.to_string())
            .collect()
    }

    /// Stops changed: tell the engine (live tones + background re-render).
    fn apply_stops(&mut self) {
        self.engine.set_stops(self.active_ids());
        self.engine.refresh();
    }

    fn refresh_midi_devices(&mut self) {
        self.midi_devices = midi_in::list_inputs();
        if self.midi_selected != "None" && !self.midi_devices.contains(&self.midi_selected) {
            self.select_midi("None");
        }
    }

    fn select_midi(&mut self, name: &str) {
        self.midi_conn = None; // dropping closes the port
        self.engine.all_notes_off();
        self.midi_selected = name.to_string();
        if name != "None" {
            match midi_in::open_input(name, self.engine.clone()) {
                Ok(c) => self.midi_conn = Some(c),
                Err(e) => {
                    self.midi_selected = "None".to_string();
                    self.toast(e, RED);
                }
            }
        }
        self.settings.last_midi_device = self.midi_selected.clone();
        self.settings.save();
    }

    fn save_preset(&mut self, key: usize) {
        let names = self.active_names();
        self.settings.presets.insert(key.to_string(), names);
        self.settings.save();
        self.toast(format!("Saved active stops to Preset {key}"), GREEN);
    }

    fn load_preset(&mut self, key: usize) {
        let Some(names) = self.settings.presets.get(&key.to_string()).cloned() else {
            return;
        };
        if names.is_empty() {
            self.toast(format!("Preset {key} is empty (Shift+{key} to save)"), AMBER);
            return;
        }
        for (i, s) in STOPS.iter().enumerate() {
            self.stop_on[i] = names.iter().any(|n| n == s.name);
        }
        self.apply_stops();
        self.toast(format!("Loaded Preset {key}"), ACCENT);
    }

    fn set_all_stops(&mut self, on: bool) {
        for v in self.stop_on.iter_mut() {
            *v = on;
        }
        self.apply_stops();
    }

    /// Returns true if at least one organ stop (physical or soundfont) is currently drawn.
    pub fn has_any_active_stops(&self) -> bool {
        !self.active_ids().is_empty() || self.engine.soundfont.lock().active_preset_count() > 0
    }

    // ---------- background work ----------

    fn start_load_dialog(&mut self, ctx: &egui::Context) {
        if self.busy != Busy::Idle {
            return;
        }
        if !self.has_any_active_stops() {
            self.toast("No selected stops! Please draw at least one stop before playing.", RED);
            return;
        }
        self.busy = Busy::Dialog;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let pick = rfd::FileDialog::new()
                .set_title("Load MIDI")
                .add_filter("MIDI files", &["mid", "midi"])
                .pick_file();
            let _ = tx.send(UiMsg::LoadPicked(pick));
            ctx.request_repaint();
        });
    }

    fn start_render(&mut self, path: PathBuf, ctx: &egui::Context) {
        if !self.has_any_active_stops() {
            self.busy = Busy::Idle;
            self.toast("No selected stops! Please draw at least one stop before playing.", RED);
            return;
        }
        self.busy = Busy::Synth;
        let engine = self.engine.clone();
        let stops = self.active_ids();
        let a4 = engine.a4();
        let temp = engine.temperament();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = engine::render_midi(&engine, &path, &stops, a4, temp);
            let _ = tx.send(UiMsg::RenderDone(r, path));
            ctx.request_repaint();
        });
    }

    fn start_export(&mut self, ctx: &egui::Context) {
        if self.busy != Busy::Idle {
            return;
        }
        if !self.has_any_active_stops() {
            self.toast("No selected stops! Please draw at least one stop before exporting.", RED);
            return;
        }
        self.busy = Busy::Dialog;
        let engine = self.engine.clone();
        let stops = self.active_ids();
        let a4 = engine.a4();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let Some(midi) = rfd::FileDialog::new()
                .set_title("Choose MIDI to export")
                .add_filter("MIDI files", &["mid", "midi"])
                .pick_file()
            else {
                let _ = tx.send(UiMsg::ExportCancelled);
                ctx.request_repaint();
                return;
            };
            let Some(wav) = rfd::FileDialog::new()
                .set_title("Save WAV")
                .add_filter("WAV files", &["wav"])
                .set_file_name("openorgel.wav")
                .save_file()
            else {
                let _ = tx.send(UiMsg::ExportCancelled);
                ctx.request_repaint();
                return;
            };
            let _ = tx.send(UiMsg::ExportStarted);
            ctx.request_repaint();
            let temp = engine.temperament();
            let result = engine::render_midi(&engine, &midi, &stops, a4, temp)
                .and_then(|r| engine::write_wav(&wav, &r.audio))
                .map(|_| wav);
            let _ = tx.send(UiMsg::ExportDone(result));
            ctx.request_repaint();
        });
    }

    fn start_import_dialog(&mut self, ctx: &egui::Context) {
        if self.busy != Busy::Idle {
            return;
        }
        self.busy = Busy::Dialog;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let picks = rfd::FileDialog::new()
                .set_title("Import Soundfonts & Samples")
                .add_filter("SoundFont 2 (*.sf2)", &["sf2"])
                .add_filter(
                    "All Sound Libraries",
                    &["sf2", "sf3", "zip", "sfz", "nkc", "nkr", "nkx", "ogi", "nki", "nicint"],
                )
                .pick_files()
                .unwrap_or_default();
            let _ = tx.send(UiMsg::ImportPicked(picks));
            ctx.request_repaint();
        });
    }

    fn pump_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                UiMsg::LoadPicked(Some(path)) => self.start_render(path, ctx),
                UiMsg::LoadPicked(None) | UiMsg::ExportCancelled => self.busy = Busy::Idle,
                UiMsg::RenderDone(result, path) => {
                    self.busy = Busy::Idle;
                    match result {
                        Ok(r) => {
                            self.engine.start_playback(r, path);
                        }
                        Err(e) => self.toast(e, RED),
                    }
                }
                UiMsg::ExportStarted => self.busy = Busy::Export,
                UiMsg::ExportDone(result) => {
                    self.busy = Busy::Idle;
                    match result {
                        Ok(p) => self.toast(format!("Audio saved to {}", p.display()), GREEN),
                        Err(e) => self.toast(format!("Export failed: {e}"), RED),
                    }
                }
                UiMsg::ImportPicked(paths) => {
                    self.busy = Busy::Idle;
                    if !paths.is_empty() {
                        let mut loaded_count = 0;
                        let mut total_presets = 0;
                        let mut last_name = String::new();
                        let mut non_sf2 = 0;

                        for p in &paths {
                            let ext = p
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            if ext == "sf2" {
                                let res = self.engine.soundfont.lock().load_sf2(p);
                                match res {
                                    Ok((count, name)) => {
                                        loaded_count += 1;
                                        total_presets += count;
                                        last_name = name;
                                    }
                                    Err(e) => {
                                        self.toast(format!("Error loading {}: {e}", p.display()), RED);
                                    }
                                }
                            } else {
                                non_sf2 += 1;
                            }
                        }

                        self.sf_presets = self.engine.soundfont.lock().all_presets();

                        if loaded_count > 0 {
                            self.console_tab = ConsoleTab::SoundFonts;
                            self.sf_page = 0;
                            if loaded_count == 1 {
                                self.toast(
                                    format!("Loaded {} ranks from '{}' onto Page 2!", total_presets, last_name),
                                    GREEN,
                                );
                            } else {
                                self.toast(
                                    format!("Loaded {} ranks across {} soundfonts onto Page 2!", total_presets, loaded_count),
                                    GREEN,
                                );
                            }
                        } else if non_sf2 > 0 {
                            self.toast(
                                format!("Selected {} file(s). Note: Only SF2 SoundFonts are directly synthesized; other formats require SF2 conversion.", non_sf2),
                                AMBER,
                            );
                        }
                    }
                }
            }
        }
    }

    fn pump_serial(&mut self) {
        while let Ok(ev) = self.link.rx.try_recv() {
            match ev {
                SerialEvent::Volume(v) => {
                    self.volume = v.clamp(0.0, 1.0);
                    self.engine.set_volume(self.volume);
                }
                SerialEvent::Stop { ident, state } => {
                    let mut changed = false;
                    if ident == "ALL" {
                        for v in self.stop_on.iter_mut() {
                            *v = state;
                        }
                        changed = true;
                    } else if let Some(i) = stops::index_of(&ident) {
                        self.stop_on[i] = state;
                        changed = true;
                    } else if let Ok(n) = ident.parse::<usize>() {
                        // legacy numeric fallback
                        if let Some(&i) = self.ordered.get(n) {
                            self.stop_on[i] = state;
                            changed = true;
                        }
                    }
                    if changed {
                        self.apply_stops();
                    }
                }
                SerialEvent::Connected(p) => self.toast(format!("Console connected on {p}"), GREEN),
                SerialEvent::Disconnected => self.toast("Console connection lost, retrying...", AMBER),
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        use egui::{Event, Key};
        let mut actions: Vec<(usize, bool)> = Vec::new(); // (preset, save?)
        ctx.input(|i| {
            for ev in &i.events {
                if let Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = ev
                {
                    if modifiers.ctrl || modifiers.alt || modifiers.command {
                        continue;
                    }
                    let k = physical_key.unwrap_or(*key);
                    let n = match k {
                        Key::Num1 => 1,
                        Key::Num2 => 2,
                        Key::Num3 => 3,
                        Key::Num4 => 4,
                        Key::Num5 => 5,
                        Key::Num6 => 6,
                        Key::Num7 => 7,
                        Key::Num8 => 8,
                        Key::Num9 => 9,
                        _ => continue,
                    };
                    actions.push((n, modifiers.shift));
                }
            }
        });
        for (n, save) in actions {
            if save {
                self.save_preset(n);
            } else {
                self.load_preset(n);
            }
        }
    }

    // ---------- drawing ----------

    fn header(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        bold(
                            format!(
                                "OPENORGEL SYNTHESIS ENGINE  (v{})",
                                env!("CARGO_PKG_VERSION")
                            ),
                            20.0,
                        )
                        .color(TEXT_FG),
                    );
                    ui.label(
                        RichText::new(
                            "High-Performance Physical Modeling & Dual-Layer RAM Resampled Organ Console",
                        )
                        .size(12.0)
                        .color(TEXT_MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    badge(
                        ui,
                        &format!("DSP Engine: {}", self.engine_version),
                        GREEN,
                    );
                    if self.link.is_connected() {
                        badge(ui, "Console: Connected", ACCENT);
                    } else {
                        badge(ui, "Console: Searching...", TEXT_MUTED);
                    }
                });
            });
        });
    }

    // ========================================================================
    // SOUNDFONT CONSOLE PAGE (PAGE 2) - DIVISION & FOOTAGE ORGAN MATRIX
    // ========================================================================
    fn soundfont_page_ui(&mut self, ui: &mut egui::Ui) {
        let total_loaded = self.sf_presets.len();
        if total_loaded == 0 {
            inner_card(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(28.0);
                    ui.label(bold("No SoundFonts Loaded Yet", 16.0).color(TEXT_FG));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Import any .sf2 SoundFont file using 'Import Samples' below to populate Page 2.")
                            .size(13.0)
                            .color(TEXT_MUTED),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Supports Classical Organs, Theatre Organs, FluidR3, MuseScore, General MIDI, and custom soundbanks.")
                            .size(11.0)
                            .color(TEXT_MUTED),
                    );
                    ui.add_space(28.0);
                });
            });
            return;
        }

        let mut sf_mgr = self.engine.soundfont.lock();
        let mut sf_changed = false;

        // --- DIVISION SELECTOR TABS ---
        ui.horizontal_wrapped(|ui| {
            ui.label(bold("Division:", 12.0).color(TEXT_MUTED));
            ui.add_space(4.0);

            // Tab for "All Divisions"
            let all_active = self.sf_division.is_none();
            let all_fill = if all_active { ACCENT } else { CHIP };
            let all_label = format!("All Divisions ({})", total_loaded);
            if pill_button(ui, &all_label, all_fill, ACCENT_HOVER, Vec2::new(130.0, 26.0), true).clicked() {
                self.sf_division = None;
            }

            // Tabs for each Division (Pedal, Great, Swell, Choir, Solo, Misc)
            for div in Division::ALL {
                let div_count = self.sf_presets.iter().filter(|p| p.division == div).count();
                if div_count == 0 {
                    continue;
                }
                let is_selected = self.sf_division == Some(div);
                let fill = if is_selected { ACCENT } else { CHIP };
                let label = format!("{} ({})", div.short_name(), div_count);
                if pill_button(ui, &label, fill, ACCENT_HOVER, Vec2::new(115.0, 26.0), true).clicked() {
                    self.sf_division = Some(div);
                }
            }
        });
        ui.add_space(6.0);

        // --- SEARCH BAR & STATS TOOLBAR ---
        ui.horizontal(|ui| {
            ui.label(bold("Search:", 12.0).color(TEXT_MUTED));
            let _r = ui.add(
                egui::TextEdit::singleline(&mut self.sf_search)
                    .hint_text("Filter by stop name or soundfont file...")
                    .desired_width(260.0),
            );
            if !self.sf_search.is_empty() {
                if pill_button(ui, "Clear", CHIP, ACCENT_HOVER, Vec2::new(60.0, 24.0), true).clicked() {
                    self.sf_search.clear();
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let active_sf = sf_mgr.active_preset_count();
                if active_sf > 0 {
                    badge(ui, &format!("Active Stops: {}", active_sf), GREEN);
                }
                if let Some(div) = self.sf_division {
                    badge(ui, div.title(), ACCENT);
                } else {
                    badge(ui, "All Library Divisions", ACCENT);
                }
            });
        });
        ui.add_space(8.0);

        // --- FILTER PRESETS ACCORDING TO DIVISION & SEARCH ---
        let search_lower = self.sf_search.trim().to_lowercase();
        let filtered: Vec<&crate::soundfont::SoundFontPreset> = self
            .sf_presets
            .iter()
            .filter(|p| {
                if let Some(div) = self.sf_division {
                    if p.division != div {
                        return false;
                    }
                }
                if !search_lower.is_empty() {
                    if !p.name.to_lowercase().contains(&search_lower)
                        && !p.font_name.to_lowercase().contains(&search_lower)
                    {
                        return false;
                    }
                }
                true
            })
            .collect();

        if filtered.is_empty() {
            inner_card(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(bold("No stops match your filter query", 14.0).color(TEXT_MUTED));
                    ui.add_space(20.0);
                });
            });
            return;
        }

        // --- FOOTAGE SUBSECTIONS IN VERTICAL SCROLL AREA ---
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for footage in Footage::ALL {
                    let section_presets: Vec<&&crate::soundfont::SoundFontPreset> = filtered
                        .iter()
                        .filter(|p| p.footage == footage)
                        .collect();

                    if section_presets.is_empty() {
                        continue;
                    }

                    // Section Header
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(bold(footage.title(), 13.0).color(ACCENT));
                        ui.label(
                            RichText::new(format!("({} stops)", section_presets.len()))
                                .size(11.0)
                                .color(TEXT_MUTED),
                        );
                        let sep_rect = ui.available_rect_before_wrap();
                        let y = sep_rect.center().y;
                        ui.painter().line_segment(
                            [Pos2::new(sep_rect.min.x + 8.0, y), Pos2::new(sep_rect.max.x - 8.0, y)],
                            Stroke::new(1.0, BORDER),
                        );
                    });
                    ui.add_space(4.0);

                    // 3 Columns of stop cards
                    ui.columns(3, |cols| {
                        for (i, &&preset) in section_presets.iter().enumerate() {
                            let col_idx = i % 3;
                            let is_active = sf_mgr.is_preset_active(preset.id);
                            let sub = format!("{} (B:{} P:{})", preset.font_name, preset.bank, preset.patch);

                            cols[col_idx].add_space(3.0);
                            let resp = stop_card_button(
                                &mut cols[col_idx],
                                &preset.name,
                                &sub,
                                preset.footage.badge(),
                                is_active,
                            );

                            if resp.clicked() {
                                sf_mgr.toggle_preset(preset.id);
                                sf_changed = true;
                            }
                        }
                    });

                    ui.add_space(6.0);
                }
            });

        drop(sf_mgr);
        if sf_changed {
            self.engine.request_rerender();
        }
    }

    fn stops_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut changed = false;
        card(ui, |ui| {
            // --- CONSOLE TAB SWITCHER (PAGE 1 vs PAGE 2) ---
            ui.horizontal(|ui| {
                let p1_active = self.console_tab == ConsoleTab::PrincipalOrgan;
                let p1_fill = if p1_active { ACCENT } else { CHIP };
                if pill_button(ui, "🏛️ Principal Organ (Page 1)", p1_fill, ACCENT_HOVER, Vec2::new(210.0, 32.0), true).clicked() {
                    self.console_tab = ConsoleTab::PrincipalOrgan;
                }

                let sf_count = self.sf_presets.len();
                let sf_title = if sf_count > 0 {
                    format!(" Imported Ranks & Samples (Page 2) [{}]", sf_count)
                } else {
                    " Imported Ranks & Samples (Page 2)".to_string()
                };
                let p2_active = self.console_tab == ConsoleTab::SoundFonts;
                let p2_fill = if p2_active { ACCENT } else { CHIP };
                if pill_button(ui, &sf_title, p2_fill, ACCENT_HOVER, Vec2::new(260.0, 32.0), true).clicked() {
                    self.console_tab = ConsoleTab::SoundFonts;
                }

                // So VCAA if ur reading this I didn't write this myself, AI did because I was really badly running out of time and I just couldn't afford to learn rust. All of the software written for this except some of the POC and most of the console control arduino whatever code is AI written, because I'm not smart enough to learn C.
                // why is coding so hard? I genuinely have no idea what Layouut::right_to_lef(Align::Center) means, apart from maybe like centering a button?? This entire thing is so absurdly overengineered and I made this project so stupidly because I knew damn well that I didn't have the resources, knowledge, or motivation to complete this stupid project in time. All I have to show for my work is a shitty incomplete folio and some half arsed soldering and code, as well as AI generated software and a shitty 3D Printed key test mechanism that is sluggish and also overcomplicated. I'm screwed.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let active_p1 = self.active_ids().len();
                    let active_sf = self.engine.soundfont.lock().active_preset_count();
                    let total_active = active_p1 + active_sf;
                    let text = if total_active == 0 {
                        "No stops drawn (0 active ranks)".to_string()
                    } else if active_sf == 0 {
                        format!("Active Organ Ranks: {} drawn", active_p1)
                    } else if active_p1 == 0 {
                        format!("Active Organ Ranks: {} drawn (from samples)", active_sf)
                    } else {
                        format!("Active Organ Ranks: {} drawn ({} native, {} sampled)", total_active, active_p1, active_sf)
                    };
                    ui.label(
                        RichText::new(text)
                            .size(12.0)
                            .color(if total_active > 0 { GREEN } else { RED }),
                    );
                });
            });
            ui.add_space(8.0);

            // --- PAGE CONTENT ---
            match self.console_tab {
                ConsoleTab::PrincipalOrgan => {
                    let n = self.groups.len();
                    let groups = self.groups.clone();
                    ui.columns(n, |cols| {
                        for (col, (group, idxs)) in cols.iter_mut().zip(groups.iter()) {
                            inner_card(col, |ui| {
                                ui.label(bold(group.title(), 13.0).color(ACCENT));
                                ui.add_space(4.0);
                                for &i in idxs {
                                    let name = STOPS[i].name;
                                    let r = ui.checkbox(
                                        &mut self.stop_on[i],
                                        RichText::new(name).size(13.0).color(TEXT_FG),
                                    );
                                    if r.changed() {
                                        changed = true;
                                    }
                                }
                            });
                        }
                    });
                }
                ConsoleTab::SoundFonts => {
                    self.soundfont_page_ui(ui);
                }
            }

            ui.add_space(8.0);

            // --- CONSOLE ACTION BAR & PRESETS ---
            ui.horizontal(|ui| {
                match self.console_tab {
                    ConsoleTab::PrincipalOrgan => {
                        if pill_button(ui, "Select All Stops", CHIP, ACCENT_HOVER, Vec2::new(140.0, 32.0), true)
                            .clicked()
                        {
                            self.set_all_stops(true);
                        }
                        if pill_button(ui, "Clear All Stops", CHIP, ACCENT_HOVER, Vec2::new(140.0, 32.0), true)
                            .clicked()
                        {
                            self.set_all_stops(false);
                        }
                    }
                    ConsoleTab::SoundFonts => {
                        let sf_active_any = self.engine.soundfont.lock().active_preset_count() > 0;
                        if pill_button(ui, "Clear SoundFont Stops", CHIP, ACCENT_HOVER, Vec2::new(170.0, 32.0), sf_active_any)
                            .clicked()
                        {
                            self.engine.soundfont.lock().clear_all_presets();
                            self.engine.request_rerender();
                        }
                    }
                }

                ui.add_space(12.0);
                ui.label(RichText::new("Presets").size(12.0).color(TEXT_MUTED));
                for k in 1..=9usize {
                    let filled = self
                        .settings
                        .presets
                        .get(&k.to_string())
                        .map(|v| !v.is_empty())
                        .unwrap_or(false);
                    let base = if filled { ACCENT } else { CHIP };
                    let r = pill_button(ui, &k.to_string(), base, ACCENT_HOVER, Vec2::new(28.0, 28.0), true)
                        .on_hover_text(format!(
                            "Click or press {k} to load. Right-click or Shift+{k} to save."
                        ));
                    if r.clicked() {
                        self.load_preset(k);
                    }
                    if r.secondary_clicked() {
                        self.save_preset(k);
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill_button(ui, "Clear Audio Cache", CHIP, AMBER_HOVER, Vec2::new(150.0, 32.0), true)
                        .clicked()
                    {
                        self.engine.clear_caches();
                        self.toast("Audio RAM cache cleared successfully!", GREEN);
                    }
                    if pill_button(
                        ui,
                        "Import Samples",
                        CHIP,
                        GREEN,
                        Vec2::new(140.0, 32.0),
                        self.busy == Busy::Idle,
                    )
                    .clicked()
                    {
                        self.start_import_dialog(ctx);
                    }
                });
            });
        });
        if changed {
            self.apply_stops();
        }
    }

    fn settings_col(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.label(bold("System Configuration", 15.0).color(ACCENT));
            ui.add_space(6.0);

            inner_card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(bold("Master Tuning (A4):", 13.0).color(TEXT_FG));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let before = self.tuning;
                        egui::ComboBox::from_id_salt("tuning")
                            .selected_text(format!("{} Hz", self.tuning))
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for t in TUNINGS {
                                    ui.selectable_value(&mut self.tuning, t, format!("{t} Hz"));
                                }
                            });
                        if before != self.tuning {
                            self.engine.set_a4(self.tuning as f64);
                            self.engine.refresh();
                            self.settings.tuning = self.tuning;
                            self.settings.save();
                        }
                    });
                });
            });
            ui.add_space(6.0);

            // Historical Temperament selector
            inner_card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(bold("Temperament:", 13.0).color(TEXT_FG));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let before = self.temperament;
                        egui::ComboBox::from_id_salt("temperament")
                            .selected_text(self.temperament.short_name())
                            .width(155.0)
                            .show_ui(ui, |ui| {
                                for &t in &Temperament::ALL {
                                    ui.selectable_value(&mut self.temperament, t, t.name());
                                }
                            });
                        if before != self.temperament {
                            self.engine.set_temperament(self.temperament);
                            self.engine.refresh();
                            self.settings.temperament = self.temperament.name().to_string();
                            self.settings.save();
                            self.toast(format!("Temperament: {}", self.temperament.name()), GREEN);
                        }
                    });
                });
            });
            ui.add_space(6.0);

            // Physically Modeled Acoustic Venue
            inner_card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(bold("Acoustic Venue:", 13.0).color(TEXT_FG));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let before = self.venue;
                        egui::ComboBox::from_id_salt("venue")
                            .selected_text(self.venue.short_name())
                            .width(170.0)
                            .show_ui(ui, |ui| {
                                for &v in &AcousticVenue::ALL {
                                    let specs = v.specs();
                                    let text = format!("{} (RT60: {:.1}s)", v.short_name(), specs.rt60_seconds);
                                    ui.selectable_value(&mut self.venue, v, text);
                                }
                            });
                        if before != self.venue {
                            self.engine.set_venue(self.venue);
                            self.settings.venue = self.venue.name().to_string();
                            self.settings.save();
                            let specs = self.venue.specs();
                            self.toast(
                                format!("Venue: {} (RT60: {:.1}s)", self.venue.short_name(), specs.rt60_seconds),
                                GREEN,
                            );
                            self.engine.request_rerender();
                        }
                    });
                });
            });
            ui.add_space(6.0);

            inner_card(ui, |ui| {
                ui.label(bold("MIDI Input Device:", 13.0).color(TEXT_FG));
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let mut choice = self.midi_selected.clone();
                    egui::ComboBox::from_id_salt("midi_dev")
                        .selected_text(choice.clone())
                        .width(ui.available_width() - 80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut choice, "None".to_string(), "None");
                            for d in self.midi_devices.clone() {
                                ui.selectable_value(&mut choice, d.clone(), d);
                            }
                        });
                    if choice != self.midi_selected {
                        self.select_midi(&choice);
                    }
                    if pill_button(ui, "Refresh", ACCENT, ACCENT_HOVER, Vec2::new(68.0, 28.0), true)
                        .clicked()
                    {
                        self.refresh_midi_devices();
                    }
                });
            });
            ui.add_space(6.0);

            inner_card(ui, |ui| {
                ui.label(bold("Master Volume:", 13.0).color(TEXT_FG));
                ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(80.0);
                let r = ui.add(
                    egui::Slider::new(&mut self.volume, 0.0..=1.0)
                        .show_value(true)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                        .custom_parser(|s| s.trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.0)),
                );
                if r.changed() {
                    self.engine.set_volume(self.volume);
                }
                if r.drag_stopped() {
                    self.settings.volume = self.volume;
                    self.settings.save();
                }
            });
        });
    }

    fn visualizer_col(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.label(bold("Pitch Class Resonance", 15.0).color(ACCENT));
            ui.add_space(6.0);
            let size = 210.0;
            ui.vertical_centered(|ui| {
                let (resp, painter) = ui.allocate_painter(Vec2::splat(size), Sense::hover());
                let center = resp.rect.center();
                let radius = size / 2.0 - 28.0;
                painter.rect_filled(resp.rect, CornerRadius::same(10), DARK_BG);
                let pos = |i: usize, r: f32| -> Pos2 {
                    let a = (i as f32 / 12.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                    Pos2::new(center.x + r * a.cos(), center.y + r * a.sin())
                };
                for i in 0..12 {
                    painter.circle_filled(pos(i, radius), 3.0, Color32::from_rgb(0x4B, 0x55, 0x63));
                    painter.text(
                        pos(i, radius + 18.0),
                        Align2::CENTER_CENTER,
                        PITCH_NAMES[i],
                        FontId::new(12.0, bold_family()),
                        TEXT_MUTED,
                    );
                }
                let active: Vec<usize> = (0..12).filter(|&i| self.pcs[i]).collect();
                if active.len() >= 2 {
                    let pts: Vec<Pos2> = active.iter().map(|&i| pos(i, radius)).collect();
                    if pts.len() == 2 {
                        painter.line_segment([pts[0], pts[1]], Stroke::new(2.0, ACCENT));
                    } else {
                        painter.add(Shape::closed_line(pts, Stroke::new(2.0, ACCENT)));
                    }
                }
                for &i in &active {
                    painter.circle_filled(pos(i, radius), 9.0, ACCENT.gamma_multiply(0.25));
                    painter.circle_filled(pos(i, radius), 6.0, ACCENT);
                }
            });
        });
    }

    fn playback_col(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        card(ui, |ui| {
            ui.label(bold("Playback & Export", 15.0).color(ACCENT));
            ui.add_space(6.0);

            let idle = self.busy == Busy::Idle;
            let load_label = match self.busy {
                Busy::Synth => "Synthesizing... Please Wait",
                Busy::Dialog => "Waiting for file...",
                _ => "Load & Play MIDI",
            };
            let export_label = if self.busy == Busy::Export {
                "Exporting..."
            } else {
                "Export WAV"
            };
            let w = (ui.available_width() - 8.0) / 2.0;
            ui.horizontal(|ui| {
                if pill_button(ui, load_label, ACCENT, ACCENT_HOVER, Vec2::new(w, 38.0), idle).clicked() {
                    self.start_load_dialog(ctx);
                }
                if pill_button(ui, export_label, GREEN, GREEN_HOVER, Vec2::new(w, 38.0), idle).clicked() {
                    self.start_export(ctx);
                }
            });
            ui.add_space(8.0);
            let w3 = (ui.available_width() - 16.0) / 3.0;
            ui.horizontal(|ui| {
                if pill_button(ui, "Pause", AMBER, AMBER_HOVER, Vec2::new(w3, 34.0), true).clicked() {
                    self.engine.pause();
                }
                if pill_button(ui, "Resume", GREEN, GREEN_HOVER, Vec2::new(w3, 34.0), true).clicked() {
                    if !self.has_any_active_stops() {
                        self.toast("No selected stops! Please draw at least one stop before playing.", RED);
                    } else {
                        self.engine.resume();
                    }
                }
                if pill_button(ui, "Stop", RED, RED_HOVER, Vec2::new(w3, 34.0), true).clicked() {
                    self.engine.stop_playback();
                }
            });
            ui.add_space(10.0);

            let (pos, total) = self.engine.progress();
            let frac = if total > 0.0 { (pos / total).clamp(0.0, 1.0) } else { 0.0 };
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(4), BORDER);
            let mut fill = rect;
            fill.set_width(rect.width() * frac);
            ui.painter().rect_filled(fill, CornerRadius::same(4), ACCENT);
            ui.add_space(4.0);
            let state = if self.engine.is_playing() {
                if self.engine.is_paused() { "Paused" } else { "Playing" }
            } else {
                "Stopped"
            };
            let mut line = format!("{state}   {} / {}", fmt_time(pos), fmt_time(total));
            if self.engine.is_rendering() {
                line.push_str("   (re-rendering with new stops...)");
            }
            ui.label(RichText::new(line).size(12.0).color(TEXT_MUTED));
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some((msg, at, color)) = &self.toast {
                if at.elapsed() < Duration::from_secs(5) {
                    ui.label(RichText::new(msg.clone()).size(13.0).color(*color));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| match &self.audio {
                Ok(a) => {
                    ui.label(
                        RichText::new(format!(
                            "Audio: {} Hz out (engine {} Hz)  |  Keys: 1-9 load preset, Shift+1-9 save",
                            a.device_rate, SAMPLE_RATE
                        ))
                        .size(11.0)
                        .color(TEXT_MUTED),
                    );
                }
                Err(e) => {
                    ui.label(RichText::new(format!("Audio error: {e}")).size(12.0).color(RED));
                }
            });
        });
    }
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(DARK_BG)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(bold(text, 12.0).color(color));
        });
}

fn fmt_time(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

impl eframe::App for OrgelApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_messages(ctx);
        self.pump_serial();
        self.handle_keys(ctx);

        // visualizer + A4 servo
        let (pcs, a4) = self.engine.visual_state();
        self.pcs = pcs;
        if a4 != self.last_a4 {
            self.last_a4 = a4;
            self.link.send_line(if a4 { "A4_1\n" } else { "A4_0\n" });
        }
        if self.engine.song_finished.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.toast("Playback finished", TEXT_MUTED);
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Frame::new()
            .fill(DARK_BG)
            .inner_margin(Margin::same(18))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    self.header(ui);
                    ui.add_space(10.0);
                    self.stops_panel(ui, &ctx);
                    ui.add_space(10.0);
                    ui.columns(3, |cols| {
                        self.settings_col(&mut cols[0]);
                        self.visualizer_col(&mut cols[1]);
                        self.playback_col(&mut cols[2], &ctx);
                    });
                    ui.add_space(10.0);
                    self.status_bar(ui);
                });
            });
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let load = |path: &str| std::fs::read(path).ok();

    if let Some(regular) = load("C:\\Windows\\Fonts\\segoeui.ttf") {
        fonts
            .font_data
            .insert("segoeui".into(), Arc::new(FontData::from_owned(regular)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "segoeui".into());
    }
    if let Some(sym) = load("C:\\Windows\\Fonts\\seguisym.ttf") {
        fonts
            .font_data
            .insert("seguisym".into(), Arc::new(FontData::from_owned(sym)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("seguisym".into());
    }
    // bold family: Segoe UI Bold when present, otherwise fall back to the regular stack
    let mut bold_stack: Vec<String> = Vec::new();
    if let Some(b) = load("C:\\Windows\\Fonts\\segoeuib.ttf") {
        fonts
            .font_data
            .insert("segoeuib".into(), Arc::new(FontData::from_owned(b)));
        bold_stack.push("segoeuib".into());
    }
    let prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    bold_stack.extend(prop);
    fonts.families.insert(bold_family(), bold_stack);
    ctx.set_fonts(fonts);
}

fn setup_visuals(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = DARK_BG;
    v.window_fill = DARK_BG;
    v.extreme_bg_color = DARK_BG;
    v.faint_bg_color = PANEL_BG;
    v.override_text_color = None;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.noninteractive.bg_fill = PANEL_BG;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_FG);
    v.widgets.inactive.bg_fill = CHIP;
    v.widgets.inactive.weak_bg_fill = CHIP;
    v.widgets.inactive.corner_radius = CornerRadius::same(6);
    v.widgets.hovered.bg_fill = ACCENT_HOVER;
    v.widgets.hovered.weak_bg_fill = ACCENT_HOVER;
    v.widgets.hovered.corner_radius = CornerRadius::same(6);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.corner_radius = CornerRadius::same(6);
    v.widgets.open.corner_radius = CornerRadius::same(6);
    ctx.set_visuals(v);
    let _ = StrokeKind::Inside;
}
