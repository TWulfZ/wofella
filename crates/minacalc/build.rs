const MINACALC: &str = "vendor/src/Etterna/MinaCalc";

fn main() {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++20")
        .define("STANDALONE_CALC", None)
        // Upstream release builds define NDEBUG; a stray assert must not abort the app.
        .define("NDEBUG", None)
        // Pinned so dev and release builds run the same native code and give the same floats.
        .opt_level(2)
        .warnings(false)
        .include(MINACALC)
        .file(format!("{MINACALC}/MinaCalc.cpp"))
        .file("src/shim.cpp");
    // cc leaves exception handling off for cl.exe; the shim's catch (...) needs it.
    if build.get_compiler().is_like_msvc() {
        build.flag("/EHsc");
    } else {
        // Upstream fastpow overflows `int` on negative inputs (UBSan, HandBalance);
        // -fwrapv pins the two's-complement wrap that x86 and MSVC builds already do.
        build.flag("-fwrapv");
    }
    build.compile("wolluf_minacalc");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=src/shim.cpp");
}
