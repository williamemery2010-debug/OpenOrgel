import sys
import re

with open("openorgelsynth.cpp", "r", encoding="utf-8") as f:
    content = f.read()

# Fix Voix Celeste detuning for samples
old_sample_freq = """        for (int h = 0; h < stop.num_harmonics; h++) {
          double rank_freq = freq * stop.harmonics[h];"""
new_sample_freq = """        for (int h = 0; h < stop.num_harmonics; h++) {
          double rank_freq = (stop_id == 24 ? freq * 1.006 : freq) * stop.harmonics[h];"""
content = content.replace(old_sample_freq, new_sample_freq)

with open("openorgelsynth.cpp", "w", encoding="utf-8") as f:
    f.write(content)

with open("newsynthesis.py", "r", encoding="utf-8") as f:
    content = f.read()

# Fix Voix Celeste detuning for samples in Python
old_py_sample = """            # Apply celeste detuning (tuning slightly sharp)
            is_celeste = "Voix Celeste" in stop_name
            stop_freq = freq * 1.006 if is_celeste else freq"""
# Wait, python's sample generation is not in generate_raw_tone_python!
# Ah! Python does not synthesize samples in `generate_raw_tone_python`?
# Wait! In `newsynthesis.py`, if a stop is a sample (`is_sample`), it generates a SINE WAVE placeholder in `generate_raw_tone_python`?
# Oh! Let's check `newsynthesis.py` around line 335.
