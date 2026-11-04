# OpenOrgel: Open-Source Hybrid Pipe Organ & Synthesis Engine

[![Build & Publish Release](https://github.com/williamemery2010-debug/OpenOrgel/actions/workflows/build_and_release.yml/badge.svg)](https://github.com/williamemery2010-debug/OpenOrgel/actions/workflows/build_and_release.yml)
[![Tests Passing](https://img.shields.io/badge/Unit%20Tests-55%2F55%20Passing-brightgreen)](unit_tests_report.md)
[![Language: Rust](https://img.shields.io/badge/Language-Rust%202021-orange.svg)](rust/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: Concluded](https://img.shields.io/badge/Project%20Status-Concluded%20%2F%20Archived-lightgrey.svg)](RELEASE_NOTES.md)

---

## 🏛️ Project Status: Concluded & Abandoned Indefinitely

> [!NOTE]
> **Notice of Project Conclusion:**  
> OpenOrgel was engineered as a capstone project for the **VCE (Victorian Certificate of Education) Systems Engineering** assessment curriculum in Victoria, Australia.
> 
> Having successfully completed the curriculum and **passed the subject**, active development on this repository is now concluded, and the project is abandoned until further notice (and likely indefinitely).
>
> In addition to satisfying the academic curriculum, physical bench testing demonstrated real-world physical limits: continuous operation of the 3D-printed centrifugal blower under aerodynamic back-pressure revealed motor brush and commutator fatigue ("RUS") and 12V bench power supply current saturation under high rotor loads. Scaling the physical windchest further would require industrial-grade blower machinery outside the scope of an educational project.
>
> The software synthesis engine, DSP library, and hardware diagnostic suite stand as a 100% complete, highly optimized, production-ready virtual pipe organ implementation in native Rust.
>
> For full details, see the [Final Release Notes](RELEASE_NOTES.md).

---

## 📖 The Vision

In 2026, OpenOrgel was conceived to design and produce an accessible, affordable, consistent, and portable pipe organ whose documentation, mechanical design, electronics, and software are entirely open source.

The pipe organ—widely regarded as the "King of Instruments"—is renowned for its versatility, vast dynamic range, and rich cultural heritage spanning ancient Greek *Hydraulis* through contemporary concert halls. However, traditional pipe organs cost hundreds of thousands of dollars, take years to build, require dedicated architectural housing, and demand extensive ongoing maintenance. Consequently, educational access to the pipe organ has diminished globally.

OpenOrgel bridges this divide by uniting:
1. **Acoustic Sound Production:** 3D-printed PLA acoustic pipes powered by a closed-loop centrifugal blower.
2. **Digital Physical Modeling Synthesis:** A native Rust DSP engine rendering 8 chromatic octaves (C0 to C8), modeling open flues, stopped gedackts, reed pipes, compound mixtures, and historical temperaments.
3. **SoundFont 2 (SF2) Sampling:** Dynamic multi-bank soundfont streaming with automated click-free crossfading.
4. **Embedded Microcontroller Telemetry:** Real-time Arduino Mega wind regulation, pitch-locking tuner feedback, high-current motor driving, and electronic stall protection.
5. **Laboratory Diagnostic Suite:** Real-time motor amperage oscilloscope and session CSV telemetry logging.

---

## 🚀 Key Achievements Across the Project Lifecycle

Across 55 distinct engineering phases, the OpenOrgel project achieved:

- **100% Native Rust Synthesis Engine (`rust/`):**
  - Ultra-low latency audio processing powered by `cpal`, `rayon`, and lock-free crossbeam channels.
  - Multi-harmonic Chebyshev additive synthesis and band-limited Nyquist wavetable oscillators.
  - Slew-rate limited audio buses and seamless anti-click voice envelope handling.
- **Physical Flue & Reed Modeling:**
  - Open Diapason 8' singing crown with realistic harmonic decay.
  - Gedeckt stopped pipes emphasizing odd harmonics (3rd/5th partials) for warm, hollow flute timbres.
  - Cylindrical Clarinet and conical Trumpet/Clarion/Posaune reed models with formant filtering.
  - Dual-rank Voix Céleste with natural acoustic beating via fractional cent detuning.
  - 4-Rank Principal chorus and classical compound mixtures with octave breaks.
  - Chiff transient noise bursts and decorrelated aerodynamic wind flutter.
- **Historical Musical Temperaments:**
  - Real-time mathematical temperament calculation: Equal Temperament, Werckmeister III, Meantone, Kirnberger III, Pythagorean, and Silbermann.
- **Physically Modeled Architectural Reverberation:**
  - 8-comb 4-allpass Schroeder reverberator modeling Hamer Hall (Melbourne), Cathedral of St. Patrick, Baroque Church, and Dry Studio.
- **Hardware Closed-Loop Blower Regulation:**
  - Real-time microphone pitch detector (NSDF / Yin) locking blower RPM to a physical reference pipe (Eb5 / A440).
  - Arduino Mega 2560 firmware (`newconsole/newconsole.ino`) with bidirectional serial communication.
  - BTS7960 IBT-2 43A H-bridge driver control with analog current sensing and 8.5A electronic stall fuse.
  - 180° servo manifold throttle valve regulation and anti-jitter debounced potentiometers.
- **Diagnostic Instrumentation & GUI:**
  - Responsive GPU-accelerated virtual organ console (`egui` / `eframe`).
  - Real-time 15-second Amperage Oscilloscope with color-graded thresholds (Green <6A, Amber 6-8.5A, Red >8.5A).
  - Full-session RFC 4180 CSV telemetry export for external data analysis in Excel, Python, or MATLAB.
- **Deterministic Verification Suite:**
  - 55 rigorous unit tests passing with 100% green status, fully documented in [unit_tests_report.md](unit_tests_report.md).
- **Automated CI/CD & Windows Installer Suite:**
  - GitHub Actions CI workflow compiling for MSVC on Windows.
  - Inno Setup installer package and standalone Windows executable generation.

---

## 🛠️ Repository Structure

```text
OpenOrgel/
├── rust/                      # Native Rust synthesis engine & desktop application
│   ├── src/
│   │   ├── main.rs            # Application entrypoint
│   │   ├── app.rs             # Egui GUI console & diagnostic oscilloscope
│   │   ├── dsp/               # Digital signal processing (pipe, reed, wavetable, reverb)
│   │   ├── engine.rs          # Audio playback and voice allocation
│   │   ├── soundfont.rs       # SoundFont 2 (SF2) streaming engine
│   │   ├── serial.rs          # Serial communication & telemetry protocol
│   │   ├── temperament.rs     # Mathematical temperament frequency mappings
│   │   └── tuner.rs           # NSDF pitch detection & oscilloscope data buffers
│   └── Cargo.toml             # Rust dependencies and release profiles
├── newconsole/                # Microcontroller embedded firmware
│   └── newconsole.ino         # Arduino Mega blower PWM, current sensing & servo control
├── installer/                 # Packaging & distribution suite
│   ├── openorgel_setup.iss    # Inno Setup 6 installer script
│   ├── package_release.py     # Windows distribution packager
│   └── install.ps1            # Standalone PowerShell installer
├── samples/                   # Curated soundfont and sampled pipe audio assets
├── project_timeline.txt       # Technical WHAT, WHY, HOW engineering log (Phases 1-55)
├── unit_tests_report.md       # Comprehensive verification & unit test report (UT-1 to UT-55)
├── RELEASE_NOTES.md           # Exhaustive retrospective & release documentation
└── publish_github_release.py  # Direct GitHub Release publishing automation
```

---

## 💻 Building & Running

### Requirements
- **Rust Toolchain:** Stable Rust (2021 edition) with `cargo`.
- **Operating System:** Windows 10/11 (64-bit).

### Quick Build
```powershell
cd rust
cargo run --release
```

### Running Unit Tests
```powershell
cd rust
cargo test
```
*(All 55 tests pass deterministically)*

---

## 📜 Historical Documentation & Timeline

For the complete technical engineering history detailing every single commit, bug fix, mathematical model, and circuit redesign across all 55 phases, refer to:
- [project_timeline.txt](project_timeline.txt)
- [unit_tests_report.md](unit_tests_report.md)
- [commits_and_releases.txt](commits_and_releases.txt)

---

## ⚖️ License

OpenOrgel is released under the [MIT License](LICENSE). You are free to study, fork, modify, and build upon this work.
