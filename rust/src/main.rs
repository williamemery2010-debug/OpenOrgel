//! OpenOrgel - Native Rust Virtual Pipe Organ Synthesis Engine.
//!
//! Entry point for the desktop GUI application and real-time audio pipeline.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
pub mod dsp;
mod engine;
mod midi_in;
mod serial;
mod settings;
pub mod classifier;
pub mod soundfont;
mod stops;
pub mod temperament;
pub mod tuner;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(format!(
                "OpenOrgel - Virtual Pipe Organ Synthesis Engine (v{})",
                env!("CARGO_PKG_VERSION")
            ))
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "OpenOrgel",
        options,
        Box::new(|cc| Ok(Box::new(app::OrgelApp::new(cc)))),
    )
}
