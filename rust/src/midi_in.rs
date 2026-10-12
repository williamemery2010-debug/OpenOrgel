//! LIVE MIDI INPUT - midir replaces the hand-rolled WinMM ctypes callback.
//! Note-on / note-off straight into the engine's voice table.
//! menthol

use midir::{Ignore, MidiInput, MidiInputConnection};
use std::sync::Arc;

use crate::engine::Engine;

/// Names of all MIDI input ports currently visible.
pub fn list_inputs() -> Vec<String> {
    let Ok(mut mi) = MidiInput::new("openorgel-scan") else {
        return Vec::new();
    };
    mi.ignore(Ignore::All);
    mi.ports()
        .iter()
        .filter_map(|p| mi.port_name(p).ok())
        .collect()
}

pub struct MidiConnection {
    _conn: MidiInputConnection<()>,
}

/// Open the input called `name`. Keep the returned value alive to stay connected.
pub fn open_input(name: &str, engine: Arc<Engine>) -> Result<MidiConnection, String> {
    let mut mi = MidiInput::new("openorgel").map_err(|e| e.to_string())?;
    mi.ignore(Ignore::All);
    let port = mi
        .ports()
        .into_iter()
        .find(|p| mi.port_name(p).map(|n| n == name).unwrap_or(false))
        .ok_or_else(|| format!("MIDI device not found: {name}"))?;

    let conn = mi
        .connect(
            &port,
            "openorgel-in",
            move |_stamp, msg, _| {
                if msg.len() < 3 {
                    return;
                }
                let status = msg[0] & 0xF0;
                let note = msg[1];
                let vel = msg[2];
                match status {
                    0x90 if vel > 0 => engine.note_on(note),
                    0x80 | 0x90 => engine.note_off(note),
                    _ => {}
                }
            },
            (),
        )
        .map_err(|e| format!("Failed to open MIDI input device: {e}"))?;
    Ok(MidiConnection { _conn: conn })
}
