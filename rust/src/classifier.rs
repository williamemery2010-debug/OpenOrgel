//! ORGAN STOP CLASSIFIER & DIVISION MATRIX
//!
//! Categorizes arbitrary SoundFont and imported organ stops into traditional
//! pipe divisions (Great, Swell, Choir/Positiv, Pedal, Solo, Misc) and
//! speaking footage pitch categories (64', 32', 16', 8', 4', Mutations, 2', 1', Mixtures, Misc).

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Division {
    Pedal,
    Great,
    Swell,
    Choir,
    Solo,
    Misc,
}

impl Division {
    pub const ALL: [Division; 6] = [
        Division::Pedal,
        Division::Great,
        Division::Swell,
        Division::Choir,
        Division::Solo,
        Division::Misc,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Division::Pedal => "Pedal Division",
            Division::Great => "Great (Hauptwerk / Grand Orgue)",
            Division::Swell => "Swell (Schwellwerk / Récit)",
            Division::Choir => "Choir & Positiv (Positif / Rückpositiv)",
            Division::Solo => "Solo & Orchestral (Bombarde)",
            Division::Misc => "Other Library Instruments",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Division::Pedal => "Pedal",
            Division::Great => "Great",
            Division::Swell => "Swell",
            Division::Choir => "Choir/Positiv",
            Division::Solo => "Solo/Orch",
            Division::Misc => "Misc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Footage {
    Ft64,
    Ft32,
    Ft16,
    Ft8,
    Ft4,
    Mutation,
    Ft2,
    Ft1,
    Mixture,
    Misc,
}

impl Footage {
    pub const ALL: [Footage; 10] = [
        Footage::Ft64,
        Footage::Ft32,
        Footage::Ft16,
        Footage::Ft8,
        Footage::Ft4,
        Footage::Mutation,
        Footage::Ft2,
        Footage::Ft1,
        Footage::Mixture,
        Footage::Misc,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Footage::Ft64 => "64' Sub-Bass Ranks",
            Footage::Ft32 => "32' Deep Bass Ranks",
            Footage::Ft16 => "16' Foundation & Bass Ranks",
            Footage::Ft8 => "8' Unison Foundation Ranks",
            Footage::Ft4 => "4' Octave & Principal Ranks",
            Footage::Mutation => "Mutations (2-2/3', 1-3/5', 1-1/3', etc.)",
            Footage::Ft2 => "2' Super-Octave & Fifteenth Ranks",
            Footage::Ft1 => "1' High Octave Ranks",
            Footage::Mixture => "Mixtures, Cornets & Compounds",
            Footage::Misc => "Miscellaneous & Unpitched",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Footage::Ft64 => "64'",
            Footage::Ft32 => "32'",
            Footage::Ft16 => "16'",
            Footage::Ft8 => "8'",
            Footage::Ft4 => "4'",
            Footage::Mutation => "Mut",
            Footage::Ft2 => "2'",
            Footage::Ft1 => "1'",
            Footage::Mixture => "Mix",
            Footage::Misc => "Misc",
        }
    }
}

// ============================================================================
// TAXONOMIC HEURISTIC ENGINE
// ============================================================================

/// Determine the footage of a stop from its name and characteristics.
pub fn classify_footage(raw_name: &str) -> Footage {
    let lower = raw_name.to_lowercase();

    // 1. Explicit pitch notation
    if lower.contains("64'") || lower.contains("64ft") || lower.contains("gravissima") {
        return Footage::Ft64;
    }
    if lower.contains("32'") || lower.contains("32ft") {
        return Footage::Ft32;
    }
    if lower.contains("16'") || lower.contains("16ft") {
        return Footage::Ft16;
    }
    if lower.contains("8'") || lower.contains("8ft") {
        return Footage::Ft8;
    }
    if lower.contains("4'") || lower.contains("4ft") {
        return Footage::Ft4;
    }
    if lower.contains("2 2/3") || lower.contains("2-2/3") || lower.contains("2.67")
        || lower.contains("1 3/5") || lower.contains("1-3/5") || lower.contains("1.6")
        || lower.contains("1 1/3") || lower.contains("1-1/3") || lower.contains("1.33")
        || lower.contains("10 2/3") || lower.contains("5 1/3") || lower.contains("3 1/5")
    {
        return Footage::Mutation;
    }
    if lower.contains("2'") || lower.contains("2ft") {
        return Footage::Ft2;
    }
    if lower.contains("1'") || lower.contains("1ft") {
        return Footage::Ft1;
    }

    // 2. Mixture roman numerals & keywords
    if lower.contains("mixture")
        || lower.contains("mixtur")
        || lower.contains("plein jeu")
        || lower.contains("fourniture")
        || lower.contains("cymbale")
        || lower.contains("cymbel")
        || lower.contains("scharf")
        || lower.contains("sharp")
        || lower.contains("cornet")
        || lower.contains("sesquialtera")
        || lower.contains("rauschpfeife")
        || lower.contains("acuta")
        || lower.contains("carillon")
        || lower.contains("harmonia aetheria")
        || lower.contains("terzian")
        || lower.contains(" ii")
        || lower.contains(" iii")
        || lower.contains(" iv")
        || lower.contains(" v")
        || lower.contains(" vi")
        || lower.contains(" vii")
    {
        return Footage::Mixture;
    }

    // 3. Known Mutation Names
    if lower.contains("nazard")
        || lower.contains("nasard")
        || lower.contains("twelfth")
        || lower.contains("tierce")
        || lower.contains("terz")
        || lower.contains("larigot")
        || lower.contains("quinte")
        || lower.contains("quint")
        || lower.contains("septieme")
        || lower.contains("none")
    {
        return Footage::Mutation;
    }

    // 4. Known 2' names
    if lower.contains("fifteenth")
        || lower.contains("super octave")
        || lower.contains("superoctave")
        || lower.contains("doublette")
        || lower.contains("piccolo")
        || lower.contains("flageolet")
        || lower.contains("blockflote")
        || lower.contains("blockflöte")
        || lower.contains("waldflote")
        || lower.contains("waldflöte")
    {
        return Footage::Ft2;
    }

    // 5. Known 4' names
    if lower.contains("prestant")
        || lower.contains("octave")
        || lower.contains("oktav")
        || lower.contains("salicet")
        || lower.contains("fugara")
        || lower.contains("dulcet")
    {
        return Footage::Ft4;
    }

    // 6. Known 16' / 32' bass names
    if lower.contains("subbass")
        || lower.contains("sub-bass")
        || lower.contains("soubasse")
        || lower.contains("contra")
        || lower.contains("kontra")
        || lower.contains("double diapason")
        || lower.contains("majorbass")
        || lower.contains("violone")
        || lower.contains("posaune")
        || lower.contains("trombone")
        || lower.contains("bombarde")
        || lower.contains("bassoon")
        || lower.contains("fagotto")
        || lower.contains("bass")
    {
        return Footage::Ft16;
    }

    // 7. Known 8' standard foundations
    if lower.contains("diapason")
        || lower.contains("montre")
        || lower.contains("principal")
        || lower.contains("prinzipal")
        || lower.contains("flute")
        || lower.contains("flöte")
        || lower.contains("gedeckt")
        || lower.contains("gedackt")
        || lower.contains("bourdon")
        || lower.contains("salicional")
        || lower.contains("gamba")
        || lower.contains("celeste")
        || lower.contains("céleste")
        || lower.contains("trumpet")
        || lower.contains("trompette")
        || lower.contains("tromba")
        || lower.contains("oboe")
        || lower.contains("hautbois")
        || lower.contains("clarinet")
        || lower.contains("clarinette")
        || lower.contains("cornopean")
        || lower.contains("krummhorn")
        || lower.contains("cromorne")
        || lower.contains("tuba")
        || lower.contains("vox humana")
        || lower.contains("voix humaine")
        || lower.contains("cor anglais")
        || lower.contains("gemshorn")
        || lower.contains("spitzflote")
        || lower.contains("spitzflöte")
        || lower.contains("doucaine")
        || lower.contains("dulciana")
        || lower.contains("clarabella")
        || lower.contains("melodia")
    {
        return Footage::Ft8;
    }

    Footage::Misc
}

/// Classify an imported stop into its typical organ division (Great, Swell, Choir, Pedal, Solo, Misc).
pub fn classify_division(raw_name: &str) -> Division {
    let lower = raw_name.to_lowercase();

    // Explicit division tags in name (e.g., "Ped. Subbass", "Sw. Oboe", "Gt. Diapason", "Ch. Clarinet")
    if lower.starts_with("ped") || lower.contains("pedal") || lower.contains("péd") {
        return Division::Pedal;
    }
    if lower.starts_with("sw") || lower.contains("swell") || lower.contains("récit") || lower.contains("recit") {
        return Division::Swell;
    }
    if lower.starts_with("gt") || lower.contains("great") || lower.contains("hauptwerk") || lower.contains("grand orgue") {
        return Division::Great;
    }
    if lower.starts_with("ch") || lower.contains("choir") || lower.contains("positif") || lower.contains("positiv") || lower.contains("rückpositiv") {
        return Division::Choir;
    }
    if lower.starts_with("so") || lower.contains("solo") || lower.contains("bombarde division") {
        return Division::Solo;
    }

    // 1. PEDAL DIVISION
    // 32', 64', deep sub-basses, heavy reeds, open woods
    if lower.contains("64'")
        || lower.contains("32'")
        || lower.contains("subbass")
        || lower.contains("sub-bass")
        || lower.contains("soubasse")
        || lower.contains("open wood")
        || lower.contains("majorbass")
        || lower.contains("choralbass")
        || lower.contains("choral bass")
        || lower.contains("posaune")
        || lower.contains("trombone")
        || lower.contains("bombarde")
        || lower.contains("violone")
        || lower.contains("contra violone")
        || lower.contains("contra bass")
        || lower.contains("contrabass")
        || lower.contains("kontrabass")
        || lower.contains("kontrabassoon")
        || lower.contains("contrabassoon")
        || lower.contains("contra bourdon")
        || lower.contains("untersatz")
    {
        return Division::Pedal;
    }

    // 2. SOLO DIVISION
    // Fanfare reeds, orchestral specialties, extreme high pressure
    if lower.contains("tuba")
        || lower.contains("tuba mirabilis")
        || lower.contains("trompette militaire")
        || lower.contains("post horn")
        || lower.contains("french horn")
        || lower.contains("orchestral horn")
        || lower.contains("stentor")
        || lower.contains("philomela")
        || lower.contains("grand gamba")
        || lower.contains("chimes")
        || lower.contains("carillon")
        || lower.contains("harp")
        || lower.contains("glockenspiel")
    {
        return Division::Solo;
    }

    // 3. SWELL DIVISION
    // Expressive reeds, celestes, ethereal strings, closed flutes
    if lower.contains("celeste")
        || lower.contains("céleste")
        || lower.contains("voix celeste")
        || lower.contains("cornopean")
        || lower.contains("oboe")
        || lower.contains("hautbois")
        || lower.contains("basson-hautbois")
        || lower.contains("vox humana")
        || lower.contains("voix humaine")
        || lower.contains("salicional")
        || lower.contains("flauto traverso")
        || lower.contains("traverse flute")
        || lower.contains("traversière")
        || lower.contains("lieblich")
        || lower.contains("gedeckt")
        || lower.contains("gedackt")
        || lower.contains("gemshorn")
        || lower.contains("contra fagotto")
        || lower.contains("fagotto")
        || lower.contains("flûte octaviante")
        || lower.contains("salicet")
        || lower.contains("viol d'orchestre")
        || lower.contains("viole")
    {
        return Division::Swell;
    }

    // 4. CHOIR / POSITIV DIVISION
    // Agile mutations, gentle accompaniment, woodwind reeds
    if lower.contains("krummhorn")
        || lower.contains("crumhorn")
        || lower.contains("cromorne")
        || lower.contains("clarinet")
        || lower.contains("clarinette")
        || lower.contains("cor anglais")
        || lower.contains("english horn")
        || lower.contains("dulciana")
        || lower.contains("dulcet")
        || lower.contains("quintadena")
        || lower.contains("quintaton")
        || lower.contains("nazard")
        || lower.contains("nasard")
        || lower.contains("larigot")
        || lower.contains("tierce")
        || lower.contains("sifflote")
        || lower.contains("sifflöte")
        || lower.contains("koppelflote")
        || lower.contains("koppelflöte")
        || lower.contains("spitzflote")
        || lower.contains("spitzflöte")
        || lower.contains("sesquialtera")
        || lower.contains("concert flute")
        || lower.contains("nason")
    {
        return Division::Choir;
    }

    // 5. GREAT DIVISION
    // Chorus backbone: Diapasons, Montres, Prestants, Mixtures, Chorus Trumpets
    if lower.contains("diapason")
        || lower.contains("montre")
        || lower.contains("open diapason")
        || lower.contains("principal")
        || lower.contains("prinzipal")
        || lower.contains("prestant")
        || lower.contains("harmonic flute")
        || lower.contains("flûte harmonique")
        || lower.contains("hohlflote")
        || lower.contains("hohlflöte")
        || lower.contains("rohrflote")
        || lower.contains("rohrflöte")
        || lower.contains("stopped diapason")
        || lower.contains("clarabella")
        || lower.contains("twelfth")
        || lower.contains("fifteenth")
        || lower.contains("super octave")
        || lower.contains("doublette")
        || lower.contains("mixture")
        || lower.contains("mixtur")
        || lower.contains("fourniture")
        || lower.contains("plein jeu")
        || lower.contains("scharf")
        || lower.contains("trumpet")
        || lower.contains("trompette")
        || lower.contains("clarion")
        || lower.contains("clairon")
    {
        return Division::Great;
    }

    // 6. Generic organ checks
    if lower.contains("flute") || lower.contains("flöte") || lower.contains("bourdon") {
        return Division::Great;
    }

    Division::Misc
}
