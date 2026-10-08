import sys
import re

with open("openorgelsynth.cpp", "r", encoding="utf-8") as f:
    content = f.read()

# Rename Hollow Gedeckt to Gedeckt
content = content.replace("Hollow Gedeckt", "Gedeckt")

# Less Tremulant
content = content.replace("double airflow_env = 1.0 + 0.005 * fast_sin(5.5 * 2.0 * M_PI * t) + wind_wobble;",
                          "double airflow_env = 1.0 + 0.001 * fast_sin(5.5 * 2.0 * M_PI * t) + wind_wobble * 0.4;")

# Increase airiness
content = content.replace("double air_amp = has_slower_drift ? 0.022 : 0.002;",
                          "double air_amp = (has_slower_drift ? 0.022 : 0.004) + 0.0015 * num_stops;")

# Add random detuning per rank for stops combining better (so they don't merge perfectly)
# Currently in cpp, rank frequency is calculated before the loop or inside?
# The ranks are pre-calculated. We need to find where rank.f is used and add a random drift.
# Since rank.f is constant, we can modify the loop over active_ranks:
old_rank_loop = """    for (const auto &rank : engine.active_ranks) {
      double current_phase = rank.phase + rank.f * two_pi_base;
      sample_val += rank.amp * fast_sin(current_phase);
    }"""
new_rank_loop = """    for (const auto &rank : engine.active_ranks) {
      // Add a tiny random walk to phase to make it imperfect and airy, but keep it stable.
      // Also add a static random detune per rank to prevent perfect merging.
      double rank_detune = 1.0 + 0.001 * ((double)fast_prng.next_float() - 0.5);
      double current_phase = rank.phase + rank.f * rank_detune * two_pi_base;
      sample_val += rank.amp * fast_sin(current_phase);
    }"""
content = content.replace(old_rank_loop, new_rank_loop)

# Voix Celeste needs to use the clarion sample, but in CPP samples are triggered separately.
# Wait, let's look at how Voix Celeste and Clarion are handled in openorgelsynth.cpp.
# Actually, the python script handles all STOPS dict logic, but C++ handles samples explicitly based on IDs?
# Let's check how openorgelsynth.cpp does samples.
with open("openorgelsynth.cpp", "w", encoding="utf-8") as f:
    f.write(content)

print("openorgelsynth.cpp updated!")
