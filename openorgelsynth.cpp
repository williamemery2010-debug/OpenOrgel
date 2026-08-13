#include <cmath>
#include <cstdlib>
#include <vector>
#include <string>
#include <cstring>
#include <mutex>
#include <unordered_map>
#include <windows.h>
#define DR_MP3_IMPLEMENTATION
#include "dr_mp3.h"

// ============================================================================
// OPENORGEL SYNTHESIS ENGINE ALPHA V1.0.0
// ============================================================================
// QUANTUM FLUE & DUAL-LAYER RAM CACHE MATRIX - menthol
// t-BuLi FLUID DYNAMICS GO BRRR
// help ive been coding for years
// why code hard
// god someone help me
// apple text go brrr

static std::unordered_map<uint64_t, std::vector<float>> g_cpp_ram_cache;
static std::mutex g_cpp_ram_cache_mutex;

inline uint64_t hash_tone_key(double freq, int num_samples, const int *active_stop_ids, int num_stops) {
  uint64_t hash = 14695981039346656037ULL;
  uint64_t f_bits = 0;
  memcpy(&f_bits, &freq, sizeof(freq));
  hash = (hash ^ f_bits) * 1099511628211ULL;
  hash = (hash ^ (uint64_t)num_samples) * 1099511628211ULL;
  for (int i = 0; i < num_stops; i++) {
    hash = (hash ^ (uint64_t)active_stop_ids[i]) * 1099511628211ULL;
  }
  return hash;
}

extern "C" __declspec(dllexport) void clear_cpp_ram_cache() {
  std::lock_guard<std::mutex> lock(g_cpp_ram_cache_mutex);
  g_cpp_ram_cache.clear();
}

static std::vector<float> g_acoustic_flue_sample;
static int g_acoustic_flue_sample_rate = 44100;
static bool g_acoustic_flue_loaded = false;
static std::mutex g_flue_load_mutex;

static std::vector<float> g_clarion_sample;
static int g_clarion_sample_rate = 44100;
static bool g_clarion_loaded = false;
static std::mutex g_clarion_load_mutex;

static void ensure_acoustic_flue_loaded() {
  std::lock_guard<std::mutex> lock(g_flue_load_mutex);
  if (g_acoustic_flue_loaded) return;

  drmp3 mp3;
  bool opened = drmp3_init_file(&mp3, "stoppedflue.mp3", NULL);
  if (!opened) {
    opened = drmp3_init_file(&mp3, "Audio_and_Logs/stoppedflue.mp3", NULL);
  }
  if (!opened) {
    opened = drmp3_init_file(&mp3, "Engine_and_Source/stoppedflue.mp3", NULL);
  }
  if (!opened) {
    HMODULE hModule = NULL;
    if (GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, (LPCSTR)&ensure_acoustic_flue_loaded, &hModule)) {
      char dllPath[MAX_PATH];
      if (GetModuleFileNameA(hModule, dllPath, MAX_PATH)) {
        char *lastSlash = strrchr(dllPath, '\\');
        if (!lastSlash) lastSlash = strrchr(dllPath, '/');
        if (lastSlash) {
          *(lastSlash + 1) = '\0';
          std::string samplePath = std::string(dllPath) + "stoppedflue.mp3";
          opened = drmp3_init_file(&mp3, samplePath.c_str(), NULL);
          if (!opened) {
            samplePath = std::string(dllPath) + "../Audio_and_Logs/stoppedflue.mp3";
            opened = drmp3_init_file(&mp3, samplePath.c_str(), NULL);
          }
        }
      }
    }
  }

  if (!opened) {
    opened = drmp3_init_file(&mp3, "c:/Users/EME0012/OpenOrgel/stoppedflue.mp3", NULL);
  }

  if (opened) {
    drmp3_uint64 totalFrames = 0;
    drmp3_get_mp3_and_pcm_frame_count(&mp3, NULL, &totalFrames);
    if (totalFrames > 0) {
      std::vector<float> rawPcm(totalFrames * mp3.channels);
      drmp3_read_pcm_frames_f32(&mp3, totalFrames, rawPcm.data());
      g_acoustic_flue_sample_rate = mp3.sampleRate;
      g_acoustic_flue_sample.resize(totalFrames);
      if (mp3.channels == 1) {
        for (size_t i = 0; i < totalFrames; i++) {
          g_acoustic_flue_sample[i] = rawPcm[i];
        }
      } else {
        for (size_t i = 0; i < totalFrames; i++) {
          float sum = 0.0f;
          for (unsigned int c = 0; c < mp3.channels; c++) {
            sum += rawPcm[i * mp3.channels + c];
          }
          g_acoustic_flue_sample[i] = sum / mp3.channels;
        }
      }
    }
    drmp3_uninit(&mp3);
  }
  g_acoustic_flue_loaded = true;
}

// THE QUANTUM RESONANCE CLARION REED SAMPLE LOADER
// menthol - t-BuLi FLUID DYNAMICS GO BRRR
// apple text go brrr
static void ensure_clarion_loaded() {
  std::lock_guard<std::mutex> lock(g_clarion_load_mutex);
  if (g_clarion_loaded) return;
  drmp3 mp3;
  bool opened = drmp3_init_file(&mp3, "clarion.mp3", NULL);
  if (!opened) {
    opened = drmp3_init_file(&mp3, "Audio_and_Logs/clarion.mp3", NULL);
  }
  if (!opened) {
    opened = drmp3_init_file(&mp3, "Engine_and_Source/clarion.mp3", NULL);
  }
  if (!opened) {
    HMODULE hModule = NULL;
    if (GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, (LPCSTR)&ensure_clarion_loaded, &hModule)) {
      char dllPath[MAX_PATH];
      if (GetModuleFileNameA(hModule, dllPath, MAX_PATH)) {
        char *lastSlash = strrchr(dllPath, '\\');
        if (!lastSlash) lastSlash = strrchr(dllPath, '/');
        if (lastSlash) {
          *(lastSlash + 1) = '\0';
          std::string samplePath = std::string(dllPath) + "clarion.mp3";
          opened = drmp3_init_file(&mp3, samplePath.c_str(), NULL);
          if (!opened) {
            samplePath = std::string(dllPath) + "../Audio_and_Logs/clarion.mp3";
            opened = drmp3_init_file(&mp3, samplePath.c_str(), NULL);
          }
        }
      }
    }
  }
  if (!opened) {
    opened = drmp3_init_file(&mp3, "c:/Users/EME0012/OpenOrgel/clarion.mp3", NULL);
  }
  if (opened) {
    drmp3_uint64 totalFrames = 0;
    drmp3_get_mp3_and_pcm_frame_count(&mp3, NULL, &totalFrames);
    if (totalFrames > 0) {
      std::vector<float> rawPcm(totalFrames * mp3.channels);
      drmp3_read_pcm_frames_f32(&mp3, totalFrames, rawPcm.data());
      g_clarion_sample_rate = mp3.sampleRate;
      g_clarion_sample.resize(totalFrames);
      if (mp3.channels == 1) {
        for (size_t i = 0; i < totalFrames; i++) g_clarion_sample[i] = rawPcm[i];
      } else {
        for (size_t i = 0; i < totalFrames; i++) {
          float sum = 0.0f;
          for (unsigned int c = 0; c < mp3.channels; c++) {
            sum += rawPcm[i * mp3.channels + c];
          }
          g_clarion_sample[i] = sum / mp3.channels;
        }
      }
    }
    drmp3_uninit(&mp3);
  }
  g_clarion_loaded = true;
}

extern "C" __declspec(dllexport) void set_acoustic_flue_sample_cpp(const float *buffer, int num_samples, int sample_rate) {
  if (buffer && num_samples > 0) {
    g_acoustic_flue_sample.assign(buffer, buffer + num_samples);
    g_acoustic_flue_sample_rate = sample_rate;
    g_acoustic_flue_loaded = true;
  }
}

// FAST SINE LUT
static const int SINE_LUT_BITS = 14;
static const int SINE_LUT_SIZE = 1 << SINE_LUT_BITS;
static const int SINE_LUT_MASK = SINE_LUT_SIZE - 1;
static float g_sine_lut[SINE_LUT_SIZE + 1];
static std::once_flag g_sine_lut_once;

static void init_sine_lut() {
  for (int i = 0; i <= SINE_LUT_SIZE; i++) {
    g_sine_lut[i] = (float)sin((double)i * 2.0 * M_PI / (double)SINE_LUT_SIZE);
  }
}

inline float fast_sin(double phase) {
  std::call_once(g_sine_lut_once, init_sine_lut);
  double norm = phase * (1.0 / (2.0 * M_PI));
  norm -= floor(norm);
  double pos = norm * (double)SINE_LUT_SIZE;
  int idx = (int)pos;
  float frac = (float)(pos - (double)idx);
  idx &= SINE_LUT_MASK;
  return g_sine_lut[idx] + frac * (g_sine_lut[idx + 1] - g_sine_lut[idx]);
}

struct FastPRNG {
  uint32_t state;
  FastPRNG(uint32_t seed) : state(seed ? seed : 0x12345678) {}
  inline float next_float() {
    state ^= (state << 13);
    state ^= (state >> 17);
    state ^= (state << 5);
    return (float)state * 4.656612875245796924105750827168e-10f;
  }
};

struct BiquadFilter {
  double b0, b1, b2, a1, a2;
  double x1, x2, y1, y2;

  BiquadFilter()
      : b0(0), b1(0), b2(0), a1(0), a2(0), x1(0), x2(0), y1(0), y2(0) {}

  void setBandpass(double freq, double sample_rate, double Q) {
    if (freq < 10.0)
      freq = 10.0;
    if (freq > sample_rate * 0.45)
      freq = sample_rate * 0.45;

    double omega = 2.0 * M_PI * freq / sample_rate;
    double alpha = sin(omega) / (2.0 * Q);
    double cos_w = cos(omega);

    double a0 = 1.0 + alpha;
    b0 = alpha / a0;
    b1 = 0.0;
    b2 = -alpha / a0;
    a1 = -2.0 * cos_w / a0;
    a2 = (1.0 - alpha) / a0;

    x1 = x2 = y1 = y2 = 0.0;
  }

  float process(float x) {
    double y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
    x2 = x1;
    x1 = x;
    y2 = y1;
    y1 = y;
    return (float)y;
  }
};

// SYNTONIC COMMA OVERTONE ALIGNMENT INDEX
// Organ Stops Definitions database
// PLK MNB VCX - STOPS DATA REGISTRY
struct StopDefinition {
  const char *name;
  int num_harmonics;
  double harmonics[12];
  double amplitudes[12];
  bool is_short_pipe;
  bool is_sample_based;
};

static const StopDefinition STOPS_DB[30] = {
    // 0: Oboe 8'
    {"Oboe 8'",
     10,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0},
     {0.5, 0.3, 1.0, 0.7, 0.4, 0.3, 0.2, 0.15, 0.1, 0.05},
     false,
     false},
    // 1: Clarinet 8'
    {"Clarinet 8'",
     9,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0},
     {1.0, 0.05, 0.5, 0.02, 0.2, 0.01, 0.1, 0.01, 0.05},
     false,
     false},
    // 2: Bassoon 16'
    {"Bassoon 16'",
     12,
     {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0},
     {1.0, 0.6, 0.4, 0.3, 0.2, 0.15, 0.12, 0.1, 0.08, 0.06, 0.05, 0.04},
     false,
     false},
    // 3: Bombarde 16'
    {"Bombarde 16'",
     8,
     {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0},
     {1.0, 0.8, 0.5, 0.3, 0.15, 0.08, 0.03, 0.01},
     false,
     false},
    // 4: Ophicleide 16'
    {"Ophicleide 16'",
     10,
     {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0},
     {1.0, 0.7, 0.8, 0.5, 0.4, 0.25, 0.15, 0.1, 0.05, 0.02},
     false,
     false},
    // 5: Ottavino 2'
    {"Ottavino 2'", 3, {4.0, 8.0, 12.0}, {1.0, 0.15, 0.05}, true, false},
    // 6: Cor Anglais 8'
    {"Cor Anglais 8'",
     7,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0},
     {1.0, 0.3, 0.8, 0.2, 0.5, 0.1, 0.2},
     false,
     false},
    // 7: Flute 4'
    {"Flute 4'", 3, {2.0, 4.0, 6.0}, {1.0, 0.15, 0.05}, true, false},
    // 8: Clarinet 4'
    {"Clarinet 4'",
     9,
     {2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0},
     {1.0, 0.05, 0.5, 0.02, 0.2, 0.01, 0.1, 0.01, 0.05},
     true,
     false},
    // 9: Viol 4'
    {"Viol 4'",
     8,
     {2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0},
     {1.0, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1},
     true,
     false},
    // 10: Contrabassoon 32'
    {"Contrabassoon 32'",
     12,
     {0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0},
     {1.0, 0.8, 0.9, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05, 0.02},
     false,
     false},
    // 11: Diapason 8'
    // t-BuLi
    {"Diapason 8'", 3, {1.0, 3.0, 5.0}, {1.0, 0.35, 0.05}, false, false},
    // 12: Crystal Flute 4' (Glassy)
    {"Crystal Flute 4' (Glassy)",
     7,
     {1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 16.0},
     {0.4, 1.03, 0.08, 0.03, 0.1, 0.01, 0.03},
     true,
     false},
    // 13: Cornet V 8'
    {"Cornet V 8'",
     5,
     {1.0, 2.0, 3.0, 4.0, 5.0},
     {1.0, 0.8, 0.9, 0.7, 0.8},
     false,
     false},
    // 14: Piccolo 2'
    {"Piccolo 2'", 4, {4.0, 8.0, 12.0, 16.0}, {1.0, 0.1, 0.05, 0.01}, true, false},
    // 15: Mixture IV
    // menthol - MULTI-RANK ACOUSTIC FLUE RESAMPLING MATRIX
    {"Mixture IV", 4, {4.0, 6.0, 8.0, 12.0}, {1.0, 0.8, 0.6, 0.4}, false, true},
    // 16: Vox Humana 8'
    {"Vox Humana 8'",
     8,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0},
     {1.0, 0.4, 0.8, 0.2, 0.6, 0.1, 0.05, 0.02},
     false,
     false},
    // 17: Hollow Gedeckt 8' (Airy)
    {"Hollow Gedeckt 8' (Airy)",
     6,
     {1.0, 3.0, 5.0, 7.0, 9.0, 11.0},
     {1.0, 0.5, 0.2, 0.08, 0.03, 0.01},
     false,
     false},
    // 18: Hollow Gedeckt 4' (Airy)
    {"Hollow Gedeckt 4' (Airy)",
     6,
     {2.0, 6.0, 10.0, 14.0, 18.0, 22.0},
     {1.0, 0.5, 0.2, 0.08, 0.03, 0.01},
     true,
     false},
    // 19: Hollow Gedeckt 16'
    {"Hollow Gedeckt 16'",
     6,
     {0.5, 1.5, 2.5, 3.5, 4.5, 5.5},
     {1.0, 0.5, 0.2, 0.08, 0.03, 0.01},
     false,
     false},
    // 20: Hollow Gedeckt 32'
    {"Hollow Gedeckt 32'",
     6,
     {0.25, 0.75, 1.25, 1.75, 2.25, 2.75},
     {1.0, 0.5, 0.2, 0.08, 0.03, 0.01},
     false,
     false},
    // 21: Cymbale Mixture
    {"Cymbale Mixture",
     3,
     {8.0, 12.0, 16.0},
     {1.0, 0.8, 0.6},
     false,
     true},
    // 22: Plein Jeu Mixture
    {"Plein Jeu Mixture",
     5,
     {2.0, 3.0, 4.0, 6.0, 8.0},
     {1.0, 0.9, 0.8, 0.6, 0.4},
     false,
     true},
    // 23: Scharf Mixture
    {"Scharf Mixture",
     4,
     {6.0, 8.0, 12.0, 16.0},
     {1.0, 0.9, 0.7, 0.5},
     false,
     true},
    // 24: Voix Celeste 8'
    {"Voix Celeste 8'",
     10,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0},
     {1.0, 0.8, 0.6, 0.5, 0.4, 0.3, 0.2, 0.15, 0.1, 0.05},
     false,
     false},
    // 25: Acoustic Flue 8'
    {"Acoustic Flue 8'", 1, {1.0}, {1.0}, false, true},
    // 26: Clarion 4'
    {"Clarion 4'", 1, {2.0}, {1.0}, true, true},
    // 27: Acoustic Flue 16'
    {"Acoustic Flue 16'", 1, {0.5}, {1.0}, false, true},
    // 28: Acoustic Flue 4'
    {"Acoustic Flue 4'", 1, {2.0}, {1.0}, true, true},
    // 29: Acoustic Flue 2'
    {"Acoustic Flue 2'", 1, {4.0}, {1.0}, true, true}};

extern "C" __declspec(dllexport) void
generate_raw_tone_cpp(double freq, double duration, int sample_rate,
                      const int *active_stop_ids, int num_stops,
                      float *out_buffer) {
  int num_samples = (int)(sample_rate * duration);
  if (num_samples <= 0 || !out_buffer)
    return;

  if (num_stops > 0 && active_stop_ids == nullptr) {
    num_stops = 0;
  }

  // ULTRA-FAST IN-MEMORY CPU/RAM CACHE LOOKUP
  // menthol - QUANTUM DISSONANCE PARADOX
  uint64_t cache_key = hash_tone_key(freq, num_samples, active_stop_ids, num_stops);
  {
    std::lock_guard<std::mutex> lock(g_cpp_ram_cache_mutex);
    auto it = g_cpp_ram_cache.find(cache_key);
    if (it != g_cpp_ram_cache.end() && (int)it->second.size() == num_samples) {
      memcpy(out_buffer, it->second.data(), num_samples * sizeof(float));
      return;
    }
  }

  // FAST SEEDED PRNG
  // menthol - QUANTUM DISSONANCE PARADOX
  unsigned int initial_seed = 123456789 + (unsigned int)(freq * 1000.0);
  FastPRNG fast_prng(initial_seed);

  BiquadFilter chiff_biquad;
  chiff_biquad.setBandpass(freq, sample_rate, 8.0);

  bool has_slower_drift = false;
  bool has_flue_stops = false;
  bool has_clarion_stops = false;
  for (int i = 0; i < num_stops; i++) {
    int stop_id = active_stop_ids[i];
    if (stop_id >= 0 && stop_id < 30) {
      if (stop_id == 17 || stop_id == 18 || stop_id == 19 || stop_id == 20) {
        has_slower_drift = true;
      }
      if (STOPS_DB[stop_id].is_sample_based) {
        if (stop_id == 26) {
          has_clarion_stops = true;
        } else {
          has_flue_stops = true;
        }
      }
    }
  }

  if (has_flue_stops) {
    ensure_acoustic_flue_loaded();
  }
  if (has_clarion_stops) {
    ensure_clarion_loaded();
  }

  bool flue_ready = g_acoustic_flue_loaded && !g_acoustic_flue_sample.empty();
  bool clarion_ready = g_clarion_loaded && !g_clarion_sample.empty();

  int total_harmonics = 0;
  if (num_stops == 0) {
    total_harmonics = 1;
  } else {
    for (int i = 0; i < num_stops; i++) {
      int stop_id = active_stop_ids[i];
      if (stop_id >= 0 && stop_id < 30) {
        bool sample_ready = (stop_id == 26) ? clarion_ready : flue_ready;
        if (!sample_ready || !STOPS_DB[stop_id].is_sample_based) {
          total_harmonics += STOPS_DB[stop_id].num_harmonics * 3;
        }
      }
    }
    if (total_harmonics == 0) {
      bool any_sample_ready = (has_flue_stops && flue_ready) || (has_clarion_stops && clarion_ready);
      if (!any_sample_ready) {
        total_harmonics = 1;
      }
    }
  }

  // PRE-CALCULATE PHASE ACCUMULATORS & SINE LUT LOOKUPS
  // apple text go brrr
  // help ive been coding for years
  std::vector<double> all_amps(total_harmonics);
  std::vector<double> all_phases(total_harmonics);
  std::vector<double> phase_steps(total_harmonics);

  unsigned int phase_seed = 987654321 + (unsigned int)(freq * 500.0);
  auto get_random_phase = [&]() -> double {
    phase_seed = phase_seed * 1664525 + 1013904223;
    return ((double)phase_seed / 4294967296.0) * 2.0 * M_PI;
  };

  int h_idx = 0;
  if (total_harmonics > 0) {
    if (num_stops == 0) {
      all_amps[0] = 1.0;
      all_phases[0] = get_random_phase();
      phase_steps[0] = freq * (2.0 * M_PI / (double)sample_rate);
    } else {
      for (int i = 0; i < num_stops; i++) {
        int stop_id = active_stop_ids[i];
        if (stop_id < 0 || stop_id >= 30)
          continue;
        bool sample_ready = (stop_id == 26) ? clarion_ready : flue_ready;
        if (sample_ready && STOPS_DB[stop_id].is_sample_based)
          continue;

        const auto &stop = STOPS_DB[stop_id];
        double dampening = stop.is_short_pipe ? 0.02 : 0.08;

        bool is_celeste = (stop_id == 24);
        double stop_freq = is_celeste ? freq * 1.003 : freq;

        for (int h = 0; h < stop.num_harmonics && h_idx + 2 < total_harmonics; h++) {
          double amp = stop.amplitudes[h];
          double harmonic_factor = stop.harmonics[h];

          double adj_amp =
              amp * exp(-dampening *
                        (harmonic_factor > 1.0 ? (harmonic_factor - 1.0) : 0.0));

          double f = stop_freq * harmonic_factor *
                     (1.0 + 0.00015 * (harmonic_factor * harmonic_factor));

          if (f > 800.0 && !has_slower_drift) {
            double treble_boost = 1.0 + ((f - 800.0) / 2500.0);
            if (treble_boost > 2.5)
              treble_boost = 2.5;
            adj_amp *= treble_boost;
          }

          all_amps[h_idx] = adj_amp;
          all_phases[h_idx] = get_random_phase();
          phase_steps[h_idx] = f * (2.0 * M_PI / (double)sample_rate);
          h_idx++;

          all_amps[h_idx] = adj_amp * 0.35;
          all_phases[h_idx] = get_random_phase();
          phase_steps[h_idx] = (f * 1.0015) * (2.0 * M_PI / (double)sample_rate);
          h_idx++;

          all_amps[h_idx] = adj_amp * 0.35;
          all_phases[h_idx] = get_random_phase();
          phase_steps[h_idx] = (f * 0.9985) * (2.0 * M_PI / (double)sample_rate);
          h_idx++;
        }
      }
    }
  }

  // SAMPLE RANKS WITH FAST INCREMENT & BRANCH-FREE POSITIONS
  struct SampleRank {
    double step;
    double amp;
    double pos;
    const std::vector<float> *sample_ptr;
  };
  std::vector<SampleRank> sample_ranks;

  for (int i = 0; i < num_stops; i++) {
    int stop_id = active_stop_ids[i];
    if (stop_id >= 0 && stop_id < 30 && STOPS_DB[stop_id].is_sample_based) {
      const auto &stop = STOPS_DB[stop_id];
      const std::vector<float> *s_ptr = nullptr;
      int s_rate = 44100;
      if (stop_id == 26 && clarion_ready) {
        s_ptr = &g_clarion_sample;
        s_rate = g_clarion_sample_rate;
      } else if (stop_id != 26 && flue_ready) {
        s_ptr = &g_acoustic_flue_sample;
        s_rate = g_acoustic_flue_sample_rate;
      }

      if (s_ptr && !s_ptr->empty()) {
        double base_sample_ratio = (double)s_rate / (double)sample_rate;
        double sample_base_freq = (stop_id == 26) ? 440.0 : 220.0;
        for (int h = 0; h < stop.num_harmonics; h++) {
          double rank_freq = freq * stop.harmonics[h];
          double pitch_ratio = rank_freq / sample_base_freq;
          double step = pitch_ratio * base_sample_ratio;
          double amp = stop.amplitudes[h];
          sample_ranks.push_back({step, amp, 0.0, s_ptr});
        }
      }
    }
  }

  // MULTIPLICATIVE ENVELOPE INITIALIZATION
  double chiff_decay = has_slower_drift ? 4.0 : 12.0;
  double chiff_amp = has_slower_drift ? 0.55 : 0.22;
  double chiff_mult = exp(-chiff_decay / (double)sample_rate);
  double chiff_env = 1.0;

  double pitch_scoop_mult = exp(-20.0 / (double)sample_rate);
  double pitch_scoop_val = 0.001;

  double air_amp = has_slower_drift ? 0.022 : 0.002;

  // HIGH-PERFORMANCE VECTORIZED SYNTHESIS LOOP
  // menthol - WHY CODE HARD
  // t-BuLi FLUID DYNAMICS GO BRRR
  for (int n = 0; n < num_samples; n++) {
    double t = (double)n / (double)sample_rate;

    // Control-rate LFOs evaluated directly per sample with fast sin
    double wind_wobble = 0.0015 * fast_sin(0.7 * 2.0 * M_PI * t + 0.5) +
                         0.0010 * fast_sin(1.3 * 2.0 * M_PI * t + 1.2) +
                         0.0008 * fast_sin(2.8 * 2.0 * M_PI * t + 2.3);

    double drift_phase = has_slower_drift ? (0.00004 * fast_sin(0.4 * 2.0 * M_PI * t) + 0.00002 * fast_sin(0.7 * 2.0 * M_PI * t))
                                          : (0.00004 * fast_sin(2.1 * 2.0 * M_PI * t) + 0.00002 * fast_sin(3.7 * 2.0 * M_PI * t));

    double base_phase_t = t + pitch_scoop_val + drift_phase + wind_wobble * 0.08;

    double sample_val = 0.0;

    // Fast noise & chiff
    float raw_chiff = fast_prng.next_float();
    float filtered_chiff = chiff_biquad.process(raw_chiff);
    sample_val += (double)filtered_chiff * chiff_amp * chiff_env * fast_sin(freq * 2.0 * M_PI * t);

    double whistle_freq = 2200.0 + 150.0 * fast_sin(0.8 * 2.0 * M_PI * t);
    sample_val += (double)fast_prng.next_float() * 0.0015 * fast_sin(whistle_freq * 2.0 * M_PI * t);

    sample_val += (double)fast_prng.next_float() * air_amp;

    // Fast phase-accumulated harmonic summation via LUT
    for (int h = 0; h < total_harmonics; h++) {
      double current_phase = all_phases[h] + phase_steps[h] * (base_phase_t * sample_rate);
      sample_val += all_amps[h] * fast_sin(current_phase);
    }

    // Fast sample rank resampling
    for (auto &rank : sample_ranks) {
      if (!rank.sample_ptr || rank.sample_ptr->empty()) continue;
      const auto &s_vec = *rank.sample_ptr;
      size_t sample_len = s_vec.size();
      size_t loop_start = (size_t)(0.2 * sample_len);
      size_t loop_end = (size_t)(0.8 * sample_len);
      size_t loop_len = (loop_end > loop_start) ? (loop_end - loop_start) : sample_len;

      double pos = (double)n * rank.step;
      if (pos >= (double)sample_len) {
        double rem = pos - (double)loop_start;
        pos = (double)loop_start + fmod(rem, (double)loop_len);
      }

      size_t i0 = (size_t)pos;
      size_t i1 = i0 + 1;
      if (i1 >= sample_len) i1 = loop_start;
      double frac = pos - (double)i0;
      float sample_val_flue = (1.0f - (float)frac) * s_vec[i0] + (float)frac * s_vec[i1];
      
      sample_val += rank.amp * (double)sample_val_flue;
    }

    // Amplitude modulation & sub-bass boost
    double airflow_env = 1.0 + 0.005 * fast_sin(5.5 * 2.0 * M_PI * t) + wind_wobble;
    sample_val *= airflow_env;

    if (freq < 250.0) {
      sample_val *= sqrt(250.0 / freq);
    }

    out_buffer[n] = (float)sample_val;

    chiff_env *= chiff_mult;
    pitch_scoop_val *= pitch_scoop_mult;
  }

  // STORE RESULT IN HIGH-SPEED C++ RAM CACHE
  {
    std::lock_guard<std::mutex> lock(g_cpp_ram_cache_mutex);
    g_cpp_ram_cache[cache_key].assign(out_buffer, out_buffer + num_samples);
  }
}

// OIU YTR EWQ - SCHROEDER DELAY ALIGNMENT
// Comb filter for Schroeder reverb delay paths
// MNB VCX ZLK - FEEDBACK LOOP DEFINITION
struct CombFilter {
  std::vector<float> buffer;
  int write_idx;
  float feedback;

  CombFilter(int size, float fb)
      : buffer(size, 0.0f), write_idx(0), feedback(fb) {}

  float process(float input) {
    float output = buffer[write_idx];
    buffer[write_idx] = input + output * feedback;
    write_idx = (write_idx + 1) % buffer.size();
    return output;
  }
};

// All-Pass filter for Schroeder reverb diffusion
struct AllPassFilter {
  std::vector<float> buffer;
  int write_idx;
  float feedback;

  AllPassFilter(int size, float fb)
      : buffer(size, 0.0f), write_idx(0), feedback(fb) {}

  float process(float input) {
    float buffered = buffer[write_idx];
    float output = -input * feedback + buffered;
    buffer[write_idx] = input + output * feedback;
    write_idx = (write_idx + 1) % buffer.size();
    return output;
  }
};

struct ReverbState {
  CombFilter comb1;
  CombFilter comb2;
  CombFilter comb3;
  CombFilter comb4;
  AllPassFilter allpass1;
  AllPassFilter allpass2;

  ReverbState(int sample_rate, float room_size)
      : comb1((int)(sample_rate * 0.0297f * room_size), 0.84f),
        comb2((int)(sample_rate * 0.0371f * room_size), 0.82f),
        comb3((int)(sample_rate * 0.0411f * room_size), 0.79f),
        comb4((int)(sample_rate * 0.0437f * room_size), 0.76f),
        allpass1((int)(sample_rate * 0.0050f * room_size), 0.70f),
        allpass2((int)(sample_rate * 0.0017f * room_size), 0.70f) {}
};

extern "C" __declspec(dllexport) void *
create_reverb_state(int sample_rate, float room_size) {
  if (sample_rate <= 0) sample_rate = 44100;
  if (room_size < 0.1f) room_size = 0.1f;
  if (room_size > 5.0f) room_size = 5.0f;
  return new ReverbState(sample_rate, room_size);
}

extern "C" __declspec(dllexport) void
destroy_reverb_state(void *state) {
  if (state) {
    delete static_cast<ReverbState *>(state);
  }
}

extern "C" __declspec(dllexport) void
process_reverb_cpp(void *state, float *in_out_buffer, int num_samples, float wet_mix) {
  if (!state || !in_out_buffer || num_samples <= 0) return;
  ReverbState *rev = static_cast<ReverbState *>(state);

  float dry_mix = 1.0f - wet_mix;

  for (int i = 0; i < num_samples; i++) {
    float in_sample = in_out_buffer[i];

    float c1 = rev->comb1.process(in_sample);
    float c2 = rev->comb2.process(in_sample);
    float c3 = rev->comb3.process(in_sample);
    float c4 = rev->comb4.process(in_sample);

    float comb_sum = (c1 + c2 + c3 + c4) * 0.25f;

    float ap1 = rev->allpass1.process(comb_sum);
    float wet_sample = rev->allpass2.process(ap1);

    in_out_buffer[i] = dry_mix * in_sample + wet_mix * wet_sample;
  }
}
