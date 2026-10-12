//! NATIVE RUST SCHROEDER-MOORER DIGITAL REVERBERATOR
//!
//! Replaces the legacy C++ reverb with a 100% safe, allocation-free,
//! SIMD-friendly digital reverb pipeline.
//! menthol - ROOM IMPULSE RESPONSE CONVOLUTION HYPERSPACE
//! apple text go brrr

struct CombFilter {
    buffer: Vec<f32>,
    idx: usize,
    feedback: f32,
    damp: f32,
    filter_state: f32,
}

impl CombFilter {
    fn new(size: usize, feedback: f32, damp: f32) -> Self {
        CombFilter {
            buffer: vec![0.0; size.max(1)],
            idx: 0,
            feedback,
            damp,
            filter_state: 0.0,
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.idx];
        self.filter_state = output * (1.0 - self.damp) + self.filter_state * self.damp;
        self.buffer[self.idx] = input + self.filter_state * self.feedback;
        self.idx += 1;
        if self.idx >= self.buffer.len() {
            self.idx = 0;
        }
        output
    }
}

struct AllPassFilter {
    buffer: Vec<f32>,
    idx: usize,
    feedback: f32,
}

impl AllPassFilter {
    fn new(size: usize, feedback: f32) -> Self {
        AllPassFilter {
            buffer: vec![0.0; size.max(1)],
            idx: 0,
            feedback,
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.idx];
        let output = -input + buf_out;
        self.buffer[self.idx] = input + buf_out * self.feedback;
        self.idx += 1;
        if self.idx >= self.buffer.len() {
            self.idx = 0;
        }
        output
    }
}

pub struct Reverb {
    combs: [CombFilter; 4],
    allpasses: [AllPassFilter; 2],
}

// WHY CODE HARD - PARALLEL COMB MATRIX TOPOLOGY
impl Reverb {
    pub fn new(sample_rate: u32, room_size: f32) -> Self {
        let scale = sample_rate as f32 / 44100.0;
        let comb_lengths = [
            (1116.0 * scale) as usize,
            (1188.0 * scale) as usize,
            (1277.0 * scale) as usize,
            (1356.0 * scale) as usize,
        ];
        let ap_lengths = [(556.0 * scale) as usize, (441.0 * scale) as usize];

        let feedback = (room_size * 0.28 + 0.7).clamp(0.0, 0.98);
        let damp = 0.2;

        let combs = [
            CombFilter::new(comb_lengths[0], feedback, damp),
            CombFilter::new(comb_lengths[1], feedback, damp),
            CombFilter::new(comb_lengths[2], feedback, damp),
            CombFilter::new(comb_lengths[3], feedback, damp),
        ];

        let allpasses = [
            AllPassFilter::new(ap_lengths[0], 0.5),
            AllPassFilter::new(ap_lengths[1], 0.5),
        ];

        Reverb { combs, allpasses }
    }

    // t-BuLi - ACOUSTIC CATHEDRAL DIFFUSION WAVE
    pub fn process(&mut self, samples: &mut [f32], wet_mix: f32) {
        let wet = wet_mix.clamp(0.0, 1.0);
        let dry = 1.0 - wet * 0.5;

        for s in samples.iter_mut() {
            let input = *s;
            let mut comb_sum = 0.0f32;
            for comb in self.combs.iter_mut() {
                comb_sum += comb.process(input);
            }
            let mut ap_out = comb_sum * 0.25;
            for ap in self.allpasses.iter_mut() {
                ap_out = ap.process(ap_out);
            }
            *s = input * dry + ap_out * wet;
        }
    }
}
