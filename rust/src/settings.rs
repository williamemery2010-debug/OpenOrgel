//! SETTINGS & PRESETS - persisted as JSON in %APPDATA%\OpenOrgel\.
//! Presets survive restarts now, unlike the legacy console where they
//! evaporated like diethyl ether in a warm lab. t-BuLi

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
}

fn default_temperament() -> String {
    "Equal Temperament (12-EDO)".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            presets: (1..=9).map(|i| (i.to_string(), Vec::new())).collect(),
            tuning: 440,
            volume: 1.0,
            last_midi_device: "None".to_string(),
            temperament: default_temperament(),
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
