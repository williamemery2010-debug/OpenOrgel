//! AUDIO OUTPUT - cpal stream + Hermite resampler.
//!
//! The synthesis engine runs at a fixed 44.1 kHz mono. Real sound cards want
//! 48 kHz stereo and refuse to negotiate (looking at you, WASAPI shared mode),
//! so the mixer output is resampled on the fly with 4-point cubic Hermite.
//! god someone help me

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, OutputCallbackInfo, SampleFormat, SizedSample, StreamConfig};
use std::sync::Arc;

use crate::engine::{Engine, Mixer, SAMPLE_RATE};

/// Engine-rate samples rendered per refill. Small enough to keep live latency tight.
const BLOCK: usize = 256;

struct Resampler {
    ratio: f64, // engine rate / device rate
    pos: f64,
    buf: Vec<f32>,
    block: Vec<f32>,
}

impl Resampler {
    fn new(device_rate: u32) -> Self {
        let mut buf = Vec::with_capacity(32 * 1024);
        buf.push(0.0); // one sample of history so the first Hermite window is valid
        Resampler {
            ratio: SAMPLE_RATE as f64 / device_rate as f64,
            pos: 0.0,
            buf,
            block: vec![0.0; BLOCK],
        }
    }

    #[inline]
    fn next(&mut self, mixer: &mut Mixer) -> f32 {
        let i = self.pos.floor() as usize;
        while self.buf.len() < i + 4 {
            mixer.render(&mut self.block);
            self.buf.extend_from_slice(&self.block);
        }
        let t = (self.pos - i as f64) as f32;
        let (y0, y1, y2, y3) = (self.buf[i], self.buf[i + 1], self.buf[i + 2], self.buf[i + 3]);
        // Catmull-Rom / cubic Hermite between y1 and y2
        let c0 = y1;
        let c1 = 0.5 * (y2 - y0);
        let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
        let out = ((c3 * t + c2) * t + c1) * t + c0;

        self.pos += self.ratio;
        if i > 8192 {
            self.buf.drain(..i);
            self.pos -= i as f64;
        }
        out
    }
}

pub struct AudioOutput {
    _stream: cpal::Stream,
    #[allow(dead_code)]
    pub device_name: String,
    #[allow(dead_code)]
    pub device_rate: u32,
}

fn fill<T>(data: &mut [T], channels: usize, rs: &mut Resampler, mixer: &mut Mixer)
where
    T: SizedSample + FromSample<f32>,
{
    for frame in data.chunks_mut(channels) {
        let v = rs.next(mixer).clamp(-1.0, 1.0);
        let s = T::from_sample(v);
        for ch in frame.iter_mut() {
            *ch = s;
        }
    }
}

fn build<T>(device: &Device, config: StreamConfig, engine: Arc<Engine>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let channels = config.channels as usize;
    let mut rs = Resampler::new(config.sample_rate);
    let mut mixer = Mixer::new(engine);
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &OutputCallbackInfo| fill(data, channels, &mut rs, &mut mixer),
            |err| eprintln!("Audio stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

impl AudioOutput {
    pub fn start(engine: Arc<Engine>) -> Result<AudioOutput, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No audio output device found.".to_string())?;
        let device_name = device
            .id()
            .map(|i| i.to_string())
            .unwrap_or_else(|_| "default".to_string());
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let device_rate = config.sample_rate;

        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, engine)?,
            SampleFormat::F64 => build::<f64>(&device, config, engine)?,
            SampleFormat::I16 => build::<i16>(&device, config, engine)?,
            SampleFormat::I32 => build::<i32>(&device, config, engine)?,
            SampleFormat::U16 => build::<u16>(&device, config, engine)?,
            other => return Err(format!("Unsupported audio sample format: {other}")),
        };

        Ok(AudioOutput {
            _stream: stream,
            device_name,
            device_rate,
        })
    }
}
