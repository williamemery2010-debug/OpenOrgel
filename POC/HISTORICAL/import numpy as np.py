#THIS IS A HISTORICAL FILE ONLY SAVED TO GITHUB FOR DOCUMENTATION PURPOSES. THIS WAS TO TEST NUMPY AND SOUNDDEVICE.




import numpy as np
import sounddevice as sd

SAMPLE_RATE = 44100

# Drawbar amplitudes (classic Hammond-style, 9 drawbars)
drawbars = np.array([1.0, 0.8, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05])

# Harmonic multiples for each drawbar
harmonics = np.array([1, 2, 3, 4, 5, 6, 8, 10, 12])

def organ_wave(frequency, duration):
    t = np.linspace(0, duration, int(SAMPLE_RATE * duration), False)
    wave = np.zeros_like(t)

    # Additive synthesis
    for amp, harm in zip(drawbars, harmonics):
        wave += amp * np.sin(2 * np.pi * frequency * harm * t)

    # Normalize
    wave /= np.max(np.abs(wave))

    # Simple envelope (organ = fast attack, slow release)
    attack = int(0.01 * SAMPLE_RATE)
    release = int(0.1 * SAMPLE_RATE)

    envelope = np.ones_like(wave)
    envelope[:attack] = np.linspace(0, 1, attack)
    envelope[-release:] = np.linspace(1, 0, release)

    return wave * envelope

def play_note(freq, duration=1.5):
    wave = organ_wave(freq, duration)
    sd.play(wave, SAMPLE_RATE)
    sd.wait()

# Example: C minor chord
notes = [261.63, 311.13, 392.00]  # C, Eb, G

combined = sum(organ_wave(n, 2) for n in notes)
combined /= np.max(np.abs(combined))

sd.play(combined, SAMPLE_RATE)
sd.wait()
