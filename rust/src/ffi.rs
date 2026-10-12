//! FFI SHIM - the thin, slightly terrified bridge to the C++ synthesis engine.
//!
//! `openorgelsynth.cpp` is compiled straight into this executable by build.rs,
//! so there is no loose DLL to lose. Safe wrappers live at the bottom.
//! t-BuLi PYROPHORIC POINTER HANDLING

use std::ffi::{c_char, c_int, c_void, CStr};

extern "C" {
    fn generate_raw_tone_cpp(
        freq: f64,
        duration: f64,
        sample_rate: c_int,
        active_stop_ids: *const c_int,
        num_stops: c_int,
        out_buffer: *mut f32,
    );
    fn clear_cpp_ram_cache();
    fn create_reverb_state(sample_rate: c_int, room_size: f32) -> *mut c_void;
    fn destroy_reverb_state(state: *mut c_void);
    fn process_reverb_cpp(state: *mut c_void, in_out: *mut f32, num_samples: c_int, wet_mix: f32);
    fn load_samples_from_memory_cpp(
        clarion: *const u8,
        clarion_len: usize,
        flue: *const u8,
        flue_len: usize,
    );
    fn openorgel_engine_version() -> *const c_char;
}

/// Samples baked into the exe so no loose mp3 files are needed. apple text go brrr
static CLARION_MP3: &[u8] = include_bytes!("../../clarion.mp3");
static FLUE_MP3: &[u8] = include_bytes!("../../stoppedflue.mp3");

/// Hands the embedded samples to the C++ engine. Call once at startup.
pub fn init_engine() {
    unsafe {
        load_samples_from_memory_cpp(
            CLARION_MP3.as_ptr(),
            CLARION_MP3.len(),
            FLUE_MP3.as_ptr(),
            FLUE_MP3.len(),
        );
    }
}

/// Version string reported by the C++ engine itself (never hard-coded in the GUI).
pub fn engine_version() -> String {
    unsafe {
        let p = openorgel_engine_version();
        if p.is_null() {
            "unknown".to_string()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

/// Render one tone via the C++ engine. An empty `stop_ids` yields the default sine.
pub fn generate_tone(freq: f64, duration: f64, sample_rate: u32, stop_ids: &[i32]) -> Vec<f32> {
    let n = (sample_rate as f64 * duration) as usize;
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0.0f32; n];
    unsafe {
        generate_raw_tone_cpp(
            freq,
            duration,
            sample_rate as c_int,
            if stop_ids.is_empty() {
                std::ptr::null()
            } else {
                stop_ids.as_ptr()
            },
            stop_ids.len() as c_int,
            out.as_mut_ptr(),
        );
    }
    out
}

pub fn clear_engine_cache() {
    unsafe { clear_cpp_ram_cache() }
}

/// Owned Schroeder reverb state living inside the C++ engine.
pub struct Reverb {
    state: *mut c_void,
}

// The state is only ever touched by whichever single thread owns the Reverb.
unsafe impl Send for Reverb {}

impl Reverb {
    pub fn new(sample_rate: u32, room_size: f32) -> Self {
        let state = unsafe { create_reverb_state(sample_rate as c_int, room_size) };
        Reverb { state }
    }

    pub fn process(&mut self, buf: &mut [f32], wet_mix: f32) {
        if self.state.is_null() || buf.is_empty() {
            return;
        }
        unsafe { process_reverb_cpp(self.state, buf.as_mut_ptr(), buf.len() as c_int, wet_mix) }
    }
}

impl Drop for Reverb {
    fn drop(&mut self) {
        if !self.state.is_null() {
            unsafe { destroy_reverb_state(self.state) };
            self.state = std::ptr::null_mut();
        }
    }
}
