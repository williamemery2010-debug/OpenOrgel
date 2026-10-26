//! SETTINGS & PRESETS - persisted as JSON in %APPDATA%\OpenOrgel\.
//! Presets, master volume, tuning, temperament, and acoustic venue settings survive restarts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    /// "1".."9" -> stop names that were drawn when the preset was saved
    pub presets: BTreeMap<String, Vec<String>>,
    pub tuning: u32,
    pub volume: f32,
    pub last_midi_device: String,
    #[serde(default = "default_temperament")]
    pub temperament: String,
    #[serde(default = "default_venue")]
    pub venue: String,
    #[serde(default = "default_chiff_volume")]
    pub chiff_volume: f32,
    #[serde(default = "default_dynamics")]
    pub dynamics: f32,
}

fn default_temperament() -> String {
    "Equal Temperament (12-EDO)".to_string()
}

fn default_venue() -> String {
    "Hamer Hall (Arts Centre Melbourne)".to_string()
}

fn default_chiff_volume() -> f32 {
    0.20
}

fn default_dynamics() -> f32 {
    1.00
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            presets: (1..=9).map(|i| (i.to_string(), Vec::new())).collect(),
            tuning: 440,
            volume: 1.0,
            last_midi_device: "None".to_string(),
            temperament: default_temperament(),
            venue: default_venue(),
            chiff_volume: default_chiff_volume(),
            dynamics: default_dynamics(),
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    let dir = PathBuf::from(base).join("OpenOrgel");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("settings.json"))
}

impl Settings {
    pub fn load() -> Settings {
        let mut s: Settings = settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|txt| serde_json::from_str(&txt).ok())
            .unwrap_or_default();
        for i in 1..=9 {
            s.presets.entry(i.to_string()).or_default();
        }
        s
    }

    pub fn save(&self) {
        if let Some(p) = settings_path() {
            if let Ok(txt) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, txt);
            }
        }
    }
}
