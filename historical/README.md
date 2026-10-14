# OpenOrgel - Historical Codebase & Prototype Archive

This directory preserves early proof-of-concept prototypes, legacy C++ implementations, and historical engineering iterations of the OpenOrgel project.

## Historical Archive Contents

1. **`newsynthesis.py` & `newsynthesis_architecture.md`**:
   - Initial Python/Tkinter prototype of the organ synthesis engine and graphical console interface.
   - Used during early research phases to test additive wavetable partials and keyboard event dispatch.

2. **`openorgelsynth.cpp`, `openorgelsynth.dll`, `dr_mp3.h`**:
   - The intermediate C++ synthesis engine compiled as a Windows dynamic link library (DLL).
   - Provided multi-threaded wavetable generation and MP3 sample loading via `dr_mp3` prior to the 100% native Rust engine overhaul.

3. **Scratch & Development Tools**:
   - `modify_celeste.py`, `modify_cpp.py`, `modify_cpp_2.py`, `modify_python.py`: Scratch automation scripts used during incremental refactoring.
   - `scratch_pitch_probe.exe`: Prototype command-line diagnostic tool for measuring pitch frequencies and MIDI velocity curves.
   - `build_distribution_package.py`: Early packaging script superseded by the modern `installer/package_release.py` framework.
   - `ARCHITECTURE.txt`: Early text notes describing legacy component relationships.

## Academic Significance (VCAA Assessment Folio)

These artifacts are maintained in the repository to provide complete engineering provenance, documenting the architectural evolution from Python prototype to C++ DLL and ultimately to the ultra-low latency, zero-dependency 100% native Rust engine (`Prealpha_v2.0.x`).
