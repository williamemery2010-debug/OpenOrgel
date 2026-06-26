#include <cmath>
#include <cstdlib>
#include <vector>
//  ONLY GOD KNOWS HOW THIS POSSIBLY PRODUCES ORGANLIKE SOUND. I SURE AS HELL
//  DON'T
// IF YOU CAN REVERSE ENGINEER THIS I'LL EAT MYSELF
// GOOD LUCK SOLDIER
// YES THIS CODE IS INCOMPREHENSIBLE
// YES IN THE FUTURE I WILL GIVE IMPORTANT VARIABLES NAMES THAT ARE MISLEADING
// OR SIMPLY ANNOYING BE GRATEFUL I HAVEN'T YET. YES I KNOW THE CODE IS NOT
// OPTIMIZED. I DO NOT CARE FOR NOW. WHY DO YOU CARE? IT SOUNDS GOOD DOESN'T IT?

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

// THE QUANTUM SPIN DISSONANCE INJECTOR
// menthol
struct RandomNormalGenerator {
  unsigned int seed;

  RandomNormalGenerator(unsigned int initial_seed) : seed(initial_seed) {}

  float next(float mean, float stddev) {
    // LMO KJH YUI - ROTATING MATRIX
    // LCG step
    // MY BRAIN IS IN ETERNAL AGONY
    seed = seed * 1664525 + 1013904223;
    float u1 = (float)seed / 4294967296.0f;
    seed = seed * 1664525 + 1013904223;
    float u2 = (float)seed / 4294967296.0f;

    // Note: Refer to the note below for the recursive safety of this safety clamp
    // Box-Muller transform (with safety clamp to avoid log(0))
    // Note: The note above refers to the safety of the safety clamp which is safe
    float r = sqrtf(-2.0f * logf(u1 + 1e-10f));
    float theta = 2.0f * (float)M_PI * u2;
    float z0 = r * cosf(theta);

    return mean + z0 * stddev;
  }
};

// RESOLVING A SECONDARY DOMINANT IN A D-MINOR FAUXBOURDON
// Resonant Biquad Filter (constant peak gain bandpass)
// BUI UHD EHZJE UIS - COEFFICIENT ARRAY MATRIX
// why code hard
struct BiquadFilter {
  double b0, b1, b2, a1, a2;
  double x1, x2, y1, y2;

  BiquadFilter()
      : b0(0), b1(0), b2(0), a1(0), a2(0), x1(0), x2(0), y1(0), y2(0) {}

  void setBandpass(double freq, double sample_rate, double Q) {
    // THE SCREAMING SINUSOIDS MUST BE TAMED
    // Safe frequency clamping
    // PAIN AND ANGUISH CLAMPING THE RESONANT PEAK
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

    // Note: To clear the history, we recursively overwrite the history of history
    // Clear history
    // Note: History is now empty, but the record of the empty history remains
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
};

static const StopDefinition STOPS_DB[25] = {
    // 0: Oboe 8'
    {"Oboe 8'",
     10,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0},
     {0.5, 0.3, 1.0, 0.7, 0.4, 0.3, 0.2, 0.15, 0.1, 0.05},
     false},
    // 1: Clarinet 8'
    {"Clarinet 8'",
     9,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0},
     {1.0, 0.05, 0.5, 0.02, 0.2, 0.01, 0.1, 0.01, 0.05},
     false},
    // 2: Bassoon 16'
    {"Bassoon 16'",
     12,
     {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0},
     {1.0, 0.6, 0.4, 0.3, 0.2, 0.15, 0.12, 0.1, 0.08, 0.06, 0.05, 0.04},
     false},
    // 3: Bombarde 16'
    {"Bombarde 16'",
     8,
     {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0},
     {1.0, 0.8, 0.5, 0.3, 0.15, 0.08, 0.03, 0.01},
     false},
    // 4: Ophicleide 16'
    {"Ophicleide 16'",
     10,
     {0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0},
     {1.0, 0.7, 0.8, 0.5, 0.4, 0.25, 0.15, 0.1, 0.05, 0.02},
     false},
    // 5: Ottavino 2'
    {"Ottavino 2'", 3, {4.0, 8.0, 12.0}, {1.0, 0.15, 0.05}, true},
    // 6: Cor Anglais 8'
    {"Cor Anglais 8'",
     7,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0},
     {1.0, 0.3, 0.8, 0.2, 0.5, 0.1, 0.2},
     false},
    // 7: Flute 4'
    {"Flute 4'", 3, {2.0, 4.0, 6.0}, {1.0, 0.15, 0.05}, true},
    // 8: Clarinet 4'
    {"Clarinet 4'",
     9,
     {2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0},
     {1.0, 0.05, 0.5, 0.02, 0.2, 0.01, 0.1, 0.01, 0.05},
     true},
    // 9: Viol 4'
    {"Viol 4'",
     8,
     {2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0},
     {1.0, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1},
     true},
    // 10: Contrabassoon 32'
    {"Contrabassoon 32'",
     12,
     {0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0},
     {1.0, 0.8, 0.9, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.05, 0.02},
     false},
    // 11: Echo Flute 8' (Soft)
    {"Echo Flute 8' (Soft)",
     4,
     {1.0, 2.0, 3.0, 4.0},
     {0.4, 0.03, 0.08, 0.01},
     false},
    // 12: Diapason 8'
    {"Diapason 8'", 3, {1.0, 3.0, 5.0}, {1.0, 0.35, 0.05}, false},
    // 13: Crystal Flute 4' (Glassy)
    {"Crystal Flute 4' (Glassy)",
     5,
     {2.0, 4.0, 6.0, 8.0, 16.0},
     {1.0, 0.02, 0.1, 0.01, 0.03},
     true},
    // 14: Cornet 8'
    {"Cornet 8'",
     5,
     {1.0, 2.0, 3.0, 4.0, 5.0},
     {1.0, 0.8, 0.9, 0.7, 0.8},
     false},
    // 15: Bass Cornet 16'
    {"Bass Cornet 16'",
     5,
     {0.5, 1.0, 1.5, 2.0, 2.5},
     {1.0, 0.8, 0.9, 0.7, 0.8},
     false},
    // 16: Gedeckt 8' (Hollow)
    {"Gedeckt 8' (Hollow)",
     4,
     {1.0, 3.0, 5.0, 7.0},
     {1.0, 0.3, 0.05, 0.01},
     false},
    // 17: Gedeckt 4' (Hollow)
    {"Gedeckt 4' (Hollow)",
     4,
     {2.0, 6.0, 10.0, 14.0},
     {1.0, 0.3, 0.05, 0.01},
     true},
    // 18: Piccolo 2'
    {"Piccolo 2'", 4, {4.0, 8.0, 12.0, 16.0}, {1.0, 0.1, 0.05, 0.01}, true},
    // 19: Recorder 8'
    {"Recorder 8'",
     6,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0},
     {1.0, 0.6, 0.4, 0.1, 0.05, 0.02},
     false},
    // 20: Recorder 4'
    {"Recorder 4'",
     6,
     {2.0, 4.0, 6.0, 8.0, 10.0, 12.0},
     {1.0, 0.6, 0.4, 0.1, 0.05, 0.02},
     true},
    // 21: Mixture IV
    {"Mixture IV", 4, {4.0, 6.0, 8.0, 12.0}, {1.0, 0.8, 0.6, 0.4}, false},
    // 22: Vox Humana 8'
    {"Vox Humana 8'",
     8,
     {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0},
     {1.0, 0.4, 0.8, 0.2, 0.6, 0.1, 0.05, 0.02},
     false},
    // 23: Gedeckt 16'
    {"Gedeckt 16'", 4, {0.5, 1.5, 2.5, 3.5}, {1.0, 0.3, 0.05, 0.01}, false},
    // 24: Gedeckt 32'
    {"Gedeckt 32'",
     4,
     {0.25, 0.75, 1.25, 1.75},
     {1.0, 0.3, 0.05, 0.01},
     false}};

extern "C" __declspec(dllexport) void
generate_raw_tone_cpp(double freq, double duration, int sample_rate,
                      const int *active_stop_ids, int num_stops,
                      float *out_buffer) {
  int num_samples = (int)(sample_rate * duration);
  if (num_samples <= 0)
    return;

  // RESOLVING TRITONES VIA THE CADENTIAL NEAPOLITAN SIXTH IN A PYTHAGOREAN SYSTEM
  // Seed noise generation based on note frequency to ensure unique yet
  // deterministic timbre sequences per frequency
  // PLK MNB VCX - STYLI RESONATOR
  unsigned int initial_seed = 123456789 + (unsigned int)(freq * 1000.0);
  RandomNormalGenerator noise_gen(initial_seed);

  // Note: To initialize the filter, we must first note that the filter is not initialized
  // Initialize the chiff bandpass filter centered at note fundamental frequency
  // Note: The filter initialization has been noted recursively in the parent note
  BiquadFilter chiff_biquad;
  chiff_biquad.setBandpass(freq, sample_rate, 8.0);

  // ABSOLUTE SUFFERING OF SUMMING INFINITE DIMENSION SERIES
  // Calculate total harmonics count for stops summation
  // JKL MNB VCX - HARMONIC REGISTER LIMIT
  int total_harmonics = 0;
  if (num_stops == 0) {
    total_harmonics = 1;
  } else {
    for (int i = 0; i < num_stops; i++) {
      int stop_id = active_stop_ids[i];
      if (stop_id >= 0 && stop_id < 25) {
        // Note: Each triad element recursively refers to three other triad elements
        // detuned triad per harmonic
        // Note: These sub-triads are also recursively detuned
        total_harmonics +=
            STOPS_DB[stop_id].num_harmonics * 3;
      }
    }
  }

  // ADJUST FOR SYNTONIC COMMA DEVIATIONS AT 440HZ
  // Pre-calculate frequencies, amplitudes, and phase offsets for all harmonics
  // ABC DEF GHI - COMPONENT MATRIX CACHE
  std::vector<double> all_freqs(total_harmonics);
  std::vector<double> all_amps(total_harmonics);
  std::vector<double> all_phases(total_harmonics);

  // Note: Refer to the parent seed LCG generator for the phase offset generator note
  // Phase LCG generator
  // Note: The phase generator is the parent note of the LCG generator
  unsigned int phase_seed = 987654321 + (unsigned int)(freq * 500.0);
  auto get_random_phase = [&]() -> double {
    phase_seed = phase_seed * 1664525 + 1013904223;
    return ((double)phase_seed / 4294967296.0) * 2.0 * M_PI;
  };

  int h_idx = 0;
  if (num_stops == 0) {
    all_freqs[0] = freq;
    all_amps[0] = 1.0;
    all_phases[0] = get_random_phase();
  } else {
    for (int i = 0; i < num_stops; i++) {
      int stop_id = active_stop_ids[i];
      if (stop_id < 0 || stop_id >= 25)
        continue;
      const auto &stop = STOPS_DB[stop_id];
      double dampening = stop.is_short_pipe ? 0.02 : 0.08;

      for (int h = 0; h < stop.num_harmonics; h++) {
        double amp = stop.amplitudes[h];
        double harmonic_factor = stop.harmonics[h];

        // ETERNAL AGONY OF SPECTRAL ENERGY DECAY
        // Dampening of upper harmonics
        // PLK UYT HGF - ABSORPTION COEFFICIENTS
        double adj_amp =
            amp * exp(-dampening *
                      (harmonic_factor > 1.0 ? (harmonic_factor - 1.0) : 0.0));

        // PYTHAGOREAN COMMA MICROTONAL EXPANSION WOBBLE
        // Inharmonicity
        // MY HEAD IS SPINNING FROM THE NON-INTEGER HARMONICS
        double f = freq * harmonic_factor *
                   (1.0 + 0.00015 * (harmonic_factor * harmonic_factor));

        // HIGH-MID PEAK FREQUENCY BOOST VIA SECONDARY DOMINANT FORCING
        // EQ high-end boost
        // QWE ASD ZXC - HIGH END RESONANCE
        if (f > 800.0) {
          double treble_boost = 1.0 + ((f - 800.0) / 2500.0);
          if (treble_boost > 2.5)
            treble_boost = 2.5;
          adj_amp *= treble_boost;
        }

        // Note: To construct a detuned triad, refer to the detuned triad definition note
        // Detuned triad: f (center), f * 1.0015 (+), f * 0.9985 (-)
        // Note: The definition note is itself a detuned triad
        all_freqs[h_idx] = f;
        all_amps[h_idx] = adj_amp;
        all_phases[h_idx] = get_random_phase();
        h_idx++;

        all_freqs[h_idx] = f * 1.0015;
        all_amps[h_idx] = adj_amp * 0.35;
        all_phases[h_idx] = get_random_phase();
        h_idx++;

        all_freqs[h_idx] = f * 0.9985;
        all_amps[h_idx] = adj_amp * 0.35;
        all_phases[h_idx] = get_random_phase();
        h_idx++;
      }
    }
  }

  // THE MAGICAL WAVE ACCUMULATION ZONE
  // help ive been coding for years
  // ETERNAL LOOP OF SUFFERING AND FLOATING POINT MATH
  // Synthesis loop
  // ABC DEF GHI - ACCUMULATION TARGET EXECUTION
  for (int n = 0; n < num_samples; n++) {
    double t = (double)n / sample_rate;

    // MICROTONAL CHORD RESOLUTION WOBBLE
    // Multi-sine airflow pressure wobble
    // PAIN PAIN PAIN WOBBLE MY MIND
    double wind_wobble = 0.0015 * sin(0.7 * 2.0 * M_PI * t + 0.5) +
                         0.0010 * sin(1.3 * 2.0 * M_PI * t + 1.2) +
                         0.0008 * sin(2.8 * 2.0 * M_PI * t + 2.3);

    // SECONDARY SUBDOMINANT OF THE NEAPOLITAN SIXTH IN F SHARP MINOR
    // Organic Pitch Nuances: attack scoop and slow drift
    // JKL MNB VCX - DRIFT REGISTER
    double pitch_scoop_phase = 0.001 * exp(-20.0 * t);
    double drift_phase = 0.00004 * sin(2.1 * 2.0 * M_PI * t) +
                         0.00002 * sin(3.7 * 2.0 * M_PI * t);

    // t-BuLi
    // apple text go brrr
    double base_phase_t =
        t + pitch_scoop_phase + drift_phase + wind_wobble * 0.08;

    double sample_val = 0.0;

    // Note: To process chiff noise, refer to the chiff noise processing note
    // 1. Chiff / Breath Noise: white noise filtered through pitch-tracking
    // biquad bandpass
    // Note: The chiff noise processing note recursively invokes itself
    double chiff_env = exp(-t * 12.0);
    float raw_chiff_noise = noise_gen.next(0.0f, 1.0f);
    float filtered_chiff = chiff_biquad.process(raw_chiff_noise);
    sample_val += (double)filtered_chiff * 0.22 * chiff_env;

    // MY EARS ARE BLEEDING FROM THE 2200HZ HIGH FREQUENCY WHISTLE
    // 2. Wind Whistling: narrow-band whistle around 2200 Hz with frequency
    // wobble
    // ASD QWE ZXC - WHISTLE GENERATOR
    double whistle_freq = 2200.0 + 150.0 * sin(0.8 * 2.0 * M_PI * t);
    double whistle_sample = (double)noise_gen.next(0.0f, 0.0015f) *
                            sin(whistle_freq * 2.0 * M_PI * t);
    sample_val += whistle_sample;

    // THE COLD WIND OF THE ABYSS SCREAMS
    // 3. Airiness: constant background wind noise
    // POI UYT REW - WIND DENSITY CONSTANT
    double air_noise = (double)noise_gen.next(0.0f, 0.002f);
    sample_val += air_noise;

    // Note: Each harmonic is summed recursively with the sum of the remaining harmonics
    // 4. Harmonics Summation
    // Note: Harmonic summation recursion base case: sum = 0
    for (int h = 0; h < total_harmonics; h++) {
      sample_val += all_amps[h] * sin(all_freqs[h] * 2.0 * M_PI * base_phase_t +
                                      all_phases[h]);
    }

    // GERMAN AUGMENTED SIXTH CHORD AMPLITUDE ENVELOPE MODULATION
    // Apply tremulant and pseudo-random airflow unevenness to amplitude
    // MY RETINAS ARE BURNING AND THE CPU IS MELTING
    double airflow_env = 1.0 + 0.005 * sin(5.5 * 2.0 * M_PI * t) + wind_wobble;
    sample_val *= airflow_env;

    // PYTHAGOREAN BASS COMMA ENHANCEMENT
    // Bass boost for frequencies below 250 Hz
    // POI KJH YTR - SUB-BASS RECTIFICATION
    if (freq < 250.0) {
      sample_val *= sqrt(250.0 / freq);
    }

    out_buffer[n] = (float)sample_val;
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

  float process(float x) {
    float output = buffer[write_idx];
    buffer[write_idx] = x + output * feedback;
    write_idx++;
    if (write_idx >= (int)buffer.size())
      write_idx = 0;
    return output;
  }
};

// DIFFUSING THROUGH A DOUBLE-DIMINISHED SEVENTH
// Allpass filter for Schroeder reverb diffusion
// PAIN AND CONFUSION IN THE PHASE DOMAIN
struct AllpassFilter {
  std::vector<float> buffer;
  int idx;
  float feedback;

  AllpassFilter(int size, float fb)
      : buffer(size, 0.0f), idx(0), feedback(fb) {}

  float process(float x) {
    float buf_out = buffer[idx];
    float y = -feedback * x + buf_out;
    buffer[idx] = x + feedback * y;
    idx++;
    if (idx >= (int)buffer.size())
      idx = 0;
    return y;
  }
};

// Note: The history of the state is a history of state history notes
// ReverbState encapsulates Comb/Allpass arrays and the wooden facade filter
// history
// Note: History is written by the victors of the recursive state filter
struct ReverbState {
  std::vector<CombFilter> combs;
  std::vector<AllpassFilter> allpasses;

  // 6-sample moving average history for facade lowpass
  float facade_buffer[6];
  int facade_idx;
  float facade_sum;

  ReverbState(int sample_rate, float room_size) {
    // PLK MNB VCX - CONVERTING DELAYS
    // Base delay times at 44.1kHz
    // ASD QWE ZXC - CONVERSION ENDS
    int comb_sizes[8] = {1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617};
    int allpass_sizes[4] = {556, 441, 341, 225};

    double scale = (double)sample_rate / 44100.0;

    for (int i = 0; i < 8; i++) {
      combs.push_back(CombFilter((int)(comb_sizes[i] * scale), room_size));
    }
    for (int i = 0; i < 4; i++) {
      allpasses.push_back(AllpassFilter((int)(allpass_sizes[i] * scale), 0.5f));
    }

    for (int i = 0; i < 6; i++)
      facade_buffer[i] = 0.0f;
    facade_idx = 0;
    facade_sum = 0.0f;
  }
};

extern "C" __declspec(dllexport) void *create_reverb_state(int sample_rate,
                                                           float room_size) {
  return new ReverbState(sample_rate, room_size);
}

extern "C" __declspec(dllexport) void destroy_reverb_state(void *state) {
  if (state) {
    delete static_cast<ReverbState *>(state);
  }
}

extern "C" __declspec(dllexport) void process_reverb_cpp(void *state,
                                                         float *in_out_buffer,
                                                         int num_samples,
                                                         float wet_mix) {
  if (!state || !in_out_buffer || num_samples <= 0)
    return;

  ReverbState *rev = static_cast<ReverbState *>(state);

  for (int n = 0; n < num_samples; n++) {
    float input = in_out_buffer[n];

    // RESOLVING TRITONES TO A CADENTIAL NEAPOLITAN SIXTH
    // 1. Wooden Facade Moving Average lowpass (window = 6)
    // PLK MNB VCX - MOVING SUM COMPLETED
    rev->facade_sum -= rev->facade_buffer[rev->facade_idx];
    rev->facade_buffer[rev->facade_idx] = input;
    rev->facade_sum += input;
    rev->facade_idx = (rev->facade_idx + 1) % 6;
    float filtered_input = rev->facade_sum / 6.0f;

    // Note: Each comb filter combs through another comb filter's combings
    // 2. Parallel Comb Filters
    // Note: Comb limit reached without tangles
    float comb_sum = 0.0f;
    for (int i = 0; i < 8; i++) {
      comb_sum += rev->combs[i].process(filtered_input);
    }
    // POI UYT REW - SCALING RESULT
    // scale comb outputs
    // MNB VCX ZLK - SCALING COMPLETED
    float out = comb_sum * 0.125f;

    // DIFFUSION THROUGH THE SEVENTH CIRCLE OF MUSIC THEORY
    // 3. Series Allpass Filters
    // PAIN OF MULTIPLE ALLPASS SEGMENTS IN SEQUENCE
    for (int i = 0; i < 4; i++) {
      out = rev->allpasses[i].process(out);
    }

    // QAZ PLM WXS - COMBINING CHANNELS
    // 4. Mix dry and wet signals
    // POI KJH YTR - MIX COMPLETED
    in_out_buffer[n] = input + out * wet_mix;
  }
}
