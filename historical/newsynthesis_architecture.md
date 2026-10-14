# OpenOrgel: `newsynthesis.py` Architecture Deep Dive

The `newsynthesis.py` script is the core digital engine of the OpenOrgel project. It acts as a hybrid software synthesizer, MIDI player, and hardware bridge. Below is a detailed breakdown of its components and how they interact with the broader physical organ system.

## 1. Constants & Configuration (Lines 1-17)
The script imports necessary libraries for audio processing (`numpy`, `sounddevice`), UI (`tkinter`), MIDI handling (`mido`), and hardware interfacing (`pyserial`). It establishes a `SAMPLE_RATE` of 44100Hz and creates a local cache directory (`organ_audio_cache`) to save generated audio waveforms, drastically speeding up subsequent playback of the same tones.

## 2. The Stop Definitions (`STOPS` Dictionary) (Lines 19-141)
This section defines the "DNA" of the digital organ voices. Each stop (e.g., "Oboe 8'", "Diapason 8'") is mapped to:
- **`harmonics`**: The specific overtone series for that pipe type (e.g., flutes have fewer harmonics, reeds like Bombarde have many).
- **`amplitudes`**: The relative volume of each harmonic.
These profiles are used by the synthesis engine to digitally construct voices that are physically missing from the acoustic portion of the organ (such as the 32' Contrabassoon, which would physically require a 32-foot pipe).

## 3. Core Audio Engine: `generate_raw_tone` (Lines 143-220)
This is the heart of the physics-based additive synthesizer. It doesn't just play static waveforms; it mathematically simulates the physical imperfections of real wind-driven pipes:
- **Pitch Scoop**: Simulates the slight flat-to-sharp pitch bend as wind pressure initially builds in the pipe.
- **Organic Drift**: Adds slow, natural wandering to the pitch so it doesn't sound artificially perfect.
- **Chiff / Breath Noise**: Adds a burst of white noise at the attack of the note to simulate air hitting the pipe's labium (mouth).
- **Material Dampening**: Applies EQ filtering based on pipe footage (e.g., 2' and 4' metal pipes retain high frequencies, while 8' and 16' wooden pipes absorb them).
- **Inharmonicity**: Causes higher overtones to drift slightly sharp, a natural phenomenon in acoustic instruments.
It uses vectorized NumPy operations in chunks to perform this complex math efficiently.

## 4. MIDI Processing & Effects: `_generate_audio_buffer` (Lines 290-394)
When a user loads a MIDI file, this function converts the MIDI data into raw audio.
- **Optimization (Tone Cache)**: It calculates the maximum duration needed for every pitch. It then checks the hard drive cache. If it has already generated a "Diapason 8' + Flute 4' at 440Hz for 2 seconds" before, it loads it instantly instead of recalculating the math, enabling fast load times.
- **Envelopes**: Applies a smooth volume attack and release (ADSR) to prevent clicking sounds when notes start or stop.
- **Acoustic Dampening (Facade)**: Applies a low-pass filter to simulate the sound passing through the physical wooden cabinet of the organ.
- **Digital Reverb**: Implements a multi-tap delay line. It splits the signal into bass and treble, applying longer decay times to bass frequencies to simulate the acoustic spatial depth of a large church or hall.

## 5. Audio Playback System (Lines 223-288, 396-412)
- **`audio_callback`**: This function is constantly called by the `sounddevice` library. It feeds the generated audio buffer to the computer's sound card in small chunks (frames).
- **Background Rendering**: If the user flips a switch to add a new stop *while* a song is playing, the `request_re_render` function triggers a background thread (`_background_re_render`) to recalculate the entire audio buffer with the new voices without stopping the current playback.

## 6. Hardware Integration: `listen_for_arduino` (Lines 442-479)
This is where the software talks to the physical OpenOrgel console.
- **Auto-Detection**: `find_arduino_port` scans the computer's USB ports to find the connected Arduino microcontroller.
- **Serial Listener**: It runs in a continuous background thread, listening for text commands from the Arduino over serial (e.g., `"Oboe 8':1"` means the switch was turned ON).
- **UI Sync**: When it receives a command, it programmatically clicks the corresponding Tkinter Checkbutton in the software GUI. Because the Checkbuttons are wired to trigger the `request_re_render` function, flipping a physical switch on the console instantly updates the digital audio engine.

## 7. Graphical User Interface (GUI) (Lines 482-588)
Built using Tkinter, it provides a control panel for the user:
- **Stop Switches**: Checkbuttons dynamically generated from the `STOPS` dictionary, grouped by footage length (32', 16', 8', etc.).
- **Tuning**: A dropdown menu to change the master A4 tuning (e.g., from modern 440Hz to baroque 415Hz).
- **Visualizer (`update_visualization`)**: A real-time graphical display that draws shapes on a circular layout (similar to a circle of fifths/chromatic circle) representing the notes currently sounding in the audio buffer.
- **Controls**: Buttons to load MIDI files, export the current synthesis to a standard `.wav` file, and pause/resume/stop playback.

## Summary of Interaction Flow
1. **User Action**: The organist flips a physical switch labeled "Oboe 8'" on the wooden console.
2. **Hardware**: The Arduino detects the switch closure and sends `"Oboe 8':1"` over the USB Serial cable.
3. **Software Listener**: `listen_for_arduino` in Python reads the serial message and toggles the hidden "Oboe 8'" Tkinter variable to `True`.
4. **Render Trigger**: The Tkinter variable change triggers `request_re_render()`.
5. **Synthesis Engine**: The `_generate_audio_buffer` looks up the Oboe's harmonics in `STOPS`, calculates the complex waveform in `generate_raw_tone` (adding wind noise and pitch drift), applies reverb, and updates the active audio buffer.
6. **Output**: The `audio_callback` feeds this new, rich digital tone alongside the acoustic pipes sounding in the real world.