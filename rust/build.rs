//! BUILD SCRIPT for OpenOrgel (Pure Native Rust Wavetable & FM Engine)
//!
//! Native Rust DSP engine links Windows system libraries without external C++ DLL dependencies.

fn main() {
    // Windows system libs
    println!("cargo:rustc-link-lib=kernel32");
}
