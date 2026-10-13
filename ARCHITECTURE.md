# OpenOrgel Architecture & Technical Specification

## 1. Executive Summary

**OpenOrgel** is a high-performance, real-time pipe organ synthesis platform and digital console. 

With **v2.0.0**, OpenOrgel transitioned to a **100% native Rust architecture**, completely replacing the legacy C++ DLL and slow additive synthesis with a cutting-edge **Wavetable + Frequency Modulation (FM)** synthesis suite:

- **100x+ Faster Synthesis**: Bandlimited 2048-sample Wavetable lookup tables replace billions of per-sample trigonometric evaluations with single-cycle RAM lookups.
- **Dynamic FM Reed Synthesis**: 2-Operator & 4-Operator Phase Modulation operators synthesize complex acoustic reed pipes (Trumpet, Clarion, Bombarde, Bassoon, Clarinet, Cor Anglais) with dynamic Bessel-function harmonic sidebands and organic shallot chiff transients.
- **Zero External C++ Compiler Dependencies**: The application compiles straight from `cargo build --release` in pure Rust with zero LLVM-MinGW or C++ toolchain prerequisites.
- **Pure Rust Schroeder Reverberator**: Built-in 4-comb + 2-allpass matrix delivering lush cathedral reverberation.

---

## 2. System Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                                 USER INTERFACE                                    |
|              egui / eframe (60 FPS Native Immediate-Mode GUI: app.rs)              |
|                                                                                   |
|  [Page 1: Principal Organ]     [Page 2: Imported Ranks & Samples]    [System Config]  |
|  - 30 Native Organ Ranks       - SoundFont 2 (.sf2) Parser           - Tuning (415-466)|
|  - 32', 16', 8', 4', 2', Mix   - Division Classifier (Swell/Great)   - MIDI Input Dev  |
|  - Registration Presets (1-9)  - Footage Sorter (32' down to Mix)    - Master Volume   |
|  - Visual Resonance Wheel      - Multi-Page Fast Filter & Search     - Playback/Export |
+------------------------------------------+----------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------+
|                        ENGINE COORDINATOR (engine.rs)                              |
|                                                                                   |
|   - Active Registration State (Atomic stop bitsets & Preset selection)            |
|   - Real-Time Voice Allocator (Live keyboard polyphony & pitch tracking)         |
|   - Dynamic Background Synthesizer (Parallel Rayon worker pool)                   |
|   - Non-Blocking Offline Snapshot (Thread-safe SoundFont cloning)                 |
|   - Seamless Mid-Song Morphing (1024-sample timeline cross-fade)                  |
+-------------------+-----------------------------------+---------------------------+
                    |                                   |
                    v                                   v
+-----------------------------------+   +-------------------------------------------+
|    NATIVE RUST WAVETABLE & FM DSP |   |        NATIVE SOUNDFONT ENGINE            |
|       (rust/src/dsp/ suite)       |   |        (soundfont.rs / rustysynth)        |
|                                   |   |                                           |
|  - 2048-sample bandlimited tables |   |  - SF2 sample extraction & RAM caching    |
|  - 2-Op & 4-Op FM reed operators  |   |  - Multi-velocity sample playback         |
|  - Organic windchests & chiff     |   |  - Interpolated looping & keyzones        |
|  - Native acoustic flue/clarion   |   |  - Multi-channel linear ADSR envelopes    |
+-------------------+---------------+   +-------------------+-----------------------+
                    \                                   /
                     \                                 /
                      v                               v
+-----------------------------------------------------------------------------------+
|                       MASTER MIXER & AUDIO PIPELINE (audio.rs)                    |
|                                                                                   |
|   - Native Sample Rate Output (cpal WASAPI stream @ 44.1 kHz / 48 kHz)            |
|   - Lock-Free Circular Buffers & Zero-Allocation Render Loop                      |
|   - Pure Rust Schroeder-Moorer Reverberator (combs + allpass matrix)              |
|   - Dynamic Soft Limiting & Master Volume Scaling                                 |
+------------------------------------------+----------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------+
|                         HARDWARE I/O & PERIPHERALS                                |
|                                                                                   |
|   [MIDI Input (midi_in.rs)]                 [Arduino Console Link (serial.rs)]    |
|   - midir hardware enumeration              - Serial COM port bidirectional link  |
|   - Zero-latency key press routing          - Hardware stop tablet sync & A4 servo|
+------------------------------------------+----------------------------------------+
```

---

## 3. Core DSP Subsystems (`rust/src/dsp/`)

### 3.1. Bandlimited Wavetable Synthesis (`wavetable.rs`)
- Replaces legacy multi-harmonic summation loops with 2048-sample single-cycle tables.
- Employs anti-aliased harmonic synthesis: partials approaching the Nyquist threshold (`sample_rate / 2`) are gracefully filtered to avoid inharmonic foldover distortion at higher registers.
- Fast linear interpolation with branchless power-of-two bitmask wrapping (`phase & 2047`).

### 3.2. Phase & Frequency Modulation (`fm.rs`)
- Models complex reed tongues striking the shallot:
  $$y(t) = \sin\left(\omega_c t + \beta(t) \cdot \sin(\omega_m t + \alpha \cdot y_m(t-1))\right)$$
- The dynamic modulation index $\beta(t)$ spikes during pipe attack to model the physical aerodynamic pressure transient ("chiff") before settling into steady-state brilliance.
- Harmonic ratios are tuned for specific pipe families:
  - **Clarinet 8' & 4'**: 1:2 ratio (emphasizes hollow odd harmonics).
  - **Bombarde & Bassoon 16'**: 0.5:1.0 with high index and feedback for brassy growl.
  - **Cor Anglais & Oboe**: 1:1 with formants.

### 3.3. Pure Rust Reverberator (`reverb.rs`)
- Implements a classic Schroeder-Moorer reverberation algorithm directly in safe Rust:
  - 4 parallel Comb filters with feedback damping.
  - 2 series All-pass filters for dense diffusion.
  - Replaces all legacy C++ FFI reverb pointers.

---

## 4. Source Tree & Component Map

| Path | Primary Function |
| :--- | :--- |
| `rust/src/main.rs` | Application entry point and egui window runner. |
| `rust/src/app.rs` | Graphical console GUI, stop tabs, sliders, keyboard shortcuts, and preset manager. |
| `rust/src/engine.rs` | Central audio engine coordinator, timeline rendering, cross-fading, and playback state. |
| `rust/src/dsp/` | **Native Rust DSP Suite**: |
| ├── `dsp/wavetable.rs` | Bandlimited 2048-sample wavetable oscillators and sine LUT. |
| ├── `dsp/fm.rs` | 2-Op & 4-Op phase-modulation reed and brass synthesis operators. |
| ├── `dsp/stops_dsp.rs`| Acoustic recipes and dispatchers for all 30 console stops. |
| └── `dsp/reverb.rs` | Pure Rust digital reverberator. |
| `rust/src/soundfont.rs` | High-level SoundFont manager, SF2 loader, offline snapshot generator, and real-time voice dispatcher. |
| `rust/src/classifier.rs` | Taxonomic classification engine: categorizes arbitrary SoundFont presets into divisions and footages. |
| `rust/src/stops.rs` | Registry of the 30 Principal Organ ranks and their footage grouping. |
| `rust/src/audio.rs` | `cpal` low-latency audio stream initialization and device management. |
| `rust/src/midi_in.rs` | Live MIDI hardware input capture. |
| `rust/src/serial.rs` | Background thread communicating with physical Arduino console hardware. |
| `rust/src/settings.rs` | Persistent user configuration stored in `%APPDATA%\OpenOrgel`. |
