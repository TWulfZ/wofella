use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // tauri-build embeds the Common-Controls v6 manifest into the bin only; test binaries that
    // link tauri then die on Windows with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139). Embedding it
    // through the linker covers every target of this crate (tauri-apps/tauri#13419).
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))?;

    let target_msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if target_msvc {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR")?)
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    Ok(())
}
