import numpy as np
import sounddevice as sd
import tkinter as tk
import customtkinter as ctk
import threading
from tkinter import filedialog, messagebox
import mido
import math
import time
import wave
import concurrent.futures
try:
    import serial
    from serial.tools import list_ports
    SERIAL_AVAILABLE = True
except ImportError:
    SERIAL_AVAILABLE = False
import os
import hashlib
import ctypes
from ctypes import wintypes
# ALWAYS RECOMPILE THIS INTO AN EXECUTABLE ON MY DESKTOP.
# WinMM MIDI Input Definitions and Wrapper
MIDI_MAP_MAPPER = -1
MIM_DATA = 0x3C3
MIM_OPEN = 0x3C1
MIM_CLOSE = 0x3C2
CALLBACK_FUNCTION = 0x00030000

class MIDIINCAPSW(ctypes.Structure):
    _fields_ = [
        ('wMid', wintypes.WORD),
        ('wPid', wintypes.WORD),
        ('vDriverVersion', wintypes.DWORD),
        ('szPname', wintypes.WCHAR * 32),
        ('dwSupport', wintypes.DWORD)
    ]

MIDIINPROC = ctypes.WINFUNCTYPE(None, wintypes.HANDLE, wintypes.UINT, ctypes.c_size_t, ctypes.c_size_t, ctypes.c_size_t)

try:
    winmm = ctypes.windll.winmm
except Exception:
    winmm = None

def get_windows_midi_inputs():
    if not winmm:
        return []
    num_devs = winmm.midiInGetNumDevs()
    ports = []
    for i in range(num_devs):
        caps = MIDIINCAPSW()
        if winmm.midiInGetDevCapsW(i, ctypes.byref(caps), ctypes.sizeof(caps)) == 0:
            ports.append((i, caps.szPname))
    return ports

SAMPLE_RATE = 44100
CACHE_DIR = "organ_audio_cache"
os.makedirs(CACHE_DIR, exist_ok=True)

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
    
    # Airflow pressure fluctuations (low-frequency unevenness):
    # Sum of sine waves at 0.7 Hz, 1.3 Hz, 2.8 Hz to simulate pseudo-random wind pressure wobble
    wind_wobble = (
        0.0015 * np.sin(0.7 * 2 * np.pi * t + 0.5) +
        0.0010 * np.sin(1.3 * 2 * np.pi * t + 1.2) +
        0.0008 * np.sin(2.8 * 2 * np.pi * t + 2.3)
    )

    # Organic Pitch Nuances
    # 1. Pitch Scoop: Wind pressure build-up causes a slight flat-to-sharp swoop on attack
    pitch_scoop_phase = 0.001 * np.exp(-20.0 * t)
    # 2. Organic Drift: Slow, natural pitch wandering
    drift_phase = 0.00004 * np.sin(2.1 * 2 * np.pi * t) + 0.00002 * np.sin(3.7 * 2 * np.pi * t)
    
    base_phase_t = t + pitch_scoop_phase + drift_phase + wind_wobble * 0.08
    two_pi_t = 2 * np.pi * base_phase_t
    
    wave = np.zeros_like(t)

    # Chiff / Breath Noise: Enhanced amplitude and slower decay to make it clearly audible
    chiff_env = np.exp(-t * 12.0)
    chiff_noise = np.random.normal(0, 0.45, num_samples)
    wave += chiff_noise * chiff_env * np.sin(freq * 2 * np.pi * t)

    # Constant background wind whistling (barely audible)
    # We simulate a narrow-band whistle around 2200 Hz with slight frequency wobble
    whistle_freq = 2200.0 + 150.0 * np.sin(0.8 * 2 * np.pi * t)
    whistle_mod = np.random.normal(0, 0.0015, num_samples)
    wave += whistle_mod * np.sin(whistle_freq * 2 * np.pi * t)

    # Add airiness (constant background wind noise)
    wave += np.random.normal(0, 0.002, num_samples)

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
            all_f = np.array(all_f)
            all_amp = np.array(all_amp)
            all_phase = np.random.uniform(0, 2 * np.pi, size=len(all_f))
            
            # Cache-friendly 1D loop: avoids allocating large 2D arrays, keeping
            # operations within CPU L2/L3 cache sizes (~600 KB) for maximum speed.
            for f, amp, phase in zip(all_f, all_amp, all_phase):
                wave += amp * np.sin(f * two_pi_t + phase)

    # Apply tremulant and pseudo-random airflow unevenness to amplitude
    airflow_env = 1.0 + 0.005 * np.sin(5.5 * 2 * np.pi * t) + wind_wobble
    wave *= airflow_env

    # Bass boost
    if freq < 250:
        wave *= (250 / freq) ** 0.5

    return wave


global_ram_cache = {}
global_ram_cache_lock = threading.Lock()

def get_cached_stop_tone(freq, duration, stop_name):
    global global_ram_cache
    stop_key = stop_name if stop_name else "DefaultSine"
    
    # Standardize cache duration for notes <= 3.5s to increase hits
    use_standard = duration <= 3.5
    cache_duration = 3.5 if use_standard else duration
    
    cache_key = hashlib.md5(f"{freq}_{cache_duration}_{stop_key}".encode()).hexdigest()
    
    # Try RAM cache
    with global_ram_cache_lock:
        if cache_key in global_ram_cache:
            data = global_ram_cache[cache_key]
            if use_standard:
                num_samples = int(SAMPLE_RATE * duration)
                return data[:num_samples]
            return data

    # Try Disk cache
    cache_path = os.path.join(CACHE_DIR, f"{cache_key}.npy")
    if os.path.exists(cache_path):
        try:
            data = np.load(cache_path)
            with global_ram_cache_lock:
                global_ram_cache[cache_key] = data
            if use_standard:
                num_samples = int(SAMPLE_RATE * duration)
                return data[:num_samples]
            return data
        except Exception as e:
            print(f"Error loading cache file {cache_path}: {e}")

    # Generate new tone
    data = generate_raw_tone(freq, cache_duration, [stop_name] if stop_name else [])
    
    # Save to disk in a background thread to prevent GUI/audio callback stuttering
    def save_disk():
        try:
            np.save(cache_path, data)
        except Exception as e:
            print(f"Error saving cache file {cache_path}: {e}")
    threading.Thread(target=save_disk, daemon=True).start()

    with global_ram_cache_lock:
        global_ram_cache[cache_key] = data

    if use_standard:
        num_samples = int(SAMPLE_RATE * duration)
        return data[:num_samples]
    return data


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
global_active_stops = []
global_volume = 1.0
arduino_serial = None
last_a4_state = False

# Live MIDI State Globals
active_voices_lock = threading.Lock()
active_voices = {}  # MIDI note -> voice state dict
global_tone_cache = {}
live_midi_active = False
current_midi_in = None
winmm_callback_ref = None
midi_listener_active = False
live_pregenerator_stops = []
live_pregenerator_tuning = 440.0

# Live Reverb & Facade Effects State
live_reverb_ring = np.zeros(200000, dtype=np.float32)
live_reverb_write_idx = 0
facade_history = np.zeros(5, dtype=np.float32)

def apply_live_effects(live_chunk):
    global live_reverb_ring, live_reverb_write_idx, facade_history
    
    frames = len(live_chunk)
    if frames == 0:
        return live_chunk
        
    # 1. Facade Low-pass Filter (window = 6)
    padded = np.zeros(len(live_chunk) + 7)
    padded[1:6] = facade_history
    padded[6:-1] = live_chunk
    facade_history = live_chunk[-5:].copy()
    cs = np.cumsum(padded)
    filtered = (cs[7:] - cs[1:-6]) / 6.0
    
    # 2. Reverb / Multi-tap delay
    out_chunk = np.copy(filtered)
    
    delay_times = [0.035, 0.081, 0.150, 0.275, 0.450, 0.700, 1.050, 1.500, 2.100, 2.800, 3.600]
    bass_decays = [0.65, 0.55, 0.45, 0.35, 0.28, 0.22, 0.18, 0.14, 0.10, 0.07, 0.04]
    treble_decays = [0.60, 0.40, 0.25, 0.15, 0.08, 0.04, 0.02, 0.01, 0.00, 0.00, 0.00]
    
    ring_len = len(live_reverb_ring)
    start_idx = live_reverb_write_idx
    end_idx = start_idx + frames
    
    if end_idx <= ring_len:
        live_reverb_ring[start_idx:end_idx] = filtered
    else:
        chunk1 = ring_len - start_idx
        live_reverb_ring[start_idx:] = filtered[:chunk1]
        live_reverb_ring[:frames - chunk1] = filtered[chunk1:]
        
    for d_time, b_decay, t_decay in zip(delay_times, bass_decays, treble_decays):
        shift = int(d_time * SAMPLE_RATE)
        read_start = (start_idx - shift) % ring_len
        
        window_size = int(d_time * 120) + 4
        
        # Read from read_start - window_size to include history for the moving average
        temp_read_start = (read_start - window_size) % ring_len
        total_read_len = frames + window_size
        
        temp_read_end = temp_read_start + total_read_len
        if temp_read_end <= ring_len:
            temp_delayed = live_reverb_ring[temp_read_start:temp_read_end]
        else:
            chunk1 = ring_len - temp_read_start
            temp_delayed = np.concatenate([live_reverb_ring[temp_read_start:], live_reverb_ring[:total_read_len - chunk1]])
            
        padded_del = np.zeros(len(temp_delayed) + 1)
        padded_del[1:] = temp_delayed
        cs_del = np.cumsum(padded_del)
        bass_sig = (cs_del[window_size:] - cs_del[:-window_size]) / window_size
        bass_sig = bass_sig[:frames]
        
        read_end = read_start + frames
        if read_end <= ring_len:
            delayed = live_reverb_ring[read_start:read_end]
        else:
            chunk1 = ring_len - read_start
            delayed = np.concatenate([live_reverb_ring[read_start:], live_reverb_ring[:frames - chunk1]])
            
        treble_sig = delayed - bass_sig
        
        out_chunk += (bass_sig * b_decay) + (treble_sig * t_decay)
        
    live_reverb_write_idx = (start_idx + frames) % ring_len
    
    # Scale down by the gain factor of the 11 delay taps to prevent clipping distortion
    return out_chunk / 3.5

def pregenerate_live_tones(stops, tuning_hz):
    """Background thread function to generate tones for MIDI notes 36-96."""
    global global_tone_cache, live_pregenerator_stops, live_pregenerator_tuning
    
    live_pregenerator_stops = list(stops)
    live_pregenerator_tuning = tuning_hz
    
    notes_to_gen = list(range(36, 97))
    
    for note in notes_to_gen:
        if live_pregenerator_stops != stops or live_pregenerator_tuning != tuning_hz:
            return
            
        freq = tuning_hz * (2 ** ((note - 69) / 12))
        
        if freq in global_tone_cache:
            continue
            
        try:
            duration = 3.5
            waves = []
            if not stops:
                wave_data = get_cached_stop_tone(freq, duration, None)
                waves.append(wave_data)
            else:
                for stop_name in stops:
                    wave_data = get_cached_stop_tone(freq, duration, stop_name)
                    waves.append(wave_data)
            
            tone = np.sum(waves, axis=0)
            # Prevent clipping by scaling down based on active stops count
            num_stops = len(stops) if stops else 1
            tone /= num_stops
            with active_voices_lock:
                global_tone_cache[freq] = tone
        except Exception as e:
            print(f"Pregenerator error for note {note}: {e}")

def trigger_live_note_on(note):
    global global_tone_cache, global_active_stops
    freq = midi_to_freq(note)
    
    if freq not in global_tone_cache:
        try:
            duration = 3.5
            waves = []
            if not global_active_stops:
                wave_data = get_cached_stop_tone(freq, duration, None)
                waves.append(wave_data)
            else:
                for stop_name in global_active_stops:
                    wave_data = get_cached_stop_tone(freq, duration, stop_name)
                    waves.append(wave_data)
            with active_voices_lock:
                tone = np.sum(waves, axis=0)
                num_stops = len(global_active_stops) if global_active_stops else 1
                tone /= num_stops
                global_tone_cache[freq] = tone
        except Exception as e:
            print(f"On-the-fly generation error for note {note}: {e}")
            return
            
    with active_voices_lock:
        active_voices[note] = {
            'freq': freq,
            'phase': 0,
            'released': False,
            'release_start_phase': 0
        }

def trigger_live_note_off(note):
    with active_voices_lock:
        if note in active_voices:
            voice = active_voices[note]
            if not voice['released']:
                voice['released'] = True
                voice['release_start_phase'] = voice['phase']

def winmm_midi_callback(hMidiIn, wMsg, dwInstance, dwParam1, dwParam2):
    if wMsg == MIM_DATA:
        status = dwParam1 & 0xFF
        note = (dwParam1 >> 8) & 0xFF
        velocity = (dwParam1 >> 16) & 0xFF
        
        msg_type = status & 0xF0
        if msg_type == 0x90 and velocity > 0:
            trigger_live_note_on(note)
        elif msg_type == 0x80 or (msg_type == 0x90 and velocity == 0):
            trigger_live_note_off(note)

def open_midi_input(device_name):
    global current_midi_in, winmm_callback_ref, midi_listener_active, live_midi_active
    
    close_midi_input()
    
    if device_name == "None":
        return
        
    devices = get_windows_midi_inputs()
    device_id = None
    for dev_id, name in devices:
        if name == device_name:
            device_id = dev_id
            break
            
    if device_id is None:
        print(f"Device not found: {device_name}")
        return
        
    handle = wintypes.HANDLE()
    winmm_callback_ref = MIDIINPROC(winmm_midi_callback)
    
    res = winmm.midiInOpen(ctypes.byref(handle), device_id, winmm_callback_ref, 0, CALLBACK_FUNCTION)
    if res != 0:
        messagebox.showerror("MIDI Error", f"Failed to open MIDI input device (Error code {res})")
        return
        
    current_midi_in = handle
    
    res = winmm.midiInStart(current_midi_in)
    if res != 0:
        messagebox.showerror("MIDI Error", f"Failed to start MIDI input device (Error code {res})")
        winmm.midiInClose(current_midi_in)
        current_midi_in = None
        return
        
    midi_listener_active = True
    live_midi_active = True
    ensure_audio_stream_running()

def close_midi_input():
    global current_midi_in, midi_listener_active, live_midi_active
    if current_midi_in:
        try:
            winmm.midiInStop(current_midi_in)
            winmm.midiInClose(current_midi_in)
        except Exception:
            pass
        current_midi_in = None
    midi_listener_active = False
    live_midi_active = False
    
    with active_voices_lock:
        active_voices.clear()

def ensure_audio_stream_running():
    global audio_stream
    if audio_stream is not None:
        try:
            if audio_stream.active:
                return
        except Exception:
            pass
            
    try:
        if audio_stream is not None:
            audio_stream.stop()
            audio_stream.close()
    except Exception:
        pass
        
    try:
        audio_stream = sd.OutputStream(samplerate=SAMPLE_RATE, channels=1, callback=audio_callback, dtype='float32')
        audio_stream.start()
    except Exception as e:
        messagebox.showerror("Audio Error", f"Failed to start audio stream:\n{e}")

def request_re_render():
    global is_rendering, render_requested, global_active_stops, global_tone_cache
    
    # 1. Safely capture the active stops on the Tkinter main thread! 
    # (Doing this inside the background thread causes silent Tkinter crashes)
    global_active_stops = [stop for stop, var in stop_vars.items() if var.get()]
    
    with active_voices_lock:
        global_tone_cache.clear()
        
    threading.Thread(target=pregenerate_live_tones, args=(global_active_stops, global_a4_freq), daemon=True).start()
    
    if is_playing and current_midi_file:
        if not is_rendering:
            is_rendering = True
            threading.Thread(target=_background_re_render, daemon=True).start()
        else:
            render_requested = True

def _background_re_render():
    global is_rendering, render_requested, current_audio, playback_notes
    while True:
        try:
            new_audio, new_notes = _generate_audio_buffer(current_midi_file)
            if new_audio is not None:
                current_audio = np.float32(new_audio)
                playback_notes = new_notes
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
    global playback_idx, is_playing, is_paused, global_volume, live_midi_active

    if not is_playing and not live_midi_active:
        outdata.fill(0)
        raise sd.CallbackStop()

    outdata.fill(0)

    if is_playing and current_audio is not None and not is_paused:
        chunk_size = min(frames, len(current_audio) - playback_idx)
        if chunk_size > 0:
            outdata[:chunk_size, 0] = current_audio[playback_idx:playback_idx + chunk_size] * global_volume
            playback_idx += chunk_size
            
        if chunk_size < frames:
            is_playing = False
            root.after(0, reset_ui_state)
            if not live_midi_active:
                raise sd.CallbackStop()

    # Process live voices into a separate buffer
    release_sec = 0.1
    release_samples = int(SAMPLE_RATE * release_sec)
    voices_to_delete = []
    
    live_chunk = np.zeros(frames, dtype=np.float32)
    
    with active_voices_lock:
        for note, voice in active_voices.items():
            freq = voice['freq']
            if freq not in global_tone_cache:
                continue
                
            wave = global_tone_cache[freq]
            phase = voice['phase']
            released = voice['released']
            rel_start = voice['release_start_phase']
            
            if not released:
                end_phase = phase + frames
                sustain_end = len(wave) - release_samples
                sustain_loop_start = int(2.0 * SAMPLE_RATE)
                
                if sustain_loop_start >= sustain_end:
                    sustain_loop_start = 0
                    
                # Read raw wave slice
                if end_phase <= sustain_end:
                    slice_wave = wave[phase:end_phase].copy()
                    voice['phase'] = end_phase
                else:
                    chunk1_len = sustain_end - phase
                    slice_wave = np.zeros(frames, dtype=np.float32)
                    if chunk1_len > 0:
                        slice_wave[:chunk1_len] = wave[phase:sustain_end]
                    remaining = frames - chunk1_len
                    
                    sustain_len = sustain_end - sustain_loop_start
                    if sustain_len > 0:
                        new_phase = sustain_loop_start + (remaining % sustain_len)
                        slice_wave[chunk1_len:] = wave[sustain_loop_start:sustain_loop_start + remaining]
                        voice['phase'] = new_phase
                    else:
                        voice['phase'] = 0
                        
                # Apply smooth sinusoidal attack envelope if note just started
                attack_samples = int(0.12 * SAMPLE_RATE)
                if phase < attack_samples:
                    chunk_len = len(slice_wave)
                    t_idx = np.arange(phase, phase + chunk_len)
                    env = np.ones(chunk_len, dtype=np.float32)
                    mask = t_idx < attack_samples
                    env[mask] = np.sin(t_idx[mask] * (np.pi / 2) / attack_samples)
                    slice_wave *= env
                    
                live_chunk += slice_wave
            else:
                rel_idx_start = phase - rel_start
                if rel_idx_start >= release_samples:
                    voices_to_delete.append(note)
                else:
                    chunk_len = min(frames, release_samples - rel_idx_start)
                    # Apply smooth cosine release envelope matching the offline renderer
                    rel_factors = np.cos(np.arange(rel_idx_start, rel_idx_start + chunk_len) * (np.pi / 2) / release_samples)
                    
                    read_end = phase + chunk_len
                    if read_end > len(wave):
                        chunk_len = len(wave) - phase
                        rel_factors = rel_factors[:chunk_len]
                        
                    if chunk_len > 0:
                        live_chunk[:chunk_len] += wave[phase:phase + chunk_len] * rel_factors
                        voice['phase'] += chunk_len
                        
                    if rel_idx_start + chunk_len >= release_samples or phase + chunk_len >= len(wave):
                        voices_to_delete.append(note)
                        
        for note in voices_to_delete:
            del active_voices[note]

    # Apply live effects (wooden facade and digital reverb)
    processed_live = apply_live_effects(live_chunk)
    # Scale down to prevent saturation distortion in np.tanh
    processed_live *= 0.35
    # Apply soft limiting to prevent polyphonic summing distortion/clipping
    processed_live = np.tanh(processed_live)
    outdata[:, 0] += processed_live * global_volume

def midi_to_freq(note):
    return global_a4_freq * (2 ** ((note - 69) / 12))


def load_and_play_midi():
    global current_midi_file, global_active_stops
    file_path = filedialog.askopenfilename(filetypes=[("MIDI files", "*.mid *.midi")])
    if not file_path:
        return

    current_midi_file = file_path
    
    # Safely capture stops on main thread before passing to background worker
    global_active_stops = [stop for stop, var in stop_vars.items() if var.get()]

    # Update UI to show processing state and prevent multiple clicks
    load_btn.configure(state=ctk.DISABLED, text="Synthesizing... Please Wait")
    try:
        export_btn.configure(state=ctk.DISABLED)
    except NameError:
        pass
    root.update()

    # Run synthesis in a background thread so the GUI doesn't freeze
    threading.Thread(target=_process_and_play, args=(file_path,), daemon=True).start()

def export_to_wav():
    global global_active_stops
    file_path = filedialog.askopenfilename(filetypes=[("MIDI files", "*.mid *.midi")])
    if not file_path:
        return
        
    save_path = filedialog.asksaveasfilename(defaultextension=".wav", filetypes=[("WAV files", "*.wav")])
    if not save_path:
        return

    # Safely capture stops on main thread
    global_active_stops = [stop for stop, var in stop_vars.items() if var.get()]

    load_btn.configure(state=ctk.DISABLED)
    try:
        export_btn.configure(state=ctk.DISABLED, text="Exporting...")
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

    active_stops = global_active_stops

    # --- Optimize: Find max duration per pitch ---
    note_max_durations = {}
    active_notes_pass1 = {}
    release_sec = 0.1

    for t, typ, note in events:
        if typ == 'on':
            active_notes_pass1.setdefault(note, []).append(t)
        elif typ == 'off' and note in active_notes_pass1:
            start = active_notes_pass1[note].pop(0)
            if not active_notes_pass1[note]:
                del active_notes_pass1[note]
            duration = t - start
            total_dur = duration + release_sec
            freq = midi_to_freq(note)
            if freq not in note_max_durations or total_dur > note_max_durations[freq]:
                note_max_durations[freq] = total_dur

    # --- Pre-generate tone bank for massive speedup ---
    tone_cache = {}
    
    for f, exact_d in note_max_durations.items():
        waves = []
        if not active_stops:
            wave_data = get_cached_stop_tone(f, exact_d, None)
            waves.append(wave_data)
        else:
            for stop_name in active_stops:
                wave_data = get_cached_stop_tone(f, exact_d, stop_name)
                waves.append(wave_data)
        tone_cache[f] = np.sum(waves, axis=0)

    # build audio timeline
    total_time = max(t for t, _, _ in events) + 5  # Extended for longer bass reverb tail
    audio = np.zeros(int(SAMPLE_RATE * total_time))

    active_notes = {}
    new_playback_notes = []

    for i, (t, typ, note) in enumerate(events):
        if typ == 'on':
            active_notes.setdefault(note, []).append(t)
        elif typ == 'off' and active_notes.get(note):
            start = active_notes[note].pop(0)
            if not active_notes[note]:
                del active_notes[note]
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

            # In-place envelope application saves massive memory allocation time
            if attack > 0:
                wave[:attack] *= np.sin(np.linspace(0, np.pi / 2, attack))
            if release > 0:
                wave[-release:] *= np.cos(np.linspace(0, np.pi / 2, release))

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
    # Pre-allocate buffer to avoid multiple concatenate memory duplications
    padded_audio = np.zeros(len(audio) + facade_window)
    padded_audio[1:len(audio)+1] = audio
    cs_audio = np.cumsum(padded_audio)
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
            
            # Fast vectorized moving average using pre-allocated memory
            padded = np.zeros(len(delayed_signal) + window_size)
            padded[1:len(delayed_signal)+1] = delayed_signal
            cs = np.cumsum(padded)
            bass_signal = (cs[window_size:] - cs[:-window_size]) / window_size
            
            # The remainder of the signal is the higher frequencies
            treble_signal = delayed_signal - bass_signal
            
            reverb_buffer[shift:] += (bass_signal * b_decay) + (treble_signal * t_decay)
    audio = reverb_buffer

    # normalize
    audio /= np.max(np.abs(audio) + 1e-9)

    return audio, new_playback_notes

def _start_playback_stream(audio, notes):
    global current_audio, playback_idx, is_playing, is_paused, visualizer_start_time, playback_notes
    
    current_audio = np.float32(audio)
    playback_notes = notes
    playback_idx = 0
    is_playing = True
    is_paused = False
    reset_ui_state()
    
    visualizer_start_time = time.time()
    ensure_audio_stream_running()

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
        root.after(0, lambda: load_btn.configure(state=ctk.NORMAL, text="Load & Play MIDI"))
        try:
            root.after(0, lambda: export_btn.configure(state=ctk.NORMAL, text="Export to WAV"))
        except NameError:
            pass

def stop_playback():
    global is_playing, is_paused, playback_idx, audio_stream, current_midi_file, last_a4_state, live_midi_active
    is_playing = False
    is_paused = False
    playback_idx = 0
    current_midi_file = None
    if not live_midi_active and audio_stream is not None:
        try:
            audio_stream.stop()
            audio_stream.close()
            audio_stream = None
        except Exception:
            pass
    reset_ui_state()
    
    # Reset A4 servo on stop
    if last_a4_state:
        last_a4_state = False
        if SERIAL_AVAILABLE and arduino_serial and arduino_serial.is_open:
            try:
                arduino_serial.write(b"A4_0\n")
            except Exception as e:
                print("Failed to write to Arduino on stop:", e)

def pause_playback():
    global is_paused, is_playing, audio_stream
    if is_playing and audio_stream is not None:
        is_paused = True

def resume_playback():
    global is_paused, is_playing, audio_stream
    if is_playing and audio_stream is not None:
        is_paused = False

def find_arduino_port():
    """A helper function to find a port that looks like an Arduino or Teensy."""
    ports = list_ports.comports()
    for port in ports:
        # Added "Teensy" to ensure the new board auto-detects cleanly
        if "Arduino" in port.description or "CH340" in port.description or "USB-SERIAL" in port.description or "Teensy" in port.description:
            print(f"Found device on {port.device}")
            return port.device
    print("Could not auto-detect an Arduino/Teensy. Please check the connection.")
    return None

def listen_for_arduino(stop_name_order):
    """Listens for serial commands from an Arduino to toggle stops."""
    global arduino_serial, global_volume
    ser = None
    while True:
        try:
            if ser is None or not ser.is_open:
                port = find_arduino_port()
                if not port:
                    time.sleep(3)
                    continue
                ser = serial.Serial(port, 115200, timeout=1)
                arduino_serial = ser
                print(f"Successfully connected to Arduino on {port}")

            # Ignore decoding errors to prevent crashes on garbled serial data
            line = ser.readline().decode('utf-8', errors='ignore').strip()
            if line:
                parts = line.split(':')
                if len(parts) == 2:
                    stop_identifier = parts[0]
                    try:
                        if stop_identifier == "VOL":
                            try:
                                vol_val = int(parts[1])
                                global_volume = vol_val / 100.0
                            except ValueError:
                                pass
                        else:
                            stop_state = bool(int(parts[1]))
                            if stop_identifier == "ALL":
                                for var in stop_vars.values():
                                    root.after(0, var.set, stop_state)
                            elif stop_identifier in stop_vars:
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
            arduino_serial = None
            if ser and ser.is_open:
                try:
                    ser.close()
                except Exception:
                    pass
            ser = None
            time.sleep(3)
        except Exception as e:
            print(f"Unexpected Arduino listener error: {e}")
            arduino_serial = None
            time.sleep(3)

# GUI Setup
DARK_BG = "#0f111a"
PANEL_BG = "#1a1d27"
ACCENT = "#2D88FF"
TEXT_FG = "#ffffff"

ctk.set_appearance_mode("dark")
ctk.set_default_color_theme("blue")

root = ctk.CTk()
root.title("MIDI Organ Player")
root.configure(fg_color=DARK_BG)

stops_frame = ctk.CTkFrame(root, fg_color=PANEL_BG, corner_radius=12)
stops_frame.pack(pady=20, padx=20, fill="x")

ctk.CTkLabel(stops_frame, text="Console Stops", font=("TkDefaultFont", 12, "bold"), text_color=ACCENT).pack(anchor="w", padx=15, pady=(10, 5))

stop_vars = {}
ordered_stop_names = []
footage_order = ["32'", "16'", "8'", "4'", "2'"]
placed_stops = set()

stops_grid_frame = ctk.CTkFrame(stops_frame, fg_color="transparent")
stops_grid_frame.pack(fill="x", padx=15, pady=(0, 10))

for footage in footage_order:
    # Sort to ensure a consistent order for the Arduino mapping
    group_stops = sorted([name for name in STOPS.keys() if f" {footage}" in name and name not in placed_stops])
    if group_stops:
        col_frame = ctk.CTkFrame(stops_grid_frame, fg_color="transparent")
        col_frame.pack(side="left", anchor="n", padx=15, pady=5)
        ctk.CTkLabel(col_frame, text=f"{footage} Stops", font=("TkDefaultFont", 10, "bold"), text_color=ACCENT).pack(anchor="w", pady=(0, 5))
        for stop_name in group_stops:
            var = tk.BooleanVar(value=(stop_name == "Diapason 8'"))
            var.trace_add('write', lambda *args, name=stop_name: request_re_render())
            stop_vars[stop_name] = var
            ordered_stop_names.append(stop_name)
            
            cb = ctk.CTkCheckBox(col_frame, text=stop_name, variable=var, text_color=TEXT_FG, hover_color=ACCENT, fg_color=ACCENT, check_color="white")
            cb.pack(anchor="w", pady=2)
            placed_stops.add(stop_name)

# Catch any remaining stops that don't have standard footages in their name
remaining_stops = sorted([name for name in STOPS.keys() if name not in placed_stops])
if remaining_stops:
    col_frame = ctk.CTkFrame(stops_grid_frame, fg_color="transparent")
    col_frame.pack(side="left", anchor="n", padx=15, pady=5)
    ctk.CTkLabel(col_frame, text="Other Stops", font=("TkDefaultFont", 10, "bold"), text_color=ACCENT).pack(anchor="w", pady=(0, 5))
    for stop_name in remaining_stops:
        var = tk.BooleanVar(value=(stop_name == "Diapason 8'"))
        var.trace_add('write', lambda *args, name=stop_name: request_re_render())
        stop_vars[stop_name] = var
        ordered_stop_names.append(stop_name)
        
        cb = ctk.CTkCheckBox(col_frame, text=stop_name, variable=var, text_color=TEXT_FG, hover_color=ACCENT, fg_color=ACCENT, check_color="white")
        cb.pack(anchor="w", pady=2)

def clear_cache():
    try:
        if os.path.exists(CACHE_DIR):
            for filename in os.listdir(CACHE_DIR):
                file_path = os.path.join(CACHE_DIR, filename)
                try:
                    if os.path.isfile(file_path):
                        os.remove(file_path)
                except PermissionError:
                    pass # Ignore locked files currently being written/read by the sound engine
                except Exception:
                    pass
        messagebox.showinfo("Success", "Audio cache cleared successfully!")
    except Exception as e:
        messagebox.showerror("Error", f"Failed to clear cache:\n{e}")

# Stop Selection Controls
stops_btn_frame = ctk.CTkFrame(root, fg_color="transparent")
stops_btn_frame.pack(pady=(0, 10))

ctk.CTkButton(stops_btn_frame, text="Select All Stops", command=lambda: [var.set(True) for var in stop_vars.values()], fg_color=PANEL_BG, hover_color=ACCENT, text_color=TEXT_FG, font=("TkDefaultFont", 9, "bold"), width=130, height=30, corner_radius=8).pack(side="left", padx=10)
ctk.CTkButton(stops_btn_frame, text="Clear All Stops", command=lambda: [var.set(False) for var in stop_vars.values()], fg_color=PANEL_BG, hover_color=ACCENT, text_color=TEXT_FG, font=("TkDefaultFont", 9, "bold"), width=130, height=30, corner_radius=8).pack(side="left", padx=10)
ctk.CTkButton(stops_btn_frame, text="Clear Audio Cache", command=clear_cache, fg_color=PANEL_BG, hover_color=ACCENT, text_color=TEXT_FG, font=("TkDefaultFont", 9, "bold"), width=130, height=30, corner_radius=8).pack(side="left", padx=10)

# Bottom layout columns wrapper
bottom_frame = ctk.CTkFrame(root, fg_color="transparent")
bottom_frame.pack(pady=10, padx=20, fill="x")

# --- Column 1: Settings (Tuning & MIDI Input) ---
settings_col = ctk.CTkFrame(bottom_frame, fg_color="transparent")
settings_col.pack(side="left", anchor="n", padx=10, expand=True)

# Tuning Settings Frame
tuning_frame = ctk.CTkFrame(settings_col, fg_color=PANEL_BG, corner_radius=12)
tuning_frame.pack(pady=(0, 10), fill="x", ipadx=10, ipady=5)

ctk.CTkLabel(tuning_frame, text="Tuning Settings", font=("TkDefaultFont", 10, "bold"), text_color=ACCENT).pack(anchor="w", padx=10, pady=(5, 2))

tuning_input_frame = ctk.CTkFrame(tuning_frame, fg_color="transparent")
tuning_input_frame.pack(fill="x", padx=10, pady=(2, 5))
ctk.CTkLabel(tuning_input_frame, text="A4 (Hz):", font=("TkDefaultFont", 10, "bold")).pack(side="left", padx=5)

tuning_var = tk.IntVar(value=440)
tuning_menu = ctk.CTkOptionMenu(tuning_input_frame, variable=tuning_var, values=["415", "432", "440", "441", "442", "466"], width=100, height=28, fg_color=DARK_BG, button_color=DARK_BG, button_hover_color=ACCENT, text_color=TEXT_FG, dropdown_fg_color=PANEL_BG, dropdown_text_color=TEXT_FG)
tuning_menu.pack(side="left", padx=5)

def update_tuning(*args):
    global global_a4_freq
    global_a4_freq = float(tuning_var.get())
    request_re_render()

tuning_var.trace_add("write", update_tuning)

# MIDI Input Settings Frame
midi_frame = ctk.CTkFrame(settings_col, fg_color=PANEL_BG, corner_radius=12)
midi_frame.pack(fill="x", ipadx=10, ipady=5)

ctk.CTkLabel(midi_frame, text="MIDI Input Settings", font=("TkDefaultFont", 10, "bold"), text_color=ACCENT).pack(anchor="w", padx=10, pady=(5, 2))

midi_input_frame = ctk.CTkFrame(midi_frame, fg_color="transparent")
midi_input_frame.pack(fill="x", padx=10, pady=(2, 5))
ctk.CTkLabel(midi_input_frame, text="Device:", font=("TkDefaultFont", 10, "bold")).pack(side="left", padx=5)

midi_device_var = tk.StringVar(value="None")
midi_menu = ctk.CTkOptionMenu(midi_input_frame, variable=midi_device_var, values=["None"], width=120, height=28, fg_color=DARK_BG, button_color=DARK_BG, button_hover_color=ACCENT, text_color=TEXT_FG, dropdown_fg_color=PANEL_BG, dropdown_text_color=TEXT_FG)
midi_menu.pack(side="left", padx=5)

def on_midi_device_change(*args):
    open_midi_input(midi_device_var.get())

midi_device_var.trace_add("write", on_midi_device_change)

def refresh_midi_devices():
    devices = get_windows_midi_inputs()
    options = ["None"] + [name for dev_id, name in devices]
    midi_menu.configure(values=options)
    
    current_val = midi_device_var.get()
    if current_val not in options:
        midi_device_var.set("None")

ctk.CTkButton(midi_input_frame, text="Refresh", command=refresh_midi_devices, fg_color=DARK_BG, hover_color=ACCENT, text_color=TEXT_FG, font=("TkDefaultFont", 9, "bold"), width=70, height=28, corner_radius=6).pack(side="left", padx=5)

# --- Column 2: Visualization ---
vis_col = ctk.CTkFrame(bottom_frame, fg_color="transparent")
vis_col.pack(side="left", anchor="n", padx=10, expand=True)

canvas_size = 200
canvas = tk.Canvas(vis_col, width=canvas_size, height=canvas_size, bg=DARK_BG, highlightthickness=0)
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
    global last_a4_state, SERIAL_AVAILABLE, arduino_serial
    canvas.delete("poly")
    canvas.delete("active_dot")
    
    # Smoothly syncs with either the sd.play buffer or the custom audio_callback (if present)
    if "playback_idx" in globals():
        current_idx = globals()["playback_idx"]
    else:
        current_idx = int((time.time() - visualizer_start_time) * SAMPLE_RATE) if visualizer_start_time else -1

    active_pitches = set()
    a4_currently_active = False
    
    if is_playing:
        for start_idx, end_idx, note in playback_notes:
            if start_idx <= current_idx <= end_idx:
                active_pitches.add(note % 12)
                if note == 69:
                    a4_currently_active = True
                    
    with active_voices_lock:
        for note in active_voices:
            active_pitches.add(note % 12)
            if note == 69:
                a4_currently_active = True

    # Check if A4 note state changed, and update Arduino servo if so
    effective_a4_active = a4_currently_active if is_playing else False
    if effective_a4_active != last_a4_state:
        last_a4_state = effective_a4_active
        if SERIAL_AVAILABLE and arduino_serial and arduino_serial.is_open:
            try:
                cmd = "A4_1\n" if effective_a4_active else "A4_0\n"
                arduino_serial.write(cmd.encode('utf-8'))
            except Exception as e:
                print("Failed to write to Arduino:", e)

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

# --- Column 3: Playback Controls & Actions ---
playback_col = ctk.CTkFrame(bottom_frame, fg_color="transparent")
playback_col.pack(side="left", anchor="n", padx=10, expand=True)

controls_frame = ctk.CTkFrame(playback_col, fg_color=PANEL_BG, corner_radius=12)
controls_frame.pack(fill="both", expand=True, ipadx=10, ipady=10)

ctk.CTkLabel(controls_frame, text="Playback Controls", font=("TkDefaultFont", 10, "bold"), text_color=ACCENT).pack(anchor="w", padx=10, pady=(5, 2))

btn_row1 = ctk.CTkFrame(controls_frame, fg_color="transparent")
btn_row1.pack(pady=5, fill="x", padx=5)

load_btn = ctk.CTkButton(btn_row1, text="Load & Play MIDI", command=load_and_play_midi, height=35, width=130, fg_color=ACCENT, hover_color="#1a6cd1", text_color="white", font=("TkDefaultFont", 9, "bold"), corner_radius=8)
load_btn.pack(side="left", padx=5)

export_btn = ctk.CTkButton(btn_row1, text="Export to WAV", command=export_to_wav, height=35, width=130, fg_color="#4CAF50", hover_color="#3e8e41", text_color="white", font=("TkDefaultFont", 9, "bold"), corner_radius=8)
export_btn.pack(side="left", padx=5)

btn_row2 = ctk.CTkFrame(controls_frame, fg_color="transparent")
btn_row2.pack(pady=5, fill="x", padx=5)

pause_btn = ctk.CTkButton(btn_row2, text="Pause", command=pause_playback, height=30, width=80, fg_color="#f39c12", hover_color="#d68910", text_color="white", font=("TkDefaultFont", 9, "bold"), corner_radius=6)
pause_btn.pack(side="left", padx=3)

resume_btn = ctk.CTkButton(btn_row2, text="Resume", command=resume_playback, height=30, width=80, fg_color="#27ae60", hover_color="#2ecc71", text_color="white", font=("TkDefaultFont", 9, "bold"), corner_radius=6)
resume_btn.pack(side="left", padx=3)

stop_btn = ctk.CTkButton(btn_row2, text="Stop", command=stop_playback, height=30, width=80, fg_color="#d93838", hover_color="#b52d2d", text_color="white", font=("TkDefaultFont", 9, "bold"), corner_radius=6)
stop_btn.pack(side="left", padx=3)

def populate_ram_cache_on_startup():
    from concurrent.futures import ThreadPoolExecutor
    print("Background cache pre-population started...")
    # 1. First scan CACHE_DIR and load all existing npy files
    if os.path.exists(CACHE_DIR):
        for filename in os.listdir(CACHE_DIR):
            if filename.endswith(".npy"):
                cache_key = filename[:-4]
                cache_path = os.path.join(CACHE_DIR, filename)
                try:
                    data = np.load(cache_path)
                    with global_ram_cache_lock:
                        global_ram_cache[cache_key] = data
                except Exception:
                    pass
    print(f"Loaded {len(global_ram_cache)} files from disk cache into RAM.")

    # 2. Pre-generate all notes (36 to 96) for all stops at standard tuning (440Hz) in parallel
    notes = list(range(36, 97))
    stops_to_pregen = [None] + list(STOPS.keys())
    
    tasks = []
    for stop_name in stops_to_pregen:
        for note in notes:
            freq = 440.0 * (2 ** ((note - 69) / 12))
            tasks.append((freq, 3.5, stop_name))
            
    max_workers = min(16, (os.cpu_count() or 4) * 2)
    print(f"Pre-synthesizing stops using {max_workers} parallel CPU threads...")
    
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        list(executor.map(lambda args: get_cached_stop_tone(*args), tasks))
            
    print("Background cache pre-population completed successfully!")

# Start the cache pre-population in a background thread
threading.Thread(target=populate_ram_cache_on_startup, daemon=True).start()

# Trigger initial stop caching
request_re_render()

# Start the Arduino listener in a background thread
if SERIAL_AVAILABLE:
    threading.Thread(target=listen_for_arduino, args=(ordered_stop_names,), daemon=True).start()
else:
    print("pyserial is not installed. Arduino stop control is disabled.")

def on_closing():
    close_midi_input()
    stop_playback()
    root.destroy()

root.protocol("WM_DELETE_WINDOW", on_closing)

# Populate MIDI devices on startup
refresh_midi_devices()

root.mainloop()