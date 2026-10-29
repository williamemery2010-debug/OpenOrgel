//! STOP REGISTRY - the one true list of console stops and their C++ engine ids.
//!
//! The ids MUST match the `STOPS_DB[]` indices in `openorgelsynth.cpp` / `STOP_DEFINITIONS` in `stops_dsp.rs`.
//! Oboe 8' (0) and Vox Humana 8' (16) are intentionally absent: they were
//! evicted from the console, and the engine simply never gets asked for them.

/// Which column of the console a stop lives in, grouped by organological pipe footage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// 32' Sub-Bass Pedal & Manual Ranks (sounding two octaves below written pitch).
    Ft32,
    /// 16' Manual & Pedal Foundation Ranks (sounding one octave below written pitch).
    Ft16,
    /// 8' Unison Foundation, Flute, String & Reed Ranks (sounding at written concert pitch).
    Ft8,
    /// 4' Octave Principal, Flute, String & Reed Ranks (sounding one octave above written pitch).
    Ft4,
    /// 2' Super-Octave Principal & Flute Ranks (sounding two octaves above written pitch).
    Ft2,
    /// Solo & Harmonic Mutation Ranks (Nazard 2 2/3', Tierce 1 3/5', Larigot 1 1/3', Septième 1 1/7', Sifflöte 1', Sesquialtera).
    Mutations,
    /// Multi-rank Compound Stops, Choruses & Mixtures.
    Special,
}

impl Group {
    /// Fixed left-to-right column display order in the Organ Console GUI.
    pub const ORDER: [Group; 7] = [
        Group::Ft32,
        Group::Ft16,
        Group::Ft8,
        Group::Ft4,
        Group::Ft2,
        Group::Mutations,
        Group::Special,
    ];

    /// Human-readable column header displayed above each stop group in the GUI.
    pub fn title(self) -> &'static str {
        match self {
            Group::Ft32 => "32' Ranks",
            Group::Ft16 => "16' Ranks",
            Group::Ft8 => "8' Ranks",
            Group::Ft4 => "4' Ranks",
            Group::Ft2 => "2' Ranks",
            Group::Mutations => "Mutations",
            Group::Special => "Mixtures & Special",
        }
    }
}

/// Metadata pairing a user-facing stop name with its internal DSP synthesis engine ID.
#[derive(Clone, Copy, Debug)]
pub struct StopInfo {
    pub name: &'static str,
    /// Index into `STOP_DEFINITIONS` in `rust/src/dsp/stops_dsp.rs`.
    pub id: i32,
}

/// Every stop the console exposes. Order here is irrelevant: the UI sorts
/// alphabetically inside each footage column, same as the legacy console.
pub const STOPS: &[StopInfo] = &[
    StopInfo { name: "Clarinet 8'", id: 1 },
    StopInfo { name: "Clarinet 4'", id: 8 },
    StopInfo { name: "Bassoon 16'", id: 2 },
    StopInfo { name: "Bombarde 16'", id: 3 },
    StopInfo { name: "Ophicleide 16'", id: 4 },
    StopInfo { name: "Ottavino 2'", id: 5 },
    StopInfo { name: "Cor Anglais 8'", id: 6 },
    StopInfo { name: "Flute 4'", id: 7 },
    StopInfo { name: "Viol 4'", id: 9 },
    StopInfo { name: "Contrabassoon 32'", id: 10 },
    StopInfo { name: "Diapason 8'", id: 11 },
    StopInfo { name: "Crystal Flute 4' (Glassy)", id: 12 },
    StopInfo { name: "Cornet V 8'", id: 13 },
    StopInfo { name: "Piccolo 2'", id: 14 },
    StopInfo { name: "Mixture IV", id: 15 },
    StopInfo { name: "Gedeckt 8'", id: 17 },
    StopInfo { name: "Gedeckt 4'", id: 18 },
    StopInfo { name: "Gedeckt 16'", id: 19 },
    StopInfo { name: "Gedeckt 32'", id: 20 },
    StopInfo { name: "Gedeckt 2'", id: 31 },
    StopInfo { name: "Cymbale Mixture", id: 21 },
    StopInfo { name: "Plein Jeu Mixture", id: 22 },
    StopInfo { name: "Scharf Mixture", id: 23 },
    StopInfo { name: "Voix Celeste 8'", id: 24 },
    StopInfo { name: "Acoustic Flue 8'", id: 25 },
    StopInfo { name: "Clarion 4'", id: 26 },
    StopInfo { name: "Acoustic Flue 16'", id: 27 },
    StopInfo { name: "Acoustic Flue 4'", id: 28 },
    StopInfo { name: "Acoustic Flue 2'", id: 29 },
    StopInfo { name: "Clarion 8'", id: 30 },
    StopInfo { name: "Principal Chorus", id: 32 },
    StopInfo { name: "Bourdon 32'", id: 33 },
    StopInfo { name: "Bourdon 16'", id: 34 },
    StopInfo { name: "Double Diapason 16'", id: 35 },
    StopInfo { name: "Subbass 16'", id: 36 },
    StopInfo { name: "Open Wood 16'", id: 37 },
    StopInfo { name: "Violone 16'", id: 38 },
    StopInfo { name: "Contra Trombone 16'", id: 39 },
    StopInfo { name: "Harmonic Flute 8'", id: 40 },
    StopInfo { name: "Hohlflöte 8'", id: 41 },
    StopInfo { name: "Melodia 8'", id: 42 },
    StopInfo { name: "Flauto Dolce 8'", id: 43 },
    StopInfo { name: "Rohrflöte 8'", id: 44 },
    StopInfo { name: "Trumpet 8'", id: 45 },
    StopInfo { name: "Trompette Harmonique 8'", id: 46 },
    StopInfo { name: "Orchestral Trumpet 8'", id: 47 },
    StopInfo { name: "State Trumpet 8'", id: 48 },
    StopInfo { name: "Cornopean 8'", id: 49 },
    StopInfo { name: "Trompette 4'", id: 50 },
    StopInfo { name: "Nazard 2 2/3'", id: 51 },
    StopInfo { name: "Tierce 1 3/5'", id: 52 },
    StopInfo { name: "Larigot 1 1/3'", id: 53 },
    StopInfo { name: "Septième 1 1/7'", id: 54 },
    StopInfo { name: "Sifflöte 1'", id: 55 },
    StopInfo { name: "Sesquialtera II", id: 56 },
];

/// The stop that is pulled out when the console first wakes up.
pub const DEFAULT_STOP: &str = "Diapason 8'";

/// Footage group of a stop, correctly routing 32', 16', 8', 4', 2', Mutations, and Mixtures.
pub fn group_of(name: &str) -> Group {
    if name.contains(" 32'") {
        Group::Ft32
    } else if name.contains(" 16'") {
        Group::Ft16
    } else if name.contains(" 8'") {
        Group::Ft8
    } else if name.contains(" 4'") {
        Group::Ft4
    } else if name.contains(" 2 2/3'")
        || name.contains(" 1 3/5'")
        || name.contains(" 1 1/3'")
        || name.contains(" 1 1/7'")
        || name.contains(" 1'")
        || name.contains("Sesquialtera")
        || name.contains("Nazard")
        || name.contains("Tierce")
        || name.contains("Larigot")
        || name.contains("Sifflöte")
        || name.contains("Septième")
    {
        Group::Mutations
    } else if name.contains(" 2'") {
        Group::Ft2
    } else {
        Group::Special
    }
}

/// Stops indices grouped for display, alphabetical within each group.
pub fn grouped() -> Vec<(Group, Vec<usize>)> {
    Group::ORDER
        .iter()
        .filter_map(|&g| {
            let mut idx: Vec<usize> = (0..STOPS.len())
                .filter(|&i| group_of(STOPS[i].name) == g)
                .collect();
            idx.sort_by_key(|&i| STOPS[i].name);
            if idx.is_empty() {
                None
            } else {
                Some((g, idx))
            }
        })
        .collect()
}

/// Flat display order (what the legacy Arduino numeric fallback indexes into).
pub fn ordered_indices() -> Vec<usize> {
    grouped().into_iter().flat_map(|(_, v)| v).collect()
}

/// Looks up the index in `STOPS` matching `name`.
pub fn index_of(name: &str) -> Option<usize> {
    STOPS.iter().position(|s| s.name == name)
}
