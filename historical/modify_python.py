import sys
import re

with open("newsynthesis.py", "r", encoding="utf-8") as f:
    content = f.read()

# 1. Remove Oboe 8'
content = re.sub(r'\s*"Oboe 8\'": \{[^}]+\},', '', content)
content = re.sub(r'\s*"Oboe 8\'": \d+,', '', content)

# 2. Remove Vox Humana 8'
content = re.sub(r'\s*"Vox Humana 8\'": \{[^}]+\},', '', content)
content = re.sub(r'\s*"Vox Humana 8\'": \d+,', '', content)

# 3. Rename Hollow Gedeckt to Gedeckt
content = content.replace("Hollow Gedeckt", "Gedeckt")

# 4. Update Voix Celeste 8' to be a detuned clarion
celeste_old = r'"Voix Celeste 8\'": \{[^}]+\}'
celeste_new = """"Voix Celeste 8'": {
        "harmonics": np.array([1.0]),
        "amplitudes": np.array([1.0]),
        "is_sample": True,
        "sample_type": "clarion"
    }"""
content = re.sub(celeste_old, celeste_new, content)

# 5. Add Trombone 16' and Contra Trombone 32'
new_stops = """    "Trombone 16'": {
        "harmonics": np.array([0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 10.0]),
        "amplitudes": np.array([1.0, 1.2, 1.1, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05, 0.02])
    },
    "Contra Trombone 32'": {
        "harmonics": np.array([0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0]),
        "amplitudes": np.array([1.0, 1.1, 1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05])
    },
"""
content = content.replace('"Cymbale Mixture"', new_stops + '    "Cymbale Mixture"')

# 6. Add Trombone to STOP_NAME_TO_ID
new_ids = """    "Trombone 16'": 30,
    "Contra Trombone 32'": 31,
"""
content = content.replace('"Cymbale Mixture": 21', new_ids + '    "Cymbale Mixture": 21')

# 7. Less Tremulant and randomize phase per rank (Python)
content = content.replace("airflow_env = 1.0 + 0.005 * np.sin(5.5 * 2 * np.pi * t) + wind_wobble",
                          "airflow_env = 1.0 + 0.001 * np.sin(5.5 * 2 * np.pi * t) + wind_wobble * 0.4")
content = content.replace("f = stop_freq * h * (1.0 + 0.00015 * (h ** 2))",
                          "f = stop_freq * h * (1.0 + 0.00015 * (h ** 2)) * np.random.uniform(0.9993, 1.0007)")

# 8. Character and Airiness (Python)
# We want to add noise to each synthesized rank.
# Actually, the user says "need more character and airiness (light) and imperfection"
# Let's increase airiness proportionally and add a slight random amplitude fluctuation.
content = content.replace("air_amp = 0.022 if has_slower_drift else 0.002",
                          "air_amp = (0.022 if has_slower_drift else 0.004) + 0.0015 * len(active_stops)")
# Replace the simple fast sin generation with something slightly noisier or just the random detune handles the imperfection.
# Also let's change Voix Celeste logic in Python:
content = content.replace('is_celeste = "Voix Celeste" in stop_name\n            stop_freq = freq * 1.003 if is_celeste else freq',
                          'is_celeste = "Voix Celeste" in stop_name\n            stop_freq = freq * 1.006 if is_celeste else freq')

with open("newsynthesis.py", "w", encoding="utf-8") as f:
    f.write(content)

print("newsynthesis.py updated!")
