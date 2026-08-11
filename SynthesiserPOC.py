import numpy as np
import sounddevice as sd
import tkinter as tk
import threading
from tkinter import filedialog, messagebox
import mido
import math
import time
import wave
import concurrent.futures
import serial
from serial.tools import list_ports

SAMPLE_RATE = 44100

# Organ Stops Definitions
STOPS = {
    "Oboe 8'": {
        "harmonics": np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
        "amplitudes": np.array([0.5, 0.3, 1.0, 0.7, 0.4, 0.3, 0.2, 0.15, 0.1, 0.05])
    },
    "Clarinet 8'": {
        "harmonics": np.array([1, 2, 3, 4, 5, 6, 7, 8, 9]),
        "amplitudes": np.array([1.0, 0.05, 0.5, 0.02, 0.2, 0.01, 0.1, 0.01, 0.05])
    },
    "Bassoon 16'": {
        "harmonics": np.array([0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0]),
        "amplitudes": np.array([1.0, 0.6, 0.4, 0.3, 0.2, 0.15, 0.12, 0.1, 0.08, 0.06, 0.05, 0.04])
    },
    "Bombarde 16'": {
        "harmonics": np.array([0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0]),
        "amplitudes": np.array([1.0, 0.8, 0.5, 0.3, 0.15, 0.08, 0.03, 0.01])
    },
    "Ophicleide 16'": {
        "harmonics": np.array([0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0]),
        "amplitudes": np.array([1.0, 0.7, 0.8, 0.5, 0.4, 0.25, 0.15, 0.1, 0.05, 0.02])
    },
    "Ottavino 2'": {
        "harmonics": np.array([4.0, 8.0, 12.0]),
        "amplitudes": np.array([1.0, 0.15, 0.05])
    },
    "Cor Anglais 8'": {
        "harmonics": np.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]),
        "amplitudes": np.array([1.0, 0.3, 0.8, 0.2, 0.5, 0.1, 0.2])
    },
    "Flute 4'": {
        "harmonics": np.array([2.0, 4.0, 6.0]),
        "amplitudes": np.array([1.0, 0.15, 0.05])
    },
    "Clarinet 4'": {
        "harmonics": np.array([2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0]),
        "amplitudes": np.array([1.0, 0.05, 0.5, 0.02, 0.2, 0.01, 0.1, 0.01, 0.05])
    },
    "Viol 4'": {
        "harmonics": np.array([2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0]),
        "amplitudes": np.array([1.0, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1])
    },
    "Contrabassoon 32'": {
        "harmonics": np.array([0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0]),
        "amplitudes": np.array([1.0, 0.8, 0.9, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05, 0.02])
    },
    "Echo Flute 8' (Soft)": {
        "harmonics": np.array([1.0, 2.0, 3.0, 4.0]),
        "amplitudes": np.array([0.4, 0.03, 0.08, 0.01])
    },
    "Diapason 8'": {
        "harmonics": np.array([1.0, 3.0, 5.0]),
        "amplitudes": np.array([1.0, 0.35, 0.05])
    },
    "Crystal Flute 4' (Glassy)": {
        "harmonics": np.array([2.0, 4.0, 6.0, 8.0, 16.0]),
        "amplitudes": np.array([1.0, 0.02, 0.1, 0.01, 0.03])
    },
    "Cornet 8'": {
        "harmonics": np.array([1.0, 2.0, 3.0, 4.0, 5.0]),
        "amplitudes": np.array([1.0, 0.8, 0.9, 0.7, 0.8])
    },
    "Bass Cornet 16'": {
        "harmonics": np.array([0.5, 1.0, 1.5, 2.0, 2.5]),
        "amplitudes": np.array([1.0, 0.8, 0.9, 0.7, 0.8])
    },
    "Gedeckt 8' (Hollow)": {
        "harmonics": np.array([1.0, 3.0, 5.0, 7.0]),
        "amplitudes": np.array([1.0, 0.3, 0.05, 0.01])
    },
    "Gedeckt 4' (Hollow)": {
        "harmonics": np.array([2.0, 6.0, 10.0, 14.0]),
        "amplitudes": np.array([1.0, 0.3, 0.05, 0.01])
    },
    "Piccolo 2'": {
        "harmonics": np.array([4.0, 8.0, 12.0, 16.0]),
        "amplitudes": np.array([1.0, 0.1, 0.05, 0.01])
    },
    "Recorder 8'": {
        "harmonics": np.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
        "amplitudes": np.array([1.0, 0.6, 0.4, 0.1, 0.05, 0.02])
    },
    "Recorder 4'": {
        "harmonics": np.array([2.0, 4.0, 6.0, 8.0, 10.0, 12.0]),
        "amplitudes": np.array([1.0, 0.6, 0.4, 0.1, 0.05, 0.02])
    },
    "Mixture IV": {
        "harmonics": np.array([4.0, 6.0, 8.0, 12.0]),
        "amplitudes": np.array([1.0, 0.8, 0.6, 0.4])
    },
    "Vox Humana 8'": {
        "harmonics": np.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
        "amplitudes": np.array([1.0, 0.4, 0.8, 0.2, 0.6, 0.1, 0.05, 0.02])
    },
    "Gedeckt 16'": {
        "harmonics": np.array([0.5, 1.5, 2.5, 3.5]),
        "amplitudes": np.array([1.0, 0.3, 0.05, 0.01])
    },
    "Gedeckt 32'": {
        "harmonics": np.array([0.25, 0.75, 1.25, 1.75]),
        "amplitudes": np.array([1.0, 0.3, 0.05, 0.01])
    }
}

def generate_raw_tone(freq, total_duration, active_stops):
    num_samples = int(SAMPLE_RATE * total_duration)
    if num_samples <= 0:
        return np.array([])

    t = np.linspace(0, total_duration, num_samples, False)
    
    # Organic Pitch Nuances
    # 1. Pitch Scoop: Wind pressure build-up causes a slight flat-to-sharp swoop on attack
    pitch_scoop_phase = 0.001 * np.exp(-20.0 * t)
    # 2. Organic Drift: Slow, natural pitch wandering
    drift_phase = 0.00004 * np.sin(2.1 * 2 * np.pi * t) + 0.00002 * np.sin(3.7 * 2 * np.pi * t)
    
    base_phase_t = t + pitch_scoop_phase + drift_phase
    two_pi_t = 2 * np.pi * base_phase_t
    
    wave = np.zeros_like(t)

    # Chiff / Breath Noise: Essential texture and airiness at the attack of flutes
    chiff_env = np.exp(-t * 15)
    breath_noise = np.random.normal(0, 1.0, num_samples)
    wave += breath_noise * chiff_env * np.sin(freq * 2 * np.pi * t) * 0.05
    wave += breath_noise * chiff_env * np.sin(freq * 2 * np.pi * t) * 0.08

    if not active_stops:
        phase = np.random.uniform(0, 2 * np.pi)
        wave += np.sin(freq * two_pi_t + phase)
    else:
        all_f = []
        all_amp = []
        
        for stop_name in active_stops:
            stop = STOPS[stop_name]
            
            # Simulate material dampening based on pipe footage
            if "2'" in stop_name or "4'" in stop_name:
                # Metal pipes: reflect and retain higher frequencies (brighter tone)
                dampening = 0.02
            else:
                # 8', 16', 32': wood/metal mix naturally absorbs higher harmonics (warmer tone)
                dampening = 0.08

            for amp, h in zip(stop["amplitudes"], stop["harmonics"]):
                # Apply dampening to upper harmonics (h > 1.0)
                adj_amp = amp * np.exp(-dampening * max(0, h - 1.0))
                
                # Inharmonicity: higher harmonics naturally drift sharp (less "digital")
                f = freq * h * (1.0 + 0.00015 * (h ** 2))
                
                # High-mid and high-end EQ boost (adds brilliance and presence)
                if f > 800:
                    treble_boost = min(2.5, 1.0 + ((f - 800) / 2500))
                    adj_amp *= treble_boost

                all_f.extend([f, f * 1.0015, f * 0.9985])
                all_amp.extend([adj_amp, adj_amp * 0.35, adj_amp * 0.35])

        if all_f:
            all_f = np.array(all_f)[:, None]
            all_amp = np.array(all_amp)[:, None]
            all_phase = np.random.uniform(0, 2 * np.pi, size=(len(all_f), 1))
            
            # Vectorized wave generation in chunks to massively reduce synthesis time
            chunk_size = 50
            for i in range(0, len(all_f), chunk_size):
                f_chunk = all_f[i:i+chunk_size]
                amp_chunk = all_amp[i:i+chunk_size]
                phase_chunk = all_phase[i:i+chunk_size]
                
                phases = f_chunk * two_pi_t + phase_chunk
                np.sin(phases, out=phases) # In-place sine evaluation
                phases *= amp_chunk
                wave += np.sum(phases, axis=0)

    # Add airiness (constant background wind noise)
    wave += np.random.normal(0, 0.002, num_samples)

    # Extremely light tremulant (amplitude modulation at 5.5 Hz, 0.5% depth)
    tremulant = 1.0 + 0.005 * np.sin(5.5 * 2 * np.pi * t)
    wave *= tremulant

    # Bass boost
    if freq < 250:
        wave *= (250 / freq) ** 0.5

    wave /= np.max(np.abs(wave) + 1e-9)

    return wave


# Playback State Globals
current_audio = None
playback_idx = 0
is_playing = False
is_paused = False
audio_stream = None
playback_notes = []
visualizer_start_time = 0
current_midi_file = None
is_rendering = False
render_requested = False
global_a4_freq = 440.0

def request_re_render():
    global is_rendering, render_requested
    if is_playing and current_midi_file:
        if not is_rendering:
            is_rendering = True
            threading.Thread(target=_background_re_render, daemon=True).start()
        else:
            render_requested = True

def _background_re_render():
    global is_rendering, render_requested, current_audio
    while True:
        try:
            new_audio, _ = _generate_audio_buffer(current_midi_file)
            if new_audio is not None:
                current_audio = np.float32(new_audio)
        except Exception as e:
            print("Background render failed:", e)
        
        if render_requested:
            render_requested = False
        else:
            is_rendering = False
            break

def reset_ui_state():
    global is_paused
    is_paused = False

def audio_callback(outdata, frames, time_info, status):
    global current_audio, playback_idx, is_playing, is_paused

    if not is_playing or current_audio is None:
        outdata.fill(0)
        raise sd.CallbackStop()

    if is_paused:
        outdata.fill(0)
        return

    chunk_size = min(frames, len(current_audio) - playback_idx)
    if chunk_size > 0:
        outdata[:chunk_size, 0] = current_audio[playback_idx:playback_idx + chunk_size]
        playback_idx += chunk_size
        
    if chunk_size < frames:
        outdata[chunk_size:].fill(0)
        is_playing = False
        root.after(0, reset_ui_state)
        raise sd.CallbackStop()

def midi_to_freq(note):
    return global_a4_freq * (2 ** ((note - 69) / 12))


def load_and_play_midi():
    global current_midi_file
    file_path = filedialog.askopenfilename(filetypes=[("MIDI files", "*.mid *.midi")])
    if not file_path:
        return

    current_midi_file = file_path

    # Update UI to show processing state and prevent multiple clicks
    load_btn.config(state=tk.DISABLED, text="Synthesizing... Please Wait")
    try:
        export_btn.config(state=tk.DISABLED)
    except NameError:
        pass
    root.update()

    # Run synthesis in a background thread so the GUI doesn't freeze
    threading.Thread(target=_process_and_play, args=(file_path,), daemon=True).start()

def export_to_wav():
    file_path = filedialog.askopenfilename(filetypes=[("MIDI files", "*.mid *.midi")])
    if not file_path:
        return
        
    save_path = filedialog.asksaveasfilename(defaultextension=".wav", filetypes=[("WAV files", "*.wav")])
    if not save_path:
        return

    load_btn.config(state=tk.DISABLED)
    try:
        export_btn.config(state=tk.DISABLED, text="Exporting...")
    except NameError:
        pass
    root.update()

    threading.Thread(target=_process_and_play, args=(file_path, save_path), daemon=True).start()

def _generate_audio_buffer(file_path):
    mid = mido.MidiFile(file_path)

    current_time = 0
    events = []

    # collect note events
    for msg in mid:
        current_time += msg.time
        if msg.type == 'note_on' and msg.velocity > 0:
            events.append((current_time, 'on', msg.note))
        elif msg.type in ['note_off', 'note_on']:
            events.append((current_time, 'off', msg.note))

    if not events:
        return None, []

    active_stops = [stop for stop, var in stop_vars.items() if var.get()]

    # --- Optimize: Find max duration per pitch ---
    note_max_durations = {}
    active_notes_pass1 = {}
    release_sec = 0.1

    for t, typ, note in events:
        if typ == 'on':
            active_notes_pass1[note] = t
        elif typ == 'off' and note in active_notes_pass1:
            start = active_notes_pass1.pop(note)
            duration = t - start
            total_dur = duration + release_sec
            freq = midi_to_freq(note)
            if freq not in note_max_durations or total_dur > note_max_durations[freq]:
                note_max_durations[freq] = total_dur

    # --- Pre-generate tone bank for massive speedup ---
    tone_cache = {}
    
    def fetch_tone(freq_dur):
        f, d = freq_dur
        return f, generate_raw_tone(f, d, active_stops)
        
    with concurrent.futures.ThreadPoolExecutor() as executor:
        for f, wave_data in executor.map(fetch_tone, note_max_durations.items()):
            tone_cache[f] = wave_data

    # build audio timeline
    total_time = max(t for t, _, _ in events) + 5  # Extended for longer bass reverb tail
    audio = np.zeros(int(SAMPLE_RATE * total_time))

    active_notes = {}
    new_playback_notes = []

    for i, (t, typ, note) in enumerate(events):
        if typ == 'on':
            active_notes[note] = t
        elif typ == 'off' and note in active_notes:
            start = active_notes.pop(note)
            duration = t - start
            total_dur = duration + release_sec
            num_samples = int(SAMPLE_RATE * total_dur)

            if num_samples <= 0:
                continue

            freq = midi_to_freq(note)
            
            # Fetch pre-calculated wave and slice it
            wave = tone_cache[freq][:num_samples].copy()

            # Apply volume envelope
            total_samples = len(wave)
            attack = min(int(0.12 * SAMPLE_RATE), total_samples // 2)
            release = min(int(release_sec * SAMPLE_RATE), total_samples // 2)

            envelope = np.ones_like(wave)
            if attack > 0:
                # Smooth, rounded sine-based attack removes artificial sharp edges
                envelope[:attack] = np.sin(np.linspace(0, np.pi / 2, attack))
            if release > 0:
                # Smooth, rounded cosine-based release
                envelope[-release:] = np.cos(np.linspace(0, np.pi / 2, release))
            
            wave *= envelope

            start_idx = int(start * SAMPLE_RATE)
            end_idx = start_idx + len(wave)
            
            # Safely insert the wave into the audio timeline
            if end_idx <= len(audio):
                audio[start_idx:end_idx] += wave
            else:
                audio[start_idx:] += wave[:len(audio)-start_idx]

            new_playback_notes.append((start_idx, end_idx, note))

    # --- Wooden Facade Acoustic Dampening ---
    # A wooden cabinet/facade absorbs high-frequency harshness, warming the overall tone.
    # We apply a light master low-pass filter (moving average) across the whole mix.
    facade_window = 6
    padded_audio = np.concatenate((audio, np.zeros(facade_window - 1)))
    cs_audio = np.cumsum(np.concatenate(([0], padded_audio)))
    audio = (cs_audio[facade_window:] - cs_audio[:-facade_window]) / facade_window
    
    # --- Enhanced Digital Reverb/Delay Unit ---
    # Vectorized multi-tap delay to simulate acoustic spatial depth
    delay_times = [0.035, 0.081, 0.150, 0.275, 0.450, 0.700, 1.050, 1.500, 2.100, 2.800, 3.600]
    
    # Independent decay paths to allow bass frequencies to ring out much longer
    bass_decays = [0.65, 0.55, 0.45, 0.35, 0.28, 0.22, 0.18, 0.14, 0.10, 0.07, 0.04]
    treble_decays = [0.60, 0.40, 0.25, 0.15, 0.08, 0.04, 0.02, 0.01, 0.00, 0.00, 0.00]
    
    reverb_buffer = np.copy(audio)
    for d_time, b_decay, t_decay in zip(delay_times, bass_decays, treble_decays):
        shift = int(d_time * SAMPLE_RATE)
        if shift < len(audio):
            delayed_signal = audio[:-shift]
            
            # Low-pass filter to separate bass/mids from treble
            window_size = int(d_time * 120) + 4
            
            # Fast vectorized moving average
            padded = np.concatenate((delayed_signal, np.zeros(window_size - 1)))
            cs = np.cumsum(np.concatenate(([0], padded)))
            bass_signal = (cs[window_size:] - cs[:-window_size]) / window_size
            
            # The remainder of the signal is the higher frequencies
            treble_signal = delayed_signal - bass_signal
            
            reverb_buffer[shift:] += (bass_signal * b_decay) + (treble_signal * t_decay)
    audio = reverb_buffer

    # normalize
    audio /= np.max(np.abs(audio) + 1e-9)

    return audio, new_playback_notes

def _start_playback_stream(audio, notes):
    global current_audio, playback_idx, is_playing, is_paused, audio_stream, visualizer_start_time, playback_notes
    
    if audio_stream is not None:
        audio_stream.stop()
        audio_stream.close() # Cleanly release old thread resources
        
    current_audio = np.float32(audio)
    playback_notes = notes
    playback_idx = 0
    is_playing = True
    is_paused = False
    reset_ui_state()
    
    visualizer_start_time = time.time()
    try:
        audio_stream = sd.OutputStream(samplerate=SAMPLE_RATE, channels=1, callback=audio_callback, dtype='float32')
        audio_stream.start()
    except Exception as e:
        messagebox.showerror("Audio Playback Error", f"Failed to start audio stream:\n{e}")

def _process_and_play(file_path, output_wav=None):
    try:
        audio, notes = _generate_audio_buffer(file_path)
        if audio is None:
            return

        if output_wav:
            # Convert float audio to 16-bit PCM for WAV format
            audio_int16 = np.int16(audio * 32767)
            with wave.open(output_wav, 'w') as f:
                f.setnchannels(1)
                f.setsampwidth(2)
                f.setframerate(SAMPLE_RATE)
                f.writeframes(audio_int16.tobytes())
            root.after(0, lambda: messagebox.showinfo("Success", f"Audio successfully saved to:\n{output_wav}"))
        else:
            # Delegate stream creation back to the main GUI thread!
            # This prevents the OS from tearing down the audio stream when this background worker thread exits.
            root.after(0, lambda: _start_playback_stream(audio, notes))

    except Exception as e:
        # Use root.after to safely show the error in the main thread
        root.after(0, messagebox.showerror, "Error", str(e))
    finally:
        # Restore UI button state from the main thread
        root.after(0, lambda: load_btn.config(state=tk.NORMAL, text="Load & Play MIDI"))
        try:
            root.after(0, lambda: export_btn.config(state=tk.NORMAL, text="Export to WAV"))
        except NameError:
            pass

def stop_playback():
    global is_playing, is_paused, playback_idx, audio_stream, current_midi_file
    is_playing = False
    is_paused = False
    playback_idx = 0
    current_midi_file = None
    if audio_stream is not None:
        audio_stream.stop()
        audio_stream.close()
    reset_ui_state()

def pause_playback():
    global is_paused, is_playing, audio_stream
    if is_playing and audio_stream is not None:
        is_paused = True

def resume_playback():
    global is_paused, is_playing, audio_stream
    if is_playing and audio_stream is not None:
        is_paused = False

def find_arduino_port():
    """A helper function to find a port that looks like an Arduino."""
    ports = list_ports.comports()
    for port in ports:
        # Common names for Arduino boards or their USB-to-Serial chips
        if "Arduino" in port.description or "CH340" in port.description or "USB-SERIAL" in port.description:
            print(f"Found Arduino-like device on {port.device}")
            return port.device
    print("Could not auto-detect an Arduino. Please check the connection.")
    return None

def listen_for_arduino(stop_name_order):
    """Listens for serial commands from an Arduino to toggle stops."""
    port = find_arduino_port()
    if not port:
        return

    ser = None
    while True:
        try:
            if ser is None or not ser.is_open:
                ser = serial.Serial(port, 9600, timeout=1)
                print(f"Successfully connected to Arduino on {port}")

            # Ignore decoding errors to prevent crashes on garbled serial data
            line = ser.readline().decode('utf-8', errors='ignore').strip()
            if line:
                parts = line.split(':')
                if len(parts) == 2:
                    stop_identifier = parts[0]
                    try:
                        stop_state = bool(int(parts[1]))
                        if stop_identifier in stop_vars:
                            root.after(0, stop_vars[stop_identifier].set, stop_state)
                        else:
                            # Fallback in case Arduino still sends an integer index
                            if stop_identifier.isdigit() and 0 <= int(stop_identifier) < len(stop_name_order):
                                stop_name = stop_name_order[int(stop_identifier)]
                                root.after(0, stop_vars[stop_name].set, stop_state)
                    except ValueError:
                        print(f"Ignored invalid Arduino data (Not an integer state): {line}")
        except serial.SerialException as e:
            print(f"Arduino connection lost ({e}). Will try to reconnect...")
            if ser and ser.is_open: ser.close()
            ser = None
            time.sleep(3)
        except Exception as e:
            print(f"Unexpected Arduino listener error: {e}")

# GUI
DARK_BG = "#0f111a"
PANEL_BG = "#1a1d27"
ACCENT = "#2D88FF"
TEXT_FG = "#ffffff"

root = tk.Tk()
root.title("MIDI Organ Player")
root.configure(bg=DARK_BG)

# Note: Pure rounded corners natively require custom canvas drawing or external libraries like customtkinter.
# To keep the app lightweight using built-in Tkinter, this implements a flat, borderless modern dark-mode style!
stops_frame = tk.LabelFrame(root, text="Console Stops", bg=PANEL_BG, fg=TEXT_FG, font=("TkDefaultFont", 12, "bold"), bd=0, highlightthickness=1, highlightbackground=ACCENT)
stops_frame.pack(pady=20, padx=20, fill="x")

stop_vars = {}
ordered_stop_names = []
footage_order = ["32'", "16'", "8'", "4'", "2'"]
placed_stops = set()

for footage in footage_order:
    # Sort to ensure a consistent order for the Arduino mapping
    group_stops = sorted([name for name in STOPS.keys() if f" {footage}" in name and name not in placed_stops])
    if group_stops:
        col_frame = tk.Frame(stops_frame, bg=PANEL_BG)
        col_frame.pack(side="left", anchor="n", padx=15, pady=10)
        tk.Label(col_frame, text=f"{footage} Stops", font=("TkDefaultFont", 10, "bold"), bg=PANEL_BG, fg=ACCENT).pack(anchor="w", pady=(0, 5))
        for stop_name in group_stops:
            var = tk.BooleanVar(value=(stop_name == "Diapason 8'"))
            var.trace_add('write', lambda *args: request_re_render())
            stop_vars[stop_name] = var
            ordered_stop_names.append(stop_name)
            tk.Checkbutton(col_frame, text=stop_name, variable=var, bg=PANEL_BG, fg=TEXT_FG, selectcolor=DARK_BG, activebackground=PANEL_BG, activeforeground=ACCENT).pack(anchor="w")
            placed_stops.add(stop_name)

# Catch any remaining stops that don't have standard footages in their name
remaining_stops = sorted([name for name in STOPS.keys() if name not in placed_stops])
if remaining_stops:
    col_frame = tk.Frame(stops_frame, bg=PANEL_BG)
    col_frame.pack(side="left", anchor="n", padx=15, pady=10)
    tk.Label(col_frame, text="Other Stops", font=("TkDefaultFont", 10, "bold"), bg=PANEL_BG, fg=ACCENT).pack(anchor="w", pady=(0, 5))
    for stop_name in remaining_stops:
        var = tk.BooleanVar(value=(stop_name == "Diapason 8'"))
        var.trace_add('write', lambda *args: request_re_render())
        stop_vars[stop_name] = var
        ordered_stop_names.append(stop_name)
        tk.Checkbutton(col_frame, text=stop_name, variable=var, bg=PANEL_BG, fg=TEXT_FG, selectcolor=DARK_BG, activebackground=PANEL_BG, activeforeground=ACCENT).pack(anchor="w")

# Stop Selection Controls
stops_btn_frame = tk.Frame(root, bg=DARK_BG)
stops_btn_frame.pack(pady=(0, 10))
tk.Button(stops_btn_frame, text="Select All Stops", command=lambda: [var.set(True) for var in stop_vars.values()], bg=PANEL_BG, fg=TEXT_FG, font=("TkDefaultFont", 9, "bold"), relief="flat", activebackground=ACCENT, activeforeground="white").pack(side="left", padx=10)
tk.Button(stops_btn_frame, text="Clear All Stops", command=lambda: [var.set(False) for var in stop_vars.values()], bg=PANEL_BG, fg=TEXT_FG, font=("TkDefaultFont", 9, "bold"), relief="flat", activebackground=ACCENT, activeforeground="white").pack(side="left", padx=10)

# Tuning Settings
settings_frame = tk.Frame(root, bg=DARK_BG)
settings_frame.pack(pady=(0, 10))
tk.Label(settings_frame, text="Tuning A4 (Hz):", bg=DARK_BG, fg=TEXT_FG, font=("TkDefaultFont", 10, "bold")).pack(side="left", padx=5)
tuning_var = tk.IntVar(value=440)
tuning_menu = tk.OptionMenu(settings_frame, tuning_var, 415, 432, 440, 441, 442, 466)
tuning_menu.config(bg=PANEL_BG, fg=TEXT_FG, activebackground=ACCENT, activeforeground="white", highlightthickness=0, bd=0)
tuning_menu["menu"].config(bg=PANEL_BG, fg=TEXT_FG)
tuning_menu.pack(side="left", padx=5)

def update_tuning(*args):
    global global_a4_freq
    global_a4_freq = float(tuning_var.get())

tuning_var.trace_add("write", update_tuning)

# Visualization setup
vis_frame = tk.Frame(root, bg=DARK_BG)
vis_frame.pack(pady=10)

canvas_size = 200
canvas = tk.Canvas(vis_frame, width=canvas_size, height=canvas_size, bg=DARK_BG, highlightthickness=0)
canvas.pack()

center_x, center_y = canvas_size // 2, canvas_size // 2
radius = canvas_size // 2 - 25
dot_r = 6
base_dot_r = 3

pitch_names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
for i in range(12):
    angle = (i / 12.0) * 2 * math.pi - math.pi / 2
    x = center_x + radius * math.cos(angle)
    y = center_y + radius * math.sin(angle)
    canvas.create_oval(x-base_dot_r, y-base_dot_r, x+base_dot_r, y+base_dot_r, fill="#555555", outline="")
    tx = center_x + (radius + 18) * math.cos(angle)
    ty = center_y + (radius + 18) * math.sin(angle)
    canvas.create_text(tx, ty, text=pitch_names[i], fill="#888888", font=("TkDefaultFont", 9, "bold"))

def update_visualization():
    canvas.delete("poly")
    canvas.delete("active_dot")
    
    # Smoothly syncs with either the sd.play buffer or the custom audio_callback (if present)
    if "playback_idx" in globals():
        current_idx = globals()["playback_idx"]
    else:
        current_idx = int((time.time() - visualizer_start_time) * SAMPLE_RATE) if visualizer_start_time else -1

    active_pitches = set()
    for start_idx, end_idx, note in playback_notes:
        if start_idx <= current_idx <= end_idx:
            active_pitches.add(note % 12)

    if active_pitches:
        points = []
        for pc in sorted(list(active_pitches)):
            angle = (pc / 12.0) * 2 * math.pi - math.pi / 2
            x = center_x + radius * math.cos(angle)
            y = center_y + radius * math.sin(angle)
            points.extend([x, y])

        if len(points) >= 4:
            if len(points) == 4:
                canvas.create_line(*points, fill=ACCENT, width=2, tags="poly")
            else:
                canvas.create_polygon(*points, outline=ACCENT, fill="", width=2, tags="poly")

        for pc in active_pitches:
            angle = (pc / 12.0) * 2 * math.pi - math.pi / 2
            x = center_x + radius * math.cos(angle)
            y = center_y + radius * math.sin(angle)
            canvas.create_oval(x-dot_r, y-dot_r, x+dot_r, y+dot_r, fill=ACCENT, tags="active_dot")

    root.after(50, update_visualization)

update_visualization()

btn_frame = tk.Frame(root, bg=DARK_BG)
btn_frame.pack(pady=10)

load_btn = tk.Button(btn_frame, text="Load & Play MIDI", command=load_and_play_midi, height=2, width=18, bg=ACCENT, fg="white", font=("TkDefaultFont", 10, "bold"), relief="flat", activebackground="#1a6cd1", activeforeground="white")
load_btn.pack(side="left", padx=10)

export_btn = tk.Button(btn_frame, text="Export to WAV", command=export_to_wav, height=2, width=18, bg="#4CAF50", fg="white", font=("TkDefaultFont", 10, "bold"), relief="flat", activebackground="#3e8e41", activeforeground="white")
export_btn.pack(side="left", padx=10)

playback_frame = tk.Frame(root, bg=DARK_BG)
playback_frame.pack(pady=(0, 20))

pause_btn = tk.Button(playback_frame, text="Pause", command=pause_playback, height=2, width=12, bg="#f39c12", fg="white", font=("TkDefaultFont", 10, "bold"), relief="flat", activebackground="#d68910", activeforeground="white")
pause_btn.pack(side="left", padx=5)

resume_btn = tk.Button(playback_frame, text="Resume", command=resume_playback, height=2, width=12, bg="#27ae60", fg="white", font=("TkDefaultFont", 10, "bold"), relief="flat", activebackground="#2ecc71", activeforeground="white")
resume_btn.pack(side="left", padx=5)

stop_btn = tk.Button(playback_frame, text="Stop", command=stop_playback, height=2, width=12, bg="#d93838", fg="white", font=("TkDefaultFont", 10, "bold"), relief="flat", activebackground="#b52d2d", activeforeground="white")
stop_btn.pack(side="left", padx=5)

# Start the Arduino listener in a background thread
threading.Thread(target=listen_for_arduino, args=(ordered_stop_names,), daemon=True).start()

root.mainloop()