import sys
import re

with open("openorgelsynth.cpp", "r", encoding="utf-8") as f:
    content = f.read()

# 1. Update STOPS_DB size
content = content.replace("static const StopDefinition STOPS_DB[30] = {", "static const StopDefinition STOPS_DB[32] = {")

# 2. Update array access limits
content = content.replace("stop_id < 30", "stop_id < 32")

# 3. Add Trombone and Contra Trombone to STOPS_DB
# First find where STOPS_DB ends (the closing }; before the generate_raw_tone_cpp)
# It's exactly:
#     {"Acoustic Flue 2'",
#      1,
#      {4.0},
#      {1.0},
#      true,
#      false}
# };
old_end = """    {"Acoustic Flue 2'",
     1,
     {4.0},
     {1.0},
     true,
     false}
};"""
new_end = """    {"Acoustic Flue 2'",
     1,
     {4.0},
     {1.0},
     true,
     false},
    // 30: Trombone 16'
    {"Trombone 16'",
     14,
     {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 10.0},
     {1.0, 1.2, 1.1, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05, 0.02},
     false,
     false},
    // 31: Contra Trombone 32'
    {"Contra Trombone 32'",
     13,
     {0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0},
     {1.0, 1.1, 1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05},
     false,
     false}
};"""
content = content.replace(old_end, new_end)

# 4. Voix Celeste to use sample clarion
# Find the Voix Celeste 8' definition and modify it to be sample-based
old_celeste = """    // 24: Voix Celeste 8'
    {"Voix Celeste 8'",
     10,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0},
     {1.0, 0.8, 0.6, 0.5, 0.4, 0.3, 0.2, 0.15, 0.1, 0.05},
     false,
     false},"""
new_celeste = """    // 24: Voix Celeste 8'
    {"Voix Celeste 8'",
     1,
     {1.0},
     {1.0},
     true,
     false},"""
content = content.replace(old_celeste, new_celeste)

# Update sample playback logic:
# `if (stop_id == 26 && clarion_ready) {`
# to `if ((stop_id == 26 || stop_id == 24) && clarion_ready) {`
content = content.replace("if (stop_id == 26 && clarion_ready) {", "if ((stop_id == 26 || stop_id == 24) && clarion_ready) {")
content = content.replace("} else if (stop_id != 26 && flue_ready) {", "} else if (stop_id != 26 && stop_id != 24 && flue_ready) {")
# and base frequency calculation
content = content.replace("double sample_base_freq = (stop_id == 26) ? 440.0 : 220.0;", "double sample_base_freq = (stop_id == 26 || stop_id == 24) ? 440.0 : 220.0;")
# and has_clarion_stops
content = content.replace("if (stop_id == 26) {", "if (stop_id == 26 || stop_id == 24) {")
# and sample_ready
content = content.replace("bool sample_ready = (stop_id == 26) ? clarion_ready : flue_ready;", "bool sample_ready = (stop_id == 26 || stop_id == 24) ? clarion_ready : flue_ready;")

with open("openorgelsynth.cpp", "w", encoding="utf-8") as f:
    f.write(content)

print("openorgelsynth.cpp fully updated!")
