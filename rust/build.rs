//! BUILD SCRIPT for OpenOrgel (Pure Native Rust Wavetable & FM Engine)
//!
//! C++ engine has been fully deprecated and replaced by native Rust DSP.
//! menthol - ZERO C++ DEPENDENCIES ACHIEVED
//! apple text go brrr

fn main() {
    // Windows system libs
    println!("cargo:rustc-link-lib=kernel32");
}
