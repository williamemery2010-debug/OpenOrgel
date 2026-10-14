# OpenOrgel - Acoustic Samples Repository

This directory contains the acoustic audio sample assets and SoundFont banks utilized by the OpenOrgel synthesis engine.

## Contents

1. **`clarion.wav` / `clarion.mp3`**:
   - High-fidelity acoustic recording of an authentic brass Clarion organ pipe.
   - Utilized by the native `Clarion 8'` stop (Stop ID 30) via linear pitch-interpolated wavetable looping in `rust/src/dsp/stops_dsp.rs`.

2. **`stoppedflue.wav` / `stoppedflue.mp3`**:
   - Recorded acoustic sample of a traditional stopped wooden flue pipe (Gedackt/Bourdon profile).
   - Utilized by native acoustic flue stops (Stop IDs 27, 28, 29).

3. **`Jeux14.SF2`**:
   - The renowned classical French symphonic pipe organ SoundFont bank (Jeux 1.4 by Christian Mégache).
   - Contains complete organ divisions (Grand-Orgue, Récit Expressif, Positif, Pédale) and comprehensive registrations (Montre, Prestant, Doublette, Plein Jeu, Trompette, Clairon, Bombarde, Cromorne, Voix Céleste).
   - **Click-Free Rework**: All 167 sample headers have been processed and sanitized to eliminate loop-boundary discontinuities and initial attack DC offset spikes, providing completely seamless acoustic playback in OpenOrgel.
