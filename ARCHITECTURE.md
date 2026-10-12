# OpenOrgel Architecture & Technical Specification

## 1. Executive Summary

**OpenOrgel** is a high-performance, real-time pipe organ synthesis platform and digital console. 

Originally built as a Python prototype utilizing Tkinter and a C++ synthesis DLL, OpenOrgel has been migrated to a native **Rust application** (`openorgelsynth.exe`) interfacing directly with an optimized **C++ physical acoustics engine** (`openorgelsynth.cpp`).

This architecture achieves:
- **Instantaneous Startup & Minimal Footprint**: ~14 MB standalone executable running in ~25 MB of RAM (compared to ~400+ MB with Python/Tkinter).
- **Zero-Drop Real-Time Audio**: Low-latency, multi-threaded audio execution via `cpal` completely decoupled from UI rendering and background synthesis.
- **Dual-Layer Organ Sound Synthesis**: Combines physical modeling (aerodynamic chiff, pipe inertia, wind fluctuation) with high-fidelity sample-based ranks and multi-sampled SoundFont libraries.

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
|    C++ PHYSICAL MODELING ENGINE   |   |        NATIVE SOUNDFONT ENGINE            |
|   (openorgelsynth.cpp / ffi.rs)   |   |        (soundfont.rs / rustysynth)        |
|                                   |   |                                           |
|  - Aerodynamic windchests & chiff |   |  - SF2 sample extraction & RAM caching    |
|  - Additive harmonic series pipes |   |  - Multi-velocity sample playback         |
|  - Temperature & pitch acoustics  |   |  - Interpolated looping & keyzones        |
|  - Native acoustic flue/reed mp3s |   |  - Multi-channel linear ADSR envelopes    |
+-------------------+---------------+   +-------------------+-----------------------+
                    \                                   /
                     \                                 /
                      v                               v
+-----------------------------------------------------------------------------------+
|                       MASTER MIXER & AUDIO PIPELINE (audio.rs)                    |
|                                                                                   |
|   - Native Sample Rate Output (cpal WASAPI stream @ 44.1 kHz / 48 kHz)            |
|   - Lock-Free Circular Buffers & Zero-Allocation Render Loop                      |
|   - Stereo Digital Reverberation (Schroeder-Moorer matrix with room damping)      |
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
+-----------------------------------------------------------------------------------+
```

---

## 3. Core Subsystems & How They Work

### 3.1. Frontend Console (`rust/src/app.rs`)
- Built using **egui** (immediate mode). It redraws each frame at 60 FPS without memory leaks or DOM overhead.
- **Two Console Pages**:
  - **Page 1 (Principal Organ)**: The built-in 30 classical organ ranks classified into footages (32', 16', 8', 4', 2', and Mixtures).
  - **Page 2 (Imported Ranks & Samples)**: Hosts external `.sf2` SoundFont banks and sample libraries. Features an automated taxonomic engine (`classifier.rs`) that sorts every imported preset into classical organ divisions (**Pedal, Great, Swell, Choir, Solo, Misc**) and footages (**32', 16', 8', 4', 2', Mixtures**).
- **Unified Stop Counting**:
  - Samples and physical modeling are treated as **equal organ ranks**. 
  - The UI displays total combined active ranks: `Active Organ Ranks: X drawn (Y native, Z sampled)`.
  - If no ranks are drawn on either page, playback and export are refused (`"No selected stops!"`).

### 3.2. Dual-Layer Sound Generation
1. **Physical Modeling (C++ Layer - `openorgelsynth.cpp` via `ffi.rs`)**:
   - Computes real-time pipe physics for flue and reed pipes using additive Fourier series, transient attack chiff, wind supply pressure fluctuations, and acoustic inertia.
   - Houses built-in sampled stops (`Stopped Flue 8'` and `Clarion 4'`).
2. **Sample-Based SoundFonts (Rust Layer - `soundfont.rs`)**:
   - Parses `.sf2` SoundFont archives directly into RAM.
   - Synthesizes realistic sampled acoustic pipe organs with per-note tuning, sample loop points, multi-zone velocity layers, and release tails.
3. **Harmonic Cleanliness & Zero-Bleed**:
   - When no physical C++ ranks are active, the C++ layer outputs absolute silence (`0.0`). This guarantees SoundFont samples play with pure tone and authentic dynamics without C++ flue pipe bleed.

### 3.3. Non-Blocking Audio & Mid-Playback Re-Synthesis
- **Real-Time Audio Thread**: Runs via `cpal` on a dedicated high-priority thread.
- **Lock-Free Timeline Morphing**:
  - When the organist changes stops while a MIDI file is playing, OpenOrgel does **not pause or stutter**.
  - An `offline_snapshot` of active SoundFont instruments is taken and cloned in under 0.01 milliseconds.
  - A background Rayon thread pool re-synthesizes the timeline while the audio thread keeps playing the existing buffer.
  - When synthesis completes, [`swap_audio`](file:///c:/Users/EME0012/OpenOrgel/rust/src/engine.rs) blends the new registration into the audio stream using a **1024-sample cross-fade**, eliminating audio pops and clicks.

### 3.4. Hardware & MIDI Integration
- **MIDI Input (`midi_in.rs`)**: Interfaces with physical MIDI keyboards and organ pedalboards via `midir`. Supports hot-plugging and device selection from the settings panel.
- **Console Link (`serial.rs`)**: Connects to physical Arduino-powered console stop jambs via USB serial. It synchronizes physical motorized drawknobs and drives external servo motors (e.g. A4 tuning indicator).

---

## 4. Source Tree & Component Map

| Path | Primary Function |
| :--- | :--- |
| `rust/src/main.rs` | Application entry point and egui window runner. |
| `rust/src/app.rs` | Complete graphical console GUI, stop tabs, sliders, keyboard shortcuts, and preset manager. |
| `rust/src/engine.rs` | Central audio engine coordinator, tone banks, timeline rendering, cross-fading, and playback state. |
| `rust/src/soundfont.rs` | High-level SoundFont manager, SF2 loader, offline snapshot generator, and real-time voice dispatcher. |
| `rust/src/classifier.rs` | Organological taxonomy engine: categorizes arbitrary SoundFont presets into divisions and footages. |
| `rust/src/stops.rs` | Registry of the 30 Principal Organ ranks and their footage grouping. |
| `rust/src/audio.rs` | `cpal` low-latency audio stream initialization and device management. |
| `rust/src/ffi.rs` | Safe Rust FFI bindings to `openorgelsynth.cpp` C ABI. |
| `rust/src/midi_in.rs` | Live MIDI hardware input capture. |
| `rust/src/serial.rs` | Background thread communicating with physical Arduino console hardware. |
| `rust/src/settings.rs` | Persistent user configuration stored in `%APPDATA%\OpenOrgel`. |
| `openorgelsynth.cpp` | C++ pipe organ physical modeling and acoustic synthesis DLL. |

---

## 5. Why Are There Still Python Files in the Repository?

You may notice files like `newsynthesis.py`, `update_history.py`, and `build_distribution_package.py` in the workspace. Here is why each exists:

1. **`newsynthesis.py` (Historical Python Console)**:
   - This was the original Python + Tkinter GUI prototype.
   - It was kept during the transition to Rust to serve as an exact baseline for verifying 1:1 feature parity (all 30 stops, Arduino serial communication, MIDI parsing, and tuning formulas).
   - The native Rust app (`openorgelsynth.exe`) has completely superseded it for everyday playing.

2. **`update_history.py` (Active Automation Script)**:
   - This Python utility is used by git release automation to generate and maintain `commits_and_releases.txt` and package changelogs upon every release tag.

3. **`POC/HISTORICAL/` and Migration Helper Scripts**:
   - Files like `SynthesiserPOC.py` and `modify_*.py` are archival milestones and one-off conversion scripts from earlier developmental stages.
