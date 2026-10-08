import os
import re

cpp_file = "openorgelsynth.cpp"
py_file = "newsynthesis.py"
ino_file = "organconsole.ino"

with open(cpp_file, "r", encoding="utf-8") as f:
    cpp_data = f.read()

# Remove Oboe and Vox Humana from STOPS_DB array in cpp
cpp_data = re.sub(r'\s*// \d+: Oboe 8\'.*?false\},', '', cpp_data, flags=re.DOTALL)
cpp_data = re.sub(r'\s*// \d+: Vox Humana 8\'.*?false\},', '', cpp_data, flags=re.DOTALL)

# Add Forceful Reed 16' and 32'
cpp_data = cpp_data.replace(
    '// 17: Hollow Gedeckt 8\' (Airy)',
    '// 30: Forceful Reed 16\'\n    {"Forceful Reed 16\'", 10, {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0}, {1.0, 1.0, 0.8, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05}, false, false},\n    // 31: Forceful Reed 32\'\n    {"Forceful Reed 32\'", 10, {0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5}, {1.0, 1.0, 0.8, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05}, false, false},\n    // 17: Hollow Gedeckt 8\' (Airy)'
)

# Rename Hollow Gedeckt to Gedeckt
cpp_data = cpp_data.replace("Hollow Gedeckt", "Gedeckt")

# Change Voix Celeste 8' to be slightly detuned clarion (sample based)
cpp_data = re.sub(r'\{"Voix Celeste 8\'",\s*\d+,\s*\{.*?\},\s*\{.*?\},\s*false,\s*false\},', '{"Voix Celeste 8\'", 1, {1.002}, {1.0}, false, true},', cpp_data, flags=re.DOTALL)

# Reduce tremulant in cpp
cpp_data = cpp_data.replace('0.005 * fast_sin(5.5', '0.002 * fast_sin(5.5')
cpp_data = cpp_data.replace('0.0015 * fast_sin(0.7', '0.0007 * fast_sin(0.7')
cpp_data = cpp_data.replace('0.0010 * fast_sin(1.3', '0.0005 * fast_sin(1.3')
cpp_data = cpp_data.replace('0.0008 * fast_sin(2.8', '0.0004 * fast_sin(2.8')

# Fix clipping / Add chorus detuning in cpp
cpp_data = cpp_data.replace('double f = stop_freq * harmonic_factor *\n                     (1.0 + 0.00015 * (harmonic_factor * harmonic_factor));',
                            'double stop_detune = (active_stop_ids[i] * 7 % 11 - 5) * 0.00015;\n          double f = stop_freq * harmonic_factor *\n                     (1.0 + 0.00015 * (harmonic_factor * harmonic_factor) + stop_detune);')

# Decrease chiff and faster response for Clarion in cpp
cpp_data = cpp_data.replace('double chiff_amp = has_slower_drift ? 0.55 : 0.22;',
                            'bool has_clarion = false;\n  for(int k=0; k<num_stops; k++) if(active_stop_ids[k] == 26) has_clarion = true;\n  double chiff_amp = has_clarion ? 0.05 : (has_slower_drift ? 0.55 : 0.22);')

with open(cpp_file, "w", encoding="utf-8") as f:
    f.write(cpp_data)


# Now Python file
with open(py_file, "r", encoding="utf-8") as f:
    py_data = f.read()

# Remove Oboe and Vox Humana
py_data = re.sub(r'\s*"Oboe 8\'":\s*\{(?:[^{}]|)*\},', '', py_data, flags=re.DOTALL)
py_data = re.sub(r'\s*"Vox Humana 8\'":\s*\{(?:[^{}]|)*\},', '', py_data, flags=re.DOTALL)
py_data = re.sub(r'\s*"Oboe 8\'": \d+,', '', py_data)
py_data = re.sub(r'\s*"Vox Humana 8\'": \d+,', '', py_data)

# Rename Hollow Gedeckt to Gedeckt
py_data = py_data.replace("Hollow Gedeckt", "Gedeckt")

# Add Forceful Reed 16' and 32'
forceful = '''
    "Forceful Reed 16'": {
        "harmonics": np.array([0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0]),
        "amplitudes": np.array([1.0, 1.0, 0.8, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05])
    },
    "Forceful Reed 32'": {
        "harmonics": np.array([0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5]),
        "amplitudes": np.array([1.0, 1.0, 0.8, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05])
    },
'''
py_data = py_data.replace('"Gedeckt 8\' (Airy)": {', forceful + '    "Gedeckt 8\' (Airy)": {')
py_data = py_data.replace('"Gedeckt 8\' (Airy)": 17,', '"Forceful Reed 16\'": 30,\n    "Forceful Reed 32\'": 31,\n    "Gedeckt 8\' (Airy)": 17,')

# Change Voix Celeste 8'
voix = '''"Voix Celeste 8'": {
        "harmonics": np.array([1.002]),
        "amplitudes": np.array([1.0]),
        "is_sample": True,
        "sample_type": "clarion"
    },'''
py_data = re.sub(r'"Voix Celeste 8\'":\s*\{(?:[^{}]|)*\},', voix, py_data, flags=re.DOTALL)

# Reduce tremulant in python
py_data = py_data.replace('0.005 * np.sin(5.5', '0.002 * np.sin(5.5')
py_data = py_data.replace('0.0015 * np.sin(0.7', '0.0007 * np.sin(0.7')
py_data = py_data.replace('0.0010 * np.sin(1.3', '0.0005 * np.sin(1.3')
py_data = py_data.replace('0.0008 * np.sin(2.8', '0.0004 * np.sin(2.8')

# Fix clipping / Add chorus detuning in python
py_data = py_data.replace('f = stop_freq * h * (1.0 + 0.00015 * (h ** 2))',
                          'stop_id_hash = sum(ord(c) for c in stop_name)\n                stop_detune = (stop_id_hash % 11 - 5) * 0.00015\n                f = stop_freq * h * (1.0 + 0.00015 * (h ** 2) + stop_detune)')

# Faster response for clarion in python
py_data = py_data.replace('attack_samples = int(0.12 * SAMPLE_RATE)', 'attack_samples = int(0.05 * SAMPLE_RATE) if "Clarion" in stop_name else int(0.12 * SAMPLE_RATE)')
py_data = py_data.replace('attack = min(int(0.12 * SAMPLE_RATE)', 'has_clarion = any("Clarion" in s for s in active_stops) if active_stops else False\n            attack_len = 0.05 if has_clarion else 0.12\n            attack = min(int(attack_len * SAMPLE_RATE)')

# Decrease chiff for clarion in python
py_data = py_data.replace('chiff_amp = 0.55 if has_slower_drift else 0.22', 'has_clarion = any("Clarion" in s for s in active_stops) if active_stops else False\n    chiff_amp = 0.05 if has_clarion else (0.55 if has_slower_drift else 0.22)')

with open(py_file, "w", encoding="utf-8") as f:
    f.write(py_data)

# Update organconsole.ino
with open(ino_file, "r", encoding="utf-8") as f:
    ino_data = f.read()

ino_data = ino_data.replace('"Oboe 8\'"', '"Forceful Reed 16\'"')
ino_data = ino_data.replace('"Vox Humana 8\'"', '"Clarion 4\'"')
ino_data = ino_data.replace('Hollow Gedeckt', 'Gedeckt')

with open(ino_file, "w", encoding="utf-8") as f:
    f.write(ino_data)

print("Patching complete!")
