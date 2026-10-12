//! STOP REGISTRY - the one true list of console stops and their C++ engine ids.
//!
//! The ids MUST match the `STOPS_DB[]` indices in `openorgelsynth.cpp`.
//! Oboe 8' (0) and Vox Humana 8' (16) are intentionally absent: they were
//! evicted from the console, and the engine simply never gets asked for them.

/// Which column of the console a stop lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Ft32,
    Ft16,
    Ft8,
    Ft4,
    Ft2,
    Special,
}

impl Group {
    pub const ORDER: [Group; 6] = [
        Group::Ft32,
        Group::Ft16,
        Group::Ft8,
        Group::Ft4,
        Group::Ft2,
        Group::Special,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Group::Ft32 => "32' Ranks",
            Group::Ft16 => "16' Ranks",
            Group::Ft8 => "8' Ranks",
            Group::Ft4 => "4' Ranks",
            Group::Ft2 => "2' Ranks",
            Group::Special => "Mixtures & Special",
        }
    }

    fn marker(self) -> Option<&'static str> {
        match self {
            Group::Ft32 => Some(" 32'"),
            Group::Ft16 => Some(" 16'"),
            Group::Ft8 => Some(" 8'"),
            Group::Ft4 => Some(" 4'"),
            Group::Ft2 => Some(" 2'"),
            Group::Special => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StopInfo {
    pub name: &'static str,
    /// Index into the C++ `STOPS_DB` table.
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
];

/// The stop that is pulled out when the console first wakes up.
pub const DEFAULT_STOP: &str = "Diapason 8'";

/// Footage group of a stop, using the same substring rule as the legacy GUI.
pub fn group_of(name: &str) -> Group {
    for g in Group::ORDER {
        if let Some(m) = g.marker() {
            if name.contains(m) {
                return g;
            }
        }
    }
    Group::Special
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

pub fn index_of(name: &str) -> Option<usize> {
    STOPS.iter().position(|s| s.name == name)
}
