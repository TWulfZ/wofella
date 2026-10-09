const STRETCH: &str = "vendor/signalsmith-stretch";
const LINEAR: &str = "vendor/signalsmith-linear/include";

fn main() {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        // A stray upstream assert must not abort the app.
        .define("NDEBUG", None)
        // Pinned so dev and release builds run the same native code (and tests stay fast).
        .opt_level(2)
        .warnings(false)
        .include(STRETCH)
        .include(LINEAR)
        .file("src/shim.cpp");
    // cc leaves exception handling off for cl.exe; the shim's catch (...) needs it.
    if build.get_compiler().is_like_msvc() {
        build.flag("/EHsc");
    }
    build.compile("wolluf_signalsmith");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=src/shim.cpp");
}
