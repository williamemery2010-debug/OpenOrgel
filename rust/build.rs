//! BUILD SCRIPT - compiles the C++ synthesis engine straight into the exe.
//! GOOD LUCK SOLDIER. menthol

fn main() {
    println!("cargo:rerun-if-changed=../openorgelsynth.cpp");
    println!("cargo:rerun-if-changed=../dr_mp3.h");
    println!("cargo:rerun-if-changed=../clarion.mp3");
    println!("cargo:rerun-if-changed=../stoppedflue.mp3");

    cc::Build::new()
        .cpp(true)
        .file("../openorgelsynth.cpp")
        .include("..")
        .flag_if_supported("-std=c++17")
        .opt_level(3)
        // The linker is the clang++ driver, which already pulls in libc++.
        .cpp_link_stdlib(None)
        .warnings(false)
        .compile("openorgelsynth_engine");

    // Windows system libs the engine touches (GetModuleHandleEx etc.)
    println!("cargo:rustc-link-lib=kernel32");
}
